// Created: 2026-11-10
// Reason: refactor: modularize segmented download engine into discovery, worker, progress submodules
// Description: HTTP byte-range capability discovery and segment division calculations.

use std::path::Path;
use tokio::fs;
use url::Url;

use crate::{
    application::{
        download_executor::DownloadControl,
        error::{AppError, AppResult},
    },
    domain::download::DownloadSegmentProgress,
};

use super::super::{
    assembler::segment_path,
    validator::{content_range, extract_validator, SegmentValidator},
};

pub struct RangeDiscovery {
    pub validator: SegmentValidator,
    pub cd_filename: Option<String>,
    pub content_type: Option<String>,
    pub final_url: Url,
    pub auth_header: Option<String>,
}

pub(crate) async fn discover_ranges(
    client: &reqwest::Client,
    url: Url,
    control: &mut DownloadControl,
    override_creds: Option<&(String, String)>,
    matcher: Option<&crate::domain::credentials::CredentialMatcher>,
    referer: Option<&str>,
    request_headers: &crate::domain::download::BrowserRequestHeaders,
    request_headers_host: &str,
) -> AppResult<Option<RangeDiscovery>> {
    let resolved = super::super::auth::send_authenticated_request_with_redirects(
        client,
        &url,
        control,
        override_creds,
        matcher,
        Some(0),
        Some(0),
        None,
        referer,
        request_headers,
        request_headers_host,
    )
    .await?;

    let response = resolved.response;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::link_expired(
            "Authentication required or session expired (HTTP 401). Refresh the link to resume.",
        ));
    }
    if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::GONE
    {
        return Err(AppError::link_expired(format!(
            "The download link has expired (HTTP {}). Refresh the link to resume.",
            response.status().as_u16()
        )));
    }

    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        || response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|encoding| encoding.as_bytes() != b"identity")
    {
        return Ok(None);
    }
    let Ok((0, 0, Some(total))) = content_range(&response) else {
        return Ok(None);
    };
    if total < 2 {
        return Ok(None);
    }
    let cd_filename = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(crate::domain::download::parse_content_disposition_filename);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    Ok(extract_validator(&response, total).map(|v| RangeDiscovery {
        validator: v,
        cd_filename,
        content_type,
        final_url: resolved.final_url,
        auth_header: resolved.auth_header,
    }))
}

pub(crate) async fn segment_ranges(
    total: u64,
    requested_connections: u8,
    directory: &Path,
    id: &str,
    existing: Option<&[DownloadSegmentProgress]>,
) -> AppResult<Vec<DownloadSegmentProgress>> {
    if let Some(existing_segs) = existing {
        if !existing_segs.is_empty() {
            let mut valid = true;
            let mut expected_start = 0_u64;
            for s in existing_segs {
                if s.start != expected_start || s.end < s.start {
                    valid = false;
                    break;
                }
                expected_start = s.end + 1;
            }
            if valid && expected_start == total {
                let mut restored = Vec::with_capacity(existing_segs.len());
                for (idx, seg) in existing_segs.iter().enumerate() {
                    let path = segment_path(directory, id, idx);
                    let downloaded = match fs::metadata(&path).await {
                        Ok(metadata) => metadata.len().min(seg.total),
                        Err(_) => 0,
                    };
                    restored.push(DownloadSegmentProgress {
                        start: seg.start,
                        end: seg.end,
                        downloaded,
                        total: seg.total,
                        status: if downloaded == seg.total {
                            "complete"
                        } else {
                            "queued"
                        }
                        .to_owned(),
                    });
                }
                return Ok(restored);
            }
        }
    }

    let count = u64::from(requested_connections.max(1)).min(total);
    let count =
        usize::try_from(count).map_err(|_| AppError::download("The segment count is invalid."))?;
    let mut segments = Vec::with_capacity(count);
    let count_u64 =
        u64::try_from(count).map_err(|_| AppError::download("The segment count is invalid."))?;
    let base_size = total / count_u64;
    let remainder = total % count_u64;
    let mut start = 0_u64;
    for index in 0..count {
        let index_u64 =
            u64::try_from(index).map_err(|_| AppError::download("The segment index is invalid."))?;
        let segment_total = base_size + u64::from(index_u64 < remainder);
        let end = start + segment_total - 1;
        let path = segment_path(directory, id, index);
        let downloaded = match fs::metadata(path).await {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => {
                return Err(AppError::download(format!(
                    "Could not inspect a partial segment: {error}"
                )));
            }
        };
        if downloaded > segment_total {
            return Err(AppError::download(
                "A partial segment is larger than its assigned byte range.",
            ));
        }
        segments.push(DownloadSegmentProgress {
            start,
            end,
            downloaded,
            total: segment_total,
            status: if downloaded == segment_total {
                "complete"
            } else {
                "queued"
            }
            .to_owned(),
        });
        start = end + 1;
    }
    Ok(segments)
}
