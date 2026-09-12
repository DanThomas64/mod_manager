use anyhow::{Context, Result};
use common::lockfile::Lockfile;
use semver::Version;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Files commonly bundled in a Thunderstore package zip that aren't part of
/// the actual mod payload and shouldn't be copied into `plugins/`.
const NON_PLUGIN_FILES: &[&str] = &["manifest.json", "readme.md", "icon.png", "changelog.md"];

/// Build the payload: unpack the cached BepInEx zip and every mod zip into a
/// scratch staging directory shaped like the real Valheim game folder
/// (`BepInEx/`, loader files, `BepInEx/plugins/{mod-id}/...`), tar it, and
/// zstd-compress the tar. Returns the compressed bytes ready to embed.
pub fn build_payload(
    cache_dir: &Path,
    lockfile: &Lockfile,
    staging_dir: &Path,
    modpack_version: &Version,
    date: &str,
) -> Result<Vec<u8>> {
    if staging_dir.exists() {
        std::fs::remove_dir_all(staging_dir)?;
    }
    std::fs::create_dir_all(staging_dir)?;

    let bepinex = lockfile
        .bepinex
        .as_ref()
        .context("lockfile has no bepinex entry; run mod-downloader first")?;
    let bepinex_zip = find_cached_file(cache_dir, "bepinex", &bepinex.version.to_string())?;
    stage_bepinex(&bepinex_zip, staging_dir)?;

    for (id, entry) in &lockfile.mods {
        let mod_file = find_cached_file(cache_dir, id, &entry.version.to_string())?;
        stage_mod(&mod_file, id, staging_dir)?;
    }

    write_marker(staging_dir, modpack_version, &bepinex.version, date)?;

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        builder.append_dir_all(".", staging_dir)?;
        builder.finish()?;
    }

    let compressed = zstd::encode_all(tar_bytes.as_slice(), 19).context("zstd-compressing payload")?;
    Ok(compressed)
}

/// Copy the staged `BepInEx/plugins/` tree out as a plain, uncompressed
/// folder — for a dedicated server host (e.g. an AMP-managed server) who
/// just needs to copy-paste it over their server's existing `plugins/`
/// folder. Must be called before the staging dir is cleaned up.
pub fn export_server_plugins(staging_dir: &Path, dest: &Path) -> Result<()> {
    let src = staging_dir.join("BepInEx").join("plugins");
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    copy_dir_contents(&src, dest)
        .with_context(|| format!("exporting server plugins to {}", dest.display()))
}

/// Write the marker `installer-runtime::marker` reads to recognize an
/// install made by this tool, at `BepInEx/.valheim-mod-installer.marker`
/// (sibling to `core/`, `config/`, `plugins/` — the usual BepInEx layout).
fn write_marker(staging_dir: &Path, modpack_version: &Version, bepinex_version: &Version, date: &str) -> Result<()> {
    let text = format!(
        "installer=valheim-mod-installer\nmodpack_version={modpack_version}\nbepinex_version={bepinex_version}\npackaged_at={date}\n"
    );
    std::fs::write(
        staging_dir.join("BepInEx").join(".valheim-mod-installer.marker"),
        text,
    )?;
    Ok(())
}

fn find_cached_file(cache_dir: &Path, id: &str, version: &str) -> Result<PathBuf> {
    let dir = cache_dir.join(id).join(version);
    let entry = std::fs::read_dir(&dir)
        .with_context(|| format!("reading cache dir {}", dir.display()))?
        .filter_map(|e| e.ok())
        .find(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .with_context(|| format!("no cached file found in {}", dir.display()))?;
    Ok(entry.path())
}

/// Unzip the BepInEx pack and copy its contents (the folder that directly
/// contains a `BepInEx/` subfolder, e.g. loader DLLs/scripts alongside it)
/// into the root of `staging_dir` — this is exactly what BepInEx's own
/// install instructions say to drop into the game folder.
fn stage_bepinex(zip_path: &Path, staging_dir: &Path) -> Result<()> {
    let scratch = staging_dir.with_file_name("bepinex-scratch");
    unzip_to(zip_path, &scratch)?;
    let pack_root = find_dir_named(&scratch, "bepinex")
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .with_context(|| format!("no BepInEx/ folder found inside {}", zip_path.display()))?;
    copy_dir_contents(&pack_root, staging_dir)?;
    std::fs::remove_dir_all(&scratch).ok();
    Ok(())
}

/// Unzip a mod archive and copy its plugin contents into
/// `staging_dir/BepInEx/plugins/{id}/`. Falls back to copying every
/// non-metadata file at the zip root if no `plugins/` folder is found. If
/// the cached file isn't a zip at all (e.g. a bare .dll GitHub asset), it is
/// copied directly.
fn stage_mod(mod_file: &Path, id: &str, staging_dir: &Path) -> Result<()> {
    let dest = staging_dir.join("BepInEx").join("plugins").join(id);
    std::fs::create_dir_all(&dest)?;

    let is_zip = mod_file
        .extension()
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if !is_zip {
        let filename = mod_file
            .file_name()
            .with_context(|| format!("cached mod file {} has no filename", mod_file.display()))?;
        std::fs::copy(mod_file, dest.join(filename))?;
        return Ok(());
    }

    let scratch = staging_dir.with_file_name(format!("mod-scratch-{id}"));
    unzip_to(mod_file, &scratch)?;

    let source = find_dir_named(&scratch, "plugins").unwrap_or_else(|| scratch.clone());
    copy_dir_contents_filtered(&source, &dest)?;
    std::fs::remove_dir_all(&scratch).ok();
    Ok(())
}

fn unzip_to(zip_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(zip_path).with_context(|| format!("opening {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).with_context(|| format!("reading zip {}", zip_path.display()))?;
    std::fs::create_dir_all(dest)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }
    Ok(())
}

/// Find the first directory named `name` (case-insensitive) anywhere under `root`.
fn find_dir_named(root: &Path, name: &str) -> Option<PathBuf> {
    if root.file_name().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false) {
        return Some(root.to_path_buf());
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.file_type().ok()?.is_dir() {
            if let Some(found) = find_dir_named(&entry.path(), name) {
                return Some(found);
            }
        }
    }
    None
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn copy_dir_contents_filtered(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy().to_lowercase();
        if entry.file_type()?.is_file() && NON_PLUGIN_FILES.contains(&name_lossy.as_str()) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
