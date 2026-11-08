// Created: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Last updated: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Description: Module definitions for the local native messaging loopback bridge server.

pub mod server;

pub use server::{BridgeConfig, LoopbackBridge};
