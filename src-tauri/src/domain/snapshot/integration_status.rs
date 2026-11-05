use serde::Serialize;

use super::browser_status::BrowserStatus;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub browsers: Vec<BrowserStatus>,
    pub connection_error: Option<String>,
}

impl IntegrationStatus {
    pub fn unavailable(message: String) -> Self {
        Self {
            browsers: Vec::new(),
            connection_error: Some(message),
        }
    }
}
