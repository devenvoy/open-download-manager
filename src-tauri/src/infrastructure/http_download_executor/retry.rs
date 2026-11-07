use std::time::Duration;

use crate::application::{
    download_executor::{stopped_error, DownloadControl},
    error::{AppError, AppResult},
};

use super::policy::{
    INITIAL_RETRY_DELAY_MILLISECONDS, MAX_RETRY_DELAY_SECONDS, MAX_TRANSIENT_RETRIES,
};

pub(crate) async fn send_with_retry(
    control: &mut DownloadControl,
    mut build_request: impl FnMut() -> reqwest::RequestBuilder,
) -> AppResult<reqwest::Response> {
    for retry in 0..=MAX_TRANSIENT_RETRIES {
        let response = tokio::select! {
            reason = control.stopped() => return Err(stopped_error(reason)),
            response = build_request().send() => response,
        };
        let response = match response {
            Ok(response) => response,
            Err(error)
                if retry < MAX_TRANSIENT_RETRIES && (error.is_connect() || error.is_timeout()) =>
            {
                wait_before_retry(control, retry_delay(retry, None)).await?;
                continue;
            }
            Err(_) => return Err(AppError::download("Could not connect to the server.")),
        };
        if retry < MAX_TRANSIENT_RETRIES && is_retryable_status(response.status()) {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after);
            drop(response);
            wait_before_retry(control, retry_delay(retry, retry_after)).await?;
            continue;
        }
        return Ok(response);
    }
    Err(AppError::download("Could not connect to the server."))
}

pub(crate) fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

pub(crate) fn retry_delay(retry: u8, retry_after_seconds: Option<u64>) -> Duration {
    let maximum_delay = Duration::from_secs(MAX_RETRY_DELAY_SECONDS);
    let delay = retry_after_seconds
        .map(Duration::from_secs)
        .unwrap_or_else(|| {
            let multiplier = 1_u64 << u32::from(retry);
            Duration::from_millis(INITIAL_RETRY_DELAY_MILLISECONDS.saturating_mul(multiplier))
        });
    delay.min(maximum_delay)
}

pub(crate) fn parse_retry_after(header_value: &str) -> Option<u64> {
    let trimmed = header_value.trim();
    if let Ok(seconds) = trimmed.parse::<u64>() {
        return Some(seconds);
    }
    if let Ok(date) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        let now = chrono::Utc::now();
        let diff = date.with_timezone(&chrono::Utc).signed_duration_since(now);
        if let Ok(secs) = diff.num_seconds().try_into() {
            return Some(secs);
        }
    }
    None
}

pub(crate) async fn wait_before_retry(
    control: &mut DownloadControl,
    delay: Duration,
) -> AppResult<()> {
    tokio::select! {
        reason = control.stopped() => Err(stopped_error(reason)),
        () = tokio::time::sleep(delay) => Ok(()),
    }
}
