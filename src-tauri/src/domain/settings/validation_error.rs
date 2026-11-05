#[derive(Debug)]
pub struct SettingsValidationError {
    pub message: String,
}

impl SettingsValidationError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
