use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Resolve the release directory (`releases/vX.Y.Z`) pointed to by
/// `releases/latest.txt`.
pub fn latest_release_dir(releases_dir: &Path) -> Result<PathBuf> {
    let latest_path = releases_dir.join("latest.txt");
    let version = std::fs::read_to_string(&latest_path)
        .with_context(|| format!("reading {} (run mod-packager first)", latest_path.display()))?;
    Ok(releases_dir.join(format!("v{}", version.trim())))
}
