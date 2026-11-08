// Created: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Last updated: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Description: Configurable post-download completion actions, execution ledger records, and power actions.

use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PowerActionKind {
    Shutdown,
    Sleep,
    Hibernate,
}

impl PowerActionKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::Sleep => "sleep",
            Self::Hibernate => "hibernate",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CompletionAction {
    Notification,
    PlaySound {
        #[serde(default)]
        sound_file: Option<String>,
    },
    OpenFile,
    OpenFolder,
    AntivirusScan {
        executable: String,
        #[serde(default)]
        arguments: Vec<String>,
    },
    ExecuteCommand {
        executable: String,
        #[serde(default)]
        arguments: Vec<String>,
    },
    PowerAction {
        kind: PowerActionKind,
        #[serde(default)]
        confirmed: bool,
    },
}

impl CompletionAction {
    pub fn action_type(&self) -> &'static str {
        match self {
            Self::Notification => "notification",
            Self::PlaySound { .. } => "playSound",
            Self::OpenFile => "openFile",
            Self::OpenFolder => "openFolder",
            Self::AntivirusScan { .. } => "antivirusScan",
            Self::ExecuteCommand { .. } => "executeCommand",
            Self::PowerAction { .. } => "powerAction",
        }
    }

    pub fn action_key(&self) -> String {
        match self {
            Self::Notification => "notification".to_string(),
            Self::PlaySound { sound_file } => {
                format!("playSound:{}", sound_file.as_deref().unwrap_or("default"))
            }
            Self::OpenFile => "openFile".to_string(),
            Self::OpenFolder => "openFolder".to_string(),
            Self::AntivirusScan {
                executable,
                arguments,
            } => format!("antivirusScan:{}:{}", executable, arguments.join(" ")),
            Self::ExecuteCommand {
                executable,
                arguments,
            } => format!("executeCommand:{}:{}", executable, arguments.join(" ")),
            Self::PowerAction { kind, .. } => format!("powerAction:{}", kind.as_str()),
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::AntivirusScan { executable, .. } => {
                if executable.trim().is_empty() {
                    return Err("Antivirus scanner executable path cannot be empty.");
                }
                if executable.contains('\0') {
                    return Err("Executable path contains invalid characters.");
                }
            }
            Self::ExecuteCommand { executable, .. } => {
                if executable.trim().is_empty() {
                    return Err("Command executable path cannot be empty.");
                }
                if executable.contains('\0') {
                    return Err("Executable path contains invalid characters.");
                }
            }
            Self::PlaySound {
                sound_file: Some(path),
            } => {
                if path.trim().is_empty() {
                    return Err("Sound file path cannot be empty.");
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Formats the argument vector safely by replacing `{file}` in argument templates
    /// with the actual validated target path. If it's an AntivirusScan and `{file}` is not present in
    /// any argument, the target path is safely appended as the last argument.
    pub fn resolve_arguments(&self, target_file: &Path) -> Vec<String> {
        let file_str = target_file.to_string_lossy();
        match self {
            Self::AntivirusScan { arguments, .. } => {
                let mut has_placeholder = false;
                let mut resolved = Vec::with_capacity(arguments.len() + 1);
                for arg in arguments {
                    if arg.contains("{file}") {
                        has_placeholder = true;
                        resolved.push(arg.replace("{file}", &file_str));
                    } else {
                        resolved.push(arg.clone());
                    }
                }
                if !has_placeholder {
                    resolved.push(file_str.into_owned());
                }
                resolved
            }
            Self::ExecuteCommand { arguments, .. } => arguments
                .iter()
                .map(|arg| {
                    if arg.contains("{file}") {
                        arg.replace("{file}", &file_str)
                    } else {
                        arg.clone()
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActionStatus {
    Success,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionLedgerEntry {
    pub action_key: String,
    pub action_type: String,
    pub status: ActionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub executed_at: String,
}

impl ActionLedgerEntry {
    pub fn success(action: &CompletionAction, message: Option<String>) -> Self {
        Self {
            action_key: action.action_key(),
            action_type: action.action_type().to_string(),
            status: ActionStatus::Success,
            message,
            executed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn failed(action: &CompletionAction, error: impl Into<String>) -> Self {
        Self {
            action_key: action.action_key(),
            action_type: action.action_type().to_string(),
            status: ActionStatus::Failed,
            message: Some(error.into()),
            executed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn skipped(action: &CompletionAction, reason: impl Into<String>) -> Self {
        Self {
            action_key: action.action_key(),
            action_type: action.action_type().to_string(),
            status: ActionStatus::Skipped,
            message: Some(reason.into()),
            executed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn cancelled(action: &CompletionAction, reason: impl Into<String>) -> Self {
        Self {
            action_key: action.action_key(),
            action_type: action.action_type().to_string(),
            status: ActionStatus::Cancelled,
            message: Some(reason.into()),
            executed_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}
