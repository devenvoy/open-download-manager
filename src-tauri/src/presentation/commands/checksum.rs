// Created: 2026-11-17
// Reason: feat(checksum): Tauri IPC command for on-demand and automatic checksum verification
// Description: Presentation command handlers for verifying and calculating file digests.

use std::path::PathBuf;

use crate::{
    application::{
        checksum_service::{ChecksumService, ChecksumVerificationReport},
        error::{AppError, AppResult},
    },
    domain::checksum::ChecksumAlgorithm,
};

#[tauri::command(async)]
pub async fn verify_checksum(
    path: String,
    expected: String,
) -> AppResult<ChecksumVerificationReport> {
    let _timer = crate::infrastructure::perf::track_command("verify_checksum");
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(AppError::storage(format!("File does not exist: {path}")));
    }

    tokio::task::spawn_blocking(move || {
        ChecksumService::verify(&file_path, &expected, None)
    })
    .await
    .map_err(|e| AppError::storage(format!("Task execution failed: {e}")))?
}

#[tauri::command(async)]
pub async fn calculate_checksum(
    path: String,
    algorithm: String,
) -> AppResult<String> {
    let _timer = crate::infrastructure::perf::track_command("calculate_checksum");
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(AppError::storage(format!("File does not exist: {path}")));
    }

    let algo = match algorithm.to_ascii_lowercase().replace('-', "").as_str() {
        "crc32" => ChecksumAlgorithm::Crc32,
        "sha256" => ChecksumAlgorithm::Sha256,
        "sha512" => ChecksumAlgorithm::Sha512,
        _ => return Err(AppError::invalid_settings("Supported algorithms: CRC32, SHA-256, SHA-512.")),
    };

    let checksum = tokio::task::spawn_blocking(move || {
        ChecksumService::calculate(&file_path, algo, None)
    })
    .await
    .map_err(|e| AppError::storage(format!("Task execution failed: {e}")))? ;

    checksum.map(|c| c.hex())
}
