#!/usr/bin/env bash
# Binaries the Tauri UI launches. `cargo tauri dev` only builds nexuskvm-ui.
# Usage: scripts/build-runtime-bins.sh [--release]
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE=debug
CARGO_FLAGS=()
if [[ "${1:-}" == "--release" ]]; then
  PROFILE=release
  CARGO_FLAGS=(--release)
fi

setup_libevdev() {
  if pkg-config --exists 'libevdev >= 1.9.0' 2>/dev/null; then
    return 0
  fi
  local pc_dir="${HOME}/.local/pkgconfig"
  if [[ -f "${pc_dir}/libevdev.pc" ]]; then
    export PKG_CONFIG_PATH="${pc_dir}${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"
    local libdir
    libdir="$(pkg-config --variable=libdir libevdev 2>/dev/null || true)"
    if [[ -n "${libdir}" ]]; then
      export LIBRARY_PATH="${libdir}${LIBRARY_PATH:+:${LIBRARY_PATH}}"
    fi
  fi
  pkg-config --exists 'libevdev >= 1.9.0' 2>/dev/null
}

if ! setup_libevdev; then
  echo "No se encontró libevdev.pc (el paquete de runtime no alcanza)." >&2
  echo "Instalá: sudo apt install libevdev-dev libclang-dev" >&2
  echo "O poné un libevdev.pc en ~/.local/pkgconfig y reintentá." >&2
  if [[ -x "target/${PROFILE}/nexus-kvmd" ]]; then
    echo "Sigo con los binarios ya compilados en target/${PROFILE}." >&2
  else
    exit 1
  fi
else
  cargo build -p nexus-daemon --bin nexus-kvmd --bin nexusctl "${CARGO_FLAGS[@]}"
  cargo build -p nexus-agent "${CARGO_FLAGS[@]}"
  cargo build -p rkvm-client --manifest-path rkvm-master/Cargo.toml "${CARGO_FLAGS[@]}"
fi

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
mkdir -p src-tauri/binaries
stage() {
  local src="$1"
  local name="$2"
  if [[ -x "${src}" ]]; then
    cp -f "${src}" "src-tauri/binaries/${name}-${TRIPLE}"
    chmod +x "src-tauri/binaries/${name}-${TRIPLE}"
  fi
}

stage "target/${PROFILE}/nexus-kvmd" nexus-kvmd
stage "target/${PROFILE}/nexus-agent" nexus-agent
stage "target/${PROFILE}/nexusctl" nexusctl
stage "rkvm-master/target/${PROFILE}/rkvm-client" rkvm-client
