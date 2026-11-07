use std::time::Duration;

use crate::{
    application::error::{AppError, AppResult},
    domain::settings::ProxySettings,
};

use super::policy::{CONNECT_TIMEOUT_SECONDS, MAX_REDIRECTS};

pub fn create_http_client(proxy: Option<&ProxySettings>) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS));

    if let Some(proxy_config) = proxy {
        if proxy_config.enabled {
            let req_proxy = proxy_config.to_reqwest_proxy()?;
            builder = builder.proxy(req_proxy);
        } else {
            builder = builder.no_proxy();
        }
    }

    builder.build().map_err(|error| {
        let msg = format!("Could not initialize HTTP client: {error}");
        let sanitized = if let Some(p) = proxy {
            p.sanitize_error(&msg)
        } else {
            msg
        };
        AppError::network(sanitized)
    })
}
