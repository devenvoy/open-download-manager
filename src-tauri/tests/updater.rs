use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use awesome_downloader_lib::{
    application::{error::AppError, updater_service::UpdateService},
    domain::updater::{
        friendly_update_error, is_newer_version, is_no_published_release,
        InstallDecision, UpdateCheckMode, UpdateCheckOutcome, UpdateReleaseInfo,
        CHECK_INTERVAL_HOURS, CHECK_INTERVAL_SECONDS,
    },
    infrastructure::updater::{BoxFuture, UpdateClient},
};

struct MockUpdateClient {
    check_count: AtomicUsize,
    download_count: AtomicUsize,
    release_result: std::sync::Mutex<Option<Result<Option<UpdateReleaseInfo>, AppError>>>,
}

impl MockUpdateClient {
    fn new(result: Result<Option<UpdateReleaseInfo>, AppError>) -> Self {
        Self {
            check_count: AtomicUsize::new(0),
            download_count: AtomicUsize::new(0),
            release_result: std::sync::Mutex::new(Some(result)),
        }
    }
}

impl UpdateClient for MockUpdateClient {
    fn check_latest_release(&self) -> BoxFuture<'_, awesome_downloader_lib::application::error::AppResult<Option<UpdateReleaseInfo>>> {
        self.check_count.fetch_add(1, Ordering::SeqCst);
        let res = self
            .release_result
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Ok(None));
        Box::pin(async move { res })
    }

    fn download_update_installer<'a>(
        &'a self,
        _url: &'a str,
        _destination: &'a Path,
    ) -> BoxFuture<'a, awesome_downloader_lib::application::error::AppResult<()>> {
        self.download_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(()) })
    }
}

#[tokio::test]
async fn manual_check_reports_when_the_installed_version_is_current() {
    let mock = Arc::new(MockUpdateClient::new(Ok(Some(UpdateReleaseInfo {
        version: "0.1.2".to_string(),
        release_notes: None,
        download_url: None,
        published_at: None,
    }))));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Manual)
        .await
        .expect("check should succeed");

    assert_eq!(mock.check_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        outcome,
        UpdateCheckOutcome::UpToDate {
            version: "0.1.2".to_string()
        }
    );
}

#[tokio::test]
async fn automatic_current_version_checks_stay_silent() {
    let mock = Arc::new(MockUpdateClient::new(Ok(Some(UpdateReleaseInfo {
        version: "0.1.2".to_string(),
        release_notes: None,
        download_url: None,
        published_at: None,
    }))));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Background)
        .await
        .expect("check should succeed");

    assert_eq!(mock.check_count.load(Ordering::SeqCst), 1);
    assert_eq!(outcome, UpdateCheckOutcome::SilentNoop);
}

#[tokio::test]
async fn automatic_missing_release_checks_stay_silent() {
    let error = AppError::network(
        "Unable to find latest version on GitHub (https://github.com/Ahmadkhattak1/awesome-downloader/releases/latest): HTTPError: 406",
    );
    let mock = Arc::new(MockUpdateClient::new(Err(error)));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Background)
        .await
        .expect("check should complete");

    assert_eq!(mock.check_count.load(Ordering::SeqCst), 1);
    assert_eq!(outcome, UpdateCheckOutcome::SilentNoop);
}

#[test]
fn the_updater_schedules_silent_background_checks_every_six_hours() {
    assert_eq!(CHECK_INTERVAL_HOURS, 6);
    assert_eq!(CHECK_INTERVAL_SECONDS, 6 * 3600);
}

#[tokio::test]
async fn a_background_update_uses_notification_instead_of_dialog() {
    let mock = Arc::new(MockUpdateClient::new(Ok(Some(UpdateReleaseInfo {
        version: "0.1.3".to_string(),
        release_notes: Some("Bug fixes".to_string()),
        download_url: Some("https://example.com/Setup.exe".to_string()),
        published_at: None,
    }))));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Background)
        .await
        .expect("check should succeed");

    assert_eq!(mock.check_count.load(Ordering::SeqCst), 1);
    match outcome {
        UpdateCheckOutcome::NotifyAvailable(info) => {
            assert_eq!(info.version, "0.1.3");
        }
        other => panic!("expected NotifyAvailable, got {:?}", other),
    }
}

