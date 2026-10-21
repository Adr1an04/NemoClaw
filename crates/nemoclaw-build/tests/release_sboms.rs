// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
use nemoclaw_build::release_sboms::generate;
use serde_json::{Value, json};
use std::{fs, path::Path};

fn inventory() -> Value {
    json!([{"name":"hermes", "repository":"ghcr.io/nvidia/nemoclaw/v1/hermes",
    "manifests": {"manifests": [
        {"digest":format!("sha256:{}", "a".repeat(64)),"platform":{"os":"linux","architecture":"amd64"}},
        {"digest":format!("sha256:{}", "b".repeat(64)),"platform":{"os":"linux","architecture":"arm64"}}
    ]}}])
}
fn scan(reference: &str, spdx: &Path, source: &Path) -> Result<(), String> {
    fs::write(
        spdx,
        serde_json::to_vec(
            &json!({"spdxVersion":"SPDX-2.3", "SPDXID":"SPDXRef-DOCUMENT",
        "dataLicense":"CC0-1.0", "name":"image", "documentNamespace":"https://example.test/sbom",
        "creationInfo":{"creators":["Tool: syft"],"created":"2026-10-20T00:00:00Z"},
        "packages":[{"name":"libc6","versionInfo":"2.41","SPDXID":"SPDXRef-libc"}]}),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(
        source,
        serde_json::to_vec(&json!({"source":{"type":"image", "metadata":{
        "manifestDigest":reference.split('@').nth(1).unwrap(),
        "os":"linux", "architecture": if reference.ends_with(&"a".repeat(64)) { "amd64" } else { "arm64" }}}}))
        .unwrap(),
    )
    .unwrap();
    Ok(())
}
#[test]
fn every_platform_gets_a_digest_bound_asset() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("sboms");
    let entries = generate(&inventory(), &output, scan).unwrap();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].asset, entries[1].asset);
    for entry in entries {
        assert!(output.join(entry.asset).is_file());
        assert!(entry.reference.ends_with(&entry.digest));
        assert_eq!(entry.sha256.len(), 64);
    }
    assert!(output.join("sboms.json").is_file());
}

#[test]
fn explicitly_annotated_auxiliary_attestations_do_not_add_executable_scan_subjects() {
    let temp = tempfile::tempdir().unwrap();
    let mut input = inventory();
    input[0]["manifests"]["manifests"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "digest":format!("sha256:{}", "c".repeat(64)),
            "platform":{"os":"unknown","architecture":"unknown"},
            "annotations":{"vnd.docker.reference.type":"attestation-manifest"}
        }));
    let mut calls = 0;
    let entries = generate(
        &input,
        &temp.path().join("assets"),
        |reference, spdx, source| {
            calls += 1;
            scan(reference, spdx, source)
        },
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(calls, 2);
}

#[test]
fn missing_duplicate_or_unexpected_platforms_never_reach_the_scanner() {
    for case in [
        "missing",
        "duplicate",
        "unsupported",
        "digest",
        "name",
        "repository",
        "duplicate_image",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut input = inventory();
        match case {
            "missing" => {
                input[0]["manifests"]["manifests"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "duplicate" => {
                input[0]["manifests"]["manifests"][1] =
                    input[0]["manifests"]["manifests"][0].clone()
            }
            "unsupported" => {
                input[0]["manifests"]["manifests"][1]["platform"]["architecture"] = json!("riscv64")
            }
            "digest" => input[0]["manifests"]["manifests"][0]["digest"] = json!("sha256:wrong"),
            "name" => input[0]["name"] = json!("../hermes"),
            "repository" => input[0]["repository"] = json!("ghcr.io/other/hermes"),
            "duplicate_image" => {
                let duplicate = input[0].clone();
                input.as_array_mut().unwrap().push(duplicate);
            }
            _ => unreachable!(),
        }
        let output = temp.path().join("sboms");
        assert!(
            generate(&input, &output, |_, _, _| panic!(
                "invalid inventory scanned"
            ))
            .is_err(),
            "{case}"
        );
        assert!(!output.exists());
    }
}

#[test]
fn substituted_digest_invalid_or_oversized_sbom_and_scanner_failure_leave_no_assets() {
    for case in [
        "digest",
        "platform",
        "missing",
        "invalid",
        "empty",
        "oversized",
        "scanner",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("sboms");
        let mut count = 0;
        let result = generate(&inventory(), &output, |reference, spdx, source| {
            count += 1;
            scan(reference, spdx, source)?;
            if count == 2 {
                match case {
                    "digest" => scan(&format!("image@sha256:{}", "c".repeat(64)), spdx, source)?,
                    "platform" => {
                        let mut evidence: Value =
                            serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
                        evidence["source"]["metadata"]["os"] = json!("windows");
                        fs::write(source, serde_json::to_vec(&evidence).unwrap()).unwrap();
                    }
                    "missing" => fs::remove_file(spdx).unwrap(),
                    "invalid" => fs::write(spdx, b"not json").unwrap(),
                    "empty" => fs::write(spdx, b"{}").unwrap(),
                    "oversized" => {
                        let mut bytes = fs::read(spdx).unwrap();
                        bytes.extend(std::iter::repeat_n(b' ', 16_000_001));
                        fs::write(spdx, bytes).unwrap();
                    }
                    "scanner" => return Err("scanner failed".into()),
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert!(result.is_err(), "{case}");
        assert!(!output.exists(), "partial assets published for {case}");
        // A failed generation can restart with a fresh complete inventory.
        assert_eq!(generate(&inventory(), &output, scan).unwrap().len(), 2);
    }
}

#[test]
fn existing_output_cannot_supply_stale_assets_to_a_rerun() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("sboms");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("hermes-linux-amd64.spdx.json"), "stale").unwrap();
    assert!(
        generate(&inventory(), &output, |_, _, _| panic!(
            "stale output reused"
        ))
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(output.join("hermes-linux-amd64.spdx.json")).unwrap(),
        "stale"
    );
}

#[cfg(unix)]
#[test]
fn stalled_sbom_generation_times_out_without_leaking_diagnostics_or_partial_assets() {
    use std::{os::unix::fs::PermissionsExt, process::Command, time::Instant};
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("inventory.json");
    fs::write(&input, serde_json::to_vec(&inventory()).unwrap()).unwrap();
    let syft = temp.path().join("syft");
    fs::write(
        &syft,
        "#!/bin/sh\nprintf secret-canary >&2\nexec /bin/sleep 30\n",
    )
    .unwrap();
    fs::set_permissions(&syft, fs::Permissions::from_mode(0o700)).unwrap();
    let assets = temp.path().join("assets");
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_nemoclaw-build"))
        .arg("release-sboms")
        .arg("--inventory")
        .arg(&input)
        .arg("--output")
        .arg(&assets)
        .arg("--syft")
        .arg(&syft)
        .args(["--timeout-seconds", "1"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SBOM scanner timed out"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-canary"));
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert!(!assets.exists());
}

#[cfg(unix)]
#[test]
fn release_command_scans_immutable_references_and_reports_the_complete_asset_mapping() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let input = root.join("inventory.json");
    fs::write(&input, serde_json::to_vec(&inventory()).unwrap()).unwrap();
    for (arch, hex) in [("amd64", "a"), ("arm64", "b")] {
        scan(
            &format!("image@sha256:{}", hex.repeat(64)),
            &root.join(format!("{arch}.spdx")),
            &root.join(format!("{arch}.source")),
        )
        .unwrap();
    }
    let syft = root.join("syft");
    // Substitute only the external scanner; run the real CLI and filesystem
    // publication path, with separate output files for each immutable input.
    fs::write(&syft, r#"#!/bin/sh
set -eu
root=$(dirname "$0")
case "$1" in
  registry:ghcr.io/nvidia/nemoclaw/v1/hermes@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) arch=amd64 ;;
  registry:ghcr.io/nvidia/nemoclaw/v1/hermes@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) arch=arm64 ;;
  *) exit 9 ;;
esac
shift
for argument do
  case "$argument" in
    spdx-json=*) cp "$root/$arch.spdx" "${argument#spdx-json=}" ;;
    syft-json=*) cp "$root/$arch.source" "${argument#syft-json=}" ;;
  esac
