// Created: 2026-11-05
// Reason: feat(updater): implement auto-updater, periodic background checks, and active transfer safety guard (Phase F)
// Last updated: 2026-11-05
// Reason: feat(updater): implement auto-updater, periodic background checks, and active transfer safety guard (Phase F)
// Description: GitHub release checker comparing SemVer tags and streaming update binaries.

use std::{future::Future, path::Path, pin::Pin};

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::{
    application::error::{AppError, AppResult},
    domain::updater::{
        clean_version_tag, UpdateReleaseInfo, DEFAULT_REPO_NAME, DEFAULT_REPO_OWNER,
        USER_AGENT as AD_USER_AGENT,
    },
};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait UpdateClient: Send + Sync {
    fn check_latest_release(&self) -> BoxFuture<'_, AppResult<Option<UpdateReleaseInfo>>>;
    fn download_update_installer<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
    ) -> BoxFuture<'a, AppResult<()>>;
}

#[derive(Clone)]
pub struct GitHubReleaseClient {
    client: reqwest::Client,
    owner: String,
    repo: String,
}

#[derive(Deserialize)]
struct GitHubReleaseResponse {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

impl GitHubReleaseClient {
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(AD_USER_AGENT));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_default();

        Self {
            client,
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    pub fn default_client() -> Self {
        Self::new(DEFAULT_REPO_OWNER, DEFAULT_REPO_NAME)
    }
}

impl UpdateClient for GitHubReleaseClient {
    fn check_latest_release(&self) -> BoxFuture<'_, AppResult<Option<UpdateReleaseInfo>>> {
        Box::pin(async move {
            let url = format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                self.owner, self.repo
            );

            let response = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|err| AppError::network(format!("Could not reach GitHub Releases: {err}")))?;

            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND
                || status == reqwest::StatusCode::NOT_ACCEPTABLE
            {
                return Err(AppError::network(format!(
                    "Unable to find latest version on GitHub ({}): HTTPError: {}",
                    url,
                    status.as_u16()
                )));
            }

            if !status.is_success() {
                return Err(AppError::network(format!(
                    "GitHub Releases request failed with status: {status}"
                )));
            }

            let release: GitHubReleaseResponse = response
                .json()
                .await
                .map_err(|err| AppError::network(format!("Invalid GitHub release payload: {err}")))?;

            let version = clean_version_tag(&release.tag_name).to_string();

            // Prefer .exe installer or first asset
            let download_url = release
                .assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().ends_with(".exe"))
                .or_else(|| release.assets.first())
                .map(|asset| asset.browser_download_url.clone());

            Ok(Some(UpdateReleaseInfo {
                version,
                release_notes: release.body,
                download_url,
                published_at: release.published_at,
            }))
        })
    }

    fn download_update_installer<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
    ) -> BoxFuture<'a, AppResult<()>> {
        Box::pin(async move {
            let mut response = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|err| AppError::download(format!("Could not download update: {err}")))?;

            if !response.status().is_success() {
                return Err(AppError::download(format!(
                    "Update download failed with status: {}",
                    response.status()
                )));
            }

            if let Some(parent) = destination.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            let mut file = tokio::fs::File::create(destination)
                .await
                .map_err(|err| AppError::storage(format!("Could not create installer file: {err}")))?;

            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|err| AppError::download(format!("Error while reading update stream: {err}")))?
            {
                file.write_all(&chunk)
                    .await
                    .map_err(|err| AppError::storage(format!("Could not write installer file: {err}")))?;
            }

            file.sync_all()
                .await
                .map_err(|err| AppError::storage(format!("Could not sync installer file: {err}")))?;

            Ok(())
        })
    }
}
