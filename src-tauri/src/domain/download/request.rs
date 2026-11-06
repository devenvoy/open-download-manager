use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::domain::{
    category::DownloadCategory,
    media::sabr::{normalize_sabr_session, SabrSession},
};

use super::{
    download_policy::{
        MAX_DESCRIPTION_CHARACTERS, MAX_DOWNLOAD_FILENAME_CHARACTERS, MAX_DOWNLOAD_URLS,
    },
    sanitize_filename, DownloadAddress, ValidatedDownloadRequest,
};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadRequest {
    pub urls: Vec<String>,
    pub filename: Option<String>,
    pub directory: Option<String>,
    pub category: Option<DownloadCategory>,
    pub description: Option<String>,
    #[serde(default)]
    pub queue_only: bool,
    #[serde(default)]
    pub review_only: Option<bool>,
    pub scheduled_at: Option<String>,
    pub audio_url: Option<String>,
    pub kind: Option<String>,
    pub sabr: Option<SabrSession>,
    #[serde(default)]
    pub reuse_active: Option<bool>,
    pub referer: Option<String>,
    pub youtube_format: Option<String>,
}

impl DownloadRequest {
    pub fn validate(self, default_directory: PathBuf) -> Result<ValidatedDownloadRequest, String> {
        if self.urls.is_empty() || self.urls.len() > MAX_DOWNLOAD_URLS {
            return Err(format!(
                "Add between 1 and {MAX_DOWNLOAD_URLS} download URLs."
            ));
        }
        if self.sabr.is_some() && self.urls.len() != 1 {
            return Err("A SABR session requires exactly one YouTube stream.".to_owned());
        }
        if self.filename.is_some() && self.urls.len() != 1 {
            return Err("A custom file name can only be used for a single address.".to_owned());
        }
        if self
            .filename
            .as_ref()
            .is_some_and(|filename| filename.chars().count() > MAX_DOWNLOAD_FILENAME_CHARACTERS)
        {
            return Err(format!(
                "File names must be at most {MAX_DOWNLOAD_FILENAME_CHARACTERS} characters."
            ));
        }
        let scheduled_at = self
            .scheduled_at
            .map(|scheduled_at| {
                let scheduled_at = DateTime::parse_from_rfc3339(&scheduled_at)
                    .map_err(|_| "Choose a valid scheduled start time.")?
                    .with_timezone(&Utc);
                if scheduled_at <= Utc::now() {
                    return Err("Choose a future scheduled start time.");
                }
                Ok(scheduled_at.to_rfc3339())
            })
            .transpose()?;

        let filename = self.filename.map(|filename| sanitize_filename(&filename));
        if filename.as_ref().is_some_and(String::is_empty) {
            return Err("Choose a valid file name.".to_owned());
        }
        let directory = match self.directory {
            Some(directory) => {
                let path = PathBuf::from(directory);
                if !path.is_absolute() {
                    return Err("Choose an absolute download folder.".to_owned());
                }
                path
            }
            None => default_directory,
        };
        let description = self
            .description
            .map(|description| description.trim().to_owned());
        if description
            .as_ref()
            .is_some_and(|description| description.chars().count() > MAX_DESCRIPTION_CHARACTERS)
        {
            return Err(format!(
                "Descriptions must be at most {MAX_DESCRIPTION_CHARACTERS} characters."
            ));
        }
        let urls = self
            .urls
            .into_iter()
            .map(|url| DownloadAddress::parse(&url))
            .collect::<Result<Vec<_>, _>>()?;

        let audio_url = if let Some(audio_url) = self.audio_url {
            let addr = DownloadAddress::parse(&audio_url)?;
            Some(addr.url().as_str().to_owned())
        } else {
            None
        };

        let sabr = if let Some(ref sabr_session) = self.sabr {
            Some(normalize_sabr_session(sabr_session)?)
        } else {
            None
        };

        let kind = self.kind.or_else(|| {
            if urls.iter().any(|u| crate::domain::media::youtube_url(u.url().as_str()).is_some()) {
                Some("youtube".to_owned())
            } else {
                None
            }
        });

        Ok(ValidatedDownloadRequest {
            urls,
            filename,
            directory,
            category: self.category,
            description,
            queue_only: self.queue_only,
            review_only: self.review_only.unwrap_or(false),
            scheduled_at,
            audio_url,
            kind,
            sabr,
            reuse_active: self.reuse_active.unwrap_or(false),
            referer: self.referer,
            youtube_format: self.youtube_format,
        })
    }
}
