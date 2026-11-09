// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: Application desktop entry point launching the Open Download Manager Tauri runtime.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    open_download_manager_lib::run();
}
