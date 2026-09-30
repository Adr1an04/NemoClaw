// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Image-owned launch and executable metadata, independent of Fabric descriptors.
use crate::{config::ExplicitPolicy, fabric_catalog::FabricAdapter};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRuntime {
    pub schema_version: u32,
    /// Prefix for the packaged bridge's serve, configure, check, status and health operations.
    pub command: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub required_paths: Vec<String>,
    pub policy: ExplicitPolicy,
    /// Resolved host interpreter and descriptor-required executables for each installed adapter.
    pub binaries: BTreeMap<String, Vec<String>>,
}

pub(crate) fn absolute(path: &str) -> bool {
    path.starts_with('/') && !path.contains('\0') && !path.split('/').any(|part| part == "..")
}

impl ImageRuntime {
    pub(crate) fn valid(&self, adapters: &[FabricAdapter]) -> bool {
        self.schema_version == 1
            && self.command.first().is_some_and(|path| absolute(path))
            && self
                .command
                .iter()
                .all(|part| !part.is_empty() && !part.contains('\0'))
            && self.environment.iter().all(|(key, value)| {
                !key.is_empty()
                    && !key.contains(['=', '\0'])
                    && !value.contains('\0')
                    && !matches!(
                        key.as_str(),
                        "NEMOCLAW_AGENT_NAME" | "NEMOCLAW_PROVIDER_NAMES"
                    )
            })
            && self
                .environment
                .get("ADAPTER_PYTHON")
                .is_some_and(|path| absolute(path))
            && !self.required_paths.is_empty()
            && self.required_paths.iter().all(|path| absolute(path))
            && self.policy.to_proto().is_ok_and(|policy| {
                policy.filesystem.is_some()
                    && policy.process.is_some()
                    && policy.network_policies.is_empty()
                    && policy.network_middlewares.is_empty()
            })
            && self.binaries.len() == adapters.len()
            && adapters.iter().all(|adapter| {
                self.binaries
                    .get(adapter.adapter_id())
                    .is_some_and(|paths| {
                        !paths.is_empty() && paths.iter().all(|path| absolute(path))
                    })
            })
    }
}
