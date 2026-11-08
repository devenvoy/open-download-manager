// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Description: Module definitions and exports for the application service orchestration layer.

pub mod advanced_integration_service;
pub mod clipboard_service;
pub mod credential_service;
mod download_concurrency_limiter;
mod download_concurrency_permit;
pub mod download_event_publisher;
pub mod download_executor;
mod download_history_policy;
pub mod download_history_repository;
pub mod download_history_service;
pub mod download_progress;
pub mod download_rate_limiter;
pub mod download_result;
mod download_transfer_policy;
pub mod download_transfer_service;
pub mod error;
pub mod integration_service;
pub mod proxy_discovery_service;
pub mod quota_service;
pub mod settings_service;
pub mod shell_service;
pub mod throughput_meter;
pub mod updater_service;
