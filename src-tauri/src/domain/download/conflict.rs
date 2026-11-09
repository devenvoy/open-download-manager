// Created: 2026-11-08
// Reason: feat(transfer): offer explicit actions when destination file already exists (Closes #26) (#41)
// Last updated: 2026-11-08
// Reason: feat(transfer): offer explicit actions when destination file already exists (Closes #26) (#41)
// Description: File conflict resolution policies (rename, overwrite, skip, cancel, ask) and collision-free naming.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileConflictPolicy {
    Ask,
    Overwrite,
    Rename,
    Skip,
    Cancel,
}

impl Default for FileConflictPolicy {
    fn default() -> Self {
        Self::Ask
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConflictResolutionOutcome {
    #[serde(rename = "resolved")]
    Resolved {
        path: PathBuf,
        policy: FileConflictPolicy,
    },
    #[serde(rename = "conflict")]
    Conflict {
        existing_path: PathBuf,
        suggested_path: PathBuf,
        candidates: Vec<FileConflictPolicy>,
    },
    #[serde(rename = "skipped")]
    Skipped {
        path: PathBuf,
    },
    #[serde(rename = "cancelled")]
    Cancelled {
        path: PathBuf,
    },
}

pub fn split_filename_for_rename(filename: &str) -> (&str, Option<&str>) {
    let lower = filename.to_ascii_lowercase();
    for compound_ext in [".tar.gz", ".tar.bz2", ".tar.xz"] {
        if lower.ends_with(compound_ext) {
            let stem = &filename[..filename.len() - compound_ext.len()];
            let ext = &compound_ext[1..];
            return (stem, Some(ext));
        }
    }
    let p = Path::new(filename);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(filename);
    let ext = p.extension().and_then(|e| e.to_str());
    (stem, ext)
}

pub fn generate_indexed_filename(filename: &str, index: usize) -> String {
    if index == 0 {
        return filename.to_string();
    }
    let (stem, ext) = split_filename_for_rename(filename);
    match ext {
        Some(ext) => format!("{stem} ({index}).{ext}"),
        None => format!("{stem} ({index})"),
    }
}

#[derive(Clone, Debug, Default)]
pub struct DestinationAllocator {
    claimed: Arc<Mutex<HashSet<PathBuf>>>,
}

pub struct ClaimGuard {
    allocator: DestinationAllocator,
    path: PathBuf,
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        self.allocator.release(&self.path);
    }
}

impl DestinationAllocator {
    pub fn new() -> Self {
        Self {
            claimed: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn is_claimed(&self, path: &Path) -> bool {
        self.claimed
            .lock()
            .map(|set| set.contains(path))
            .unwrap_or(false)
    }

    pub fn claim(&self, path: &Path) {
        if let Ok(mut set) = self.claimed.lock() {
            set.insert(path.to_path_buf());
        }
    }

    pub fn release(&self, path: &Path) {
        if let Ok(mut set) = self.claimed.lock() {
            set.remove(path);
        }
    }

    pub fn claim_guard(&self, path: PathBuf) -> ClaimGuard {
        self.claim(&path);
        ClaimGuard {
            allocator: self.clone(),
            path,
        }
    }

    pub fn suggest_conflict_free_destination(&self, directory: &Path, filename: &str) -> PathBuf {
        let set = self.claimed.lock().ok();
        let target = directory.join(filename);
        let exists = target.exists() || set.as_ref().map_or(false, |s| s.contains(&target));
        if !exists {
            return target;
        }
        for index in 1..=10000 {
            let candidate_name = generate_indexed_filename(filename, index);
            let candidate_path = directory.join(candidate_name);
            let candidate_exists = candidate_path.exists()
                || set.as_ref().map_or(false, |s| s.contains(&candidate_path));
            if !candidate_exists {
                return candidate_path;
            }
        }
        directory.join(generate_indexed_filename(filename, 10001))
    }

    pub fn allocate_conflict_free_destination(&self, directory: &Path, filename: &str) -> PathBuf {
        let mut set = self.claimed.lock().expect("Claimed lock poisoned");
        let initial_path = directory.join(filename);
        if !initial_path.exists() && !set.contains(&initial_path) {
            set.insert(initial_path.clone());
            return initial_path;
        }

        for index in 1..=10000 {
            let candidate_name = generate_indexed_filename(filename, index);
            let candidate_path = directory.join(candidate_name);
            if !candidate_path.exists() && !set.contains(&candidate_path) {
                set.insert(candidate_path.clone());
                return candidate_path;
            }
        }

        let fallback = directory.join(generate_indexed_filename(filename, 10001));
        set.insert(fallback.clone());
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_and_index_regular_extension() {
        assert_eq!(generate_indexed_filename("archive.zip", 0), "archive.zip");
        assert_eq!(generate_indexed_filename("archive.zip", 1), "archive (1).zip");
        assert_eq!(generate_indexed_filename("archive.zip", 2), "archive (2).zip");
    }

    #[test]
    fn test_split_and_index_compound_extension() {
        assert_eq!(generate_indexed_filename("archive.tar.gz", 0), "archive.tar.gz");
        assert_eq!(generate_indexed_filename("archive.tar.gz", 1), "archive (1).tar.gz");
        assert_eq!(generate_indexed_filename("archive.tar.bz2", 2), "archive (2).tar.bz2");
    }

    #[test]
    fn test_split_and_index_no_extension() {
        assert_eq!(generate_indexed_filename("LICENSE", 1), "LICENSE (1)");
    }

    #[test]
    fn test_concurrent_allocation_is_collision_free() {
        let allocator = DestinationAllocator::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let dir = temp_dir.path().to_path_buf();

        let mut handles = Vec::new();
        for _ in 0..10 {
            let alloc = allocator.clone();
            let d = dir.clone();
            handles.push(std::thread::spawn(move || {
                alloc.allocate_conflict_free_destination(&d, "file.iso")
            }));
        }

        let mut allocated = HashSet::new();
        for handle in handles {
            let path = handle.join().unwrap();
            assert!(allocated.insert(path), "Duplicate path allocated concurrently!");
        }
        assert_eq!(allocated.len(), 10);
    }
}
