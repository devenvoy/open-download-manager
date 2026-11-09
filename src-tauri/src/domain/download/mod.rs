// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Description: Domain models, conflict resolution, address parsing, and records for download transfers.

mod action;
mod address;
mod conflict;
mod download_policy;
mod filename;
mod headers;
mod protocol;
mod record;
mod request;
mod review_request;
mod status;
mod validated_request;
mod validator;

pub use action::DownloadAction;
pub use address::DownloadAddress;
pub use conflict::{
    generate_indexed_filename, split_filename_for_rename, ClaimGuard, ConflictResolutionOutcome,
    DestinationAllocator, FileConflictPolicy,
};
pub use filename::{
    extension_for_mime_type, filename_from_url, is_generic_filename,
    parse_content_disposition_filename, percent_decode, redact_download_url,
    resolve_response_filename, sanitize_diagnostic_text, sanitize_filename,
};
pub use headers::{
    sanitize_forward_headers, BrowserRequestHeaders, MAX_FORWARDED_HEADER_VALUE_BYTES,
};
pub use protocol::DownloadProtocol;
pub use record::{DownloadRecord, DownloadSegmentProgress, SerializableDownloadRecordRef};
pub use request::{DownloadCredentials, DownloadRequest};
pub use review_request::ReviewDownloadRequest;
pub use status::DownloadStatus;
pub use validated_request::ValidatedDownloadRequest;
pub use validator::{deserialize_optional_transfer_validator, TransferValidator};
