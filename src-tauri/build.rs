// Created: 2026-11-12
// Reason: fix(build): ship the browser extension without the `_up_` resource folder
// Last updated: 2026-11-12
// Description: Copies the browser extension into the two folders the app reads it from, then runs the Tauri build script.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

const EXCLUDED_FILES: [&str; 1] = ["pack.mjs"];

fn is_excluded(name: &str) -> bool {
    name.starts_with('.') || name.ends_with(".test.cjs") || EXCLUDED_FILES.contains(&name)
}

/// Copies the extension sources, skipping test files and the packaging script.
fn sync(source: &Path, target: &Path) {
    let Ok(entries) = fs::read_dir(source) else {
        return;
    };

    let _ = fs::remove_dir_all(target);
    if fs::create_dir_all(target).is_err() {
        return;
    }

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_excluded(&name) {
            continue;
        }
        let from = entry.path();
        let to = target.join(&*name);
        let copied = if from.is_dir() {
            fs::create_dir_all(&to).is_ok() && copy_dir(&from, &to)
        } else {
            fs::copy(&from, &to).is_ok()
        };
        if !copied {
            println!("cargo:warning=Could not copy {} into {}", from.display(), to.display());
        }
    }
}
fn copy_dir(source: &Path, target: &Path) -> bool {
    let Ok(entries) = fs::read_dir(source) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let from = entry.path();
        let to = target.join(&name);
        if from.is_dir() {
            if fs::create_dir_all(&to).is_err() || !copy_dir(&from, &to) {
                return false;
            }
        } else if fs::copy(&from, &to).is_err() {
            return false;
        }
    }
    true
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let source = workspace_root.join("extension");

    // Bundled resources: `bundle.resources` ships both folders, so the files
    // have to sit next to the Tauri configuration file.
    //   extension          -> Chromium (manifest.json)
    //   extension-firefox  -> Firefox (manifest.firefox.json as manifest.json)
    sync(&source, &manifest_dir.join("extension"));
    sync_firefox(&source, &manifest_dir.join("extension-firefox"));

    // Development runtime: the app reads its resources from the folder of the
    // running executable, so the extension is copied next to the debug binary.
    if let Ok(out_dir) = env::var("OUT_DIR") {
        if let Some(profile_dir) = Path::new(&out_dir).ancestors().nth(3) {
            sync(&source, &profile_dir.join("extension"));
            sync_firefox(&source, &profile_dir.join("extension-firefox"));
        }
    }

    println!("cargo:rerun-if-changed={}", source.display());
    tauri_build::build()
}

/// Copies the extension with the Firefox manifest in place of the Chromium one.
fn sync_firefox(source: &Path, target: &Path) {
    sync(source, target);
    let firefox_manifest = source.join("manifest.firefox.json");
    if firefox_manifest.exists() {
        let _ = fs::copy(&firefox_manifest, target.join("manifest.json"));
    }
}
