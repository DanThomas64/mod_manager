# Maintainer Guide

## Config schema (`modpack.toml`)

One config describes one game — there's no built-in multi-game registry. To manage a
second game, copy `modpack.toml` elsewhere and pass `--config` to the tools.

```toml
[game]
name = "Valheim"                   # display name, shown to players
steam_folder_name = "Valheim"      # steamapps/common/<this> — used for install detection
windows_exe = "valheim.exe"
linux_exe = "valheim.x86_64"
steam_appid = 892970                # optional; required for steamworkshop mod sources
dedicated_server_appid = 896660     # optional; enables `mod-downloader --update-server`
thunderstore_community = "valheim"  # optional; required for thunderstore mod sources

[loader]
type = "bepinex"                   # or "generic"
source = "thunderstore"            # loader's own source, same shape as a mod's (omit for generic with no framework)
author = "denikson"
name = "BepInExPack_Valheim"

[[mods]]
id = "some-mod"                    # stable local key; also the mods-folder subdirectory name
source = "thunderstore"            # "thunderstore" | "github" | "steamworkshop"
author = "someauthor"
name = "SomeMod"
```

A GitHub-sourced mod instead: `source = "github"`, `owner = "..."`, `repo = "..."`,
`asset_pattern = "*.zip"` (glob matched against release asset filenames). A Workshop-sourced
mod: `source = "steamworkshop"`, `workshop_id = 123456` (requires `game.steam_appid`).

### Loaders

`type = "bepinex"` is the proven path — BepInEx-based Unity/Steam games (Valheim, Risk of
Rain 2, Subnautica, and many others). It finds the `BepInEx/` folder inside the loader
package, stages it at the game root, and places each mod under `BepInEx/plugins/<id>/`.

`type = "generic"` covers games with no code-loader framework at all — mods go straight into
one configured folder. Add `mods_subpath = "mods"` (or whatever the game expects) under
`[loader]`; leave `source` unset since there's no framework package to fetch. Useful for
Workshop-only games.

Adding a third loader (e.g. MelonLoader) means implementing the `Loader` trait in
`crates/packager/src/loader/` (see `bepinex.rs` for the shape: `stage_loader`, `stage_mod`,
`mods_subpath`, `marker_subpath`) and a new `LoaderType` variant in
`crates/common/src/config.rs`. Not done here — no way to verify one without a concrete game
to test against.

## One-time setup

```sh
rustup target add x86_64-pc-windows-gnu
# Arch/CachyOS:
sudo pacman -S mingw-w64-gcc
# Debian/Ubuntu:
sudo apt install mingw-w64
```

