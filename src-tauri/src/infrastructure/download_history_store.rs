use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::{
    application::{
        download_history_service::DownloadHistoryRepository,
        error::{AppError, AppResult},
    },
    domain::download::DownloadRecord,
};

const LEGACY_HISTORY_VERSION: u8 = 1;

pub struct DownloadHistoryStore {
    path: PathBuf,
}

impl DownloadHistoryStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            path: data_directory.into().join("downloads.json"),
        }
    }

    fn load_history(&self) -> AppResult<Vec<DownloadRecord>> {
        let contents = match fs::read(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => {
                return Err(AppError::storage(format!(
                    "Could not read download history at {}: {error}",
                    self.path.display()
                )));
            }
        };

        let history: LegacyHistory = serde_json::from_slice(&contents).map_err(|error| {
            AppError::storage(format!(
                "Could not read download history at {}: {error}",
                self.path.display()
            ))
        })?;
        if history.version != LEGACY_HISTORY_VERSION {
            return Err(AppError::storage(format!(
                "Unsupported download history version {} at {}.",
                history.version,
                self.path.display()
            )));
        }

        history
            .downloads
            .into_iter()
            .map(|mut download| {
                download.validate().map_err(|message| {
                    AppError::storage(format!("{message} History file: {}", self.path.display()))
                })?;
                download.prepare_for_restart();
                Ok(download)
            })
            .collect()
    }
}

impl DownloadHistoryRepository for DownloadHistoryStore {
    fn load(&self) -> AppResult<Vec<DownloadRecord>> {
        self.load_history()
    }
}

#[derive(Deserialize)]
struct LegacyHistory {
    version: u8,
    #[serde(default)]
    downloads: Vec<DownloadRecord>,
}
