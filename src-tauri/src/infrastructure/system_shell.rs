use std::path::Path;

use crate::application::{
    error::{AppError, AppResult},
    shell_service::ShellService,
};

#[derive(Clone, Default)]
pub struct SystemShellService;

impl SystemShellService {
    pub fn new() -> Self {
        Self
    }
}

impl ShellService for SystemShellService {
    fn open_path(&self, path: &Path) -> AppResult<()> {
        opener::open(path).map_err(|error| {
            AppError::download(format!("Could not open file: {error}"))
        })
    }

    fn reveal_path(&self, path: &Path) -> AppResult<()> {
        opener::reveal(path).map_err(|error| {
            AppError::download(format!("Could not reveal file: {error}"))
        })
    }

    fn open_with(&self, path: &Path) -> AppResult<()> {
        #[cfg(windows)]
        {
            let system_root =
                std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
            let open_with_path = std::path::PathBuf::from(system_root)
                .join("System32")
                .join("OpenWith.exe");
            let mut command = std::process::Command::new(open_with_path);
            command.arg(path);
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
            command.spawn().map_err(|error| {
                AppError::download(format!("Could not launch Open With: {error}"))
            })?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            self.open_path(path)
        }
    }
}
