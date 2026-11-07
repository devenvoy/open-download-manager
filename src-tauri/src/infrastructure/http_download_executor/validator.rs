use reqwest::header::HeaderName;

use crate::application::error::{AppError, AppResult};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct SegmentValidator {
    pub(crate) header: HeaderName,
    pub(crate) entity_header: HeaderName,
    pub(crate) value: String,
    pub(crate) total: u64,
}

pub(crate) fn extract_validator(response: &reqwest::Response, total: u64) -> Option<SegmentValidator> {
    response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.starts_with("W/"))
        .map(|value| SegmentValidator {
            header: reqwest::header::IF_RANGE,
            entity_header: reqwest::header::ETAG,
            value: value.to_owned(),
            total,
        })
        .or_else(|| {
            response
                .headers()
                .get(reqwest::header::LAST_MODIFIED)
                .and_then(|value| value.to_str().ok())
                .map(|value| SegmentValidator {
                    header: reqwest::header::IF_RANGE,
                    entity_header: reqwest::header::LAST_MODIFIED,
                    value: value.to_owned(),
                    total,
                })
        })
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
