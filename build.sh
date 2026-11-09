#!/usr/bin/env bash
# Ai-View — Linux 构建
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release

BIN=target/release/ai-view
printf '\n产物: %s  (%s)\n' "$BIN" "$(du -h "$BIN" | cut -f1)"
