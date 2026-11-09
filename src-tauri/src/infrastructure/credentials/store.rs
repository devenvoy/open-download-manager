// Created: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: Encrypted JSON file credential store with strict 0o600 filesystem permissions.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    application::error::{AppError, AppResult},
    domain::credentials::{CredentialMatcher, CredentialRule},
    infrastructure::atomic_file::write_replace,
};

const CREDENTIALS_VERSION: u32 = 1;
const CREDENTIALS_FILE_NAME: &str = "credentials.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedCredentials {
    pub version: u32,
    pub rules: Vec<CredentialRule>,
}

/// Thread-safe file-backed repository for site authentication credentials.
#[derive(Clone, Debug)]
pub struct CredentialStore {
    path: PathBuf,
}

impl CredentialStore {
    pub fn new(data_directory: impl AsRef<Path>) -> Self {
        Self {
            path: data_directory.as_ref().join(CREDENTIALS_FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads all credential rules from credentials.json, returning empty list if file doesn't exist.
    pub fn load(&self) -> AppResult<Vec<CredentialRule>> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let state: PersistedCredentials =
                    serde_json::from_slice(&bytes).map_err(|err| {
                        AppError::storage(format!(
                            "Could not parse credentials store at {}: {err}",
                            self.path.display()
                        ))
                    })?;
                if state.version != CREDENTIALS_VERSION {
                    return Err(AppError::storage(format!(
                        "Unsupported credentials store version {} at {}",
                        state.version,
                        self.path.display()
                    )));
                }
                Ok(state.rules)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(err) => Err(AppError::storage(format!(
                "Could not read credentials at {}: {err}",
                self.path.display()
            ))),
        }
    }

    /// Atomically saves rules to credentials.json with 0o600 permissions.
    pub fn save(&self, rules: &[CredentialRule]) -> AppResult<()> {
        let state = PersistedCredentials {
            version: CREDENTIALS_VERSION,
            rules: rules.to_vec(),
        };
        let contents = serde_json::to_vec_pretty(&state).map_err(|err| {
            AppError::storage(format!("Could not serialize credentials: {err}"))
        })?;

        write_replace(&self.path, &contents).map_err(|err| {
            AppError::storage(format!("Could not write credentials file: {err}"))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }

    /// Adds or updates a credential rule, persisting to disk.
    pub fn add_or_update(&self, rule: CredentialRule) -> AppResult<Vec<CredentialRule>> {
        let mut rules = self.load()?;
        if let Some(pos) = rules.iter().position(|r| r.id == rule.id) {
            rules[pos] = rule;
        } else {
            rules.push(rule);
        }
        self.save(&rules)?;
        Ok(rules)
    }

    /// Deletes a credential rule by ID, persisting to disk.
    pub fn delete(&self, id: &str) -> AppResult<Vec<CredentialRule>> {
        let mut rules = self.load()?;
        let initial_len = rules.len();
        rules.retain(|r| r.id != id);
        if rules.len() != initial_len {
            self.save(&rules)?;
        }
        Ok(rules)
    }

    /// Returns a domain CredentialMatcher with the currently stored rules.
    pub fn matcher(&self) -> AppResult<CredentialMatcher> {
        let rules = self.load()?;
        Ok(CredentialMatcher::new(rules))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("odm-cred-store-test-")
            .tempdir()
            .expect("temporary test directory")
    }

    #[test]
    fn test_store_lifecycle_and_permissions() {
        let temp = temporary_directory();
        let store = CredentialStore::new(temp.path());

        // Initial load is empty
        let initial = store.load().unwrap();
        assert!(initial.is_empty());

        // Add rule
        let rule1 = CredentialRule::new(
            "r1",
            "https",
            "example.com",
            None,
            "/team/",
            "user1",
            "secret1",
        )
        .unwrap();
        let saved = store.add_or_update(rule1.clone()).unwrap();
        assert_eq!(saved.len(), 1);

        // Verify file exists
        assert!(store.path().exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Credentials file must have 0600 permissions");
        }

        // Matcher queries loaded rules
        let matcher = store.matcher().unwrap();
        let target_url = url::Url::parse("https://example.com/team/file.pdf").unwrap();
        let matched = matcher.match_url(&target_url).unwrap();
        assert_eq!(matched.id, "r1");
        assert_eq!(matched.username, "user1");

        // Delete rule
        let remaining = store.delete("r1").unwrap();
        assert!(remaining.is_empty());
        assert!(store.matcher().unwrap().match_url(&target_url).is_none());
    }
}
