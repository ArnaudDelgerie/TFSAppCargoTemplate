#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RES_DIR="$ROOT_DIR/desktop/src-tauri/resources"
mkdir -p "$RES_DIR"

# Pick the right release asset for the host. The sidecar ships as a bundled
# resource (see tauri.conf.json), so it lands under /usr/lib/<ProductName>/resources
# instead of /usr/bin — no collision between two TFS apps on the same machine.
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)  ASSET="frankenphp-linux-x86_64" ;;
  Darwin-arm64)  ASSET="frankenphp-mac-arm64" ;;
  Darwin-x86_64) ASSET="frankenphp-mac-x86_64" ;;
  *)
    echo "Unsupported host. Download FrankenPHP manually into $RES_DIR/frankenphp." >&2
    exit 1
    ;;
esac

DEST="$RES_DIR/frankenphp"
if [[ -x "$DEST" ]]; then
  echo "FrankenPHP sidecar already exists: $DEST"
  exit 0
fi

URL="https://github.com/dunglas/frankenphp/releases/latest/download/$ASSET"
curl -L "$URL" -o "$DEST"
chmod +x "$DEST"
echo "Downloaded $DEST"
