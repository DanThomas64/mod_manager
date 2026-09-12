#!/usr/bin/env bash
# Maintainer deploy script: builds the installer shells (if needed), resolves
# and downloads the latest mod versions, and packages a new release.
# Linux is the primary target for this script; the Windows installer shell
# is built too if the mingw cross-compile target is set up (see
# docs/MAINTAINER.md), otherwise that step is skipped with a clear message.
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "$REPO_ROOT"

echo "==> Building installer-shell (Linux)"
cargo build -p installer-shell --release

if rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$'; then
    echo "==> Building installer-shell (Windows, x86_64-pc-windows-gnu)"
    cargo build -p installer-shell --release --target x86_64-pc-windows-gnu
else
    echo "==> Skipping Windows installer-shell build: x86_64-pc-windows-gnu target not installed."
    echo "    Run: rustup target add x86_64-pc-windows-gnu (and install mingw-w64) to enable it."
    echo "    See docs/MAINTAINER.md for details."
fi

echo "==> Resolving + downloading latest mod versions"
cargo run -p downloader --bin mod-downloader

LATEST_FILE="$REPO_ROOT/releases/latest.txt"
PREV_VERSION=""
if [[ -f "$LATEST_FILE" ]]; then
    PREV_VERSION="$(cat "$LATEST_FILE")"
fi

echo "==> Packaging release"
cargo run -p packager --bin mod-packager

NEW_VERSION=""
if [[ -f "$LATEST_FILE" ]]; then
    NEW_VERSION="$(cat "$LATEST_FILE")"
fi

if [[ -z "$NEW_VERSION" ]]; then
    echo
    echo "No release exists yet (mod-packager needs modpack.lock.toml from mod-downloader to have run)."
    exit 1
fi

if [[ "$NEW_VERSION" == "$PREV_VERSION" ]]; then
    echo
    echo "==> No changes since v$NEW_VERSION — nothing new to release."
    exit 0
fi

RELEASE_DIR="$REPO_ROOT/releases/v$NEW_VERSION"
echo
echo "==> Release v$NEW_VERSION ready:"
ls -la "$RELEASE_DIR"
echo
echo "--- Release notes ---"
cat "$RELEASE_DIR/RELEASE_NOTES.md"
