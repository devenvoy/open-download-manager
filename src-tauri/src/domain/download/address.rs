// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: URL parsing, protocol validation, and security sanitization for download destination addresses.

use url::Url;

use crate::domain::protocol::mms::MmsProtocolDetector;

use super::{
    download_policy::{
        DASH_MANIFEST_EXTENSION, MAX_URL_LENGTH_BYTES,
    },
    DownloadProtocol,
};

#[derive(Clone, Debug)]
pub struct DownloadAddress {
    url: Url,
    protocol: DownloadProtocol,
}

impl DownloadAddress {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.len() > MAX_URL_LENGTH_BYTES {
            return Err("A download address exceeds the allowed length.".to_owned());
        }

        if MmsProtocolDetector::is_playlist_wrapper(value) {
            return Err(
                "ASX and WVX playlists are stream wrappers, not direct media files. \
                 Use playlist resolution to download individual streams."
                    .to_owned(),
            );
        }

        if let Some(mms_variant) = MmsProtocolDetector::detect_scheme(value) {
            if !mms_variant.is_supported() {
                return Err(mms_variant.unsupported_guidance().to_owned());
            }
        }

        let url = Url::parse(value)
            .map_err(|_| "Enter a valid HTTP, HTTPS, FTP, or MMS address.".to_owned())?;
        let protocol = DownloadProtocol::parse(url.scheme())
            .ok_or_else(|| "Downloads must use HTTP, HTTPS, FTP, or MMS.".to_owned())?;
        if url.host_str().is_none() {
            return Err("Downloads must use a valid host address.".to_owned());
        }
        if protocol != DownloadProtocol::Ftp && (!url.username().is_empty() || url.password().is_some()) {
            return Err(
                "Downloads must use HTTP, HTTPS, or MMS without embedded credentials.".to_owned(),
            );
        }
        let path = url.path().to_ascii_lowercase();
        if path.ends_with(DASH_MANIFEST_EXTENSION) {
            return Err("DASH manifests are not supported.".to_owned());
        }
        if path.ends_with(".asx") || path.ends_with(".wvx") || path.ends_with(".wmx") || path.ends_with(".wax") {
            return Err(
                "ASX and WVX playlists are stream wrappers, not direct media files. \
                 Use playlist resolution to download individual streams."
                    .to_owned(),
            );
        }
        Ok(Self { url, protocol })
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn protocol(&self) -> DownloadProtocol {
        self.protocol
    }
}
