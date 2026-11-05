mod document;
mod policy;

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    application::{
        download_history_repository::DownloadHistoryRepository,
        error::{AppError, AppResult},
    },
    domain::download::{redact_download_url, DownloadRecord},
    infrastructure::{
        atomic_file::write_replace,
        data_file_policy::{LEGACY_HISTORY_FILE_NAME, TAURI_HISTORY_FILE_NAME},
    },
};

use self::{
    document::{BorrowedDownloadHistoryDocument, DownloadHistoryDocument},
    policy::HISTORY_VERSION,
};

pub struct DownloadHistoryStore {
    legacy_path: PathBuf,
    path: PathBuf,
    legacy_ids: std::sync::Mutex<HashSet<String>>,
}

impl DownloadHistoryStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        let data_directory = data_directory.into();
        Self {
            legacy_path: data_directory.join(LEGACY_HISTORY_FILE_NAME),
            path: data_directory.join(TAURI_HISTORY_FILE_NAME),
            legacy_ids: std::sync::Mutex::new(HashSet::new()),
        }
    }

    fn load_history_file(&self, path: &Path) -> AppResult<Vec<DownloadRecord>> {
        let contents = match fs::read(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => {
                return Err(AppError::storage(format!(
                    "Could not read download history at {}: {error}",
                    path.display()
                )));
            }
        };

        let history: DownloadHistoryDocument =
            serde_json::from_slice(&contents).map_err(|error| {
                AppError::storage(format!(
                    "Could not read download history at {}: {error}",
                    path.display()
                ))
            })?;
        if history.version != HISTORY_VERSION {
            return Err(AppError::storage(format!(
                "Unsupported download history version {} at {}.",
                history.version,
                path.display()
            )));
        }

        history
            .downloads
            .into_iter()
            .map(|mut download| {
                download.validate().map_err(|message| {
                    AppError::storage(format!("{message} History file: {}", path.display()))
                })?;
                download.prepare_for_restart();
                Ok(download)
            })
            .collect()
    }

    fn save_history(&self, downloads: &[DownloadRecord]) -> AppResult<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::storage("Download history path has no parent directory."))?;
        fs::create_dir_all(parent).map_err(|error| {
            AppError::storage(format!(
                "Could not create download history directory {}: {error}",
                parent.display()
            ))
        })?;

        let legacy_ids = self
            .legacy_ids
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let history = BorrowedDownloadHistoryDocument {
            version: HISTORY_VERSION,
            downloads: downloads
                .iter()
                .filter(|download| !legacy_ids.contains(&download.id))
                .map(|download| download.to_serialized_ref(redact_download_url(&download.url)))
                .collect(),
        };
        let contents = serde_json::to_vec_pretty(&history).map_err(|error| {
            AppError::storage(format!("Could not encode download history: {error}"))
        })?;
        write_replace(&self.path, &contents).map_err(|error| {
            AppError::storage(format!("Could not replace download history: {error}"))
        })
    }
}

impl DownloadHistoryRepository for DownloadHistoryStore {
    fn load(&self) -> AppResult<Vec<DownloadRecord>> {
        let mut legacy = self.load_history_file(&self.legacy_path)?;
        let current = self.load_history_file(&self.path)?;
        let mut legacy_ids = self
            .legacy_ids
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        legacy_ids.extend(legacy.iter().map(|download| download.id.clone()));
        drop(legacy_ids);
        let legacy_ids: HashSet<_> = legacy
            .iter()
            .map(|download| download.id.as_str().to_owned())
            .collect();
        legacy.extend(
            current
                .into_iter()
                .filter(|download| !legacy_ids.contains(&download.id)),
        );
        Ok(legacy)
    }

    fn save(&self, downloads: &[DownloadRecord]) -> AppResult<()> {
        self.save_history(downloads)
    }
}
