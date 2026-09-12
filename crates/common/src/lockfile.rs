use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedEntry {
    pub version: semver::Version,
    pub source_url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Lockfile {
    pub bepinex: Option<LockedEntry>,
    #[serde(default)]
    pub mods: BTreeMap<String, LockedEntry>,
}

impl Lockfile {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    /// Load a lockfile if it exists, otherwise return an empty one. Used when
    /// diffing against "no previous release".
    pub fn load_or_default(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::default())
        }
    }

    /// Atomically write this lockfile to `path` (write to a temp file, then
    /// rename) so a crash mid-write never leaves a truncated lockfile.
    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let text = toml::to_string_pretty(self)?;
        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, text)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}