For SteamCMD features (dedicated server updates, Workshop mod sources), install `steamcmd`
separately and make sure it's on `PATH` — see
[Valve's SteamCMD docs](https://developer.valvesoftware.com/wiki/SteamCMD). This tool
doesn't reimplement Steam's download protocol; it drives Valve's own CLI.

## Building the installer shells (only needed when installer-runtime/installer-shell code changes)

```sh
cargo build -p installer-shell --release
cargo build -p installer-shell --release --target x86_64-pc-windows-gnu
```

This produces the two reusable installer binaries that `mod-packager` embeds mod payloads
into — game-agnostic; the payload carries per-game facts (exe names, Steam folder, loader
paths) at packaging time, so the same shell binary works for any game/loader:
- `target/release/installer-shell` (Linux)
- `target/x86_64-pc-windows-gnu/release/installer-shell.exe` (Windows)

## Checking for mod updates without cutting a release

```sh
cargo run -p downloader --bin mod-downloader -- --check
```

Resolves the latest version of every configured mod/loader (no downloads, no cache writes,
doesn't touch `modpack.lock.toml`) and diffs against the current lockfile. Prints "up to
date" and exits 0 if nothing's changed, or lists what's newer and exits 1 — useful for a
quick look, or wiring into a cron/CI check, before deciding whether `./deploy.sh` is worth
running.

## Updating a dedicated server's game files via SteamCMD

```sh
cargo run -p downloader --bin mod-downloader -- --update-server --server-dir /path/to/server
```

Requires `game.dedicated_server_appid` in config. Runs
`steamcmd +force_install_dir <dir> +login anonymous +app_update <appid> validate +quit`.
This is separate from mod downloading/packaging — it updates the game server binaries
themselves, not the mods.

## Cutting a release (every time mods change)

1. Edit `modpack.toml` to add/remove/change mods.
2. From the repo root, run:
   ```sh
   ./deploy.sh
   ```
   This builds the installer shells (Linux always; Windows too if the mingw cross-compile
   target is set up, otherwise it's skipped with a message), downloads the latest mod
   versions, and packages a release — printing the release notes and file listing at the
   end, or telling you there's nothing new to release if nothing changed.
3. Share `releases/vX.Y.Z/installer-windows.exe` and `installer-linux` (and point players
   at that version's `INSTRUCTIONS.md`). For a dedicated server (e.g. AMP-managed), hand the
   host `releases/vX.Y.Z/server-plugins/` and `SERVER.md` — it's a plain, uncompressed copy of
   the mod files to copy-paste over the server's mods folder.

The individual steps `deploy.sh` runs (`cargo build -p installer-shell --release`,
`cargo run -p downloader --bin mod-downloader`, `cargo run -p packager --bin mod-packager`)
still work standalone if you want more control over any one of them.

`mod-packager` auto-bumps the version by diffing against the previous release's lockfile:
adding/removing a mod bumps minor, a version-only change bumps patch, no changes means
nothing to release. See `releases/CHANGELOG.md` for the running history.

## Repo layout

- `crates/common` — shared config/lockfile types (`GameConfig`, `LoaderConfig`, `Source`),
  semver diff logic.
- `crates/downloader` (`mod-downloader`) — resolves + caches latest loader/mod versions
  (Thunderstore, GitHub, Steam Workshop); drives SteamCMD for dedicated-server updates.
- `crates/packager` (`mod-packager`) — builds the compressed payload via the `Loader` trait
  (`crates/packager/src/loader/`), embeds it into the installer shells, handles versioning/
  changelog, exports `server-plugins/`.
- `crates/installer-runtime` — extraction, game-install detection, backup/restore logic
  shared by both installer binaries. Reads per-game facts from `.mod-installer.game`
  (baked into the payload by the packager) rather than having them compiled in.
- `crates/installer-shell` — the trivial per-target binary `mod-packager` embeds payloads
  into.
- `cache/` — downloaded mod/loader files, gitignored.
- `releases/` — per-version lockfile, changelog, instructions, installer binaries, and
  `server-plugins/`.

## Backups and restore (what end users see)

Before an install or update touches anything, the installer snapshots **every top-level
subfolder of the loader's root directory** (`BepInEx/` for the BepInEx loader, or the
configured mods folder for the generic loader) that the release is about to overwrite —
typically `core/`, `config/`, and `plugins/` for BepInEx, not just mods — into
`<loader-root>/_installer_backups/<timestamp>/` before writing anything new. If a subfolder
being backed up contains symlinks (e.g. a Vortex-style symlinked mod deployment), the backup
preserves the symlinks themselves rather than copying resolved file contents, so restoring
puts the exact previous setup back, links included.

Every packaged release bakes a small marker (`<loader-root>/.mod-installer.marker`) into
the payload recording its modpack/loader version, so a later backup can say whether what
it's backing up was installed by this tool (and which version) or came from somewhere else
(manual install, another mod manager) — shown to the user at backup time.

On startup, if backups exist, the installer lists them (origin, folders captured, file/
symlink counts) and lets the user pick one to restore instead of installing — pressing
Enter with no input still does the normal single-click install/update. Restoring itself
snapshots whatever's currently in place first, so it's symmetric with install and can never
lose data. **Backups are never deleted automatically** — cleanup under
`<loader-root>/_installer_backups/` is left entirely to the user, and the installer says so
every time it runs.
