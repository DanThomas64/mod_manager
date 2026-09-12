use std::path::{Path, PathBuf};

/// Identifies an install as having been made by this tool. Baked into the
/// payload by `mod-packager` at `BepInEx/.valheim-mod-installer.marker`, so
/// it travels with the mod files themselves and survives being overwritten
/// on each update — no need for the shipped installer binary to construct
/// one itself.
pub struct InstallMarker {
    pub modpack_version: String,
    pub bepinex_version: String,
    pub packaged_at: String,
}

const MARKER_FILENAME: &str = ".valheim-mod-installer.marker";

pub fn marker_path(bepinex_dir: &Path) -> PathBuf {
    bepinex_dir.join(MARKER_FILENAME)
}

pub fn read_marker(bepinex_dir: &Path) -> Option<InstallMarker> {
    let text = std::fs::read_to_string(marker_path(bepinex_dir)).ok()?;
    parse_marker(&text)
}

fn parse_marker(text: &str) -> Option<InstallMarker> {
    let mut modpack_version = None;
    let mut bepinex_version = None;
    let mut packaged_at = None;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "modpack_version" => modpack_version = Some(value.trim().to_string()),
                "bepinex_version" => bepinex_version = Some(value.trim().to_string()),
                "packaged_at" => packaged_at = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    Some(InstallMarker {
        modpack_version: modpack_version?,
        bepinex_version: bepinex_version?,
        packaged_at: packaged_at?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_marker_text() {
        let text = "installer=valheim-mod-installer\nmodpack_version=0.1.0\nbepinex_version=5.4.2350\npackaged_at=2026-09-12\n";
        let marker = parse_marker(text).unwrap();
        assert_eq!(marker.modpack_version, "0.1.0");
        assert_eq!(marker.bepinex_version, "5.4.2350");
        assert_eq!(marker.packaged_at, "2026-09-12");
    }

    #[test]
    fn missing_fields_yield_none() {
        assert!(parse_marker("modpack_version=0.1.0\n").is_none());
    }
}
