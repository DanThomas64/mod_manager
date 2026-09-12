mod cache;
mod github;
mod steamcmd;
mod steamworkshop;
mod thunderstore;

use anyhow::{Context, Result};
use clap::Parser;
use common::config::{GameConfig, Source};
use common::diff::diff_lockfiles;
use common::lockfile::{LockedEntry, Lockfile};
use common::ModpackConfig;
use std::path::{Path, PathBuf};

/// Downloads the latest loader and mod versions configured in modpack.toml,
/// or (with --update-server) updates a dedicated server's own game files
/// via steamcmd.
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "modpack.toml")]
    config: PathBuf,
    #[arg(long, default_value = "cache")]
    cache_dir: PathBuf,
    #[arg(long, default_value = "modpack.lock.toml")]
    lockfile: PathBuf,
    /// Only check whether newer mod/loader versions are available (no
    /// download, no cache writes, doesn't touch modpack.lock.toml). Exits 1
    /// if updates are available, 0 if everything's already up to date.
    #[arg(long)]
    check: bool,
    /// Install/update the dedicated server's own game files via steamcmd
    /// (requires game.dedicated_server_appid in config) instead of
    /// resolving/downloading mods.
    #[arg(long)]
    update_server: bool,
    /// Install directory for --update-server.
    #[arg(long)]
    server_dir: Option<PathBuf>,
}

/// Resolve one configured source's latest version, optionally downloading/
/// caching it (`download: false` is used by `--check`, which must stay
/// read-only and fast).
fn resolve_entry(
    client: &reqwest::blocking::Client,
    game: &GameConfig,
    cache_dir: &Path,
    id: &str,
    source: &Source,
    download: bool,
) -> Result<LockedEntry> {
    match source {
        Source::Thunderstore { author, name } => {
            let community = game
                .thunderstore_community
                .as_deref()
                .with_context(|| format!("{id}: game.thunderstore_community is required for thunderstore sources"))?;
            let r = thunderstore::resolve_latest(client, community, author, name)?;
            let sha256 = if download {
                let (_, hash) =
                    cache::fetch_cached(client, cache_dir, id, &r.version.to_string(), &r.filename, &r.download_url)?;
                hash
            } else {
                String::new()
            };
            Ok(LockedEntry {
                version: r.version.to_string(),
                source_url: r.download_url,
                sha256,
            })
        }
        Source::Github { owner, repo, asset_pattern } => {
            let r = github::resolve_latest(client, owner, repo, asset_pattern)?;
            let sha256 = if download {
                let (_, hash) =
                    cache::fetch_cached(client, cache_dir, id, &r.version.to_string(), &r.filename, &r.download_url)?;
                hash
            } else {
                String::new()
            };
            Ok(LockedEntry {
                version: r.version.to_string(),
                source_url: r.download_url,
                sha256,
            })
        }
        Source::Steamworkshop { workshop_id } => {
            let appid = game
                .steam_appid
                .with_context(|| format!("{id}: game.steam_appid is required for steamworkshop sources"))?;
            let r = steamworkshop::resolve_latest(client, *workshop_id)?;
            if download {
                let dest = cache_dir.join(id).join(&r.version).join("content");
                steamcmd::download_workshop_item(appid, *workshop_id, &dest)?;
            }
            Ok(LockedEntry {
                version: r.version,
                source_url: format!("https://steamcommunity.com/sharedfiles/filedetails/?id={workshop_id}"),
                sha256: "n/a (steam workshop item, multi-file)".to_string(),
            })
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = ModpackConfig::load(&args.config)?;
    let client = reqwest::blocking::Client::new();

    if args.update_server {
        let appid = config
            .game
            .dedicated_server_appid
            .context("game.dedicated_server_appid is not set in config")?;
        let server_dir = args.server_dir.context("--server-dir is required with --update-server")?;
        println!("Updating dedicated server (appid {appid}) at {}...", server_dir.display());
        steamcmd::update_server(appid, &server_dir, None, None)?;
        println!("Server update complete.");
        return Ok(());
    }

    if args.check {
        return check(&client, &config, &args.cache_dir, &args.lockfile);
    }

    println!("Resolving loader...");
    let loader_source = config
        .loader
        .source
        .as_ref()
        .context("loader.source is required (except for the generic loader with no framework)")?;
    let loader_entry = resolve_entry(&client, &config.game, &args.cache_dir, "loader", loader_source, true)?;
    println!("  -> {}", loader_entry.version);

    let mut lockfile = Lockfile {
        loader: Some(loader_entry),
        mods: Default::default(),
    };

    for mod_entry in &config.mods {
        println!("Resolving {}...", mod_entry.id);
        let entry = resolve_entry(&client, &config.game, &args.cache_dir, &mod_entry.id, &mod_entry.source, true)?;
        println!("  -> {}", entry.version);
        lockfile.mods.insert(mod_entry.id.clone(), entry);
    }

    lockfile.save(&args.lockfile)?;
    println!("Wrote {}", args.lockfile.display());
    Ok(())
}

/// Resolve the latest version of every configured entry (no download, no
/// cache writes, no lockfile write) and diff against the current
/// `modpack.lock.toml` to report whether a new release would be worth
/// cutting. Exits 1 if updates are available, 0 if everything matches.
fn check(client: &reqwest::blocking::Client, config: &ModpackConfig, cache_dir: &Path, lockfile_path: &Path) -> Result<()> {
    let current = Lockfile::load_or_default(lockfile_path)?;

    println!("Checking for updates...");
    let loader_source = config
        .loader
        .source
        .as_ref()
        .context("loader.source is required (except for the generic loader with no framework)")?;
    let loader_entry = resolve_entry(client, &config.game, cache_dir, "loader", loader_source, false)?;
    let mut candidate = Lockfile {
        loader: Some(loader_entry),
        mods: Default::default(),
    };
    for mod_entry in &config.mods {
        let entry = resolve_entry(client, &config.game, cache_dir, &mod_entry.id, &mod_entry.source, false)?;
        candidate.mods.insert(mod_entry.id.clone(), entry);
    }

    let changes = diff_lockfiles(&current, &candidate);
    if changes.is_empty() {
        println!("Up to date — no new mod or loader versions available.");
        return Ok(());
    }

    println!("Updates available:");
    for (id, version) in &changes.added {
        println!("  + {id} {version} (new)");
    }
    for (id, old, new) in &changes.changed {
        println!("  ~ {id}: {old} -> {new}");
    }
    for (id, version) in &changes.removed {
        println!("  - {id} {version} (no longer in modpack.toml)");
    }
    if let Some((old, new)) = &changes.loader_changed {
        match old {
            Some(old) => println!("  ~ loader: {old} -> {new}"),
            None => println!("  + loader {new} (new)"),
        }
    }
    println!("\nRun mod-downloader (without --check) and mod-packager to cut a new release.");
    std::process::exit(1);
}
