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
            delivery, extension_dir_for, is_store_browser, launch_browser, register_browser_manifest,
            resolve_host_exe, scan_browsers, user_downloads_dir, user_extension_dir,
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
    resource_dir: PathBuf,
    downloads_dir: Option<PathBuf>,
    shell_service: Arc<dyn ShellService>,
    bridge_port: Arc<AtomicU16>,
}

impl Clone for IntegrationService {
    fn clone(&self) -> Self {
        Self {
            data_directory: self.data_directory.clone(),
            resource_dir: self.resource_dir.clone(),
            downloads_dir: self.downloads_dir.clone(),
            shell_service: self.shell_service.clone(),
            bridge_port: self.bridge_port.clone(),
        }
    }
}

impl IntegrationService {
    pub fn new(data_directory: PathBuf, resource_dir: PathBuf) -> Self {
        let host_exe = resolve_host_exe();
        sync_browser_manifests(&data_directory, &host_exe);
        Self {
            data_directory,
            resource_dir,
            downloads_dir: user_downloads_dir(),
            shell_service: Arc::new(SystemShellService),
            bridge_port: Arc::new(AtomicU16::new(0)),
        }
    }

    pub fn with_shell(
        data_directory: PathBuf,
        resource_dir: PathBuf,
        downloads_dir: Option<PathBuf>,
        shell_service: impl ShellService + 'static,
    ) -> Self {
        Self {
            data_directory,
            resource_dir,
            downloads_dir,
            shell_service: Arc::new(shell_service),
            bridge_port: Arc::new(AtomicU16::new(0)),
        }
    }

    /// The folder the user loads as an unpacked extension: a visible folder in
    /// the Downloads directory. A browser cannot pick a folder inside an
    /// application bundle, so the app delivers the extension there.
    pub fn resolve_extension_path(&self, browser: BrowserType) -> PathBuf {
        match &self.downloads_dir {
            Some(downloads) => user_extension_dir(downloads, browser),
            None => extension_dir_for(&self.resource_dir, browser),
        }
    }

    /// The bundled extension that is copied into the Downloads folder.
    fn bundled_extension_dir(&self, browser: BrowserType) -> PathBuf {
        extension_dir_for(&self.resource_dir, browser)
    }

    /// Copies the bundled extension into the Downloads folder and reveals it.
    pub fn deliver_extension(&self, browser: BrowserType) -> AppResult<PathBuf> {
        let target = self.resolve_extension_path(browser);
        let bundled = self.bundled_extension_dir(browser);
        delivery::deliver_extension(&bundled, &target).map_err(|error| {
            AppError::browser(format!(
                "Could not save the extension to {}: {error}",
                target.to_string_lossy()
            ))
        })?;
        Ok(target)
    }

    pub fn sync_manifests(&self) -> Vec<BrowserType> {
        let host_exe = resolve_host_exe();
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
            &self.resource_dir,
            self.downloads_dir.as_deref(),
            port,
        ))
    }

    pub async fn register_browser(&self, browser_name: &str, extension_id: &str) -> AppResult<()> {
        let browser = BrowserType::parse(browser_name)
            .ok_or_else(|| AppError::browser("Choose Chrome, Edge, Brave, Opera, or Firefox."))?;

        let host_exe = resolve_host_exe();
        let _ = register_browser_manifest(browser, extension_id, &self.data_directory, &host_exe)
            .await?;

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
                let host_exe = resolve_host_exe();
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
                    let host_exe = resolve_host_exe();
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
                    let host_exe = resolve_host_exe();
                    let _ = register_browser_manifest(
                        browser,
                        FIREFOX_EXTENSION_ID,
                        &self.data_directory,
                        &host_exe,
                    )
                    .await?;
                }
                Ok(BrowserSetupResult {
                    browser: Some(browser_name.to_owned()),
                    extension_path: Some(extension_path_str),
                })
            }
            "download" => {
                // Save the extension in the Downloads folder so the user can
                // pick it in the browser, and open the folder right away.
                let delivered = self.deliver_extension(browser)?;
                let _ = self.shell_service.reveal_path(&delivered);
                Ok(BrowserSetupResult {
                    browser: Some(browser_name.to_owned()),
                    extension_path: Some(delivered.to_string_lossy().into_owned()),
                })
            }
            "copy" => Ok(BrowserSetupResult {
                browser: Some(browser_name.to_owned()),
                extension_path: Some(extension_path_str),
            }),
            "folder" => {
                // Save the extension first so the folder the user is sent to
                // always exists.
                if !extension_path.join("manifest.json").exists() {
                    let _ = self.deliver_extension(browser);
                }
                // Reveal the extension folder in the file manager so the user
                // can pick it in the browser without hunting for the path.
                self.shell_service.reveal_path(&extension_path)?;
                Ok(BrowserSetupResult {
                    browser: Some(browser_name.to_owned()),
                    extension_path: Some(extension_path_str),
                })
            }
            _ => Err(AppError::browser("Unknown browser setup action.")),
        }
    }
}
