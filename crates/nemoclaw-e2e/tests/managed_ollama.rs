// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
#![cfg(target_os = "linux")]

mod service_images;

use service_images::managed;
use service_images::support::Scenario;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit bundle, agent image/profile and proxy image; protocol Ollama service with real image execution"]
// E04-S02
async fn managed_ollama_recovers_and_retains_model_storage() {
    let mut scenario = Scenario::start().await;
    let service = managed::ManagedService::start(&mut scenario).await;
    service.control(serde_json::json!({"capacity_failure":true}));
    service.run(&scenario, "plan", true).await;
    assert_eq!(service.engine()["effects"], 0);
    assert!(!service.has_capacity_reads());
    scenario.assert_no_resources();

    service.control(serde_json::json!({"create_failure":true}));
    service.run(&scenario, "apply", false).await;
    let partial = service.engine();
    assert!(partial["volume"].is_object());
    assert!(partial["container"].is_null());
    scenario.assert_agent_absent("assistant-0");

    service.control(serde_json::json!({}));
    service.run(&scenario, "apply", true).await;
    let ready = service.engine();
    service.assert_runtime_configuration();
    assert_eq!(ready["volume"], partial["volume"]);
    assert_eq!(ready["container"]["State"]["Running"], true);
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");
    let agent = scenario.agent_identity("assistant-0");
    let exported = service.run(&scenario, "export", true).await;
    assert_eq!(exported, serde_json::to_value(&service.document).unwrap());
    let unchanged = service.run(&scenario, "apply", true).await;
    assert_eq!(unchanged["changes"], serde_json::json!([]));
    assert_eq!(
        service.engine()["container"]["Id"],
        ready["container"]["Id"]
    );
    assert_eq!(scenario.agent_identity("assistant-0"), agent);
    scenario.assert_agent_responds("assistant-0");

    service.run(&scenario, "destroy", true).await;
    let destroyed = service.engine();
    assert!(destroyed["container"].is_null());
    assert!(destroyed["network"].is_null());
    assert_eq!(destroyed["volume"], ready["volume"]);
    scenario.assert_agent_absent("assistant-0");
}
