// Created: 2026-11-17
// Reason: feat(checksum): streamed whole-file hashing, digest verification, and antivirus-safe atomic completion
// Description: Application service for calculating file checksums and verifying against published digests.

use std::{
    fs::File,
    io::Read,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use sha2::Digest as _;

use crate::{
    application::error::{AppError, AppResult},
    domain::checksum::{Checksum, ChecksumAlgorithm},
};

const READ_BUFFER_SIZE: usize = 1024 * 1024; // 1 MB streaming buffer
const RENAME_RETRY_INTERVALS: [Duration; 5] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
];

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksumVerificationReport {
    pub matched: bool,
    pub algorithm: String,
    pub expected_hex: String,
    pub actual_hex: String,
    pub file_bytes: u64,
}

pub struct ChecksumService;

impl ChecksumService {
    pub fn new() -> Self {
        Self
    }

    /// Calculates a file's checksum using the specified algorithm.
    /// Supports cooperative cancellation via `cancel_flag`.
    pub fn calculate(
        path: &Path,
        algorithm: ChecksumAlgorithm,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> AppResult<Checksum> {
        let mut file = File::open(path).map_err(|e| {
            AppError::storage(format!("Could not open file for checksum calculation: {e}"))
        })?;

        let mut buffer = vec![0u8; READ_BUFFER_SIZE];

        let value = match algorithm {
            ChecksumAlgorithm::Crc32 => {
                let mut hasher = crc32fast::Hasher::new();
                loop {
                    if let Some(ref flag) = cancel_flag {
                        if flag.load(Ordering::Relaxed) {
                            return Err(AppError::download("Checksum calculation was cancelled."));
                        }
                    }
                    let n = file.read(&mut buffer).map_err(|e| {
                        AppError::storage(format!("Error reading file during CRC32 hashing: {e}"))
                    })?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                hasher.finalize().to_be_bytes().to_vec()
            }
            ChecksumAlgorithm::Sha256 => {
                let mut hasher = sha2::Sha256::new();
                loop {
                    if let Some(ref flag) = cancel_flag {
                        if flag.load(Ordering::Relaxed) {
                            return Err(AppError::download("Checksum calculation was cancelled."));
                        }
                    }
                    let n = file.read(&mut buffer).map_err(|e| {
                        AppError::storage(format!("Error reading file during SHA-256 hashing: {e}"))
                    })?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                hasher.finalize().to_vec()
            }
            ChecksumAlgorithm::Sha512 => {
                let mut hasher = sha2::Sha512::new();
                loop {
                    if let Some(ref flag) = cancel_flag {
                        if flag.load(Ordering::Relaxed) {
                            return Err(AppError::download("Checksum calculation was cancelled."));
                        }
                    }
                    let n = file.read(&mut buffer).map_err(|e| {
                        AppError::storage(format!("Error reading file during SHA-512 hashing: {e}"))
                    })?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                hasher.finalize().to_vec()
            }
        };

        Checksum::new(algorithm, value)
            .ok_or_else(|| AppError::download("Calculated digest size mismatch."))
    }

    /// Verifies a file against an expected checksum string.
    pub fn verify(
        path: &Path,
        expected_checksum_text: &str,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> AppResult<ChecksumVerificationReport> {
        let expected = Checksum::parse(expected_checksum_text).ok_or_else(|| {
            AppError::invalid_settings(
                "Invalid checksum format. Expected CRC32 (8 hex), SHA-256 (64 hex), or SHA-512 (128 hex)."
            )
        })?;

        let file_bytes = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        let actual = Self::calculate(path, expected.algorithm, cancel_flag)?;
        let matched = actual.value == expected.value;

        Ok(ChecksumVerificationReport {
            matched,
            algorithm: expected.algorithm.name().to_string(),
            expected_hex: expected.hex(),
            actual_hex: actual.hex(),
            file_bytes,
        })
    }

    /// Performs an antivirus-safe atomic rename of a completed `.part` file to its final destination.
    /// Handles Windows Defender and real-time antivirus scanner file locks by retrying with
    /// exponential backoff (50ms, 100ms, 200ms, 400ms, 800ms) on sharing violations (error 32)
    /// or permission errors (error 5).
    pub async fn rename_with_antivirus_retry(from: &Path, to: &Path) -> std::io::Result<()> {
        let mut last_err = None;

        for (attempt, &interval) in RENAME_RETRY_INTERVALS.iter().enumerate() {
            match std::fs::rename(from, to) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    // Check for antivirus lock condition
                    let is_lock_error = err.kind() == std::io::ErrorKind::PermissionDenied
                        || err.raw_os_error() == Some(32) // ERROR_SHARING_VIOLATION on Windows
                        || err.raw_os_error() == Some(5); // ERROR_ACCESS_DENIED on Windows

                    if is_lock_error && attempt < RENAME_RETRY_INTERVALS.len() - 1 {
                        tokio::time::sleep(interval).await;
                        last_err = Some(err);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Atomic rename timed out after antivirus retries.")
        }))
    }
}

impl Default for ChecksumService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_and_verify_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_file.txt");
        let payload = b"Hello Open Download Manager Checksum Verification!";
        std::fs::write(&file_path, payload).unwrap();

        // SHA-256 of payload:
        let mut hasher = sha2::Sha256::new();
        hasher.update(payload);
        let expected_sha256 = hex::encode(hasher.finalize());

        let report = ChecksumService::verify(&file_path, &expected_sha256, None).unwrap();
        assert!(report.matched);
        assert_eq!(report.algorithm, "SHA-256");
        assert_eq!(report.expected_hex, expected_sha256);
        assert_eq!(report.actual_hex, expected_sha256);
        assert_eq!(report.file_bytes, payload.len() as u64);

        // Mismatched hash
        let bogus_sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
        let mismatch_report = ChecksumService::verify(&file_path, bogus_sha256, None).unwrap();
        assert!(!mismatch_report.matched);
        assert_eq!(mismatch_report.expected_hex, bogus_sha256);
        assert_eq!(mismatch_report.actual_hex, expected_sha256);
    }

    #[test]
    fn test_calculate_and_verify_crc32() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("crc_file.bin");
        let payload = b"123456789";
        std::fs::write(&file_path, payload).unwrap();

        // Standard CRC32 of "123456789" is 0xcbf43926
        let report = ChecksumService::verify(&file_path, "cbf43926", None).unwrap();
        assert!(report.matched);
        assert_eq!(report.algorithm, "CRC32");
        assert_eq!(report.actual_hex, "cbf43926");
    }

    #[tokio::test]
    async fn test_rename_with_antivirus_retry_success() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("download.part");
        let to = dir.path().join("download.final");
        std::fs::write(&from, b"content").unwrap();

        let res = ChecksumService::rename_with_antivirus_retry(&from, &to).await;
        assert!(res.is_ok());
        assert!(!from.exists());
        assert!(to.exists());
    }
}
