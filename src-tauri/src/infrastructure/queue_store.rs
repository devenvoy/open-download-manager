// Created: 2026-11-08
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Persistent JSON queue store with atomic write guarantees and schema migration.

use std::{fs, path::PathBuf};

use crate::{
    application::error::{AppError, AppResult},
    domain::queue::{DownloadQueue, DEFAULT_QUEUE_ID},
    infrastructure::atomic_file::write_replace,
};

pub trait QueueRepository: Send + Sync {
    fn load(&self) -> AppResult<Vec<DownloadQueue>>;
    fn save(&self, queues: &[DownloadQueue]) -> AppResult<()>;
}

#[derive(Clone)]
pub struct QueueStore {
    path: PathBuf,
}

impl QueueStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            path: data_directory.into().join("queues.json"),
        }
    }
}

impl QueueRepository for QueueStore {
    fn load(&self) -> AppResult<Vec<DownloadQueue>> {
        if !self.path.exists() {
            let default_queues = vec![DownloadQueue::default_queue()];
            let _ = self.save(&default_queues);
            return Ok(default_queues);
        }

        let bytes = fs::read(&self.path).map_err(|e| {
            AppError::storage(format!("Could not read queues file: {e}"))
        })?;

        let mut queues: Vec<DownloadQueue> = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::storage(format!("Could not parse queues file: {e}"))
        })?;

        // Ensure default queue always exists
        if !queues.iter().any(|q| q.id == DEFAULT_QUEUE_ID) {
            queues.insert(0, DownloadQueue::default_queue());
            let _ = self.save(&queues);
        }

        Ok(queues)
    }

    fn save(&self, queues: &[DownloadQueue]) -> AppResult<()> {
        let json = serde_json::to_vec_pretty(queues).map_err(|e| {
            AppError::storage(format!("Could not serialize queues: {e}"))
        })?;
        write_replace(&self.path, &json).map_err(|e| {
            AppError::storage(format!("Could not write queues file: {e}"))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_store_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let store = QueueStore::new(dir.path());

        // First load creates default queue
        let queues = store.load().unwrap();
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].id, DEFAULT_QUEUE_ID);

        // Add a new queue and save
        let mut custom = DownloadQueue::new("nightly".into(), "Nightly".into(), 1, 500);
        custom.add_item("dl-123".into());
        let updated = vec![queues[0].clone(), custom];
        store.save(&updated).unwrap();

        // Reload from disk
        let reloaded = store.load().unwrap();
        assert_eq!(reloaded.len(), 2);
        assert_eq!(reloaded[1].name, "Nightly");
        assert_eq!(reloaded[1].item_ids, vec!["dl-123"]);
    }
}
