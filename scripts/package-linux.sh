#!/usr/bin/env bash
# Build the Linux .deb with tray/appindicator detection fixed for Ubuntu.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
source scripts/ensure-tray-pkgconfig.sh
exec npx tauri build --bundles deb "$@"
