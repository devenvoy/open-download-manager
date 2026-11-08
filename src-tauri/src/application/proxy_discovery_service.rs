// Created: 2026-11-08
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Description: Application service and ports for discovering system/browser proxy configurations.

use std::sync::Arc;

use crate::{
    application::error::{AppError, AppResult},
    domain::settings::DetectedProxyConfig,
};

/// Port for platform-specific proxy detection.
pub trait ProxyDiscoveryAdapter: Send + Sync {
    fn detect(&self) -> AppResult<DetectedProxyConfig>;
}

/// Service orchestrating discovery of system and browser proxy configurations.
pub struct ProxyDiscoveryService {
    adapter: Arc<dyn ProxyDiscoveryAdapter>,
}

impl ProxyDiscoveryService {
    pub fn new(adapter: Arc<dyn ProxyDiscoveryAdapter>) -> Self {
        Self { adapter }
    }

    pub fn detect(&self) -> AppResult<DetectedProxyConfig> {
        self.adapter.detect()
    }
}

/// Test adapter with configurable responses.
#[derive(Default)]
pub struct MockProxyDiscoveryAdapter {
    next_result: std::sync::Mutex<Option<DetectedProxyConfig>>,
}

impl MockProxyDiscoveryAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_result(&self, config: DetectedProxyConfig) {
        *self.next_result.lock().unwrap() = Some(config);
    }
}

impl ProxyDiscoveryAdapter for MockProxyDiscoveryAdapter {
    fn detect(&self) -> AppResult<DetectedProxyConfig> {
        self.next_result
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::network("No mock proxy configured"))
    }
}
