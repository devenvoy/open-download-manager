use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

mod policy;

use self::policy::{TEMPORARY_FILE_EXTENSION, TEMPORARY_FILE_NAME_FALLBACK};

pub fn write_replace(destination: &Path, contents: &[u8]) -> io::Result<()> {
    let temporary = temporary_path(destination);
    let result =
        write_and_sync(&temporary, contents).and_then(|()| fs::rename(&temporary, destination));
    if result.is_err() {
        if let Err(error) = fs::remove_file(&temporary) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("Could not remove a temporary settings/history file: {error}");
            }
        }
    }
    result
}

fn temporary_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(TEMPORARY_FILE_NAME_FALLBACK);
    destination.with_file_name(format!(
        "{file_name}.{}.{}",
        std::process::id(),
        TEMPORARY_FILE_EXTENSION
    ))
}

fn write_and_sync(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}
