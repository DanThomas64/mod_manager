use anyhow::{Context, Result};
use common::diff::{ChangeSet, apply_bump, bump_kind};
use common::lockfile::Lockfile;
use semver::Version;
use std::path::Path;

/// Read `releases/latest.txt` and load the previous release's lockfile, if
/// any. Returns `(previous_version, previous_lockfile)`; an empty/default
/// lockfile and `None` version mean this is the first release.
pub fn load_previous(releases_dir: &Path) -> Result<(Option<Version>, Lockfile)> {
    let latest_path = releases_dir.join("latest.txt");
    if !latest_path.exists() {
        return Ok((None, Lockfile::default()));
    }
    let version_str = std::fs::read_to_string(&latest_path)?.trim().to_string();
    let version = Version::parse(&version_str)
        .with_context(|| format!("'{version_str}' in {} is not valid semver", latest_path.display()))?;
    let lockfile_path = releases_dir
        .join(format!("v{version}"))
        .join("modpack.lock.toml");
    let lockfile = Lockfile::load_or_default(&lockfile_path)?;
    Ok((Some(version), lockfile))
}

/// Compute the next release version from the previous version and the
/// change set. First release always starts at 0.1.0; otherwise applies the
/// minor/patch bump rule from `common::diff`.
pub fn next_version(previous: &Option<Version>, changes: &ChangeSet) -> Option<Version> {
    match previous {
        None => Some(Version::new(0, 1, 0)),
        Some(prev) => bump_kind(changes).map(|bump| apply_bump(prev, bump)),
    }
}

/// Render a markdown changelog entry for one release.
pub fn render_changelog_entry(version: &Version, bepinex_version: &Version, changes: &ChangeSet, date: &str) -> String {
    let mut out = format!("## v{version} — {date}\nBepInEx: {bepinex_version}\n");

    if !changes.added.is_empty() {
        out.push_str("\n### Added\n");
        for (id, version) in &changes.added {
            out.push_str(&format!("- {id} ({version})\n"));
        }
    }
    if !changes.changed.is_empty() {
        out.push_str("\n### Changed\n");
        for (id, old, new) in &changes.changed {
            out.push_str(&format!("- {id}: {old} → {new}\n"));
        }
    }
    if let Some((old, new)) = &changes.bepinex_changed {
        out.push_str("\n### BepInEx\n");
        match old {
            Some(old) => out.push_str(&format!("- {old} → {new}\n")),
            None => out.push_str(&format!("- {new}\n")),
        }
    }
    if !changes.removed.is_empty() {
        out.push_str("\n### Removed\n");
        for (id, version) in &changes.removed {
            out.push_str(&format!("- {id} ({version})\n"));
        }
    }
    out.push('\n');
    out
}

pub fn prepend_changelog(releases_dir: &Path, entry: &str) -> Result<()> {
    let path = releases_dir.join("CHANGELOG.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, format!("{entry}{existing}"))?;
    Ok(())
}

pub fn render_instructions(version: &Version, bepinex_version: &Version) -> String {
    format!(
        "# Modpack v{version} — Install Instructions\n\n\
         1. Download the file matching your OS:\n   \
         - Windows: `installer-windows.exe`\n   \
         - Linux (native, not Proton): `installer-linux`\n\n\
         2. Make sure Valheim and Steam are already installed, and you've run the game at least once.\n\n\
         3. Run the file:\n   \
         - Windows: double-click `installer-windows.exe`.\n   \
         - Linux: open a terminal in the download folder and run:\n     \
         `chmod +x installer-linux && ./installer-linux`\n\n\
         4. If it can't find your Valheim folder automatically, it will ask you to paste the path \
         (the folder containing `valheim.exe` or `valheim.x86_64`).\n\n\
         This installs modpack v{version} (BepInEx v{bepinex_version}). See CHANGELOG.md for what changed.\n\n\
         Linux note: BepInEx on Linux requires a Steam launch option. In Steam, right-click Valheim -> \
         Properties -> Launch Options, and set it to run `start_game_bepinex.sh` from the game folder \
         (see BepInEx's own README included in this install for the exact command). Windows needs no \
         extra setup — BepInEx loads automatically.\n"
    )
}

pub fn render_server_instructions(version: &Version, bepinex_version: &Version) -> String {
    format!(
        "# Modpack v{version} — Dedicated Server Instructions\n\n\
         The `server-plugins/` folder in this release is a plain, uncompressed copy of every \
         configured mod's plugin files — exactly what needs to be present under your server's \
         `BepInEx/plugins/` folder.\n\n\
         ## Manual update (copy-paste)\n\n\
         1. Stop the server.\n\
         2. Make sure BepInEx v{bepinex_version} is already installed on the server (same \
         `denikson-BepInExPack_Valheim` pack the client installers use).\n\
         3. Copy the contents of `server-plugins/` into the server's `BepInEx/plugins/` folder, \
         overwriting existing files.\n\
         4. Start the server back up.\n\n\
         This is modpack v{version} — see CHANGELOG.md for what changed. Server-side mods should \
         match the version installed on clients; mismatched BepInEx/mod versions between server \
         and clients can cause connection or desync issues.\n\n\
         ## AMP\n\n\
         No direct AMP automation yet — for now this is a manual copy-paste step via AMP's file \
         manager (or SFTP) onto the server's `BepInEx/plugins/` folder. Wiring this into AMP's \
         update/deployment flow is a planned improvement.\n"
    )
}

pub fn update_latest_pointer(releases_dir: &Path, version: &Version) -> Result<()> {
    std::fs::write(releases_dir.join("latest.txt"), version.to_string())?;
    Ok(())
}
