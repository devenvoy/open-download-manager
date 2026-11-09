// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Description: Policies defining maximum retry attempts, backoff schedules, and connection bounds.

pub(super) const HTTP_DOWNLOAD_KIND: &str = "http";
pub(super) const NEW_DOWNLOAD_IS_ESTIMATED: bool = false;
pub(super) const NEW_DOWNLOAD_IS_RESUMABLE: bool = false;
pub(super) const SCHEDULER_POLL_INTERVAL_MILLISECONDS: u64 = 1_000;
