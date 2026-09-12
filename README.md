# Mod Installer

A small Rust toolchain that turns a list of Thunderstore/GitHub/Steam Workshop mods into a
single, shareable, double-click installer for a Steam game's modpack — no manual
DLL-juggling for your players. Works with any Steam game via a `[game]` config section, and
any mod loader via a pluggable loader (BepInEx included; a `generic` loader covers games
with no code loader at all). Valheim + BepInEx is the shipped example (`modpack.toml`), not
a hardcoded assumption.

## What it does

- **Resolves and downloads** the latest loader and mod versions from Thunderstore, GitHub
  Releases, or Steam Workshop, based on a plain `modpack.toml` you maintain — and can drive
  **SteamCMD** to install/update a dedicated server's own game files headlessly.
- **Packages** everything into a single self-extracting binary per OS
  (`installer-windows.exe`, `installer-linux`) — a player just runs it and it finds their
  game install (or asks) and drops the mods in the right place. No dependencies needed on
  their machine.
- **Auto-versions** each release by diffing against the last one (mod added/removed →
  minor bump, version change → patch bump) and writes a changelog summarizing exactly what
  changed, mod by mod.
- **Backs up before touching anything** — every folder an update is about to overwrite
  (core, config, mods) is snapshotted first, symlinks and all (so a Vortex-style symlinked
  mod setup survives a backup/restore round-trip intact). The installer can list and restore
  any previous backup on a later run. Nothing is ever deleted automatically.
- **Exports a plain `server-plugins/` folder** alongside each release, for copy-pasting
  onto a dedicated server (e.g. AMP-managed) without needing the client installer at all.
- **Checks for mod updates** on demand (no download/package cycle) so you know at a glance
  whether a new release is worth cutting.
- Optimized for size: mod payloads are zstd-compressed and embedded directly in the
  installer binary, so the whole thing stays small enough to send over Discord/email.

## Quick start

**Maintaining a modpack?** See [`docs/MAINTAINER.md`](docs/MAINTAINER.md) — config schema,
one-time setup, cutting a release, adding a new game/loader, repo layout.

**Got a release from someone?** See the `INSTRUCTIONS.md` (players) or `SERVER.md`
(dedicated server hosts) inside whatever `releases/vX.Y.Z/` folder you were handed.

## How it's put together

| Crate | Role |
|---|---|
| `common` | Shared config/lockfile types, semver diff logic. |
| `downloader` (`mod-downloader`) | Resolves + caches the latest loader/mod versions; drives SteamCMD for dedicated-server updates and Workshop items. |
| `packager` (`mod-packager`) | Builds the compressed payload via a pluggable `Loader` (BepInEx, generic), embeds it into the installer shells, handles versioning/changelog/server export. |
| `installer-runtime` | Extraction, game-install detection, backup/restore — shared logic linked into both installer binaries. Game-specific facts (exe names, Steam folder, loader paths) travel in the payload, not compiled in. |
| `installer-shell` | The trivial per-target binary `mod-packager` embeds payloads into — game-agnostic, built once, reused across every game/release. |

See [`docs/MAINTAINER.md`](docs/MAINTAINER.md) for the full breakdown, config schema, build
commands, and backup/restore behavior in detail.
