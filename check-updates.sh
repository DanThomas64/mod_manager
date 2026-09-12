#!/usr/bin/env bash
# Maintainer update-check script: resolves the latest mod/BepInEx versions
# and diffs them against modpack.lock.toml, without downloading anything or
# cutting a release. Exits 1 if updates are available (see docs/MAINTAINER.md),
# 0 if everything's already up to date. Safe to run from cron/CI.
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "$REPO_ROOT"

cargo run -p downloader --bin mod-downloader -- --check
