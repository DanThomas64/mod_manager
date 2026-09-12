use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize, Clone)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub struct Resolved {
    pub version: semver::Version,
    pub download_url: String,
    pub filename: String,
}

/// Resolve the latest GitHub release for owner/repo, picking the first
/// release asset whose filename matches `asset_pattern` (a simple glob).
pub fn resolve_latest(
    client: &reqwest::blocking::Client,
    owner: &str,
    repo: &str,
    asset_pattern: &str,
) -> Result<Resolved> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let mut request = client
        .get(&url)
        .header("User-Agent", "mod-installer")
        .header("Accept", "application/vnd.github+json");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.header("Authorization", format!("Bearer {token}"));
    }

    let release: Release = request
        .send()
        .with_context(|| format!("requesting latest release for {owner}/{repo}"))?
        .error_for_status()
        .with_context(|| format!("GitHub returned an error status for {owner}/{repo}"))?
        .json()
        .with_context(|| format!("parsing GitHub release JSON for {owner}/{repo}"))?;

    let pattern = glob::Pattern::new(asset_pattern)
        .with_context(|| format!("invalid asset_pattern '{asset_pattern}' for {owner}/{repo}"))?;
    let asset = release
        .assets
        .iter()
        .find(|a| pattern.matches(&a.name))
        .with_context(|| {
            format!("no release asset matching '{asset_pattern}' found for {owner}/{repo}")
        })?;

    let version_str = release.tag_name.trim_start_matches('v');
    let version = semver::Version::parse(version_str).with_context(|| {
        format!("GitHub release tag '{}' for {owner}/{repo} is not valid semver", release.tag_name)
    })?;

    Ok(Resolved {
        version,
        download_url: asset.browser_download_url.clone(),
        filename: asset.name.clone(),
    })
}
