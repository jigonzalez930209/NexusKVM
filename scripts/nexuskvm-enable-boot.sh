#!/bin/bash
# Enable NexusKVM as a system service so it is available at the GDM login screen.
# Usage (via pkexec): nexuskvm-enable-boot.sh <host|client> <user-data-dir>
set -euo pipefail

ROLE="${1:-}"
SRC="${2:-}"

if [ "$(id -u)" -ne 0 ]; then
  echo "must run as root (via pkexec)" >&2
  exit 1
fi

case "$ROLE" in
  host|client) ;;
  *)
    echo "usage: $0 host|client /path/to/user/data" >&2
    exit 2
    ;;
esac

if [ -z "$SRC" ] || [ ! -d "$SRC" ]; then
  echo "missing user data dir: $SRC" >&2
  exit 2
fi

LIB=/var/lib/nexuskvm
BIN_DIR=/usr/libexec/nexuskvm

install -d -m 0750 "$LIB"
install -d -m 0755 "$BIN_DIR"

# Prefer packaged binaries; fall back to PATH for development installs.
copy_bin() {
  name="$1"
  dest="$BIN_DIR/$name"
  for candidate in \
    "/usr/libexec/nexuskvm/$name" \
    "/usr/bin/$name" \
    "$(command -v "$name" 2>/dev/null || true)"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      if [ "$candidate" != "$dest" ]; then
        install -Dm755 "$candidate" "$dest"
      fi
      return 0
    fi
  done
  echo "binary not found: $name" >&2
  return 1
}

# Always refresh the unit file so Group=input / socket permissions stay correct.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_UNIT_HOST="${SCRIPT_DIR}/../systemd/nexuskvm-host.service"
REPO_UNIT_CLIENT="${SCRIPT_DIR}/../systemd/nexuskvm-client.service"
PKG_UNIT_HOST=/usr/lib/systemd/system/nexuskvm-host.service
PKG_UNIT_CLIENT=/usr/lib/systemd/system/nexuskvm-client.service

install_unit() {
  name="$1"
  repo="$2"
  pkg="$3"
  if [ -f "$repo" ]; then
    install -Dm644 "$repo" "$pkg"
  elif [ -f "$pkg" ]; then
    :
  else
    echo "unit file missing for $name" >&2
    return 1
  fi
}

# Disable the opposite role so a machine stays host XOR client.
systemctl disable --now nexuskvm-host.service 2>/dev/null || true
systemctl disable --now nexuskvm-client.service 2>/dev/null || true

# Stop session-spawned copies so the system unit can bind the port.
pkill -x nexus-kvmd 2>/dev/null || true
pkill -x rkvm-client 2>/dev/null || true
sleep 0.5

# Desktop user must reach /run/nexuskvm (unit uses Group=input).
install -d -m 0750 -o root -g input /run/nexuskvm 2>/dev/null || install -d -m 0750 /run/nexuskvm
chgrp input /var/lib/nexuskvm 2>/dev/null || true
chmod 0750 /var/lib/nexuskvm 2>/dev/null || true

if [ "$ROLE" = host ]; then
  copy_bin nexus-kvmd
  # Sync TLS + daemon config; rewrite socket/cert paths for the system copy.
  install -m 0640 -o root -g input "$SRC/password" "$LIB/password"
  install -m 0640 -o root -g input "$SRC/certificate.pem" "$LIB/certificate.pem"
  install -m 0600 -o root -g root "$SRC/key.pem" "$LIB/key.pem"
  PASS=$(tr -d '\n' <"$LIB/password")
  cat >"$LIB/daemon.toml" <<EOF
socket = "/run/nexuskvm/control.sock"
listen = "0.0.0.0:5258"
switch-keys = ["left-alt", "left-ctrl"]
certificate = "$LIB/certificate.pem"
key = "$LIB/key.pem"
password = "$PASS"
EOF
  chgrp input "$LIB/daemon.toml" 2>/dev/null || true
  chmod 0640 "$LIB/daemon.toml"
  install_unit host "$REPO_UNIT_HOST" "$PKG_UNIT_HOST"
  systemctl daemon-reload
  systemctl enable --now nexuskvm-host.service
  # Ensure socket dir group after RuntimeDirectory is created.
  chgrp input /run/nexuskvm 2>/dev/null || true
  chmod 0770 /run/nexuskvm 2>/dev/null || true
  echo "nexuskvm-host.service enabled"
else
  copy_bin rkvm-client
  install -m 0640 -o root -g input "$SRC/certificate.pem" "$LIB/certificate.pem"
  if [ -f "$SRC/password" ]; then
    install -m 0640 -o root -g input "$SRC/password" "$LIB/password"
  fi
  # Rewrite client.toml paths to the system store.
  SERVER=$(awk -F'= *' '/^server/{gsub(/"/,"",$2); print $2; exit}' "$SRC/client.toml")
  PASS=$(awk -F'= *' '/^password/{gsub(/"/,"",$2); print $2; exit}' "$SRC/client.toml")
  if [ -z "$PASS" ] && [ -f "$LIB/password" ]; then
    PASS=$(tr -d '\n' <"$LIB/password")
  fi
  cat >"$LIB/client.toml" <<EOF
server = "$SERVER"
certificate = "$LIB/certificate.pem"
password = "$PASS"
EOF
  chgrp input "$LIB/client.toml" 2>/dev/null || true
  chmod 0640 "$LIB/client.toml"
  install_unit client "$REPO_UNIT_CLIENT" "$PKG_UNIT_CLIENT"
  systemctl daemon-reload
  systemctl enable --now nexuskvm-client.service
  echo "nexuskvm-client.service enabled"
fi
