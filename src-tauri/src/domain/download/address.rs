use url::Url;

use super::{
    download_policy::{
        DASH_MANIFEST_EXTENSION, MAX_URL_LENGTH_BYTES, UNSUPPORTED_MEDIA_HOSTS,
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
        let url =
            Url::parse(value).map_err(|_| "Enter a valid HTTP or HTTPS address.".to_owned())?;
        let protocol = DownloadProtocol::parse(url.scheme())
            .ok_or_else(|| "Downloads must use HTTP or HTTPS.".to_owned())?;
        let host = url
            .host_str()
            .ok_or_else(|| "Downloads must use an HTTP or HTTPS address.".to_owned())?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(
                "Downloads must use HTTP or HTTPS without embedded credentials.".to_owned(),
            );
        }
        let path = url.path().to_ascii_lowercase();
        if path.ends_with(DASH_MANIFEST_EXTENSION) {
            return Err("DASH manifests are not supported.".to_owned());
        }
        if UNSUPPORTED_MEDIA_HOSTS.iter().any(|unsupported_host| {
            host == *unsupported_host
                || host
                    .strip_suffix(unsupported_host)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        }) {
            return Err(
                "YouTube video downloads require the browser extension or format selection."
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
