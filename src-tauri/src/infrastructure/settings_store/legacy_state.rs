use serde::Deserialize;

use super::legacy_settings::LegacySettings;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyState {
    pub(super) version: u8,
    pub(super) settings: Option<LegacySettings>,
}
