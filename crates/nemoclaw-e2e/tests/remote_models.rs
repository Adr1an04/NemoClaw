// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
#![cfg(target_os = "linux")]

mod service_images;

use service_images::managed;
use service_images::support::Scenario;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit bundle, agent image/profile and proxy image; isolated SSH/Docker protocol and real agent image"]
// E04-S07
async fn remote_models_preserve_bindings_when_ssh_observation_fails() {
    let mut scenario = Scenario::start().await;
    let service = managed::ManagedService::start(&mut scenario).await;
    service.run(&scenario, "plan", true).await;
    assert_eq!(service.engine()["effects"], 0);
    scenario.assert_no_resources();
    service.run(&scenario, "apply", true).await;
    let remote = service.engine();
    service.assert_runtime_configuration();
    let agent = scenario.agent_identity("assistant-0");
    service.assert_remote_publication();
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");

    service.control(serde_json::json!({"transport_failure":true}));
    service.run(&scenario, "plan", false).await;
    assert_eq!(
        service.engine(),
        remote,
        "failed observation must not mutate remote resources"
    );
    assert_eq!(scenario.agent_identity("assistant-0"), agent);
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");

    service.control(serde_json::json!({}));
    let exported = service.run(&scenario, "export", true).await;
    assert_eq!(exported, serde_json::to_value(&service.document).unwrap());
    let unchanged = service.run(&scenario, "apply", true).await;
    assert_eq!(unchanged["changes"], serde_json::json!([]));
    assert_eq!(service.engine(), remote);
    assert_eq!(scenario.agent_identity("assistant-0"), agent);
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");

    service.run(&scenario, "destroy", true).await;
    assert_eq!(service.engine()["volume"], remote["volume"]);
    assert!(service.engine()["container"].is_null());
    assert!(service.engine()["network"].is_null());
    scenario.assert_agent_absent("assistant-0");
}
