#[cfg(windows)]
use crate::application::error::AppError;
use crate::application::error::AppResult;

pub const AUTOSTART_KEY_NAME: &str = "Awesome Downloader";

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
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}
