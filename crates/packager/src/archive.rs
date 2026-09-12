use crate::fsutil::{copy_dir_contents, find_cached_file};
use crate::loader::Loader;
use anyhow::{Context, Result};
use common::config::GameConfig;
use common::lockfile::Lockfile;
use semver::Version;
use std::path::Path;

/// Build the payload: unpack the cached loader package (if any) and every
/// mod into a scratch staging directory shaped like the real game folder,
/// tar it, and zstd-compress the tar. Returns the compressed bytes ready to
/// embed.
pub fn build_payload(
    cache_dir: &Path,
    lockfile: &Lockfile,
    staging_dir: &Path,
    modpack_version: &Version,
    date: &str,
    game: &GameConfig,
    mod_loader: &dyn Loader,
) -> Result<Vec<u8>> {
    if staging_dir.exists() {
        std::fs::remove_dir_all(staging_dir)?;
    }
    std::fs::create_dir_all(staging_dir)?;

    let loader_zip = match &lockfile.loader {
        Some(entry) => Some(find_cached_file(cache_dir, "loader", &entry.version)?),
        None => None,
    };
    mod_loader.stage_loader(loader_zip.as_deref(), staging_dir)?;

    for (id, entry) in &lockfile.mods {
        let mod_file = find_cached_file(cache_dir, id, &entry.version)?;
        mod_loader.stage_mod(&mod_file, id, staging_dir)?;
    }

    let loader_version = lockfile.loader.as_ref().map(|e| e.version.as_str()).unwrap_or("n/a");
    write_marker(staging_dir, mod_loader, modpack_version, loader_version, date)?;
    write_game_metadata(staging_dir, game, mod_loader)?;

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        builder.append_dir_all(".", staging_dir)?;
        builder.finish()?;
    }

    let compressed = zstd::encode_all(tar_bytes.as_slice(), 19).context("zstd-compressing payload")?;
    Ok(compressed)
}

/// Copy the staged mods tree out as a plain, uncompressed folder — for a
/// dedicated server host (e.g. an AMP-managed server) who just needs to
/// copy-paste it over their server's existing mods folder. Must be called
/// before the staging dir is cleaned up.
pub fn export_server_plugins(staging_dir: &Path, dest: &Path, mod_loader: &dyn Loader) -> Result<()> {
    let src = staging_dir.join(mod_loader.mods_subpath());
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    copy_dir_contents(&src, dest).with_context(|| format!("exporting server plugins to {}", dest.display()))
}

/// Write the marker `installer-runtime::marker` reads to recognize an
/// install made by this tool, at the loader's configured marker path.
fn write_marker(
    staging_dir: &Path,
    mod_loader: &dyn Loader,
    modpack_version: &Version,
    loader_version: &str,
    date: &str,
) -> Result<()> {
    let text = format!(
        "installer=mod-installer\nmodpack_version={modpack_version}\nloader_version={loader_version}\npackaged_at={date}\n"
    );
    let marker_path = staging_dir.join(mod_loader.marker_subpath());
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(marker_path, text)?;
    Ok(())
}

/// Write the per-game facts `installer-runtime` needs at install time
/// (exe names, Steam folder, loader paths) — baked into the payload since
/// the shipped installer binary is built once and reused across every
/// game/release, so it can't know these at compile time.
fn write_game_metadata(staging_dir: &Path, game: &GameConfig, mod_loader: &dyn Loader) -> Result<()> {
    let text = format!(
        "game_name={}\nsteam_folder_name={}\nwindows_exe={}\nlinux_exe={}\nmods_subpath={}\n",
        game.name,
        game.steam_folder_name,
        game.windows_exe,
        game.linux_exe,
        mod_loader.mods_subpath(),
    );
    std::fs::write(staging_dir.join(".mod-installer.game"), text)?;
    Ok(())
}
