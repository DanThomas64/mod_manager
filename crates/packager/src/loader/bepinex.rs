use super::Loader;
use crate::fsutil::{copy_dir_contents, copy_dir_contents_filtered, find_dir_named, unzip_to};
use anyhow::{Context, Result};
use std::path::Path;

pub struct BepinexLoader;

impl Loader for BepinexLoader {
    /// Unzip the BepInEx pack and copy its contents (the folder that
    /// directly contains a `BepInEx/` subfolder, e.g. loader DLLs/scripts
    /// alongside it) into the root of `staging_dir` — this is exactly what
    /// BepInEx's own install instructions say to drop into the game folder.
    fn stage_loader(&self, loader_zip: Option<&Path>, staging_dir: &Path) -> Result<()> {
        let zip_path = loader_zip.context("the bepinex loader requires a [loader] source in config")?;
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
    /// non-metadata file at the zip root if no `plugins/` folder is found.
    /// If the cached file isn't a zip at all (e.g. a bare .dll GitHub
    /// asset), it is copied directly.
    fn stage_mod(&self, mod_file: &Path, id: &str, staging_dir: &Path) -> Result<()> {
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

    fn mods_subpath(&self) -> String {
        "BepInEx/plugins".to_string()
    }

    fn marker_subpath(&self) -> String {
        "BepInEx/.mod-installer.marker".to_string()
    }
}
