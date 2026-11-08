use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::domain::completion::PowerActionKind;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait ActionExecutor: Send + Sync {
    fn execute_process<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        timeout: Duration,
    ) -> BoxFuture<'a, Result<String, String>>;

    fn play_sound<'a>(&'a self, sound_file: Option<&'a str>) -> BoxFuture<'a, Result<(), String>>;

    fn power_action<'a>(&'a self, kind: PowerActionKind) -> BoxFuture<'a, Result<(), String>>;

    fn open_file(&self, path: &Path) -> Result<(), String>;

    fn open_folder(&self, path: &Path) -> Result<(), String>;
}

#[derive(Clone, Default)]
pub struct SystemActionExecutor;

impl SystemActionExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl ActionExecutor for SystemActionExecutor {
    fn execute_process<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        timeout: Duration,
    ) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let mut cmd = tokio::process::Command::new(executable);
            cmd.args(args);
            cmd.kill_on_drop(true);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let child = match cmd.spawn() {
                Ok(child) => child,
                Err(error) => {
                    return if error.kind() == std::io::ErrorKind::NotFound {
                        Err(format!("Executable not found: {executable}"))
                    } else {
                        Err(format!("Failed to start executable '{executable}': {error}"))
                    };
                }
            };

            let output_future = child.wait_with_output();
            match tokio::time::timeout(timeout, output_future).await {
                Ok(Ok(output)) => {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        Ok(stdout.trim().to_string())
                    } else {
                        let code = output
                            .status
                            .code()
                            .map_or_else(|| "terminated by signal".to_string(), |c| c.to_string());
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let err_msg = if stderr.trim().is_empty() {
                            format!("Process exited with status code {code}")
                        } else {
                            format!("Process exited with status code {code}: {}", stderr.trim())
                        };
                        Err(err_msg)
                    }
                }
                Ok(Err(error)) => Err(format!("Process execution error: {error}")),
                Err(_) => Err(format!(
                    "Process timed out after {} seconds",
                    timeout.as_secs()
                )),
            }
        })
    }

    fn play_sound<'a>(&'a self, sound_file: Option<&'a str>) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let Some(sound_path) = sound_file {
                let path = Path::new(sound_path);
                if !path.is_file() {
                    return Err(format!("Sound file not found: {sound_path}"));
                }
                #[cfg(target_os = "macos")]
                {
                    let mut cmd = tokio::process::Command::new("afplay");
                    cmd.arg(sound_path);
                    let _ = cmd.status().await;
                }
                #[cfg(target_os = "linux")]
                {
                    let mut cmd = tokio::process::Command::new("paplay");
                    cmd.arg(sound_path);
                    let _ = cmd.status().await;
                }
                #[cfg(target_os = "windows")]
                {
                    let mut cmd = tokio::process::Command::new("powershell");
                    let escaped = sound_path.replace('\'', "''");
                    cmd.args([
                        "-NoProfile",
                        "-Command",
                        &format!("(New-Object Media.SoundPlayer '{escaped}').PlaySync()"),
                    ]);
                    let _ = cmd.status().await;
                }
            } else {
                #[cfg(target_os = "macos")]
                {
                    let mut cmd = tokio::process::Command::new("afplay");
                    cmd.arg("/System/Library/Sounds/Glass.aiff");
                    let _ = cmd.status().await;
                }
                #[cfg(not(target_os = "macos"))]
                {
                    print!("\x07");
                }
            }
            Ok(())
        })
    }

    fn power_action<'a>(&'a self, kind: PowerActionKind) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            #[cfg(test)]
            {
                let _ = kind;
                Ok(())
            }
            #[cfg(not(test))]
            {
                match kind {
                    PowerActionKind::Shutdown => {
                        #[cfg(target_os = "windows")]
                        {
                            let mut cmd = tokio::process::Command::new("shutdown");
                            cmd.args(["/s", "/t", "30", "/c", "Open Download Manager finished downloads"]);
                            cmd.status().await.map_err(|e| format!("Shutdown command failed: {e}"))?;
                        }
                        #[cfg(target_os = "macos")]
                        {
                            let mut cmd = tokio::process::Command::new("osascript");
                            cmd.args(["-e", "tell app \"System Events\" to shut down"]);
                            cmd.status().await.map_err(|e| format!("Shutdown command failed: {e}"))?;
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let mut cmd = tokio::process::Command::new("systemctl");
                            cmd.arg("poweroff");
                            cmd.status().await.map_err(|e| format!("Shutdown command failed: {e}"))?;
                        }
                    }
                    PowerActionKind::Sleep => {
                        #[cfg(target_os = "windows")]
                        {
                            let mut cmd = tokio::process::Command::new("rundll32.exe");
                            cmd.args(["powrprof.dll,SetSuspendState", "0,1,0"]);
                            cmd.status().await.map_err(|e| format!("Sleep command failed: {e}"))?;
                        }
                        #[cfg(target_os = "macos")]
                        {
                            let mut cmd = tokio::process::Command::new("pmset");
                            cmd.arg("sleepnow");
                            cmd.status().await.map_err(|e| format!("Sleep command failed: {e}"))?;
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let mut cmd = tokio::process::Command::new("systemctl");
                            cmd.arg("suspend");
                            cmd.status().await.map_err(|e| format!("Sleep command failed: {e}"))?;
                        }
                    }
                    PowerActionKind::Hibernate => {
                        #[cfg(target_os = "windows")]
                        {
                            let mut cmd = tokio::process::Command::new("shutdown");
                            cmd.args(["/h"]);
                            cmd.status().await.map_err(|e| format!("Hibernate command failed: {e}"))?;
                        }
                        #[cfg(target_os = "macos")]
                        {
                            return Err("Hibernate is not natively supported on macOS.".to_string());
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let mut cmd = tokio::process::Command::new("systemctl");
                            cmd.arg("hibernate");
                            cmd.status().await.map_err(|e| format!("Hibernate command failed: {e}"))?;
                        }
                    }
                }
                Ok(())
            }
        })
    }

    fn open_file(&self, path: &Path) -> Result<(), String> {
        if !path.is_file() {
            return Err("Target file does not exist.".to_string());
        }
        opener::open(path).map_err(|e| format!("Failed to open file: {e}"))
    }

    fn open_folder(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err("Target folder does not exist.".to_string());
        }
        opener::reveal(path).map_err(|e| format!("Failed to reveal file: {e}"))
    }
}

