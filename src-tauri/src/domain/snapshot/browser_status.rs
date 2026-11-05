use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub name: String,
    pub installed: bool,
    pub connected: bool,
    #[serde(default = "default_setup_state")]
    pub setup_state: String,
    #[serde(default)]
    pub profiles: usize,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub extension_page: String,
    #[serde(default)]
    pub store_url: Option<String>,
    #[serde(default)]
    pub extension_path: String,
    #[serde(default = "default_support")]
    pub support: String,
}

fn default_setup_state() -> String {
    "missing".to_owned()
}

fn default_support() -> String {
    "supported".to_owned()
}
