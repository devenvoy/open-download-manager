// Created: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: Module definitions and exports for site credential rules and secure storage domain.

pub mod matcher;
pub mod rule;

pub use matcher::{format_basic_auth_header, path_prefix_matches, CredentialMatcher};
pub use rule::{default_port_for_scheme, CredentialRule};
