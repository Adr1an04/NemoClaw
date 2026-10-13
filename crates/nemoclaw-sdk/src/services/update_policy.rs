// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// Whether updates may stop an existing model service before its replacement is ready.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum ServiceUpdatePolicy {
    /// Keep existing services and reject changes that require their replacement.
    #[default]
    Preserve,
    /// Permit sequential replacement, including downtime if the replacement cannot start.
    Recreate,
}

impl ServiceUpdatePolicy {
    pub(crate) fn is_preserve(&self) -> bool {
        *self == Self::Preserve
    }
}
