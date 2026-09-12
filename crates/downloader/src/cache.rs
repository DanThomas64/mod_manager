use anyhow::{Context, Result};
use common::hash::sha256_reader;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Path a downloaded file for `id`@`version` is cached at: `cache/{id}/{version}/{filename}`.
pub fn cache_path(cache_dir: &Path, id: &str, version: &str, filename: &str) -> PathBuf {
    cache_dir.join(id).join(version).join(filename)
}

/// Ensure `id`@`version`'s file is present in the cache, downloading it if
/// absent, and return its path plus sha256 hash.
pub fn fetch_cached(
    client: &reqwest::blocking::Client,
    cache_dir: &Path,
    id: &str,
    version: &str,
    filename: &str,
    download_url: &str,
) -> Result<(PathBuf, String)> {
    let path = cache_path(cache_dir, id, version, filename);
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap())
            .with_context(|| format!("creating cache dir for {id}@{version}"))?;
        let tmp_path = path.with_extension("part");
        let mut response = client
            .get(download_url)
            .header("User-Agent", "valheim-mod-installer")
            .send()
            .with_context(|| format!("downloading {id}@{version} from {download_url}"))?
            .error_for_status()
            .with_context(|| format!("download of {id}@{version} returned an error status"))?;
        let mut file = File::create(&tmp_path)
            .with_context(|| format!("creating temp file for {id}@{version}"))?;
        std::io::copy(&mut response, &mut file)
            .with_context(|| format!("writing downloaded bytes for {id}@{version}"))?;
        drop(file);
        std::fs::rename(&tmp_path, &path)
            .with_context(|| format!("finalizing cached file for {id}@{version}"))?;
    }

    let file = File::open(&path).with_context(|| format!("re-opening cached file for {id}@{version}"))?;
    let hash = sha256_reader(file).with_context(|| format!("hashing cached file for {id}@{version}"))?;
    Ok((path, hash))
}
