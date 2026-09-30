<!-- SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Runtime Policy Rejection on Linux ARM64

The [issue #12464](https://github.com/NVIDIA/NemoClaw/issues/12464) rejection regression passed at implementation commit `0a423e35ce15c34ae9d8141f9373482f8a119ac6`, using bundle `0.1.0-dev.954f5b7677b8f680`.
The bundle was built with Rust 1.98.1 and OpenTofu 1.12.6 on Linux ARM64 on 2026-09-29.
Tests use temporary state and local OpenShell protocol fixtures.
They create no live containers or model services and send no model or agent requests.

## Reproduction and Observed Behavior

The previous bundle, `0.1.0-dev.652ef254724527fe`, ignored an explicit configuration-admission rejection from the fixture gateway.
The bundled regression failed after 123.17 seconds with `observation query failed`, without the sandbox name or repair reason.
The direct startup regression also failed before implementation because the reported rejection did not stop the wait.

Agent configuration and sandbox completion now check the bound sandbox's admission before issuing runtime commands.
An explicit `Rejected` state stops the wait, including while the sandbox is `Starting`.
The provider preserves four fixed diagnostics from the pinned gateway and replaces unknown text with a fixed repair message.
CLI text and JSON failures name the sandbox and preserve created bindings.
The [policy guide](../sandbox-network.md#recover-from-runtime-policy-rejection) describes recovery and policy-replacement constraints.

The [direct OpenShell regression](../../crates/nemoclaw-e2e/tests/openshell.rs) checks rejection in `Starting` and `Ready`, safe diagnostics, no exec or resource mutation, identity substitution, authentication and transport failures, retained refresh, and teardown.
All 12 direct OpenShell tests passed.
The [bundled lifecycle regression](../../crates/nemoclaw-e2e/tests/deployment.rs) starts from the explicit-policy example and requires each rejected CLI apply to finish within 15 seconds.
It verifies immediate destroy after failed first apply and, separately, recovery after simulated gateway acceptance without replacing the sandbox, followed by export, unchanged apply, and destroy.
The complete two-branch test passed in 18.46 seconds.
The [standalone sandbox completion fixture](../../crates/nemoclaw-e2e/tests/sandbox_readiness.rs) verifies prompt rejection after earlier successful configuration, a safe fallback in retained observations, unchanged bindings, and teardown without readiness.
It passed against the same bundled provider.
See [fixture instructions](../testing/fixtures.md#opentofu-and-bundle-lifecycle) for explicit bundle selection.

Workspace checks passed: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --no-fail-fast -- --test-threads=4` (933 passed, zero failed, 131 ignored).
Ignored tests require explicit selection and are not counted as workspace passes.
All 46 explicitly selected bundle fixtures passed: 29 deployment, four export-observation, 12 Fabric, and one multiple-provider test.
The standalone sandbox completion test adds one separately selected pass against the same bundled provider.
Independent documentation review and documentation validation passed with zero errors and one existing Fern warning.

## Qualification Limits

These checks qualify explicit gateway rejection handling and recovery through real OpenTofu and the production provider with simulated OpenShell responses.
They do not identify the exact policy rule that caused the original live report or qualify kernel enforcement, image contents, native agent responses, or other platforms.
A pending or absent admission report still follows the ordinary startup wait.
A rejected policy update represented by a previously accepted admission carrying an error is outside this startup-rejection check.
The image catalog records adapter requirements and runtime files, not a complete filesystem or executable inventory; arbitrary executable paths and process identities are not newly validated by this change.
Existing explicit filesystem-grant checks remain in effect.
