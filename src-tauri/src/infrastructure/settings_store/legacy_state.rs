// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Legacy state deserializer migrating pre-v1 download history schemas.

use serde::Deserialize;

use super::legacy_settings::LegacySettings;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyState {
    pub(super) version: u8,
    pub(super) settings: Option<LegacySettings>,
}
