// Created: 2026-11-08
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Description: Application orchestration service managing advanced integration state, bounded health verification, and auto-rollback.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    application::error::{AppError, AppResult},
    domain::browser::advanced_integration::{
        filter_advanced_media_candidate, AdvancedIntegrationAvailability,
        AdvancedIntegrationCapability, AdvancedIntegrationState, AdvancedMediaCandidate,
        HealthCheckResult,
    },
    infrastructure::browser::AdvancedIntegrationPlatformAdapter,
};

pub const DEFAULT_HEALTH_CHECK_TIMEOUT_MS: u64 = 3000;

pub struct AdvancedIntegrationService {
    adapter: Arc<dyn AdvancedIntegrationPlatformAdapter>,
    capability: Mutex<AdvancedIntegrationCapability>,
    health_timeout: Duration,
}

impl AdvancedIntegrationService {
    pub fn new(adapter: Arc<dyn AdvancedIntegrationPlatformAdapter>) -> Self {
        let availability = adapter.probe_availability();
        let capability = AdvancedIntegrationCapability::new(availability);
        Self {
            adapter,
            capability: Mutex::new(capability),
            health_timeout: Duration::from_millis(DEFAULT_HEALTH_CHECK_TIMEOUT_MS),
        }
    }

    pub fn with_timeout(
        adapter: Arc<dyn AdvancedIntegrationPlatformAdapter>,
        timeout: Duration,
    ) -> Self {
        let availability = adapter.probe_availability();
        let capability = AdvancedIntegrationCapability::new(availability);
        Self {
            adapter,
            capability: Mutex::new(capability),
            health_timeout: timeout,
        }
    }

    pub fn capability(&self) -> AdvancedIntegrationCapability {
        let guard = self.capability.lock().unwrap();
        guard.clone()
    }

    pub fn enable(&self, consent: bool) -> AppResult<AdvancedIntegrationCapability> {
        let availability = self.adapter.probe_availability();
        if !availability.is_supported() {
            let reason = match availability {
                AdvancedIntegrationAvailability::Unsupported { reason, .. } => reason,
                _ => "Platform does not support advanced integration.".to_owned(),
            };
            let mut guard = self.capability.lock().unwrap();
            guard.state = AdvancedIntegrationState::Unsupported {
                reason: reason.clone(),
            };
            return Err(AppError::browser(format!(
                "Cannot enable advanced integration: {reason}"
            )));
        }

        if !consent {
            let mut guard = self.capability.lock().unwrap();
            guard.state = AdvancedIntegrationState::AwaitingConsent;
            return Err(AppError::browser(
                "Explicit user consent is required before installing or activating advanced integration.",
            ));
        }

        // Install or activate privileged component
        if let Err(e) = self.adapter.install_or_activate(true) {
            let action = self.adapter.rollback(&e);
            let mut guard = self.capability.lock().unwrap();
            guard.state = AdvancedIntegrationState::Degraded {
                reason: e.clone(),
                rolled_back: true,
            };
            guard.rollback_action = Some(action);
            return Err(AppError::browser(format!(
                "Failed to activate advanced integration, rolled back to standard integration: {e}"
            )));
        }

        // Run bounded non-destructive health verification
        match self.adapter.check_health(self.health_timeout) {
            Ok(check) if check.success && !check.conflict_detected => {
                let mut guard = self.capability.lock().unwrap();
                guard.state = AdvancedIntegrationState::Active;
                guard.last_health_check = Some(check);
                guard.rollback_action = None;
                Ok(guard.clone())
            }
            Ok(check) if check.conflict_detected => {
                let reason = format!(
                    "Conflict with third-party software detected: {}",
                    check.details
                );
                let action = self.adapter.rollback(&reason);
                let mut guard = self.capability.lock().unwrap();
                guard.state = AdvancedIntegrationState::Degraded {
                    reason: reason.clone(),
                    rolled_back: true,
                };
                guard.last_health_check = Some(check);
                guard.rollback_action = Some(action);
                Err(AppError::browser(format!(
                    "Health check failed due to conflict, automatically rolled back to standard integration: {reason}"
                )))
            }
            Ok(check) => {
                let reason = format!("Health verification check failed: {}", check.details);
                let action = self.adapter.rollback(&reason);
                let mut guard = self.capability.lock().unwrap();
                guard.state = AdvancedIntegrationState::Degraded {
                    reason: reason.clone(),
                    rolled_back: true,
                };
                guard.last_health_check = Some(check);
                guard.rollback_action = Some(action);
                Err(AppError::browser(format!(
                    "Health check failed, automatically rolled back to standard integration: {reason}"
                )))
            }
            Err(e) => {
                let reason = format!("Health check error: {e}");
                let action = self.adapter.rollback(&reason);
                let mut guard = self.capability.lock().unwrap();
                guard.state = AdvancedIntegrationState::Degraded {
                    reason: reason.clone(),
                    rolled_back: true,
                };
                guard.last_health_check = Some(HealthCheckResult::failure(&reason));
                guard.rollback_action = Some(action);
                Err(AppError::browser(format!(
                    "Health check failed, automatically rolled back to standard integration: {reason}"
                )))
            }
        }
    }

    pub fn disable(&self) -> AppResult<AdvancedIntegrationCapability> {
        let action = self
            .adapter
            .rollback("User disabled advanced integration mode.");
        let mut guard = self.capability.lock().unwrap();
        if guard.is_supported() {
            guard.state = AdvancedIntegrationState::Disabled;
        }
        guard.rollback_action = Some(action);
        Ok(guard.clone())
    }

    pub fn repair(&self) -> AppResult<AdvancedIntegrationCapability> {
        self.enable(true)
    }

    pub fn filter_candidate(
        &self,
        candidate: AdvancedMediaCandidate,
    ) -> AppResult<AdvancedMediaCandidate> {
        let guard = self.capability.lock().unwrap();
        if !matches!(guard.state, AdvancedIntegrationState::Active) {
            return Err(AppError::browser(
                "Advanced integration is not active. Media request must be handled through standard extension handoff.",
            ));
        }
        drop(guard);

        filter_advanced_media_candidate(&candidate)
            .map_err(|e| AppError::browser(format!("Media candidate rejected: {e}")))
    }
}
