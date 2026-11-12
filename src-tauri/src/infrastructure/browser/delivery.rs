// Created: 2026-11-12
// Reason: fix(browser): deliver the extension to a visible folder the browser can load
// Last updated: 2026-11-12
// Description: Copies the bundled extension into the user's Downloads folder so a browser can load it.

use std::path::{Path, PathBuf};

use crate::infrastructure::browser::registration::BrowserType;

/// Copies the bundled extension into the folder the user loads it from.
///
/// The copy lands in the Downloads directory, because a browser cannot pick an
/// extension folder that lives inside an application bundle.
pub fn deliver_extension(
    bundled_extension_dir: &Path,
    target_dir: &Path,
) -> std::io::Result<PathBuf> {
    let _ = std::fs::remove_dir_all(target_dir);
    std::fs::create_dir_all(target_dir)?;
    copy_dir_recursive(bundled_extension_dir, target_dir)?;
    Ok(target_dir.to_path_buf())
}

pub fn copy_dir_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    if !target.exists() {
        std::fs::create_dir_all(target)?;
    }
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

/// Removes a delivered extension folder.
pub fn remove_extension(target_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(target_dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The browser specific name of the delivered extension folder.
pub fn delivered_folder_name(browser: BrowserType) -> &'static str {
    if browser.is_firefox() {
        "extension-firefox"
    } else {
        "extension"
    }
}
