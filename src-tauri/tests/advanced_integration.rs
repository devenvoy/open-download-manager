// Created: 2026-11-08
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Description: Integration tests verifying advanced integration capability, consent, auto-rollback, and privacy guardrails.

use std::{path::PathBuf, sync::Arc, time::Duration};

use open_download_manager_lib::{
    application::advanced_integration_service::AdvancedIntegrationService,
    domain::browser::advanced_integration::{
        AdvancedIntegrationAvailability, AdvancedIntegrationState, AdvancedMediaCandidate,
    },
    infrastructure::browser::advanced_adapter::PlatformAdvancedAdapter,
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("odm-advanced-integration-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[test]
fn human_scenario_disabled_by_default_and_independent_from_standard_integration() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    let service = AdvancedIntegrationService::new(adapter);

    let cap = service.capability();
    assert_eq!(cap.state, AdvancedIntegrationState::Disabled);
    assert!(cap.can_enable());
    assert!(!cap.is_active());
    assert!(cap.required_permission.is_some());
    assert!(cap.rollback_action.is_none());
}

#[test]
fn human_scenario_consent_rejection_prevents_activation() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    let service = AdvancedIntegrationService::new(adapter);

    // Attempting to enable without explicit consent must fail
    let result = service.enable(false);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("consent is required"));

    let cap = service.capability();
    assert_eq!(cap.state, AdvancedIntegrationState::AwaitingConsent);
    assert!(!cap.is_active());
}

#[test]
fn human_scenario_successful_activation_with_bounded_health_verification() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    let service = AdvancedIntegrationService::new(adapter);

    let cap = service.enable(true).expect("enable with consent succeeds");
    assert_eq!(cap.state, AdvancedIntegrationState::Active);
    assert!(cap.is_active());

    let health = cap.last_health_check.expect("health check completed");
    assert!(health.success);
    assert!(!health.conflict_detected);
    assert!(health.latency_ms < 3000);
    assert!(cap.rollback_action.is_none());
}

#[test]
fn human_scenario_health_check_failure_triggers_automatic_rollback() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    adapter.set_force_health_failure(Some("Packet filter loopback handshake failed".to_owned()));

    let service = AdvancedIntegrationService::new(adapter);
    let result = service.enable(true);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.message.contains("rolled back to standard integration"));

    let cap = service.capability();
    match &cap.state {
        AdvancedIntegrationState::Degraded { reason, rolled_back } => {
            assert!(rolled_back);
            assert!(reason.contains("Packet filter loopback handshake failed"));
        }
        other => panic!("Expected Degraded state, got {other:?}"),
    }

    let rollback = cap.rollback_action.expect("rollback action recorded");
    assert!(rollback.executed);
    assert!(rollback.standard_integration_intact);
    assert!(rollback.uninstalled);
}

#[test]
fn human_scenario_conflict_detection_triggers_automatic_rollback() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    adapter.set_force_conflict(Some("Third-Party Endpoint Firewall / WFP filter".to_owned()));

    let service = AdvancedIntegrationService::new(adapter);
    let result = service.enable(true);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.message.contains("Conflict with third-party software detected"));

    let cap = service.capability();
    match &cap.state {
        AdvancedIntegrationState::Degraded { reason, rolled_back } => {
            assert!(rolled_back);
            assert!(reason.contains("Third-Party Endpoint Firewall"));
        }
        other => panic!("Expected Degraded state, got {other:?}"),
    }
}

#[test]
fn human_scenario_bounded_health_check_timeout_prevents_hanging() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    // Simulate a hung probe taking 500ms when timeout is 50ms
    adapter.set_simulated_delay(Some(Duration::from_millis(500)));

    let service = AdvancedIntegrationService::with_timeout(adapter, Duration::from_millis(50));
    let result = service.enable(true);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.message.contains("timed out") || err.message.contains("Health check error"));

    let cap = service.capability();
    assert!(matches!(cap.state, AdvancedIntegrationState::Degraded { .. }));
    assert!(cap.rollback_action.is_some());
}

