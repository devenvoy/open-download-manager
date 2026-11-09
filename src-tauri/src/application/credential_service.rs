// Created: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: Application service managing site authentication rules, password storage, and credential queries.

use std::sync::Arc;

use crate::{
    application::error::AppResult,
    domain::credentials::{CredentialMatcher, CredentialRule},
    infrastructure::credentials::CredentialStore,
};

/// Service managing persistent site/path credential rules.
#[derive(Clone, Debug)]
pub struct CredentialService {
    store: Arc<CredentialStore>,
}

impl CredentialService {
    pub fn new(store: Arc<CredentialStore>) -> Self {
        Self { store }
    }

    /// Returns all stored rules.
    pub fn list(&self) -> AppResult<Vec<CredentialRule>> {
        self.store.load()
    }

    /// Returns all stored rules with secrets redacted as "******" for presentation.
    pub fn list_redacted(&self) -> AppResult<Vec<CredentialRule>> {
        let rules = self.store.load()?;
        Ok(rules.into_iter().map(|r| r.redacted()).collect())
    }

    /// Saves or updates a rule.
    pub fn save(&self, rule: CredentialRule) -> AppResult<CredentialRule> {
        self.store.add_or_update(rule.clone())?;
        Ok(rule)
    }

    /// Deletes a rule by ID.
    pub fn delete(&self, id: &str) -> AppResult<()> {
        let _ = self.store.delete(id)?;
        Ok(())
    }

    /// Constructs a matcher from the active rules.
    pub fn matcher(&self) -> AppResult<CredentialMatcher> {
        self.store.matcher()
    }
}
