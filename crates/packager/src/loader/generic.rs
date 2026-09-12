use super::Loader;
use crate::fsutil::{copy_dir_contents_filtered, unzip_to};
use anyhow::{Context, Result};
use std::path::Path;

/// No mod-loader framework at all — mods are copied straight into one
/// configured subfolder of the game root. Covers games that take mods
/// directly (no BepInEx-style plugin framework), e.g. Workshop-only games.
pub struct GenericLoader {
    pub mods_subpath: String,
}

impl Loader for GenericLoader {
    fn stage_loader(&self, _loader_zip: Option<&Path>, _staging_dir: &Path) -> Result<()> {
        // No framework to stage; the generic loader has no [loader] source.
        Ok(())
    }

    /// Copy a mod's contents (a zip's contents, a downloaded content
    /// directory, or a single raw file) into
    /// `staging_dir/{mods_subpath}/{id}/`.
    fn stage_mod(&self, mod_file: &Path, id: &str, staging_dir: &Path) -> Result<()> {
        let dest = staging_dir.join(&self.mods_subpath).join(id);
        std::fs::create_dir_all(&dest)?;

        if mod_file.is_dir() {
            return copy_dir_contents_filtered(mod_file, &dest);
        }

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
        copy_dir_contents_filtered(&scratch, &dest)?;
        std::fs::remove_dir_all(&scratch).ok();
        Ok(())
    }

    fn mods_subpath(&self) -> String {
        self.mods_subpath.clone()
    }

    fn marker_subpath(&self) -> String {
        format!("{}/.mod-installer.marker", self.mods_subpath)
    }
}