#[test]
fn human_scenario_unsupported_platform_shows_explicit_disabled_status() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::with_supported(temp_dir, false));
    let service = AdvancedIntegrationService::new(adapter);

    let cap = service.capability();
    assert!(!cap.is_supported());
    assert!(!cap.can_enable());
    assert!(matches!(cap.state, AdvancedIntegrationState::Unsupported { .. }));
    assert!(matches!(cap.availability, AdvancedIntegrationAvailability::Unsupported { .. }));

    // Cannot enable on unsupported platform
    let result = service.enable(true);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("Cannot enable advanced integration"));
}

#[test]
fn human_scenario_manual_disable_uninstalls_cleanly() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    let service = AdvancedIntegrationService::new(adapter);

    // First enable
    service.enable(true).expect("enable succeeds");
    assert!(service.capability().is_active());

    // Then user manually disables
    let cap = service.disable().expect("disable succeeds");
    assert_eq!(cap.state, AdvancedIntegrationState::Disabled);
    assert!(!cap.is_active());

    let rollback = cap.rollback_action.expect("rollback action recorded");
    assert!(rollback.executed);
    assert!(rollback.standard_integration_intact);
}

#[test]
fn human_scenario_privacy_guardrail_only_approves_media_and_redacts_tokens() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    let service = AdvancedIntegrationService::new(adapter);
    service.enable(true).expect("enable succeeds");

    // 1. Unapproved candidate is rejected
    let unapproved = AdvancedMediaCandidate {
        url: "https://example.com/video.mp4".to_owned(),
        media_type: Some("video/mp4".to_owned()),
        user_approved: false,
        filename: None,
        referer: None,
    };
    assert!(service.filter_candidate(unapproved).is_err());

    // 2. Unrelated browsing traffic (e.g. html, js, analytics) is rejected
    let unrelated = AdvancedMediaCandidate {
        url: "https://example.com/tracker.js".to_owned(),
        media_type: Some("application/javascript".to_owned()),
        user_approved: true,
        filename: None,
        referer: None,
    };
    let err = service.filter_candidate(unrelated).unwrap_err();
    assert!(err.message.contains("Unrelated web traffic discarded"));

    // 3. Embedded credentials rejected
    let with_creds = AdvancedMediaCandidate {
        url: "https://admin:secret@example.com/stream.m3u8".to_owned(),
        media_type: Some("application/vnd.apple.mpegurl".to_owned()),
        user_approved: true,
        filename: None,
        referer: None,
    };
    let err = service.filter_candidate(with_creds).unwrap_err();
    assert!(err.message.contains("URLs with embedded credentials"));

    // 4. Valid media with ephemeral token has token scrubbed
    let valid_media = AdvancedMediaCandidate {
        url: "https://cdn.example.com/stream/file.mp4?auth_token=supersecret123&signature=abc&expires=99999&track=1".to_owned(),
        media_type: Some("video/mp4".to_owned()),
        user_approved: true,
        filename: Some("movie.mp4".to_owned()),
        referer: Some("https://example.com/watch".to_owned()),
    };
    let approved = service.filter_candidate(valid_media).expect("valid media accepted");
    assert_eq!(approved.url, "https://cdn.example.com/stream/file.mp4?track=1");
    assert!(!approved.url.contains("auth_token"));
    assert!(!approved.url.contains("signature"));
    assert!(!approved.url.contains("expires"));
}

#[test]
fn human_scenario_repair_flow_recovers_from_degraded_state() {
    let temp_dir = temporary_directory();
    let adapter = Arc::new(PlatformAdvancedAdapter::new(temp_dir));
    // Fail first
    adapter.set_force_health_failure(Some("Temporary interface conflict".to_owned()));
    let service = AdvancedIntegrationService::new(adapter.clone());
    let _ = service.enable(true);
    assert!(matches!(service.capability().state, AdvancedIntegrationState::Degraded { .. }));

    // Conflict resolves; repair is triggered
    adapter.set_force_health_failure(None);
    let repaired = service.repair().expect("repair succeeds");
    assert_eq!(repaired.state, AdvancedIntegrationState::Active);
    assert!(repaired.is_active());
}
