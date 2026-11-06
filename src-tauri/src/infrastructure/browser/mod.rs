pub mod registration;
pub mod scanner;

pub use registration::{
    browser_native_messaging_dir, create_manifest, register_browser_manifest,
    unregister_browser_manifest, BrowserType, NativeHostManifest, NATIVE_HOST_NAME,
};
pub use scanner::{
    find_browser_executable, is_store_browser, launch_browser, prepare_firefox_extension,
    resolve_host_exe, resolve_integration_root, scan_browsers, EXTENSION_ID, FIREFOX_EXTENSION_ID,
    STORE_URL,
};
