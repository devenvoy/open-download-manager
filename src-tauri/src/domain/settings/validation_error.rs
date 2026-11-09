// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Validation error representation for out-of-bounds or invalid configuration settings.

#[derive(Debug)]
pub struct SettingsValidationError {
    pub message: String,
}

impl SettingsValidationError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
