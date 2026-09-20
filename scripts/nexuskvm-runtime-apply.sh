#!/bin/sh
# Mandatory runtime apply for NexusKVM package install/upgrade.
#
# - Mirrors packaged binaries into /usr/libexec/nexuskvm (system services).
# - Kills stale session-spawned processes so the new binaries are the ones
#   that actually run.
# - Restarts enabled system units and verifies they stay active.
# - Verifies deploy markers so an old daemon/agent can never pass silently.
#
# Runs as root. NEXUSKVM_POSTINSTALL_STRICT=0 turns failures into warnings.
set -u

LIB=/usr/libexec/nexuskvm
USRBIN=/usr/bin
ERR=0

log() { echo "nexuskvm: $*"; }

err() {
  log "ERROR: $*"
  ERR=1
}

strict_finish() {
  if [ "$ERR" -eq 1 ]; then
    if [ "${NEXUSKVM_POSTINSTALL_STRICT:-1}" = "1" ]; then
      log "runtime apply FAILED (set NEXUSKVM_POSTINSTALL_STRICT=0 to warn only)"
      exit 1
    fi
    log "runtime apply finished with warnings"
    exit 0
  fi
  log "runtime apply OK"
  exit 0
}

if [ "$(id -u)" -ne 0 ]; then
  echo "nexuskvm: runtime apply must run as root" >&2
  exit 1
fi

install -d -m 0755 "$LIB" || err "cannot create $LIB"

mirror() {
  name=$1
  required=$2
  src="$USRBIN/$name"
  dst="$LIB/$name"
  if [ -x "$src" ]; then
    install -Dm755 "$src" "$dst" || { err "failed to install $dst"; return; }
    log "synced $dst"
  elif [ "$required" = "required" ]; then
    err "missing packaged binary: $src"
  else
    log "optional binary not present: $src"
  fi
}

mirror nexus-kvmd required
mirror rkvm-client required
mirror nexus-agent optional
mirror nexusctl optional

if [ -f "$LIB/nexuskvm-enable-boot.sh" ]; then
  chmod 0755 "$LIB/nexuskvm-enable-boot.sh" 2>/dev/null \
    || err "cannot chmod $LIB/nexuskvm-enable-boot.sh"
fi

find_bin() {
  name=$1
  for c in "$USRBIN/$name" "$LIB/$name"; do
    if [ -x "$c" ]; then
      printf '%s\n' "$c"
      return 0
    fi
  done
  if command -v "$name" >/dev/null 2>&1; then
    command -v "$name"
    return 0
  fi
  found=$(find /usr/bin /usr/lib /usr/libexec -maxdepth 3 -type f -name "$name" -print -quit 2>/dev/null)
  if [ -n "$found" ]; then
    printf '%s\n' "$found"
    return 0
  fi
  return 1
}

verify_marker() {
  bin=$1
  marker=$2
  label=$3
  if [ -z "$bin" ] || [ ! -x "$bin" ]; then
    err "missing binary for $label"
    return
  fi
  if grep -aq "$marker" "$bin"; then
    log "verified $label ($bin)"
  else
    err "$label missing marker '$marker' in $bin (stale build?)"
  fi
}

verify_marker "$LIB/nexus-kvmd" "ipc features: ipc-next" "daemon Next protocol"
verify_marker "$(find_bin nexus-agent || true)" "edge-strip mode: InputCapture portal disabled" "portal-free agent"

if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
  systemctl daemon-reload || err "systemctl daemon-reload failed"

  HOST_ENABLED=0
  CLIENT_ENABLED=0
  if systemctl is-enabled nexuskvm-host.service >/dev/null 2>&1; then
    HOST_ENABLED=1
  fi
  if systemctl is-enabled nexuskvm-client.service >/dev/null 2>&1; then
    CLIENT_ENABLED=1
  fi

  # Kill session-spawned copies (UI/agent) so they cannot keep old code or
  # hold the portal open after the upgrade.
  pkill -x nexus-agent 2>/dev/null || true
  pkill -x nexus-kvmd 2>/dev/null || true
  pkill -x rkvm-client 2>/dev/null || true
  sleep 0.3

  if [ "$HOST_ENABLED" -eq 1 ]; then
    systemctl restart nexuskvm-host.service || err "nexuskvm-host.service restart failed"
    systemctl is-active --quiet nexuskvm-host.service \
      || err "nexuskvm-host.service is not active after restart"
  fi
  if [ "$CLIENT_ENABLED" -eq 1 ]; then
    systemctl restart nexuskvm-client.service || err "nexuskvm-client.service restart failed"
    systemctl is-active --quiet nexuskvm-client.service \
      || err "nexuskvm-client.service is not active after restart"
  fi
else
  log "systemd not running; skipped unit restart"
fi

strict_finish
