use serde::{Deserialize, Serialize};

use crate::domain::{category::DownloadCategory, download::DownloadStatus};

use super::download_policy::{
    MAX_DOWNLOAD_FILENAME_CHARACTERS, MAX_DOWNLOAD_ID_CHARACTERS, MAX_URL_LENGTH_BYTES,
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
        Ok(())
    }
}
