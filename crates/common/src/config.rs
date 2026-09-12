use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModpackConfig {
    pub bepinex: Source,
    #[serde(default, rename = "mods")]
    pub mods: Vec<ModEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModEntry {
    /// Stable local key for this mod, independent of its source. Used as the
    /// lockfile key and as the plugin subfolder name.
    pub id: String,
    #[serde(flatten)]
    pub source: Source,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum Source {
    Thunderstore { author: String, name: String },
    Github {
        owner: String,
        repo: String,
        asset_pattern: String,
    },
}

impl ModpackConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let text = std::fs::read_to_string(path_ref).map_err(|source| ConfigError::Read {
            path: path_ref.display().to_string(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path_ref.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bepinex_and_mods() {
        let toml_text = r#"
            [bepinex]
            source = "thunderstore"
            author = "denikson"
            name = "BepInExPack_Valheim"

            [[mods]]
            id = "some-mod"
            source = "thunderstore"
            author = "someauthor"
            name = "SomeMod"

            [[mods]]
            id = "other-mod"
            source = "github"
            owner = "someuser"
            repo = "somerepo"
            asset_pattern = "*.zip"
        "#;
        let config: ModpackConfig = toml::from_str(toml_text).unwrap();
        assert!(matches!(config.bepinex, Source::Thunderstore { .. }));
        assert_eq!(config.mods.len(), 2);
        assert_eq!(config.mods[0].id, "some-mod");
        assert!(matches!(config.mods[1].source, Source::Github { .. }));
    }
}
