// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-08
// Reason: fix(transfer): preserve HTTP resume validator type across interrupted downloads (Closes #25) (#40)
// Description: HTTP validator parser extracting Content-Range, ETag, and Last-Modified headers.

use reqwest::header::HeaderName;

use crate::{
    application::error::{AppError, AppResult},
    domain::download::TransferValidator,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct SegmentValidator {
    pub(crate) validator: TransferValidator,
    pub(crate) total: u64,
}

impl SegmentValidator {
    pub(crate) fn new(validator: TransferValidator, total: u64) -> Self {
        Self { validator, total }
    }

    pub(crate) fn header(&self) -> HeaderName {
        reqwest::header::IF_RANGE
    }

    pub(crate) fn entity_header(&self) -> HeaderName {
        self.validator.entity_header_name()
    }

    pub(crate) fn value(&self) -> &str {
        self.validator.value()
    }

    pub(crate) fn if_range_value(&self) -> &str {
        self.validator.if_range_value()
    }
}

pub(crate) fn extract_validator(
    response: &reqwest::Response,
    total: u64,
) -> Option<SegmentValidator> {
    let strong_etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.starts_with("W/") && value.starts_with('"') && value.ends_with('"'))
        .map(|value| TransferValidator::ETag(value.to_owned()));

    if let Some(validator) = strong_etag {
        return Some(SegmentValidator { validator, total });
    }

    let last_modified = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(|value| TransferValidator::LastModified(value.to_owned()));

    if let Some(validator) = last_modified {
        return Some(SegmentValidator { validator, total });
    }

    None
}

pub(crate) fn content_range(response: &reqwest::Response) -> AppResult<(u64, u64, Option<u64>)> {
    let value = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes "))
        .ok_or_else(|| AppError::download("The server returned an invalid byte range."))?;
    let (range, total) = value
        .split_once('/')
        .ok_or_else(|| AppError::download("The server returned an invalid byte range."))?;
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| AppError::download("The server returned an invalid byte range."))?;
    let start = start
        .parse()
        .map_err(|_| AppError::download("The server returned an invalid byte range."))?;
    let end = end
        .parse()
        .map_err(|_| AppError::download("The server returned an invalid byte range."))?;
    let total = if total == "*" {
        None
    } else {
        Some(
            total
                .parse()
                .map_err(|_| AppError::download("The server returned an invalid byte range."))?,
        )
    };
    Ok((start, end, total))
}
