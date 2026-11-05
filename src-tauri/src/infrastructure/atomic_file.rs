use std::{
    fs,
    io::{self, Write},
    path::Path,
};
use tempfile::NamedTempFile;

pub fn write_replace(destination: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = destination
        .parent()
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(destination).map_err(|err| err.error)?;
    Ok(())
}
