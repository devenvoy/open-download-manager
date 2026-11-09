// Created: 2026-11-08
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Description: Domain entity representing application theme choices (System, Light, Dark).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_default_is_system() {
        assert_eq!(AppTheme::default(), AppTheme::System);
    }

    #[test]
    fn test_theme_serialization_lowercase() {
        assert_eq!(
            serde_json::to_string(&AppTheme::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&AppTheme::Light).unwrap(),
            "\"light\""
        );
        assert_eq!(
            serde_json::to_string(&AppTheme::Dark).unwrap(),
            "\"dark\""
        );
    }

    #[test]
    fn test_theme_deserialization() {
        assert_eq!(
            serde_json::from_str::<AppTheme>("\"system\"").unwrap(),
            AppTheme::System
        );
        assert_eq!(
            serde_json::from_str::<AppTheme>("\"light\"").unwrap(),
            AppTheme::Light
        );
        assert_eq!(
            serde_json::from_str::<AppTheme>("\"dark\"").unwrap(),
            AppTheme::Dark
        );
        assert!(serde_json::from_str::<AppTheme>("\"invalid\"").is_err());
    }
}
