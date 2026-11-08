// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-06
// Reason: perf: streamline snapshot IPC construction and tune segment assembly buffer
// Description: High-level state snapshot combining downloads, settings, and integration health for frontend.

mod browser_status;
mod download_summary;
mod file_icon;
mod integration_status;
mod policy;

pub use browser_status::BrowserStatus;
pub use download_summary::DownloadSummary;
pub use file_icon::FileIcon;
pub use integration_status::{ConnectedBrowser, IntegrationStatus};

use std::collections::BTreeMap;
use serde::Serialize;

use crate::domain::{download::DownloadRecord, settings::Settings};

use self::policy::BROWSER_HANDOFF_UNAVAILABLE_MESSAGE;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub downloads: Vec<DownloadSummary>,
    pub settings: Settings,
    pub integration: IntegrationStatus,
    pub file_icons: BTreeMap<String, String>,
}

impl Snapshot {
    pub fn new(settings: Settings, downloads: Vec<DownloadRecord>) -> Self {
        Self {
            downloads: downloads.into_iter().map(DownloadSummary::from).collect(),
            settings,
            integration: IntegrationStatus::unavailable(
                BROWSER_HANDOFF_UNAVAILABLE_MESSAGE.to_owned(),
            ),
            file_icons: BTreeMap::new(),
        }
    }

    pub fn with_integration(
        settings: Settings,
        downloads: Vec<DownloadRecord>,
        integration: IntegrationStatus,
    ) -> Self {
        Self {
            downloads: downloads.into_iter().map(DownloadSummary::from).collect(),
            settings,
            integration,
            file_icons: BTreeMap::new(),
        }
    }

    pub fn with_summaries(
        settings: Settings,
        downloads: Vec<DownloadSummary>,
        integration: IntegrationStatus,
    ) -> Self {
        Self {
            downloads,
            settings,
            integration,
            file_icons: BTreeMap::new(),
        }
    }

    pub fn with_file_icons(mut self, file_icons: BTreeMap<String, String>) -> Self {
        self.file_icons = file_icons;
        self
    }
}
