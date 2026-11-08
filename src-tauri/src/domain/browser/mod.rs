// Created: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Last updated: 2026-11-08
// Reason: feat(browser): add configurable context menu actions & file capture rules (#29)
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Description: Domain models and capture policy types for browser extension integration.

pub mod advanced_integration;
pub mod capture_policy;
pub mod request;

pub use advanced_integration::{
    filter_advanced_media_candidate, AdvancedIntegrationAvailability, AdvancedIntegrationCapability,
    AdvancedIntegrationState, AdvancedMediaCandidate, HealthCheckResult, MediaFilterError,
    RollbackAction,
};
pub use capture_policy::{CaptureDecision, CapturePolicy, ConfirmationMode};
pub use request::{sanitize_browser_request, BrowserIncomingMessage, ValidatedBrowserRequest};
