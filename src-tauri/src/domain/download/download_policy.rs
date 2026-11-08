pub(super) const MAX_DOWNLOAD_URLS: usize = 500;
pub(super) const MAX_URL_LENGTH_BYTES: usize = 8_192;
pub(super) const MAX_DOWNLOAD_FILENAME_CHARACTERS: usize = 180;
pub(super) const MAX_DESCRIPTION_CHARACTERS: usize = 1_000;
pub(super) const MAX_DOWNLOAD_ID_CHARACTERS: usize = 128;
pub(super) const MAX_DOWNLOAD_CONNECTIONS: u8 = 32;
pub(super) const HTTP_PROTOCOL: &str = "http";
pub(super) const HTTPS_PROTOCOL: &str = "https";
pub(super) const FTP_PROTOCOL: &str = "ftp";
pub(super) const DASH_MANIFEST_EXTENSION: &str = ".mpd";
pub(super) const WINDOWS_RESERVED_BASENAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];
pub(super) const DEFAULT_DOWNLOAD_FILENAME: &str = "download";
pub(super) const INVALID_URL_DISPLAY_TEXT: &str = "Invalid URL";
