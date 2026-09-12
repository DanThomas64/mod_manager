mod bepinex;
mod generic;

use anyhow::Result;
use common::LoaderType;
use std::path::Path;

/// A pluggable mod-loader strategy: how to stage the loader framework itself
/// and each mod into the payload's staging directory, and where (relative to
/// the game root) mods/the install marker end up living — needed by
/// `installer-runtime` at install time, so it travels in the payload as
/// `.mod-installer.game` rather than being hardcoded per loader there.
pub trait Loader {
    /// Stage the loader framework's own files into the root of `staging_dir`.
    /// `loader_zip` is `None` when this loader has no separate framework
    /// package configured (e.g. a `generic` loader with no `[loader]`
    /// source).
    fn stage_loader(&self, loader_zip: Option<&Path>, staging_dir: &Path) -> Result<()>;
    /// Stage one mod (an unzipped archive or a raw file) under this loader's
    /// convention.
    fn stage_mod(&self, mod_file: &Path, id: &str, staging_dir: &Path) -> Result<()>;
    /// Path (relative to the game root) mods live under, e.g. `"BepInEx/plugins"`.
    fn mods_subpath(&self) -> String;
    /// Path (relative to the game root) the install marker lives at.
    fn marker_subpath(&self) -> String;
}

pub fn for_config(loader_type: LoaderType, mods_subpath: Option<&str>) -> Result<Box<dyn Loader>> {
    match loader_type {
        LoaderType::Bepinex => Ok(Box::new(bepinex::BepinexLoader)),
        LoaderType::Generic => {
            let mods_subpath = mods_subpath
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("the generic loader requires loader.mods_subpath in config"))?;
            Ok(Box::new(generic::GenericLoader { mods_subpath }))
        }
    }
}
