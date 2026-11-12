// Created: 2026-11-05
// Reason: feat: implement Phase B desktop shell, system tray, and autostart
// Last updated: 2026-11-11
// Reason: feat(tray): clean tray icon with active download badge; macOS menu bar auto-tints based on system appearance
// Description: System tray icon controller managing tray menu, minimization to tray, and background status.
// On macOS the menu bar tray icon is auto-tinted by the system based on light/dark appearance,
// so no runtime theme switching is needed. The icon.png bundled by Tauri is used directly.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
    RwLock,
    OnceLock,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State,
};

pub const TRAY_TOOLTIP: &str = "Open Download Manager";

static TRAY_HANDLE: OnceLock<TrayIcon<tauri::Wry>> = OnceLock::new();

pub struct TrayState {
    pub active_downloads: RwLock<u32>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            active_downloads: RwLock::new(0),
        }
    }
}

pub fn show_main_window<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn update_tray_tooltip(app: &AppHandle<tauri::Wry>) {
    let tray = match TRAY_HANDLE.get() {
        Some(t) => t,
        None => return,
    };
    let state = match app.try_state::<TrayState>() {
        Some(s) => s,
        None => return,
    };
    let count = *state.active_downloads.read().unwrap();
    let _ = tray.set_tooltip(if count > 0 {
        Some(format!("{} — {} active download{}", TRAY_TOOLTIP, count, if count == 1 { "" } else { "s" }))
    } else {
        Some(TRAY_TOOLTIP.to_string())
    });
}

pub fn setup_system_tray(
    app: &AppHandle<tauri::Wry>,
    is_quitting: Arc<AtomicBool>,
) -> Result<(), tauri::Error> {
    let show_item = MenuItem::with_id(app, "show", "Show downloads", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let icon_bytes = include_bytes!("../../icons/tray-icon.png");
    let icon = tauri::image::Image::from_bytes(icon_bytes)?;

    let app_handle = app.clone();
    let is_quitting_menu = is_quitting;
    let tray = TrayIconBuilder::new()
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
            } | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    let _ = TRAY_HANDLE.set(tray);

    app.manage(TrayState::default());

    // Initialize tooltip
    update_tray_tooltip(&app_handle);

    Ok(())
}

/// Sets the number of active downloads and updates the tray tooltip.
#[tauri::command]
pub fn tray_set_active_downloads(
    app: AppHandle<tauri::Wry>,
    state: State<'_, TrayState>,
    count: u32,
) -> Result<(), String> {
    *state.active_downloads.write().unwrap() = count;
    update_tray_tooltip(&app);
    Ok(())
}
