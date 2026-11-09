// Created: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Last updated: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Description: Infrastructure module definitions for executing post-download system commands and actions.

pub mod executor;

pub use executor::{ActionExecutor, BoxFuture, MockActionExecutor, SystemActionExecutor};
