// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: Buffer sizes, retry backoffs, and timeout limits for HTTP download streams.

pub(crate) const CONNECT_TIMEOUT_SECONDS: u64 = 30;
pub(crate) const READ_TIMEOUT_SECONDS: u64 = 30;
pub(crate) const MAX_REDIRECTS: usize = 10;
pub(crate) const PROGRESS_EVENT_INTERVAL_MILLISECONDS: u64 = 250;
pub(crate) const MAX_TRANSIENT_RETRIES: u8 = 3;
pub(crate) const INITIAL_RETRY_DELAY_MILLISECONDS: u64 = 250;
pub(crate) const MAX_RETRY_DELAY_SECONDS: u64 = 30;
pub(crate) const TEMPORARY_DOWNLOAD_PREFIX: &str = ".open-download-manager-";
pub(crate) const CONCAT_BUFFER_SIZE: usize = 256 * 1024;
