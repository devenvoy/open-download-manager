// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-08
// Reason: feat(integration): decouple from upstream Web Store, purge legacy IDs, and add macOS native messaging support
// Reason: fix(browser): auto-sync native messaging manifests on refresh and initialization
// Description: Application service detecting installed browsers and registering native messaging manifests.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
};

use serde::{Deserialize, Serialize};

use crate::{
    application::{
        error::{AppError, AppResult},
        shell_service::ShellService,
    },
    domain::snapshot::IntegrationStatus,
    infrastructure::{
        browser::{
            is_store_browser, launch_browser, prepare_chrome_extension, prepare_firefox_extension,
            register_browser_manifest, resolve_host_exe, resolve_integration_root, scan_browsers,
            sync_browser_manifests, BrowserType, EXTENSION_ID, FIREFOX_EXTENSION_ID, STORE_URL,
        },
        system_shell::SystemShellService,
    },
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSetupResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_path: Option<String>,
}

pub struct IntegrationService {
    data_directory: PathBuf,
    integration_root: PathBuf,
    shell_service: Arc<dyn ShellService>,
    bridge_port: Arc<AtomicU16>,
}

impl Clone for IntegrationService {
    fn clone(&self) -> Self {
        Self {
            data_directory: self.data_directory.clone(),
            integration_root: self.integration_root.clone(),
            shell_service: self.shell_service.clone(),
            bridge_port: self.bridge_port.clone(),
        }
    }
}

impl IntegrationService {
    pub fn new(data_directory: PathBuf) -> Self {
        let integration_root = resolve_integration_root();
        let host_exe = resolve_host_exe(&integration_root);
        sync_browser_manifests(&data_directory, &host_exe);
        let _ = prepare_chrome_extension(&data_directory, &integration_root);
        Self {
            data_directory,
            integration_root,
            shell_service: Arc::new(SystemShellService),
            bridge_port: Arc::new(AtomicU16::new(0)),
        }
    }

    pub fn with_shell(
        data_directory: PathBuf,
        integration_root: PathBuf,
        shell_service: impl ShellService + 'static,
    ) -> Self {
        let _ = prepare_chrome_extension(&data_directory, &integration_root);
        Self {
            data_directory,
            integration_root,
            shell_service: Arc::new(shell_service),
            bridge_port: Arc::new(AtomicU16::new(0)),
        }
    }

    pub fn resolve_extension_path(&self, browser: BrowserType) -> PathBuf {
        if browser.is_firefox() {
            self.data_directory
                .join("NativeMessaging")
                .join("firefox-extension")
        } else {
            let direct = self.integration_root.join("extension");
            let data_chrome = self
                .data_directory
                .join("NativeMessaging")
                .join("chrome-extension");
            if direct.join("manifest.json").exists() {
                direct
            } else if data_chrome.join("manifest.json").exists() {
                data_chrome
            } else {
                let _ = prepare_chrome_extension(&self.data_directory, &self.integration_root);
                if data_chrome.exists() {
                    data_chrome
                } else {
                    direct
                }
            }
        }
    }

    pub fn sync_manifests(&self) -> Vec<BrowserType> {
        let host_exe = resolve_host_exe(&self.integration_root);
        sync_browser_manifests(&self.data_directory, &host_exe)
    }

    pub fn set_bridge_port(&self, port: u16) {
        self.bridge_port.store(port, Ordering::SeqCst);
    }

