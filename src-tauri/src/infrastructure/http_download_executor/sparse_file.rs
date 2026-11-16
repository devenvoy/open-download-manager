// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(engine): high-performance sparse-file single-part allocation and positional writes (W3)

use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(windows)]
use std::time::Duration;

use crate::application::error::{AppError, AppResult};

pub const FOUR_GIBIBYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Global atomic counter for measuring fsync invocations during benchmark and runtime.
static FSYNC_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn reset_fsync_count() -> u64 {
    FSYNC_COUNT.swap(0, Ordering::SeqCst)
}

pub fn get_fsync_count() -> u64 {
    FSYNC_COUNT.load(Ordering::Relaxed)
}

/// Allocates a single-part sparse file on disk for the given total byte size.
/// Performs FAT32 4 GiB size checks and applies FSCTL_SET_SPARSE on Windows.
pub fn allocate_sparse_file(path: &Path, total_size: u64) -> AppResult<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::download(format!("Could not create download folder: {e}"))
        })?;
    }

    // Pre-flight check: FAT32 4 GiB single-file limit
    if total_size >= FOUR_GIBIBYTES && is_fat32_filesystem(path) {
        return Err(AppError::download(
            "FAT32 file system does not support files 4 GiB or larger.",
        ));
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| AppError::download(format!("Could not create sparse file: {e}")))?;

    #[cfg(windows)]
    {
        // Try to enable sparse flag via FSCTL_SET_SPARSE
        let _ = set_sparse_flag(&file);
    }

    // Set file length to full expected download size
    if total_size > 0 {
        file.set_len(total_size).map_err(|e| {
            AppError::download(format!("Could not allocate sparse file capacity: {e}"))
        })?;
    }

    Ok(file)
}

/// Open an independent file handle for a parallel worker thread.
/// Each worker task owns its own handle to avoid lock contention on seek pointers.
pub fn open_worker_handle(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
}

/// Write bytes at an exact byte offset without modifying or sharing seek pointers.
pub fn write_at(file: &File, offset: u64, data: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.write_all_at(data, offset)?;
        Ok(())
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut written_total = 0;
        while written_total < data.len() {
            let to_write = &data[written_total..];
            let current_offset = offset + written_total as u64;
            let written = file.seek_write(to_write, current_offset)?;
            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Zero bytes written to sparse file",
                ));
            }
            written_total += written;
        }
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = file;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(data)?;
        Ok(())
    }
}

/// Flush order: flush data bytes to disk before updating range map state.
/// Increments the atomic fsync counter.
pub fn sync_worker_data(file: &File) -> io::Result<()> {
    FSYNC_COUNT.fetch_add(1, Ordering::Relaxed);
    #[cfg(unix)]
    {
        // sync_data flushes data blocks without redundant directory inode metadata sync
        file.sync_data()
    }
    #[cfg(not(unix))]
    {
        file.sync_all()
    }
}

/// Moves completed file into place with Windows Defender / antivirus retry backoff.
pub async fn rename_with_av_retry(src: &Path, dst: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        let backoffs = [
            Duration::from_millis(50),
            Duration::from_millis(100),
            Duration::from_millis(200),
            Duration::from_millis(400),
            Duration::from_millis(800),
        ];

        for (attempt, delay) in backoffs.iter().enumerate() {
            match tokio::fs::rename(src, dst).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let code = e.raw_os_error().unwrap_or(0);
                    // ERROR_SHARING_VIOLATION = 32, ERROR_ACCESS_DENIED = 5
                    if (code == 32 || code == 5) && attempt < backoffs.len() - 1 {
                        tokio::time::sleep(*delay).await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        tokio::fs::rename(src, dst).await
    }

    #[cfg(not(windows))]
    {
        tokio::fs::rename(src, dst).await
    }
}

#[cfg(windows)]
fn set_sparse_flag(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    let handle = file.as_raw_handle();
    let mut bytes_returned = 0u32;
    let success = unsafe {
        windows_sys::Win32::System::Ioctl::DeviceIoControl(
            handle as _,
            windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn is_fat32_filesystem(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let root = match path.components().next() {
            Some(std::path::Component::Prefix(p)) => format!("{}:\\", p.as_os_str().to_string_lossy()),
            _ => return false,
        };
        let root_wide: Vec<u16> = std::ffi::OsStr::new(&root).encode_wide().chain(std::iter::once(0)).collect();
        let mut fs_name = [0u16; 256];
        let success = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW(
                root_wide.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs_name.as_mut_ptr(),
                fs_name.len() as u32,
            )
        };
        if success != 0 {
            let name = String::from_utf16_lossy(&fs_name);
            name.to_uppercase().contains("FAT")
        } else {
            false
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        if let Ok(c_path) = CString::new(path.to_string_lossy().as_bytes()) {
            unsafe {
                let mut s: libc::statfs = std::mem::zeroed();
                if libc::statfs(c_path.as_ptr(), &mut s) == 0 {
                    let fs_name = std::ffi::CStr::from_ptr(s.f_fstypename.as_ptr()).to_string_lossy();
                    return fs_name.to_ascii_uppercase().contains("MSDOS") || fs_name.to_ascii_uppercase().contains("FAT");
                }
            }
        }
        false
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = path;
        false
    }
}

pub fn check_fat32_size_limit(is_fat: bool, total_size: u64) -> AppResult<()> {
    if is_fat && total_size >= FOUR_GIBIBYTES {
        Err(AppError::download(
            "FAT32 file system does not support files 4 GiB or larger.",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_sparse_file_allocation_and_positional_writes() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let path = tmp_dir.path().join("sparse_test.odm.part");

        reset_fsync_count();
        let file = allocate_sparse_file(&path, 1024 * 1024).expect("allocate succeeds");
        assert_eq!(file.metadata().unwrap().len(), 1024 * 1024);

        // Independent worker handle
        let worker_file = open_worker_handle(&path).expect("open worker handle");

        // Write at offset 500,000
        let payload = b"positional-sparse-write-data";
        write_at(&worker_file, 500_000, payload).expect("write_at succeeds");
        sync_worker_data(&worker_file).expect("sync succeeds");
        assert_eq!(get_fsync_count(), 1);

        drop(file);
        drop(worker_file);

        // Read back and verify bytes at offset
        let mut read_back = File::open(&path).unwrap();
        use std::io::Seek;
        read_back.seek(std::io::SeekFrom::Start(500_000)).unwrap();
        let mut buf = vec![0u8; payload.len()];
        read_back.read_exact(&mut buf).unwrap();
        assert_eq!(buf, payload);
    }

    #[test]
    fn test_fat32_4gib_limit_rejection() {
        assert!(check_fat32_size_limit(true, FOUR_GIBIBYTES).is_err());
        assert!(check_fat32_size_limit(true, FOUR_GIBIBYTES + 1024).is_err());
        assert!(check_fat32_size_limit(true, 1024 * 1024).is_ok());
        assert!(check_fat32_size_limit(false, FOUR_GIBIBYTES * 2).is_ok());
    }
}
