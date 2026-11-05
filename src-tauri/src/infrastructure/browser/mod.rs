pub mod registration;

pub use registration::{
    create_manifest, register_browser_manifest, unregister_browser_manifest, BrowserType,
    NativeHostManifest, NATIVE_HOST_NAME,
};
