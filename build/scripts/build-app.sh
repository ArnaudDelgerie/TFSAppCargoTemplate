#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_DIR="$ROOT_DIR/app"
RESOURCE_DIR="$ROOT_DIR/desktop/src-tauri/resources/app"

docker compose run --rm -e APP_ENV=prod -e APP_DEBUG=0 app composer install --no-dev --no-scripts --optimize-autoloader
docker compose run --rm node npm install
docker compose run --rm node npm run build
docker compose run --rm -e APP_ENV=prod -e APP_DEBUG=0 app frankenphp php-cli bin/console cache:clear --env=prod --no-debug
docker compose run --rm -e APP_ENV=prod -e APP_DEBUG=0 app frankenphp php-cli bin/console cache:warmup --env=prod --no-debug

rm -rf "$RESOURCE_DIR"
mkdir -p "$RESOURCE_DIR"
rsync -a \
  --exclude='.env.local' \
  --exclude='node_modules' \
  --exclude='var/cache' \
  --exclude='var/data/*.db' \
  --exclude='var/data/*.db-*' \
  --exclude='var/log' \
  "$APP_DIR/" "$RESOURCE_DIR/"

mkdir -p "$RESOURCE_DIR/var/data" "$RESOURCE_DIR/var/cache" "$RESOURCE_DIR/var/log"
