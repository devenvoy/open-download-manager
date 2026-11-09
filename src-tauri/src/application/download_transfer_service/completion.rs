// Created: 2026-11-10
// Reason: refactor: modularize monolithic download_transfer_service into focused coordinators
// Description: Post-download completion actions: sound, notifications, folder reveal, process execution, antivirus scan.

use std::path::Path;

use crate::{
    application::error::{AppError, AppResult},
    domain::{
        completion::{ActionLedgerEntry, ActionStatus, CompletionAction},
        download::DownloadStatus,
    },
};

use super::DownloadTransferService;

impl DownloadTransferService {
    pub async fn retry_completion_actions(&self, id: &str) -> AppResult<()> {
        let record = self.history.download(id)?;
        if record.status != DownloadStatus::Completed {
            return Err(AppError::download(
                "Actions can only be run on completed downloads.",
            ));
        }
        let path_str = record
            .path
            .ok_or_else(|| AppError::download("Download file path is missing."))?;
        let path = std::path::PathBuf::from(path_str);
        if !path.is_file() {
            return Err(AppError::download(
                "Downloaded file does not exist at final destination.",
            ));
        }
        self.history.update_download(id, |d| {
            d.action_ledger.clear();
            d.notice = None;
        })?;
        self.run_completion_actions(id, &path).await;
        Ok(())
    }

    pub fn update_download_completion_actions(
        &self,
        id: &str,
        actions: Option<Vec<CompletionAction>>,
    ) -> AppResult<()> {
        if let Some(ref actions) = actions {
            for action in actions {
                if let Err(msg) = action.validate() {
                    return Err(AppError::invalid_download(msg));
                }
            }
        }
        self.history.update_download(id, |d| {
            d.completion_actions = actions;
        })
    }

    pub async fn run_completion_actions(&self, id: &str, final_path: &Path) {
        if !final_path.is_file() {
            eprintln!(
                "Skipping completion actions for {id}: destination file does not exist or is not a file."
            );
            return;
        }

        let record = match self.history.download(id) {
            Ok(record) => record,
            Err(error) => {
                eprintln!(
                    "Could not retrieve download record for {id} to run completion actions: {}",
                    error.message
                );
                return;
            }
        };

        if record.status != DownloadStatus::Completed {
            return;
        }

        let current_settings = self.settings();
        let effective_actions = if let Some(ref actions) = record.completion_actions {
            actions.clone()
        } else if let Some(ref settings) = current_settings {
            if !settings.completion_actions.is_empty() {
                settings.completion_actions.clone()
            } else {
                let mut legacy = Vec::new();
                if settings.notify_on_complete {
                    legacy.push(CompletionAction::Notification);
                }
                if settings.reveal_on_complete {
                    legacy.push(CompletionAction::OpenFolder);
                }
                legacy
            }
        } else {
            Vec::new()
        };

        if effective_actions.is_empty() {
            return;
        }

        let timeout_duration = std::time::Duration::from_secs(60);

        for action in effective_actions {
            let already_run = self
                .history
                .with_downloads(|downloads| {
                    downloads.iter().find(|d| d.id == id).is_some_and(|d| {
                        d.action_ledger
                            .iter()
                            .any(|entry| entry.action_key == action.action_key())
                    })
                })
                .unwrap_or(false);

            if already_run {
                continue;
            }

            let entry = match &action {
                CompletionAction::Notification => {
                    let _ = self.history.notify_complete(id);
                    ActionLedgerEntry::success(&action, None)
                }
                CompletionAction::PlaySound { sound_file } => {
                    match self.action_executor.play_sound(sound_file.as_deref()).await {
                        Ok(()) => ActionLedgerEntry::success(&action, None),
                        Err(err) => ActionLedgerEntry::failed(&action, err),
                    }
                }
                CompletionAction::OpenFile => {
                    match self.action_executor.open_file(final_path) {
                        Ok(()) => ActionLedgerEntry::success(&action, None),
                        Err(err) => ActionLedgerEntry::failed(&action, err),
                    }
                }
                CompletionAction::OpenFolder => {
                    match self.action_executor.open_folder(final_path) {
                        Ok(()) => ActionLedgerEntry::success(&action, None),
                        Err(err) => ActionLedgerEntry::failed(&action, err),
                    }
                }
                CompletionAction::AntivirusScan { executable, .. } => {
                    let safe_args = action.resolve_arguments(final_path);
                    match self
                        .action_executor
                        .execute_process(executable, &safe_args, timeout_duration)
                        .await
                    {
                        Ok(output) => {
                            let msg = if output.is_empty() {
                                None
                            } else {
                                Some(output)
                            };
                            ActionLedgerEntry::success(&action, msg)
                        }
                        Err(err) => ActionLedgerEntry::failed(&action, err),
                    }
                }
                CompletionAction::ExecuteCommand {
                    executable, ..
                } => {
                    let safe_args = action.resolve_arguments(final_path);
                    match self
                        .action_executor
                        .execute_process(executable, &safe_args, timeout_duration)
                        .await
                    {
                        Ok(output) => {
                            let msg = if output.is_empty() {
                                None
                            } else {
                                Some(output)
                            };
                            ActionLedgerEntry::success(&action, msg)
                        }
                        Err(err) => ActionLedgerEntry::failed(&action, err),
                    }
                }
                CompletionAction::PowerAction {
                    kind,
                    confirmed,
                } => {
                    if !*confirmed {
                        ActionLedgerEntry::skipped(
                            &action,
                            "Power action requires explicit confirmation.",
                        )
                    } else {
                        match self.action_executor.power_action(*kind).await {
                            Ok(()) => ActionLedgerEntry::success(&action, None),
                            Err(err) => ActionLedgerEntry::failed(&action, err),
                        }
                    }
                }
            };

            let is_failure = entry.status == ActionStatus::Failed;
            let failure_message = entry.message.clone();
            let action_type = entry.action_type.clone();

            let _ = self.history.update_download(id, |d| {
                d.action_ledger.push(entry);
                if is_failure {
                    if let Some(msg) = failure_message {
                        d.notice = Some(format!("Action '{action_type}' failed: {msg}"));
                    }
                }
            });
        }
    }
}
