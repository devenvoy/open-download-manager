// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: JSON schema model for persisted configuration files on disk.

use serde::{Deserialize, Serialize};

use crate::domain::settings::Settings;

#[derive(Deserialize, Serialize)]
pub(super) struct PersistedSettings {
    pub(super) version: u8,
    pub(super) settings: Settings,
}
