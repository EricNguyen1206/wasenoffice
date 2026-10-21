#!/bin/bash
# web-dev.sh — Web dev mode (T-068, DD docs/design/web-dev-mode/DD.md).
#
# Boots the app in the BACKGROUND (rule 6b: never takes focus) with the HTTP
# invoke bridge ON, so http://127.0.0.1:1420/ opened in ANY browser — ZCode
# browser-use, Chrome — drives the real Rust command layer: start page,
# Quick View, Nano Light (edit + save), Settings. The engine editor (sdkjs)
# stays Tauri-only and is refused with a clear message (DD W01 §5 ceilings).
#
# Usage: harness/scripts/web-dev.sh          # Ctrl-C stops the whole tree
#        RUST_LOG=debug harness/scripts/web-dev.sh   # per-invoke `web bridge ←` lines
# The browser gets no native picker: use "or paste a path…" on the start page.
set -u
cd "$(dirname "$0")/../.."

if lsof -nP -iTCP:1420 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "web-dev: port 1420 already has a listener (another vite/tauri dev?) — stop it first" >&2
  exit 1
fi
echo "web-dev: booting with WASEN_OFFICE_WEB_BRIDGE=1 (background) → open http://localhost:1420/ (vite binds localhost, not 127.0.0.1)"
exec env WASEN_OFFICE_WEB_BRIDGE=1 WASEN_OFFICE_BACKGROUND=1 RUST_LOG="${RUST_LOG:-info}" pnpm tauri dev
