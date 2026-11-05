use serde::{Deserialize, Serialize};

use crate::domain::settings::Settings;

#[derive(Deserialize, Serialize)]
pub(super) struct PersistedSettings {
    pub(super) version: u8,
    pub(super) settings: Settings,
}
