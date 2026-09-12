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
    pub game: GameConfig,
    pub loader: LoaderConfig,
    #[serde(default, rename = "mods")]
    pub mods: Vec<ModEntry>,
}

/// Facts about the target game needed to find its install and package a
/// working payload for it. Baked into the packaged payload (as
/// `.mod-installer.game`) so the shipped installer binary — built once and
/// reused across every game/release — stays game-agnostic at compile time.
#[derive(Debug, Clone, Deserialize)]
pub struct GameConfig {
    pub name: String,
    /// The folder name under `steamapps/common/` for this game.
    pub steam_folder_name: String,
    pub windows_exe: String,
    pub linux_exe: String,
    /// Optional; not required for install detection, but needed for
    /// Steam Workshop mod sources (the Workshop API is scoped per-app).
    pub steam_appid: Option<u32>,
    /// Optional; enables `mod-downloader --update-server`.
    pub dedicated_server_appid: Option<u32>,
    /// Optional; required if any `thunderstore` source is used. Thunderstore
    /// is organized per-game "community" (e.g. `valheim`, `riskofrain2`) —
    /// each has its own package index at `/c/<community>/api/v1/package/`.
    pub thunderstore_community: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderConfig {
    #[serde(rename = "type")]
    pub loader_type: LoaderType,
    /// Where the loader itself is fetched from — same `Source` shapes as a
    /// mod. Not required for loaders (like `generic`) that have no separate
    /// framework package to install.
    #[serde(flatten)]
    pub source: Option<Source>,
    /// Only used by the `generic` loader: the subfolder (relative to the
    /// game root) mods get copied into, e.g. `"mods"`.
    #[serde(default)]
    pub mods_subpath: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderType {
    Bepinex,
    Generic,
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
    Thunderstore {
        author: String,
        name: String,
    },
    Github {
        owner: String,
        repo: String,
        asset_pattern: String,
    },
    Steamworkshop {
        workshop_id: u64,
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
    fn parses_game_loader_and_mods() {
        let toml_text = r#"
            [game]
            name = "Valheim"
            steam_folder_name = "Valheim"
            windows_exe = "valheim.exe"
            linux_exe = "valheim.x86_64"
            steam_appid = 892970
            dedicated_server_appid = 896660
            thunderstore_community = "valheim"

            [loader]
            type = "bepinex"
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

            [[mods]]
            id = "workshop-mod"
            source = "steamworkshop"
            workshop_id = 123456
        "#;
        let config: ModpackConfig = toml::from_str(toml_text).unwrap();
        assert_eq!(config.game.name, "Valheim");
        assert_eq!(config.loader.loader_type, LoaderType::Bepinex);
        assert!(matches!(config.loader.source, Some(Source::Thunderstore { .. })));
        assert_eq!(config.mods.len(), 3);
        assert_eq!(config.mods[0].id, "some-mod");
        assert!(matches!(config.mods[1].source, Source::Github { .. }));
        assert!(matches!(config.mods[2].source, Source::Steamworkshop { workshop_id: 123456 }));
    }

    #[test]
    fn generic_loader_needs_no_source() {
        let toml_text = r#"
            [game]
            name = "SomeGame"
            steam_folder_name = "SomeGame"
            windows_exe = "somegame.exe"
            linux_exe = "somegame"

            [loader]
            type = "generic"
            mods_subpath = "mods"
        "#;
        let config: ModpackConfig = toml::from_str(toml_text).unwrap();
        assert_eq!(config.loader.loader_type, LoaderType::Generic);
        assert!(config.loader.source.is_none());
        assert_eq!(config.loader.mods_subpath.as_deref(), Some("mods"));
    }
}
