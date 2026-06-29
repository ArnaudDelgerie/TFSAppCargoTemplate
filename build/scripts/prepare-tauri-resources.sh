#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

mkdir -p "$ROOT_DIR/desktop/src-tauri/resources"
cp "$ROOT_DIR/build/Caddyfile.desktop" "$ROOT_DIR/desktop/src-tauri/resources/Caddyfile.desktop"

test -d "$ROOT_DIR/desktop/src-tauri/resources/app/public"
test -f "$ROOT_DIR/desktop/src-tauri/resources/app/vendor/autoload_runtime.php"
test -f "$ROOT_DIR/desktop/src-tauri/resources/app/public/build/entrypoints.json"
test -f "$ROOT_DIR/desktop/src-tauri/resources/Caddyfile.desktop"
test -x "$ROOT_DIR/desktop/src-tauri/resources/frankenphp"