done
"#).unwrap();
    fs::set_permissions(&syft, fs::Permissions::from_mode(0o700)).unwrap();
    let output = root.join("assets");
    let result = Command::new(env!("CARGO_BIN_EXE_nemoclaw-build"))
        .arg("release-sboms")
        .arg("--inventory")
        .arg(input)
        .arg("--output")
        .arg(&output)
        .arg("--syft")
        .arg(syft)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reported: Value = serde_json::from_slice(&result.stdout).unwrap();
    let saved: Value =
        serde_json::from_slice(&fs::read(output.join("sboms.json")).unwrap()).unwrap();
    assert_eq!(reported, saved);
    assert_eq!(saved.as_array().unwrap().len(), 2);
    for entry in saved.as_array().unwrap() {
        let bytes = fs::read(output.join(entry["asset"].as_str().unwrap())).unwrap();
        use sha2::{Digest, Sha256};
        assert_eq!(entry["sha256"], nemoclaw_build::hex(&Sha256::digest(bytes)));
    }
}

#[cfg(unix)]
#[test]
fn public_attestation_approval_rejects_missing_or_empty_required_reviewers() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let workflow: Value =
        serde_saphyr::from_str(include_str!("../../../.github/workflows/release.yml")).unwrap();
    let script = workflow["jobs"]["approve-sboms"]["steps"][0]["run"]
        .as_str()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let gh = root.join("gh");
    // Evaluate the workflow's actual jq policy against each environment fixture.
    fs::write(
        &gh,
        r#"#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
  if [ "$1" = --jq ]; then
    shift
    exec jq -r "$1" "$POLICY"
  fi
  shift
done
exit 9
"#,
    )
    .unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o700)).unwrap();
    for (policy, expected) in [
        (json!({"protection_rules":[]}), false),
        (
            json!({"protection_rules":[{"type":"branch_policy"}]}),
            false,
        ),
        (
            json!({"protection_rules":[{"type":"required_reviewers","reviewers":[]}]}),
            false,
        ),
        (
            json!({"protection_rules":[{"type":"required_reviewers","reviewers":[{"type":"User","reviewer":{"login":"release-reviewer"}}]}]}),
            true,
        ),
    ] {
        let path = root.join("policy.json");
        fs::write(&path, serde_json::to_vec(&policy).unwrap()).unwrap();
        let result = Command::new("bash")
            .args([
                "--noprofile",
                "--norc",
                "-e",
                "-o",
                "pipefail",
                "-c",
                script,
            ])
            .env_clear()
            .env(
                "PATH",
                format!("{}:{}", root.display(), std::env::var("PATH").unwrap()),
            )
            .env("POLICY", path)
            .env("GH_TOKEN", "fixture-token")
            .env("GITHUB_REPOSITORY", "NVIDIA/NemoClaw")
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            expected,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
