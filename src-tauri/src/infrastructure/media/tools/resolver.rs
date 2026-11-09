// Created: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Last updated: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Description: Path resolver locating installed system or sandboxed media tool binaries.

use std::{path::PathBuf, sync::Arc};

use crate::domain::media_tools::{MediaToolStatus, MediaToolType, ToolResolutionSource};

use super::{adapter::MediaToolAdapter, adapters::current_adapter};

/// Path resolver that locates media tools across the four resolution tiers.
///
/// The platform adapter supplies executable names, bundled sidecar locations,
/// and any supplemental PATH directories, so resolution behavior is testable
/// for every supported OS from any host.
#[derive(Clone)]
pub struct ToolPathResolver {
    adapter: Arc<dyn MediaToolAdapter>,
    managed_dir_override: Option<PathBuf>,
}

impl ToolPathResolver {
    pub fn new() -> Self {
        Self::with_adapter(current_adapter())
    }

    pub fn with_adapter(adapter: Arc<dyn MediaToolAdapter>) -> Self {
        Self {
            adapter,
            managed_dir_override: None,
        }
    }

    pub fn with_managed_dir(managed_dir: PathBuf) -> Self {
        Self::with_adapter_and_managed_dir(current_adapter(), managed_dir)
    }

    pub fn with_adapter_and_managed_dir(
        adapter: Arc<dyn MediaToolAdapter>,
        managed_dir: PathBuf,
    ) -> Self {
        Self {
            adapter,
            managed_dir_override: Some(managed_dir),
        }
    }

    pub fn adapter(&self) -> &dyn MediaToolAdapter {
        self.adapter.as_ref()
    }

    /// Resolves the managed user data tools directory (`~/.open-download-manager/tools/`).
    pub fn managed_tools_dir(&self) -> Option<PathBuf> {
        if let Some(ref custom) = self.managed_dir_override {
            return Some(custom.clone());
        }

        let home = std::env::var("HOME")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())
            .map(PathBuf::from)?;

        Some(home.join(".open-download-manager").join("tools"))
    }

    /// Resolves a tool's executable location, identifying its resolution tier.
    pub fn resolve_tool(&self, tool: MediaToolType) -> Option<(PathBuf, ToolResolutionSource)> {
        // Tier 1: Environment variable override
        let env_key = tool.env_var_override();
        if let Ok(env_val) = std::env::var(env_key) {
            let path = PathBuf::from(env_val);
            if path.is_file() {
                return Some((path, ToolResolutionSource::EnvironmentOverride));
            }
        }

        let exe_name = self.adapter.executable_name(tool);

        // Tier 2: Bundled application sidecars
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(app_dir) = current_exe.parent() {
                for candidate in self.adapter.bundled_tool_candidates(exe_name, app_dir) {
                    if candidate.is_file() {
                        return Some((candidate, ToolResolutionSource::BundledSidecar));
                    }
                }
            }
        }

        // Tier 3: In-App Managed user directory
        if let Some(tools_dir) = self.managed_tools_dir() {
            let candidate = tools_dir.join(exe_name);
            if candidate.is_file() {
                return Some((candidate, ToolResolutionSource::ManagedUserData));
            }
        }

        // Tier 4: System PATH
        if let Ok(system_path) = which::which(tool.name()) {
            return Some((system_path, ToolResolutionSource::SystemPath));
        }

        // Tier 4b: Platform-specific PATH supplements (for example, macOS
        // path_helper when launched with a minimal launchd environment).
        for dir in self.adapter.supplemental_path_dirs() {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Some((candidate, ToolResolutionSource::SystemPath));
            }
        }

        None
    }

    /// Evaluates current tool status.
    pub fn tool_status(&self, tool: MediaToolType) -> MediaToolStatus {
        match self.resolve_tool(tool) {
            Some((path, source)) => MediaToolStatus {
                tool_type: tool,
                is_available: true,
                path: Some(path),
                source: Some(source),
            },
            None => MediaToolStatus {
                tool_type: tool,
                is_available: false,
                path: None,
                source: None,
            },
        }
    }
}

impl Default for ToolPathResolver {
    fn default() -> Self {
        Self::new()
    }
}
