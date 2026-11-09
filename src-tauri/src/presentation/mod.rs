// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-05
// Reason: feat(updater): implement auto-updater, periodic background checks, and active transfer safety guard (Phase F)
// Description: Presentation layer module exposing Tauri commands, window manager, and system tray integration.

pub mod app_state;
pub mod commands;
pub mod download_event_publisher;
mod event_policy;
pub mod menu;
pub mod tray;
pub mod updater_controller;
pub mod window_manager;
