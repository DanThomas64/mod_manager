mod archive;
mod embed;
mod fsutil;
mod loader;
mod release;

use anyhow::{Context, Result};
use clap::Parser;
use common::diff::diff_lockfiles;
use common::lockfile::Lockfile;
use common::ModpackConfig;
use std::path::PathBuf;

/// Packages the mods resolved by mod-downloader into self-extracting
/// per-OS installer binaries, with auto-versioning and release notes.
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "modpack.toml")]
    config: PathBuf,
    #[arg(long, default_value = "modpack.lock.toml")]
    lockfile: PathBuf,
    #[arg(long, default_value = "cache")]
    cache_dir: PathBuf,
    #[arg(long, default_value = "releases")]
    releases_dir: PathBuf,
    /// Prebuilt installer-shell binary for the Linux target.
    #[arg(long, default_value = "target/release/installer-shell")]
    shell_linux: PathBuf,
    /// Prebuilt installer-shell binary for the Windows target (x86_64-pc-windows-gnu).
    #[arg(long, default_value = "target/x86_64-pc-windows-gnu/release/installer-shell.exe")]
    shell_windows: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = ModpackConfig::load(&args.config)?;
    let mod_loader = loader::for_config(config.loader.loader_type, config.loader.mods_subpath.as_deref())?;

    let new_lockfile = Lockfile::load(&args.lockfile)
        .with_context(|| format!("loading {} (run mod-downloader first)", args.lockfile.display()))?;
    let loader_version = new_lockfile
        .loader
        .as_ref()
        .context("lockfile has no loader entry")?
        .version
        .clone();

    std::fs::create_dir_all(&args.releases_dir)?;
    let (previous_version, previous_lockfile) = release::load_previous(&args.releases_dir)?;
    let changes = diff_lockfiles(&previous_lockfile, &new_lockfile);

    let Some(version) = release::next_version(&previous_version, &changes) else {
        println!(
            "No changes since v{} — nothing to release.",
            previous_version.unwrap()
        );
        return Ok(());
    };

    println!("Packaging release v{version}...");
    let release_dir = args.releases_dir.join(format!("v{version}"));
    std::fs::create_dir_all(&release_dir)?;

    let date = current_date();
    let staging_dir = std::env::temp_dir().join("mod-packager-staging");
    let payload = archive::build_payload(
        &args.cache_dir,
        &new_lockfile,
        &staging_dir,
        &version,
        &date,
        &config.game,
        mod_loader.as_ref(),
    )
    .context("building mod payload")?;
    println!(
        "Payload built: {:.1} MiB compressed",
        payload.len() as f64 / (1024.0 * 1024.0)
    );

    let server_plugins_dir = release_dir.join("server-plugins");
    archive::export_server_plugins(&staging_dir, &server_plugins_dir, mod_loader.as_ref())
        .context("exporting server plugins")?;
    println!("  wrote server-plugins/ (for dedicated server hosts)");

    std::fs::remove_dir_all(&staging_dir).ok();

    if args.shell_linux.exists() {
        embed::embed_payload(&args.shell_linux, &payload, &release_dir.join("installer-linux"))
            .context("embedding Linux installer")?;
        println!("  wrote installer-linux");
    } else {
        println!(
            "  skipping Linux installer: {} not found (build it with `cargo build -p installer-shell --release`)",
            args.shell_linux.display()
        );
    }

    if args.shell_windows.exists() {
        embed::embed_payload(
            &args.shell_windows,
            &payload,
            &release_dir.join("installer-windows.exe"),
        )
        .context("embedding Windows installer")?;
        println!("  wrote installer-windows.exe");
    } else {
        println!(
            "  skipping Windows installer: {} not found (see docs/MAINTAINER.md for the mingw cross-compile setup)",
            args.shell_windows.display()
        );
    }

    new_lockfile.save(release_dir.join("modpack.lock.toml"))?;

    let entry = release::render_changelog_entry(&version, &loader_version, &changes, &date);
    release::prepend_changelog(&args.releases_dir, &entry)?;
    std::fs::write(release_dir.join("RELEASE_NOTES.md"), &entry)?;

    let instructions =
        release::render_instructions(&version, &loader_version, &config.game, config.loader.loader_type);
    std::fs::write(release_dir.join("INSTRUCTIONS.md"), instructions)?;

    let server_instructions =
        release::render_server_instructions(&version, &loader_version, &config.game, &mod_loader.mods_subpath());
    std::fs::write(release_dir.join("SERVER.md"), server_instructions)?;

    release::update_latest_pointer(&args.releases_dir, &version)?;

    println!("Release v{version} written to {}", release_dir.display());
    Ok(())
}

fn current_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
