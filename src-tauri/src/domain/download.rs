use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use url::Url;

const MAX_DOWNLOAD_URLS: usize = 500;
const MAX_URL_LENGTH: usize = 8_192;
const MAX_DOWNLOAD_FILENAME_LENGTH: usize = 180;
const MAX_DESCRIPTION_LENGTH: usize = 1_000;
const MAX_DOWNLOAD_ID_LENGTH: usize = 128;
const ALLOWED_CATEGORIES: [&str; 6] = [
    "Video",
    "Audio",
    "Archives",
    "Documents",
    "Programs",
    "Other",
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Scheduled,
    Paused,
    Cancelled,
    Downloading,
    #[serde(rename = "probing", alias = "connecting")]
    Connecting,
    Assembling,
    #[serde(rename = "complete", alias = "completed", alias = "done")]
    Completed,
    #[serde(rename = "error", alias = "failed")]
    Failed,
}

impl DownloadStatus {
    pub fn after_restart(self) -> Self {
        match self {
            Self::Downloading | Self::Connecting | Self::Assembling => Self::Paused,
            status => status,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadRequest {
    pub urls: Vec<String>,
    pub filename: Option<String>,
    pub directory: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub queue_only: bool,
    pub scheduled_at: Option<String>,
}

impl DownloadRequest {
    pub fn validate(self, default_directory: PathBuf) -> Result<ValidatedDownloadRequest, String> {
        if self.urls.is_empty() || self.urls.len() > MAX_DOWNLOAD_URLS {
            return Err(format!(
                "Add between 1 and {MAX_DOWNLOAD_URLS} download URLs."
            ));
        }
        if self.filename.is_some() && self.urls.len() != 1 {
            return Err("A custom file name can only be used for a single address.".to_owned());
        }
        if self
            .filename
            .as_ref()
            .is_some_and(|filename| filename.chars().count() > MAX_DOWNLOAD_FILENAME_LENGTH)
        {
            return Err(format!(
                "File names must be at most {MAX_DOWNLOAD_FILENAME_LENGTH} characters."
            ));
        }
        if self.scheduled_at.is_some() {
            return Err("Scheduled downloads are not available in this migration yet.".to_owned());
        }

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
        if let Some(category) = self.category.as_deref() {
            if !ALLOWED_CATEGORIES.contains(&category) {
                return Err("Choose a supported download category.".to_owned());
            }
        }
        let description = self
            .description
            .map(|description| description.trim().to_owned());
        if description
            .as_ref()
            .is_some_and(|description| description.chars().count() > MAX_DESCRIPTION_LENGTH)
        {
            return Err(format!(
                "Descriptions must be at most {MAX_DESCRIPTION_LENGTH} characters."
            ));
        }

        let urls = self
            .urls
            .into_iter()
            .map(validate_download_url)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ValidatedDownloadRequest {
            urls,
            filename,
            directory,
            category: self.category,
            description,
            queue_only: self.queue_only,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedDownloadRequest {
    pub urls: Vec<Url>,
    pub filename: Option<String>,
    pub directory: PathBuf,
    pub category: Option<String>,
    pub description: Option<String>,
    pub queue_only: bool,
}

fn validate_download_url(value: String) -> Result<Url, String> {
    if value.len() > MAX_URL_LENGTH {
        return Err("A download address exceeds the allowed length.".to_owned());
    }
    let url = Url::parse(&value).map_err(|_| "Enter a valid HTTP or HTTPS address.".to_owned())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Downloads must use HTTP or HTTPS without embedded credentials.".to_owned());
    }
    let host = url.host_str().unwrap_or_default();
    if url.path().to_ascii_lowercase().ends_with(".m3u8")
        || url.path().to_ascii_lowercase().ends_with(".mpd")
        || host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtu.be"
    {
        return Err(
            "This address requires a media or browser integration that is not migrated yet."
                .to_owned(),
        );
    }
    Ok(url)
}

pub fn sanitize_filename(value: &str) -> String {
    let mut filename: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    while matches!(filename.chars().last(), Some('.' | ' ')) {
        filename.pop();
    }
    if filename.chars().count() > MAX_DOWNLOAD_FILENAME_LENGTH {
        filename = filename
            .chars()
            .take(MAX_DOWNLOAD_FILENAME_LENGTH)
            .collect();
    }
    let basename = filename
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' ']);
    if filename.is_empty() {
        return filename;
    }
    if matches!(
        basename.to_ascii_lowercase().as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    ) {
        filename.insert_str(0, "download_");
    }
    if filename.chars().count() > MAX_DOWNLOAD_FILENAME_LENGTH {
        filename = filename
            .chars()
            .take(MAX_DOWNLOAD_FILENAME_LENGTH)
            .collect();
    }
    filename
}

pub fn redact_download_url(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .to_owned();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

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
    pub category: Option<String>,
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
            || self.id.len() > MAX_DOWNLOAD_ID_LENGTH
            || !self.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return Err("A download record has an invalid identifier.");
        }
        if self.url.is_empty() || self.url.len() > MAX_URL_LENGTH {
            return Err("A download record has an invalid source address.");
        }
        if self.filename.chars().count() > MAX_DOWNLOAD_FILENAME_LENGTH {
            return Err("A download record has an invalid file name.");
        }
        Ok(())
    }
}
