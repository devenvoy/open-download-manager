// Created: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Last updated: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Description: Concrete clipboard adapters using Tauri plugin clipboard manager with worker isolation.

use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::application::clipboard_service::ClipboardReader;

pub struct TauriClipboardReader {
    app: AppHandle,
}

impl TauriClipboardReader {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ClipboardReader for TauriClipboardReader {
    fn read_text(&self) -> Result<Option<String>, String> {
        let app = self.app.clone();
        // Execute on a dedicated worker thread to avoid deadlocking with Linux X11/Wayland event loops
        let join_handle = std::thread::spawn(move || {
            app.clipboard().read_text()
        });

        match join_handle.join() {
            Ok(Ok(text)) => Ok(Some(text)),
            Ok(Err(_)) => Ok(None),
            Err(_) => Err("Clipboard thread failed".to_string()),
        }
    }
}

pub struct MockClipboardReader {
    content: Arc<Mutex<Option<String>>>,
}

impl MockClipboardReader {
    pub fn new() -> Self {
        Self {
            content: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_content(text: &str) -> Self {
        Self {
            content: Arc::new(Mutex::new(Some(text.to_string()))),
        }
    }

    pub fn set_text(&self, text: Option<String>) {
        if let Ok(mut guard) = self.content.lock() {
            *guard = text;
        }
    }
}

impl Default for MockClipboardReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardReader for MockClipboardReader {
    fn read_text(&self) -> Result<Option<String>, String> {
        Ok(self.content.lock().ok().and_then(|g| g.clone()))
    }
}
