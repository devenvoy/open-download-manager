use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::domain::{
    category::DownloadCategory,
    download::DownloadStatus,
    media::sabr::NormalizedSabrSession,
};

use super::download_policy::{
    MAX_DOWNLOAD_CONNECTIONS, MAX_DOWNLOAD_FILENAME_CHARACTERS, MAX_DOWNLOAD_ID_CHARACTERS,
    MAX_URL_LENGTH_BYTES,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub filename: String,
    pub status: DownloadStatus,
    #[serde(default)]
    pub downloaded: u64,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub speed: u64,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<DownloadCategory>,
    #[serde(default)]
    pub directory: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub last_try: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub notice: Option<String>,
    #[serde(default)]
    pub estimated: bool,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub resumable: Option<bool>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub connections: u8,
    #[serde(default)]
    pub connections_progress: Vec<DownloadSegmentProgress>,
    #[serde(default)]
    pub audio_url: Option<String>,
    #[serde(default)]
    pub referer: Option<String>,
    #[serde(default)]
    pub youtube_format: Option<String>,
    #[serde(default)]
    pub transfer_kind: Option<String>,
    #[serde(skip)]
    pub sabr: Option<NormalizedSabrSession>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSegmentProgress {
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub total: u64,
    pub status: String,
}

impl DownloadRecord {
    pub fn prepare_for_restart(&mut self) {
        self.status = self.status.clone().after_restart();
        self.speed = 0;
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.id.is_empty()
            || self.id.chars().count() > MAX_DOWNLOAD_ID_CHARACTERS
            || !self.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return Err("A download record has an invalid identifier.");
        }
        if self.url.is_empty() || self.url.len() > MAX_URL_LENGTH_BYTES {
            return Err("A download record has an invalid source address.");
        }
        if self.filename.chars().count() > MAX_DOWNLOAD_FILENAME_CHARACTERS {
            return Err("A download record has an invalid file name.");
        }
        if self.connections > MAX_DOWNLOAD_CONNECTIONS {
            return Err("A download record has an invalid connection count.");
        }
        if self
            .scheduled_at
            .as_deref()
            .is_some_and(|value| DateTime::parse_from_rfc3339(value).is_err())
        {
            return Err("A download record has an invalid scheduled start time.");
        }
        Ok(())
    }

    pub fn to_serialized_ref<'a>(&'a self, redacted_url: String) -> SerializableDownloadRecordRef<'a> {
        SerializableDownloadRecordRef {
            id: &self.id,
            url: redacted_url,
            filename: &self.filename,
            status: &self.status,
            downloaded: self.downloaded,
            total: self.total,
            speed: self.speed,
            created_at: &self.created_at,
            needs_review: self.needs_review,
            scheduled_at: self.scheduled_at.as_deref(),
            description: self.description.as_deref(),
            category: self.category.as_ref(),
            directory: self.directory.as_deref(),
            path: self.path.as_deref(),
            last_try: self.last_try.as_deref(),
            error: self.error.as_deref(),
            notice: self.notice.as_deref(),
            estimated: self.estimated,
            kind: self.kind.as_deref(),
            resumable: self.resumable,
            duration: self.duration,
            connections: self.connections,
            connections_progress: &self.connections_progress,
            audio_url: self.audio_url.as_deref(),
            referer: self.referer.as_deref(),
            transfer_kind: self.transfer_kind.as_deref(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableDownloadRecordRef<'a> {
    pub id: &'a str,
    pub url: String,
    pub filename: &'a str,
    pub status: &'a DownloadStatus,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: u64,
    pub created_at: &'a str,
    pub needs_review: bool,
    pub scheduled_at: Option<&'a str>,
    pub description: Option<&'a str>,
    pub category: Option<&'a DownloadCategory>,
    pub directory: Option<&'a str>,
    pub path: Option<&'a str>,
    pub last_try: Option<&'a str>,
    pub error: Option<&'a str>,
    pub notice: Option<&'a str>,
    pub estimated: bool,
    pub kind: Option<&'a str>,
    pub resumable: Option<bool>,
    pub duration: Option<f64>,
    pub connections: u8,
    pub connections_progress: &'a [DownloadSegmentProgress],
    pub audio_url: Option<&'a str>,
    pub referer: Option<&'a str>,
    pub transfer_kind: Option<&'a str>,
}
