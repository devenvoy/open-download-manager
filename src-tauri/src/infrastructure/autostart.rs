use std::path::{Path, PathBuf};
use crate::application::error::{AppError, AppResult};

pub const AUTOSTART_KEY_NAME: &str = "Open Download Manager";

pub fn sync_autostart(enabled: bool) -> AppResult<()> {
    #[cfg(windows)]
    {
        let current_exe = std::env::current_exe()
            .map_err(|e| AppError::storage(format!("Could not get current exe path: {e}")))?;
        let exe_str = current_exe.to_string_lossy();
        let run_cmd = format!("\"{exe_str}\" --background");

        const REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

        if enabled {
            let mut command = std::process::Command::new("reg.exe");
            command
                .arg("ADD")
                .arg(REG_KEY)
                .arg("/v")
                .arg(AUTOSTART_KEY_NAME)
                .arg("/t")
                .arg("REG_SZ")
                .arg("/d")
                .arg(&run_cmd)
                .arg("/f");

            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);

            let status = command
                .status()
                .map_err(|e| AppError::storage(format!("Could not run reg.exe: {e}")))?;
            if !status.success() {
                return Err(AppError::storage(
                    "Could not register autostart in Windows registry.",
                ));
            }
        } else {
            let mut command = std::process::Command::new("reg.exe");
            command
                .arg("DELETE")
                .arg(REG_KEY)
                .arg("/v")
                .arg(AUTOSTART_KEY_NAME)
                .arg("/f");

            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);

            let _ = command.status();
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            sync_autostart_macos(enabled, &home)?;
        }
        Ok(())
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            sync_autostart_linux(enabled, &home)?;
        }
        Ok(())
    }
}

pub fn sync_autostart_macos(enabled: bool, home: &Path) -> AppResult<()> {
    let launch_agents = home.join("Library").join("LaunchAgents");
    let plist_path = launch_agents.join("com.open.downloadmanager.plist");

    if enabled {
        let current_exe = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("open-download-manager"));
        let exe_str = current_exe.to_string_lossy();

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.open.downloadmanager</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--background</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
            exe_str
        );

        let _ = std::fs::create_dir_all(&launch_agents);
        std::fs::write(&plist_path, plist_content)
            .map_err(|e| AppError::storage(format!("Could not write macOS LaunchAgent: {e}")))?;
    } else if plist_path.exists() {
        let _ = std::fs::remove_file(&plist_path);
    }

    Ok(())
}

pub fn sync_autostart_linux(enabled: bool, home: &Path) -> AppResult<()> {
    let autostart_dir = home.join(".config").join("autostart");
    let desktop_path = autostart_dir.join("open-download-manager.desktop");

    if enabled {
        let current_exe = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("open-download-manager"));
        let exe_str = current_exe.to_string_lossy();

        let desktop_content = format!(
            r#"[Desktop Entry]
Type=Application
Name=Open Download Manager
Comment=Open Download Manager Autostart
Exec="{}" --background
Terminal=false
Categories=Network;FileTransfer;
StartupNotify=false
"#,
            exe_str
        );

        let _ = std::fs::create_dir_all(&autostart_dir);
        std::fs::write(&desktop_path, desktop_content)
            .map_err(|e| AppError::storage(format!("Could not write Linux autostart desktop entry: {e}")))?;
    } else if desktop_path.exists() {
        let _ = std::fs::remove_file(&desktop_path);
    }

    Ok(())
}
