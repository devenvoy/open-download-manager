use std::path::Path;

use crate::application::error::{AppError, AppResult};

/// Marks a downloaded or extracted binary executable on platforms that use a
/// Unix permission model. No-op on Windows.
pub fn make_executable(path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(path, permissions).map_err(|error| {
            AppError::download(format!("Failed to set executable permissions: {error}"))
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}
