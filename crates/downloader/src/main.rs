mod cache;
mod github;
mod thunderstore;

use anyhow::Result;
use clap::Parser;
use common::config::Source;
use common::lockfile::{LockedEntry, Lockfile};
use common::ModpackConfig;
use std::path::PathBuf;

/// Downloads the latest BepInEx and mod versions configured in modpack.toml.
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "modpack.toml")]
    config: PathBuf,
    #[arg(long, default_value = "cache")]
    cache_dir: PathBuf,
    #[arg(long, default_value = "modpack.lock.toml")]
    lockfile: PathBuf,
}

struct Resolved {
    version: semver::Version,
    download_url: String,
    filename: String,
}

fn resolve(client: &reqwest::blocking::Client, source: &Source) -> Result<Resolved> {
    match source {
        Source::Thunderstore { author, name } => {
            let r = thunderstore::resolve_latest(client, author, name)?;
            Ok(Resolved {
                version: r.version,
                download_url: r.download_url,
                filename: r.filename,
            })
        }
        Source::Github {
            owner,
            repo,
            asset_pattern,
        } => {
            let r = github::resolve_latest(client, owner, repo, asset_pattern)?;
            Ok(Resolved {
                version: r.version,
                download_url: r.download_url,
                filename: r.filename,
            })
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = ModpackConfig::load(&args.config)?;
    let client = reqwest::blocking::Client::new();

    println!("Resolving BepInEx...");
    let bepinex_resolved = resolve(&client, &config.bepinex)?;
    println!("  -> {}", bepinex_resolved.version);
    let (_, bepinex_hash) = cache::fetch_cached(
        &client,
        &args.cache_dir,
        "bepinex",
        &bepinex_resolved.version.to_string(),
        &bepinex_resolved.filename,
        &bepinex_resolved.download_url,
    )?;

    let mut lockfile = Lockfile {
        bepinex: Some(LockedEntry {
            version: bepinex_resolved.version,
            source_url: bepinex_resolved.download_url,
            sha256: bepinex_hash,
        }),
        mods: Default::default(),
    };

    for mod_entry in &config.mods {
        println!("Resolving {}...", mod_entry.id);
        let resolved = resolve(&client, &mod_entry.source)?;
        println!("  -> {}", resolved.version);
        let (_, hash) = cache::fetch_cached(
            &client,
            &args.cache_dir,
            &mod_entry.id,
            &resolved.version.to_string(),
            &resolved.filename,
            &resolved.download_url,
        )?;
        lockfile.mods.insert(
            mod_entry.id.clone(),
            LockedEntry {
                version: resolved.version,
                source_url: resolved.download_url,
                sha256: hash,
            },
        );
    }

    lockfile.save(&args.lockfile)?;
    println!("Wrote {}", args.lockfile.display());
    Ok(())
}
