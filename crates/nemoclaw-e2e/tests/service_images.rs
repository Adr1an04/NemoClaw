// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
#![cfg(target_os = "linux")]

#[path = "service_images/managed.rs"]
mod managed;
#[path = "service_images/model_server.rs"]
mod model_server;
#[path = "service_images/support.rs"]
mod support;

use nemoclaw_sdk::{CancellationToken, Deployment};
use support::Scenario;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit bundle, agent image/profile and proxy image; creates owned Docker resources"]
// E02-S10
async fn shared_service_preserves_consumers_and_export_reapply() {
    let scenario = Scenario::start().await;
    let mut document = scenario.proxy_document(2).await;
    let deployment = Deployment::new(scenario.state.path(), &scenario.bundle);
    let cancel = CancellationToken::new();

    let plan = deployment.plan(&document, &cancel).await.unwrap();
    assert!(!plan.changes.is_empty());
    scenario.assert_no_resources();
    deployment.apply(&document, &cancel).await.unwrap();
    let service = scenario.service_identity(&document);
    let survivor = scenario.agent_identity("assistant-1");
    scenario.assert_shared_service(&document, 2);
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");
    scenario.assert_agent_service_access("assistant-1");
    scenario.assert_agent_responds("assistant-1");

    let exported = scenario.export();
    assert_eq!(exported, document);
    assert!(
        deployment
            .apply(&exported, &cancel)
            .await
            .unwrap()
            .changes
            .is_empty()
    );
    assert_eq!(scenario.service_identity(&document), service);
    assert_eq!(scenario.agent_identity("assistant-1"), survivor);

    let original = document.clone();
    document.spec.sandboxes.remove(0);
    let error = deployment.apply(&document, &cancel).await.unwrap_err();
    assert!(
        matches!(error, nemoclaw_sdk::Error::SandboxChangeRefused { sandbox, action: "remove" } if sandbox == "assistant-0")
    );
    document = original;
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");
    assert_eq!(scenario.agent_identity("assistant-1"), survivor);
    assert_eq!(scenario.service_identity(&document), service);
    scenario.assert_shared_service(&document, 2);
    scenario.assert_agent_service_access("assistant-1");
    scenario.assert_agent_responds("assistant-1");

    // Reopen from exported configuration: the service reference and both
    // consumers must survive the configuration handoff without reconstruction.
    let exported = scenario.export();
    assert_eq!(exported, document);
    let reopened = Deployment::new(scenario.state.path(), &scenario.bundle);
    assert!(
        reopened
            .apply(&exported, &cancel)
            .await
            .unwrap()
            .changes
            .is_empty()
    );
    assert_eq!(scenario.service_identity(&document), service);
    scenario.destroy();
    scenario.assert_agent_absent("assistant-0");
    scenario.assert_agent_absent("assistant-1");
    scenario.assert_service_destroyed_with_credentials_retained(&document);
    scenario.assert_external_server_alive().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit bundle, agent image/profile and proxy image; creates owned Docker resources"]
// E04-S01
async fn existing_ollama_is_verified_reused_and_retained() {
    let scenario = Scenario::start().await;
    let document = scenario.proxy_document(1).await;
    let deployment = Deployment::new(scenario.state.path(), &scenario.bundle);
    let cancel = CancellationToken::new();

    // A pre-existing model is identified by its digest, not just its name.
    let mut wrong: serde_json::Value = serde_json::to_value(&document).unwrap();
    wrong["spec"]["services"]["shared"]["upstream"]["model"]["digest"] =
        serde_json::json!("b".repeat(64));
    let wrong = nemoclaw_sdk::config::Document::parse(wrong.to_string().as_bytes()).unwrap();
    let error = deployment.apply(&wrong, &cancel).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("external Ollama model digest changed"),
        "{error}"
    );
    scenario.assert_no_resources();
    scenario.assert_external_server_alive().await;

    deployment.apply(&document, &cancel).await.unwrap();
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");
    let agent = scenario.agent_identity("assistant-0");
    let service = scenario.service_identity(&document);
    let exported = scenario.export();
    assert_eq!(exported, document);
    let reopened = Deployment::new(scenario.state.path(), &scenario.bundle);
    assert!(
        reopened
            .apply(&exported, &cancel)
            .await
            .unwrap()
            .changes
            .is_empty()
    );
    assert_eq!(scenario.agent_identity("assistant-0"), agent);
    assert_eq!(scenario.service_identity(&document), service);
    scenario.assert_agent_service_access("assistant-0");
    scenario.assert_agent_responds("assistant-0");

    scenario.destroy();
    scenario.assert_agent_absent("assistant-0");
    scenario.assert_service_destroyed_with_credentials_retained(&document);
    scenario.assert_external_server_alive().await;
}

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
