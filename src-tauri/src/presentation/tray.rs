// Created: 2026-11-05
// Reason: feat: implement Phase B desktop shell, system tray, and autostart
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: System tray icon controller managing tray menu, minimization to tray, and background status.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub const TRAY_TOOLTIP: &str = "Open Download Manager";

pub fn show_main_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn setup_system_tray<R: tauri::Runtime>(
    app: &AppHandle<R>,
    is_quitting: Arc<AtomicBool>,
) -> Result<TrayIcon<R>, tauri::Error> {
    let show_item = MenuItem::with_id(app, "show", "Show downloads", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let icon_bytes = include_bytes!("../../icons/icon.png");
    let icon = tauri::image::Image::from_bytes(icon_bytes)?;

    let is_quitting_menu = is_quitting;
    TrayIconBuilder::new()
        .icon(icon)
        .tooltip(TRAY_TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quit" => {
                is_quitting_menu.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)
}
