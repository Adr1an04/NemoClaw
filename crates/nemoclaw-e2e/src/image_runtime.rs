// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use nemoclaw_sdk::{
    compile::{Generations, Target},
    config::Document,
    image_runtime::RuntimeBinding,
};

pub fn binding(adapter_id: &str) -> RuntimeBinding {
    let mut runtime: serde_json::Value =
        serde_json::from_str(include_str!("../../../image/fabric/runtime.json")).unwrap();
    runtime["binaries"] = serde_json::json!({adapter_id:["/usr/local/bin/python3.99"]});
    RuntimeBinding::from_json(
        &serde_json::json!({"runtime":runtime,"adapter_id":adapter_id}).to_string(),
    )
    .unwrap()
}

pub fn policy() -> openshell_core::proto::SandboxPolicy {
    binding("fixture").runtime.policy.to_proto().unwrap()
}

/// Resolve metadata inputs explicitly for protocol tests that bypass OpenTofu discovery.
pub fn targets(
    document: &Document,
    generations: &Generations,
) -> Result<Vec<Target>, nemoclaw_sdk::config::ConfigError> {
    let mut targets = nemoclaw_sdk::compile::targets(document, generations)?;
    for target in &mut targets {
        if target.kind == "provider_profile" {
            target.values.insert(
                "binaries_json".into(),
                serde_json::json!(["/usr/local/bin/python3.99"]).to_string(),
            );
        } else if target.kind == "sandbox" {
            let sandbox = document
                .spec
                .sandboxes
                .iter()
                .find(|sandbox| sandbox.name == target.values["name"])
                .unwrap();
            let requirements = nemoclaw_sdk::fabric_capabilities::FabricRequirements::for_sandbox(
                document, sandbox,
            )?;
            let adapter = requirements.configuration["harness"]["adapter_id"]
                .as_str()
                .unwrap();
            target.values.insert(
                "runtime_json".into(),
                serde_json::to_string(&binding(adapter)).unwrap(),
            );
        }
    }
    Ok(targets)
}