#[tokio::test]
async fn missing_github_production_release_is_treated_as_up_to_date_without_raw_response_data() {
    let error = AppError::network(
        "Unable to find latest version on GitHub (https://github.com/Ahmadkhattak1/awesome-downloader/releases/latest), please ensure a production release exists: HTTPError: 406\nHeaders: secret-noisy-response",
    );
    let mock = Arc::new(MockUpdateClient::new(Err(error)));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Manual)
        .await
        .expect("check should complete");

    assert_eq!(mock.check_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        outcome,
        UpdateCheckOutcome::UpToDate {
            version: "0.1.2".to_string()
        }
    );
}

#[tokio::test]
async fn real_update_errors_show_a_short_safe_explanation() {
    let error = AppError::network("ETIMEDOUT with a very long internal response");
    let mock = Arc::new(MockUpdateClient::new(Err(error)));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Manual)
        .await
        .expect("check should complete");

    match outcome {
        UpdateCheckOutcome::Error {
            friendly_detail, ..
        } => {
            assert!(friendly_detail.contains("internet connection"));
            assert!(!friendly_detail.contains("internal response"));
        }
        other => panic!("expected Error outcome, got {:?}", other),
    }
}

#[tokio::test]
async fn available_update_downloads_only_after_explicit_approval() {
    let mock = Arc::new(MockUpdateClient::new(Ok(Some(UpdateReleaseInfo {
        version: "0.1.3".to_string(),
        release_notes: None,
        download_url: Some("https://example.com/Setup.exe".to_string()),
        published_at: None,
    }))));

    let service = UpdateService::new(mock.clone(), "0.1.2", false);
    let outcome = service
        .check(UpdateCheckMode::Manual)
        .await
        .expect("check should succeed");

    assert_eq!(mock.check_count.load(Ordering::SeqCst), 1);
    assert_eq!(mock.download_count.load(Ordering::SeqCst), 0);

    match outcome {
        UpdateCheckOutcome::PromptToDownload(info) => {
            assert_eq!(info.version, "0.1.3");
        }
        other => panic!("expected PromptToDownload, got {:?}", other),
    }
}

#[test]
fn ready_update_defaults_to_later_while_downloads_are_active() {
    let mock = Arc::new(MockUpdateClient::new(Ok(None)));
    let service = UpdateService::new(mock, "0.1.2", false);

    assert_eq!(
        service.evaluate_install_decision(2),
        InstallDecision::WarnActiveDownloads(2)
    );
    assert_eq!(
        service.evaluate_install_decision(0),
        InstallDecision::ReadyToInstall
    );
}

#[tokio::test]
async fn development_build_never_contacts_github() {
    let mock = Arc::new(MockUpdateClient::new(Ok(Some(UpdateReleaseInfo {
        version: "0.1.3".to_string(),
        release_notes: None,
        download_url: None,
        published_at: None,
    }))));

    let service = UpdateService::new(mock.clone(), "0.1.2", true);

    let manual = service
        .check(UpdateCheckMode::Manual)
        .await
        .expect("manual check");
    assert_eq!(manual, UpdateCheckOutcome::DevModeUnavailable);
    assert_eq!(mock.check_count.load(Ordering::SeqCst), 0);

    let bg = service
        .check(UpdateCheckMode::Background)
        .await
        .expect("bg check");
    assert_eq!(bg, UpdateCheckOutcome::SilentNoop);
    assert_eq!(mock.check_count.load(Ordering::SeqCst), 0);
}

#[test]
fn version_comparison_and_error_classification_functions() {
    assert!(is_newer_version("0.1.2", "0.1.3"));
    assert!(is_newer_version("0.1.2", "1.0.0"));
    assert!(is_newer_version("v0.1.2", "v0.1.3"));
    assert!(!is_newer_version("0.1.2", "0.1.2"));
    assert!(!is_newer_version("0.1.3", "0.1.2"));

    assert!(is_no_published_release("Unable to find latest version on GitHub"));
    assert!(is_no_published_release("releases/latest HTTPError: 404"));
    assert!(!is_no_published_release("DNS resolution failure"));

    assert_eq!(
        friendly_update_error("ETIMEDOUT"),
        "Could not reach GitHub Releases. Check your internet connection and try again."
    );
    assert_eq!(
        friendly_update_error("500 Internal Server Error"),
        "GitHub Releases returned an unexpected response. Please try again later."
    );
}
