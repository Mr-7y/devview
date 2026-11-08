#!/usr/bin/env bash
# DevView — Linux 构建
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release

BIN=target/release/devview
printf '\n产物: %s  (%s)\n' "$BIN" "$(du -h "$BIN" | cut -f1)"
