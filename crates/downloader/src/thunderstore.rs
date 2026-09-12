use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Package {
    owner: String,
    name: String,
    versions: Vec<PackageVersion>,
}

#[derive(Debug, Deserialize, Clone)]
struct PackageVersion {
    version_number: String,
    download_url: String,
    date_created: String,
}

pub struct Resolved {
    pub version: semver::Version,
    pub download_url: String,
    pub filename: String,
}

/// Resolve the newest version of a Thunderstore package by owner+name,
/// within the given community. Thunderstore's global `/api/v1/package/`
/// index is scoped to whatever the "default" community happens to be
/// (historically Risk of Rain 2), not any particular game — each game's
/// community has its own scoped index at `/c/{community}/api/v1/package/`,
/// which is what `community` (from `game.thunderstore_community` in config)
/// selects.
///
/// The v1 package index returns each package's `versions` already sorted
/// newest-first, but we don't trust that ordering blindly — pick the entry
/// with the max `date_created` (ISO 8601 strings sort correctly lexically).
pub fn resolve_latest(client: &reqwest::blocking::Client, community: &str, author: &str, name: &str) -> Result<Resolved> {
    let url = format!("https://thunderstore.io/c/{community}/api/v1/package/");
    let packages: Vec<Package> = client
        .get(&url)
        .header("User-Agent", "mod-installer")
        .send()
        .context("requesting Thunderstore package index")?
        .error_for_status()
        .context("Thunderstore package index returned an error status")?
        .json()
        .context("parsing Thunderstore package index JSON")?;

    let package = packages
        .into_iter()
        .find(|p| p.owner.eq_ignore_ascii_case(author) && p.name.eq_ignore_ascii_case(name))
        .with_context(|| format!("no Thunderstore package found for {author}/{name}"))?;

    let newest = package
        .versions
        .iter()
        .max_by(|a, b| a.date_created.cmp(&b.date_created))
        .with_context(|| format!("Thunderstore package {author}/{name} has no versions"))?
        .clone();

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