    pub fn bridge_port(&self) -> Option<u16> {
        let port = self.bridge_port.load(Ordering::SeqCst);
        if port != 0 {
            return Some(port);
        }
        let bridge_path = self.data_directory.join("bridge.json");
        if let Ok(content) = std::fs::read_to_string(bridge_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(p) = json.get("port").and_then(|p| p.as_u64()) {
                    let port_u16 = p as u16;
                    self.bridge_port.store(port_u16, Ordering::SeqCst);
                    return Some(port_u16);
                }
            }
        }
        None
    }

    pub fn get_integration_info(&self) -> AppResult<IntegrationStatus> {
        self.sync_manifests();
        let port = self.bridge_port();
        Ok(scan_browsers(
            &self.data_directory,
            &self.integration_root,
            port,
        ))
    }

    pub async fn register_browser(&self, browser_name: &str, extension_id: &str) -> AppResult<()> {
        let browser = BrowserType::parse(browser_name)
            .ok_or_else(|| AppError::browser("Choose Chrome, Edge, Brave, Opera, or Firefox."))?;

        let host_exe = resolve_host_exe(&self.integration_root);
        let _ = register_browser_manifest(browser, extension_id, &self.data_directory, &host_exe)
            .await?;

        if browser.is_firefox() {
            let _ = prepare_firefox_extension(&self.data_directory, &self.integration_root);
        }

        Ok(())
    }

    pub async fn browser_setup(&self, action: &str, browser_name: &str) -> AppResult<BrowserSetupResult> {
        let browser = BrowserType::parse(browser_name)
            .ok_or_else(|| AppError::browser("Choose Chrome, Edge, Brave, Opera, or Firefox."))?;

        let extension_path = self.resolve_extension_path(browser);
        let extension_path_str = extension_path.to_string_lossy().into_owned();

        match action {
            "store" => {
                let Some(store_url) = STORE_URL else {
                    return Err(AppError::browser("Store setup is unavailable for this browser."));
                };
                if !is_store_browser(browser) {
                    return Err(AppError::browser("Store setup is unavailable for this browser."));
                }
                let host_exe = resolve_host_exe(&self.integration_root);
                if let Some(ext_id) = EXTENSION_ID {
                    let _ = register_browser_manifest(
                        browser,
                        ext_id,
                        &self.data_directory,
                        &host_exe,
                    )
                    .await?;
                }
                launch_browser(browser, Some(store_url))?;
                Ok(BrowserSetupResult {
                    browser: Some(browser_name.to_owned()),
                    extension_path: None,
                })
            }
            "open" => {
                if let Some(ext_id) = EXTENSION_ID {
                    let host_exe = resolve_host_exe(&self.integration_root);
                    let _ = register_browser_manifest(
                        browser,
                        ext_id,
                        &self.data_directory,
                        &host_exe,
                    )
                    .await;
                }
                launch_browser(browser, None)?;
                Ok(BrowserSetupResult {
                    browser: Some(browser_name.to_owned()),
                    extension_path: Some(extension_path_str),
                })
            }
            "prepare" => {
                if browser.is_firefox() {
                    let host_exe = resolve_host_exe(&self.integration_root);
                    let _ = register_browser_manifest(
                        browser,
                        FIREFOX_EXTENSION_ID,
                        &self.data_directory,
                        &host_exe,
                    )
                    .await?;
                    let path = prepare_firefox_extension(
                        &self.data_directory,
                        &self.integration_root,
                    )
                    .map_err(|e| AppError::browser(format!("Failed to prepare Firefox extension: {e}")))?;
                    Ok(BrowserSetupResult {
                        browser: Some(browser_name.to_owned()),
                        extension_path: Some(path.to_string_lossy().into_owned()),
                    })
                } else {
                    let path = prepare_chrome_extension(
                        &self.data_directory,
                        &self.integration_root,
                    )
                    .map_err(|e| AppError::browser(format!("Failed to prepare Chrome extension: {e}")))?;
                    Ok(BrowserSetupResult {
                        browser: Some(browser_name.to_owned()),
                        extension_path: Some(path.to_string_lossy().into_owned()),
                    })
                }
            }
            "copy" => Ok(BrowserSetupResult {
                browser: Some(browser_name.to_owned()),
                extension_path: Some(extension_path_str),
            }),
            "folder" => {
                if browser.is_firefox() && !extension_path.exists() {
                    let host_exe = resolve_host_exe(&self.integration_root);
                    let _ = register_browser_manifest(
                        browser,
                        FIREFOX_EXTENSION_ID,
                        &self.data_directory,
                        &host_exe,
                    )
                    .await?;
                    let _ = prepare_firefox_extension(
                        &self.data_directory,
                        &self.integration_root,
                    );
                } else if !browser.is_firefox() && !extension_path.exists() {
                    let _ = prepare_chrome_extension(
                        &self.data_directory,
                        &self.integration_root,
                    );
                }
                if !extension_path.exists() {
                    let _ = std::fs::create_dir_all(&extension_path);
                }
                self.shell_service.open_path(&extension_path)?;
                Ok(BrowserSetupResult {
                    browser: Some(browser_name.to_owned()),
                    extension_path: Some(extension_path_str),
                })
            }
            _ => Err(AppError::browser("Unknown browser setup action.")),
        }
    }
}
