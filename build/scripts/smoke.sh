#!/usr/bin/env bash
set -euo pipefail

# Smoke test for the cargo-generate template: the template repo itself does NOT
# `cargo check` at the root (Cargo.toml and main.rs carry Liquid placeholders),
# so the only meaningful verification surface is the GENERATED project. This
# script generates a few representative variants into a temp dir and checks each
# one — catching Liquid-rendering breakage, conditional `ignore` mistakes, and
# (most importantly) Tauri/Rust version drift before a real `make tauri-build`.
#
# Usage: build/scripts/smoke.sh   (from anywhere; needs cargo-generate + cargo)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Share one target dir across variants so the (heavy) dependency graph is
# compiled once instead of per-variant.
export CARGO_TARGET_DIR="$WORK/target"

fail() { echo "SMOKE FAIL: $*" >&2; exit 1; }

# gen_and_check <name> <expect_app:yes|no> <expect_async:true|false> <-d args...>
gen_and_check() {
  local name="$1" expect_app="$2" expect_async="$3"
  shift 3
  local dest="$WORK/$name"

  echo "=== variant: $name ($*) ==="
  ( cd "$WORK" && cargo generate --path "$ROOT_DIR" --name "$name" "$@" >/dev/null )

  # app/ presence follows with_app
  if [ "$expect_app" = yes ]; then
    test -d "$dest/app" || fail "$name: app/ expected but missing"
  else
    test ! -e "$dest/app" || fail "$name: app/ should be absent (with_app=false)"
  fi

  # ASYNC_ENABLED const rendered from with_async
  grep -q "const ASYNC_ENABLED: bool = $expect_async;" \
    "$dest/desktop/src-tauri/src/main.rs" \
    || fail "$name: ASYNC_ENABLED != $expect_async"

  # Template-only files must not leak into the generated project. README.md is the
  # template front door (GitHub landing) and must NOT ship — the dev writes their own.
  for leak in Plan cargo-generate.toml CHANGELOG.md README.md; do
    test ! -e "$dest/$leak" || fail "$name: template artifact '$leak' leaked into output"
  done

  # The app reference doc ships as TFSAPP_README.md (verbatim, generic placeholders).
  test -f "$dest/TFSAPP_README.md" || fail "$name: TFSAPP_README.md missing"
  grep -q "^## Adapter cette app" "$dest/TFSAPP_README.md" \
    || fail "$name: TFSAPP_README.md is not the app reference doc"

  # Stub the build-time Tauri resources (normally produced by build-app/sidecar)
  # so the tauri build script's existence checks pass and rustc actually runs.
  mkdir -p "$dest/desktop/src-tauri/resources/app"
  : > "$dest/desktop/src-tauri/resources/Caddyfile.desktop"
  : > "$dest/desktop/src-tauri/resources/frankenphp"

  cargo check --manifest-path "$dest/desktop/src-tauri/Cargo.toml" \
    || fail "$name: cargo check failed"
  echo "--- $name OK ---"
}

# Representative matrix (not the full cross-product — these cover every code path):
#  1. greenfield default        : app present, worker started
#  2. async disabled            : app present, worker gated off
#  3. brownfield (no base app)  : app absent, worker started
gen_and_check greenfield  yes true  -d product_name=Greenfield -d identifier=dev.local.greenfield -d with_app=true  -d with_async=true
gen_and_check asyncoff    yes false -d product_name=AsyncOff    -d identifier=dev.local.async-off  -d with_app=true  -d with_async=false
gen_and_check brownfield  no  true  -d product_name=Brownfield  -d identifier=dev.local.brownfield -d with_app=false -d with_async=true

# Validate the desktop Caddyfile if a frankenphp binary is available (skipped in
# CI without one). The Caddyfile is copied verbatim, so validating the repo copy
# once is enough.
if command -v frankenphp >/dev/null 2>&1; then
  echo "=== Caddyfile.desktop validate ==="
  APP_PORT=38124 APP_ORIGIN=http://127.0.0.1:38124 APP_PUBLIC_DIR=/tmp \
  MERCURE_JWT_SECRET=0123456789abcdef0123456789abcdef \
    frankenphp adapt --config "$ROOT_DIR/build/Caddyfile.desktop" --validate \
    || fail "Caddyfile.desktop did not validate"
  echo "--- Caddyfile OK ---"
else
  echo "(frankenphp not on PATH — skipping Caddyfile validation)"
fi

echo "SMOKE OK"
