use anyhow::{Context, Result};
use std::path::Path;

const METADATA_FILENAME: &str = ".mod-installer.game";

/// Per-game facts baked into the payload by `mod-packager` (since
/// `installer-shell` is built once and reused across every game/release, it
/// can't know these at compile time). Read from the extracted payload
/// before detection/backup/install.
pub struct GameMetadata {
    pub game_name: String,
    pub steam_folder_name: String,
    pub windows_exe: String,
    pub linux_exe: String,
    /// Game-root-relative path mods live under, e.g. `BepInEx/plugins`.
    pub mods_subpath: String,
}

impl GameMetadata {
    /// The game-root-relative loader directory — the parent of wherever
    /// mods/the marker actually live. Derived from `mods_subpath`'s first
    /// path component (`BepInEx/plugins` -> `BepInEx`; `mods` -> `mods`).
    pub fn loader_root(&self) -> &str {
        self.mods_subpath.split('/').next().unwrap_or(&self.mods_subpath)
    }
}

pub fn read_game_metadata(extracted_root: &Path) -> Result<GameMetadata> {
    let path = extracted_root.join(METADATA_FILENAME);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {} (this binary wasn't packaged with mod-packager?)", path.display()))?;
    parse_game_metadata(&text).with_context(|| format!("parsing {}", path.display()))
}

fn parse_game_metadata(text: &str) -> Result<GameMetadata> {
    let mut game_name = None;
    let mut steam_folder_name = None;
    let mut windows_exe = None;
    let mut linux_exe = None;
    let mut mods_subpath = None;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "game_name" => game_name = Some(value.trim().to_string()),
                "steam_folder_name" => steam_folder_name = Some(value.trim().to_string()),
                "windows_exe" => windows_exe = Some(value.trim().to_string()),
                "linux_exe" => linux_exe = Some(value.trim().to_string()),
                "mods_subpath" => mods_subpath = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    Ok(GameMetadata {
        game_name: game_name.context("missing game_name")?,
        steam_folder_name: steam_folder_name.context("missing steam_folder_name")?,
        windows_exe: windows_exe.context("missing windows_exe")?,
        linux_exe: linux_exe.context("missing linux_exe")?,
        mods_subpath: mods_subpath.context("missing mods_subpath")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_game_metadata_text() {
        let text = "game_name=Valheim\nsteam_folder_name=Valheim\nwindows_exe=valheim.exe\nlinux_exe=valheim.x86_64\nmods_subpath=BepInEx/plugins\n";
        let meta = parse_game_metadata(text).unwrap();
        assert_eq!(meta.game_name, "Valheim");
        assert_eq!(meta.loader_root(), "BepInEx");
    }

    #[test]
    fn loader_root_of_single_segment_mods_subpath_is_itself() {
        let text = "game_name=G\nsteam_folder_name=G\nwindows_exe=g.exe\nlinux_exe=g\nmods_subpath=mods\n";
        let meta = parse_game_metadata(text).unwrap();
        assert_eq!(meta.loader_root(), "mods");
    }
}
