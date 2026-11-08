pub mod matcher;
pub mod rule;

pub use matcher::{format_basic_auth_header, path_prefix_matches, CredentialMatcher};
pub use rule::{default_port_for_scheme, CredentialRule};