#[derive(Clone, Default)]
pub struct MockActionExecutor {
    pub executed_commands: Arc<Mutex<Vec<(String, Vec<String>)>>>,
    pub played_sounds: Arc<Mutex<Vec<Option<String>>>>,
    pub power_actions: Arc<Mutex<Vec<PowerActionKind>>>,
    pub opened_files: Arc<Mutex<Vec<PathBuf>>>,
    pub opened_folders: Arc<Mutex<Vec<PathBuf>>>,
    pub command_error: Arc<Mutex<Option<String>>>,
    pub power_error: Arc<Mutex<Option<String>>>,
}

impl MockActionExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_command_error(error: impl Into<String>) -> Self {
        let instance = Self::default();
        if let Ok(mut lock) = instance.command_error.lock() {
            *lock = Some(error.into());
        }
        instance
    }
}

impl ActionExecutor for MockActionExecutor {
    fn execute_process<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<String, String>> {
        let exec = executable.to_string();
        let arguments = args.to_vec();
        Box::pin(async move {
            if let Ok(mut commands) = self.executed_commands.lock() {
                commands.push((exec, arguments));
            }
            if let Ok(err) = self.command_error.lock() {
                if let Some(ref e) = *err {
                    return Err(e.clone());
                }
            }
            Ok("Mock process output".to_string())
        })
    }

    fn play_sound<'a>(&'a self, sound_file: Option<&'a str>) -> BoxFuture<'a, Result<(), String>> {
        let sound = sound_file.map(|s| s.to_string());
        Box::pin(async move {
            if let Ok(mut sounds) = self.played_sounds.lock() {
                sounds.push(sound);
            }
            Ok(())
        })
    }

    fn power_action<'a>(&'a self, kind: PowerActionKind) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let Ok(mut actions) = self.power_actions.lock() {
                actions.push(kind);
            }
            if let Ok(err) = self.power_error.lock() {
                if let Some(ref e) = *err {
                    return Err(e.clone());
                }
            }
            Ok(())
        })
    }

    fn open_file(&self, path: &Path) -> Result<(), String> {
        if let Ok(mut files) = self.opened_files.lock() {
            files.push(path.to_path_buf());
        }
        Ok(())
    }

    fn open_folder(&self, path: &Path) -> Result<(), String> {
        if let Ok(mut folders) = self.opened_folders.lock() {
            folders.push(path.to_path_buf());
        }
        Ok(())
    }
}
