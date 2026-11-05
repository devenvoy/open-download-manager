use serde::{Deserialize, Serialize};

use super::browser_status::BrowserStatus;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedBrowser {
    pub browser: String,
    pub name: String,
    pub connected: bool,
    pub profiles: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub port: Option<u16>,
    pub connected_browsers: Vec<ConnectedBrowser>,
    pub browsers: Vec<BrowserStatus>,
    pub connection_error: Option<String>,
    pub extension_path: String,
    pub integration_path: String,
    pub packaged: bool,
    pub executable: String,
}

impl IntegrationStatus {
    pub fn unavailable(message: String) -> Self {
        Self {
            port: None,
            connected_browsers: Vec::new(),
            browsers: Vec::new(),
            connection_error: Some(message),
            extension_path: String::new(),
            integration_path: String::new(),
            packaged: false,
            executable: String::new(),
        }
    }
}
