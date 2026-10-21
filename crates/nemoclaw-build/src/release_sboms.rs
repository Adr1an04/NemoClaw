// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//! Bind upstream-generated inventories to the release's complete platform set.
//! Imported from NVIDIA/NemoClaw PR #13590 at
//! 64b8aa7f32cc0bcaa2a5672ecb1d6d92a46a7a4f; local integration preserves its producer.
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const LIMIT: u64 = 16_000_000;
#[derive(Debug, Serialize)]
pub struct Entry {
    pub reference: String,
    pub repository: String,
    pub digest: String,
    pub platform: String,
    pub asset: String,
    pub sha256: String,
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("missing {name}"))
}
fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > limit {
        return Err("release evidence is empty or exceeds its size limit".into());
    }
    Ok(bytes)
}
fn plan(inventory: &Value) -> Result<Vec<Entry>, String> {
    let images = inventory
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or("empty release inventory")?;
    let mut names = BTreeSet::new();
    let mut entries = Vec::new();
    for image in images {
        let name = field(image, "name")?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !names.insert(name)
        {
            return Err("invalid or duplicate release image name".into());
        }
        let repository = field(image, "repository")?;
        if repository != format!("ghcr.io/nvidia/nemoclaw/v1/{name}") {
            return Err("unexpected release repository".into());
        }
        let manifests = image["manifests"]["manifests"]
            .as_array()
            .ok_or("missing platform manifests")?;
        let mut platforms = BTreeSet::new();
        for manifest in manifests {
            let digest = field(manifest, "digest")?;
            if !self::digest(digest) {
                return Err("invalid platform digest".into());
            }
            if crate::release_assessment::input::auxiliary_attestation(manifest) {
                continue;
            }
            let platform = &manifest["platform"];
            let arch = field(platform, "architecture")?;
            if field(platform, "os")? != "linux"
                || !matches!(arch, "amd64" | "arm64")
                || !platforms.insert(arch)
            {
                return Err("unexpected or duplicate release platform".into());
            }
            entries.push(Entry {
                reference: format!("{repository}@{digest}"),
                repository: repository.into(),
                digest: digest.into(),
                platform: format!("linux/{arch}"),
                asset: format!("{name}-linux-{arch}-{}.spdx.json", &digest[7..]),
                sha256: String::new(),
            });
        }
        if platforms != BTreeSet::from(["amd64", "arm64"]) {
            return Err("release requires both linux/amd64 and linux/arm64".into());
        }
    }
    Ok(entries)
}
/// Generate into a fresh temporary directory and expose it only after every
/// scanner result has passed binding and size checks. Existing output is never
/// accepted as evidence for a rerun.
pub fn generate(
    inventory: &Value,
    output: &Path,
    mut scan: impl FnMut(&str, &Path, &Path) -> Result<(), String>,
) -> Result<Vec<Entry>, String> {
    let mut entries = plan(inventory)?;
    if output.exists() {
        return Err("SBOM output already exists; use a fresh directory".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = tempfile::tempdir_in(parent).map_err(|e| e.to_string())?;
    for entry in &mut entries {
        let spdx = temporary.path().join(&entry.asset);
        let source = temporary.path().join("source.syft.json");
        scan(&entry.reference, &spdx, &source)?;
        let evidence: Value =
            serde_json::from_slice(&read(&source, 128 << 20)?).map_err(|e| e.to_string())?;
        if evidence["source"]["type"] != "image"
            || evidence["source"]["metadata"]["manifestDigest"] != entry.digest
            || evidence["source"]["metadata"]["os"] != "linux"
            || evidence["source"]["metadata"]["architecture"]
                != entry.platform.trim_start_matches("linux/")
        {
            return Err("scanner source differs from release platform digest".into());
        }
        let bytes = read(&spdx, LIMIT)?;
        let sbom: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if sbom["spdxVersion"] != "SPDX-2.3"
            || sbom["SPDXID"] != "SPDXRef-DOCUMENT"
            || sbom["dataLicense"] != "CC0-1.0"
            || sbom["documentNamespace"].as_str().is_none_or(str::is_empty)
            || sbom["creationInfo"].as_object().is_none()
            || sbom["packages"].as_array().is_none_or(Vec::is_empty)
        {
            return Err("scanner did not produce a populated SPDX 2.3 document".into());
        }
        entry.sha256 = crate::hex(&Sha256::digest(&bytes));
        fs::remove_file(source).map_err(|e| e.to_string())?;
    }
    fs::write(
        temporary.path().join("sboms.json"),
        serde_json::to_vec_pretty(&entries).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary.path(), output).map_err(|e| e.to_string())?;
    Ok(entries)
}

/// Syft owns package discovery and SPDX serialization; this tool only checks
/// release coverage and the immutable image subject returned by Syft.
pub fn run(inventory: &Path, output: &Path, syft: &Path, timeout: Duration) -> Result<(), String> {
    if timeout.is_zero() {
        return Err("SBOM scanner timeout must be positive".into());
    }
    let inventory: Value =
        serde_json::from_slice(&read(inventory, LIMIT)?).map_err(|e| e.to_string())?;
    let entries = generate(&inventory, output, |reference, spdx, source| {
        let child = Command::new(syft)
            .arg(format!("registry:{reference}"))
            .args(["--select-catalogers", "+rust-cargo-lock-cataloger"])
            .arg("-o")
            .arg(format!("spdx-json={}", spdx.display()))
            .arg("-o")
            .arg(format!("syft-json={}", source.display()))
            .env("SYFT_CHECK_FOR_APP_UPDATE", "false")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "SBOM scanner could not start")?;
        let mut child = OwnedScanner(child);
        let started = Instant::now();
        loop {
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| "SBOM scanner could not be observed")?
            {
                if !status.success() {
                    return Err(format!(
                        "SBOM scanner failed with exit status {:?}",
                        status.code()
                    ));
                }
                break;
            }
            if started.elapsed() >= timeout {
                return Err("SBOM scanner timed out".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })?;
    println!(
        "{}",
        serde_json::to_string(&entries).map_err(|e| e.to_string())?
    );
    Ok(())
}

struct OwnedScanner(Child);
impl Drop for OwnedScanner {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
