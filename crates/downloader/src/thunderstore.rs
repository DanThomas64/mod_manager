use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Per-package endpoint. Thunderstore's community-scoped v1 index
/// (`/c/valheim/api/v1/package/`) lists every Valheim package and is tens of
/// MB, slow enough to time out — and was being fetched once per mod. This
/// endpoint returns just the one package with its latest version.
const PACKAGE_URL_BASE: &str = "https://thunderstore.io/api/experimental/package";

#[derive(Debug, Deserialize)]
struct Package {
    latest: PackageVersion,
}

#[derive(Debug, Deserialize)]
struct PackageVersion {
    version_number: String,
    download_url: String,
}

pub struct Resolved {
    pub version: semver::Version,
    pub download_url: String,
    pub filename: String,
}

/// Resolve the newest version of a Thunderstore package by owner+name.
pub fn resolve_latest(client: &reqwest::blocking::Client, author: &str, name: &str) -> Result<Resolved> {
    let url = format!("{PACKAGE_URL_BASE}/{author}/{name}/");
    let response = client
        .get(&url)
        .header("User-Agent", "valheim-mod-installer")
        .send()
        .with_context(|| format!("requesting Thunderstore package {author}/{name}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("no Thunderstore package found for {author}/{name}");
    }
    let package: Package = response
        .error_for_status()
        .with_context(|| format!("Thunderstore returned an error status for {author}/{name}"))?
        .json()
        .with_context(|| format!("parsing Thunderstore package JSON for {author}/{name}"))?;
    let newest = package.latest;

    let version = semver::Version::parse(&newest.version_number).with_context(|| {
        format!(
            "Thunderstore version '{}' for {author}/{name} is not valid semver",
            newest.version_number
        )
    })?;

    if newest.download_url.is_empty() {
        bail!("Thunderstore package {author}/{name} has an empty download_url");
    }
    // Thunderstore's download URL serves the zip via Content-Disposition
    // rather than a `.zip`-suffixed path, so we name the cached file ourselves.
    let filename = format!("{author}-{name}-{}.zip", newest.version_number);

    Ok(Resolved {
        version,
        download_url: newest.download_url,
        filename,
    })
}
