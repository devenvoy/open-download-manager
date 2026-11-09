// Created: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: Credential storage infrastructure module using encrypted/restricted file permissions.

pub mod store;

pub use store::CredentialStore;
