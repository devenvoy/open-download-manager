use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIcon {
    pub extension: String,
    pub data_url: String,
}
