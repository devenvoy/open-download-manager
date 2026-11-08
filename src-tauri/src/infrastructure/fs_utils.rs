use std::path::Path;

use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::application::error::AppError;

/// Validates that a directory is an absolute path, exists (or can be created), and is writable.
pub fn validate_directory_writable(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "Path must be an absolute path: {}",
            path.display()
        ));
    }

    if let Err(err) = std::fs::create_dir_all(path) {
        return Err(format!(
            "Could not access or create directory {}: {err}",
            path.display()
        ));
    }

    let probe_filename = format!(".odm_probe_{}", uuid::Uuid::new_v4());
    let probe_path = path.join(probe_filename);

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
    {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe_path);
            Ok(())
        }
        Err(err) => Err(format!(
            "Directory {} is not writable: {err}",
            path.display()
        )),
    }
}

/// Queries available disk space in bytes for the volume containing the given path.
pub fn available_space(path: &Path) -> Result<u64, String> {
    let mut check_path = path.to_path_buf();
    while !check_path.exists() {
        match check_path.parent() {
            Some(parent) => check_path = parent.to_path_buf(),
            None => break,
        }
    }

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(check_path.as_os_str().as_bytes())
            .map_err(|e| format!("Invalid path for disk check: {e}"))?;

        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0 {
            let bavail = stat.f_bavail as u64;
            let frsize = stat.f_frsize as u64;
            Ok(bavail * frsize)
        } else {
            Err(format!(
                "Could not query free space for {}: {}",
                check_path.display(),
                std::io::Error::last_os_error()
            ))
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let mut wide_path: Vec<u16> = check_path.as_os_str().encode_wide().collect();
        wide_path.push(0);

        let mut free_bytes_available: u64 = 0;
        let mut total_number_of_bytes: u64 = 0;
        let mut total_number_of_free_bytes: u64 = 0;

        extern "system" {
            fn GetDiskFreeSpaceExW(
                lpDirectoryName: *const u16,
                lpFreeBytesAvailableToCaller: *mut u64,
                lpTotalNumberOfBytes: *mut u64,
                lpTotalNumberOfFreeBytes: *mut u64,
            ) -> i32;
        }

        let res = unsafe {
            GetDiskFreeSpaceExW(
                wide_path.as_ptr(),
                &mut free_bytes_available,
                &mut total_number_of_bytes,
                &mut total_number_of_free_bytes,
            )
        };

        if res != 0 {
            Ok(free_bytes_available)
        } else {
            Err(format!(
                "Could not query free space for {}: {}",
                check_path.display(),
                std::io::Error::last_os_error()
            ))
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        Ok(u64::MAX)
    }
}

/// Safely moves a file across volumes using a verified copy/flush/rename strategy.
///
/// 1. Validates that the destination volume has sufficient free space.
/// 2. Copies into a temporary staging file on the destination volume.
/// 3. Flushes and syncs buffers to physical disk.
/// 4. Verifies the staged file size against the source.
/// 5. Atomically renames the staging file to the destination path.
/// 6. Removes the source file.
pub async fn verified_cross_volume_copy(
    source: &Path,
    destination: &Path,
    expected_len: u64,
    id: &str,
) -> Result<(), AppError> {
    let dest_parent = destination.parent().ok_or_else(|| {
        AppError::download("The destination file path has no valid parent folder.")
    })?;

    fs::create_dir_all(dest_parent).await.map_err(|e| {
        AppError::download(format!(
            "Could not create destination folder {}: {e}",
            dest_parent.display()
        ))
    })?;

    // Check free space on destination volume
    if let Ok(free) = available_space(dest_parent) {
        if free < expected_len {
            return Err(AppError::download(format!(
                "Insufficient disk space on destination volume: {expected_len} bytes required, but only {free} bytes available."
            )));
        }
    }

    let staging_path = dest_parent.join(format!(".staging-{}-{}.part", id, uuid::Uuid::new_v4()));

    let copy_result: Result<(), AppError> = async {
        let mut reader = fs::File::open(source).await.map_err(|e| {
            AppError::download(format!(
                "Could not read source file {}: {e}",
                source.display()
            ))
        })?;

        let mut writer = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging_path)
            .await
            .map_err(|e| {
                AppError::download(format!(
                    "Could not create destination staging file {}: {e}",
                    staging_path.display()
                ))
            })?;

        let mut buffer = vec![0u8; 64 * 1024];
        let mut total_copied: u64 = 0;
        loop {
            let n = reader.read(&mut buffer).await.map_err(|e| {
                AppError::download(format!("Error reading during cross-volume copy: {e}"))
            })?;
            if n == 0 {
                break;
            }
            writer.write_all(&buffer[..n]).await.map_err(|e| {
                AppError::download(format!("Error writing during cross-volume copy: {e}"))
            })?;
            total_copied += n as u64;
        }

        writer.flush().await.map_err(|e| {
            AppError::download(format!("Could not flush cross-volume destination file: {e}"))
        })?;
        writer.sync_all().await.map_err(|e| {
            AppError::download(format!("Could not sync cross-volume destination file: {e}"))
        })?;
        drop(writer);

        if total_copied != expected_len {
            return Err(AppError::download(format!(
                "Cross-volume copy size mismatch: expected {expected_len} bytes, wrote {total_copied} bytes."
            )));
        }

        // Verify metadata on staging file
        let staged_meta = fs::metadata(&staging_path).await.map_err(|e| {
            AppError::download(format!("Could not inspect staged download file: {e}"))
        })?;
        if staged_meta.len() != expected_len {
            return Err(AppError::download(format!(
                "Cross-volume copy verification failed: staged file is {} bytes, expected {expected_len} bytes.",
                staged_meta.len()
            )));
        }

        // Atomically rename staging file to destination (same volume)
        fs::rename(&staging_path, destination).await.map_err(|e| {
            AppError::download(format!(
                "Could not finalize cross-volume destination file: {e}"
            ))
        })?;

        // Remove source file now that destination is verified and finalized
        let _ = fs::remove_file(source).await;

        Ok(())
    }
    .await;

    if copy_result.is_err() {
        let _ = fs::remove_file(&staging_path).await;
    }

    copy_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_directory_writable() {
        let temp = tempfile::tempdir().unwrap();
        assert!(validate_directory_writable(temp.path()).is_ok());

        let nested = temp.path().join("a").join("b").join("c");
        assert!(validate_directory_writable(&nested).is_ok());

        let relative = Path::new("relative/folder");
        assert!(validate_directory_writable(relative).is_err());
    }

    #[test]
    fn test_available_space() {
        let temp = tempfile::tempdir().unwrap();
        let free = available_space(temp.path()).unwrap();
        assert!(free > 0);
    }
}
