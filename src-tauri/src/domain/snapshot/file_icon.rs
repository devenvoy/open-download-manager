// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Cached base64 SVG file icon representation corresponding to file format categories.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIcon {
    pub extension: String,
    pub data_url: String,
}
