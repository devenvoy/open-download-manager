// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-10
// Reason: refactor: modularize browser scanner into focused submodules
// Description: Extension directory packaging and file replication routines for Chrome and Firefox extensions.

use std::path::{Path, PathBuf};

pub fn prepare_chrome_extension(
    data_directory: &Path,
    integration_root: &Path,
) -> std::io::Result<PathBuf> {
    let folder = data_directory.join("NativeMessaging").join("chrome-extension");
    std::fs::create_dir_all(&folder)?;

    let source = if integration_root.join("extension").join("manifest.json").exists() {
        integration_root.join("extension")
    } else if integration_root.join("_up_").join("extension").join("manifest.json").exists() {
        integration_root.join("_up_").join("extension")
    } else if integration_root.join("resources").join("extension").join("manifest.json").exists() {
        integration_root.join("resources").join("extension")
    } else if integration_root.join("manifest.json").exists() {
        integration_root.to_path_buf()
    } else {
        integration_root.join("extension")
    };

    if source.exists() && source.join("manifest.json").exists() {
        copy_dir_recursive(&source, &folder)?;
    }

    Ok(folder)
}

pub fn prepare_firefox_extension(
    data_directory: &Path,
    integration_root: &Path,
) -> std::io::Result<PathBuf> {
    let folder = data_directory.join("NativeMessaging").join("firefox-extension");
    std::fs::create_dir_all(&folder)?;

    let source = integration_root.join("extension");
    if source.exists() {
        copy_dir_recursive(&source, &folder)?;
        let firefox_manifest = folder.join("manifest.firefox.json");
        if firefox_manifest.exists() {
            let target_manifest = folder.join("manifest.json");
            std::fs::copy(&firefox_manifest, &target_manifest)?;
        }
    }

    Ok(folder)
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
