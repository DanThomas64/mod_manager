use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// A Steam Workshop item's "version" surrogate: Workshop items don't carry
/// real semver, so the item's last-update timestamp stands in for it — good
/// enough for change detection (`common::diff` only ever compares versions
/// with `!=`, never orders them).
pub struct Resolved {
    pub version: String,
}

#[derive(Deserialize)]
struct DetailsResponse {
    response: DetailsInner,
}

#[derive(Deserialize)]
struct DetailsInner {
    publishedfiledetails: Vec<PublishedFileDetails>,
}

#[derive(Deserialize)]
struct PublishedFileDetails {
    result: u32,
    time_updated: Option<u64>,
}

/// Resolve a Workshop item's current "version" via the public
/// `ISteamRemoteStorage/GetPublishedFileDetails` API — no API key required
/// for public item details.
pub fn resolve_latest(client: &reqwest::blocking::Client, workshop_id: u64) -> Result<Resolved> {
    let response: DetailsResponse = client
        .post("https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/")
        .form(&[("itemcount", "1".to_string()), ("publishedfileids[0]", workshop_id.to_string())])
        .send()
        .context("requesting Steam Workshop item details")?
        .error_for_status()
        .context("Steam Workshop API returned an error status")?
        .json()
        .context("parsing Steam Workshop API response")?;

    let details = response
        .response
        .publishedfiledetails
        .into_iter()
        .next()
        .with_context(|| format!("no Workshop item details returned for {workshop_id}"))?;
    if details.result != 1 {
        bail!("Workshop item {workshop_id} not found or not public (result code {})", details.result);
    }
    let time_updated = details
        .time_updated
        .with_context(|| format!("Workshop item {workshop_id} has no time_updated"))?;

    Ok(Resolved {
        version: format!("updated-{time_updated}"),
    })
}
