#!/usr/bin/env bash
# Build Linux release packages (.deb, .AppImage, .rpm) with tray/appindicator detection fixed.
set -euo pipefail
cd "$(dirname "$0")/.."

# shellcheck disable=SC1091
source scripts/ensure-tray-pkgconfig.sh

BUNDLES="${1:-deb,appimage,rpm}"
shift || true

echo "Building Linux packages: ${BUNDLES}..."
exec npx tauri build --bundles "${BUNDLES}" "$@"
