use std::path::Path;

use crate::application::error::AppResult;

pub trait ShellService: Send + Sync {
    fn open_path(&self, path: &Path) -> AppResult<()>;
    fn reveal_path(&self, path: &Path) -> AppResult<()>;
    fn open_with(&self, path: &Path) -> AppResult<()>;
}
