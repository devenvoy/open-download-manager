// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: integrate standard crates (mime_guess, sanitize-filename, tempfile, which) and FileType vector icon catalog
// Description: Atomic file write utilities using temporary staging files and atomic renames to prevent corruption.

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
