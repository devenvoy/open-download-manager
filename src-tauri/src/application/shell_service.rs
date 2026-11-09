// Created: 2026-11-05
// Reason: feat: implement Phase A download context actions and filesystem parity
// Last updated: 2026-11-05
// Reason: feat: implement Phase A download context actions and filesystem parity
// Description: Shell service abstraction for opening files, revealing directories, and executing external processes.

use std::path::Path;

use crate::application::error::AppResult;

pub trait ShellService: Send + Sync {
    fn open_path(&self, path: &Path) -> AppResult<()>;
    fn reveal_path(&self, path: &Path) -> AppResult<()>;
    fn open_with(&self, path: &Path) -> AppResult<()>;
}
