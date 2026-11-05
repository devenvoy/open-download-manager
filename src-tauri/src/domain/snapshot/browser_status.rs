use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub name: String,
    pub installed: bool,
    pub connected: bool,
}
