#!/bin/bash
# Dev hotfix: install freshly built binaries and force the same mandatory
# runtime apply the package postinstall runs (mirror + kill stale + restart +
# verify markers). Keep this thin so package and dev flows cannot drift.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
install -Dm755 "$ROOT/target/release/nexus-kvmd" /usr/libexec/nexuskvm/nexus-kvmd
install -Dm755 "$ROOT/target/release/nexus-kvmd" /usr/bin/nexus-kvmd
install -Dm755 "$ROOT/target/release/nexus-agent" /usr/bin/nexus-agent
install -Dm755 "$ROOT/target/release/nexusctl" /usr/bin/nexusctl
install -Dm755 "$ROOT/rkvm-master/target/release/rkvm-client" /usr/bin/rkvm-client
install -Dm644 "$ROOT/systemd/nexuskvm-host.service" /usr/lib/systemd/system/nexuskvm-host.service
install -Dm644 "$ROOT/systemd/nexuskvm-client.service" /usr/lib/systemd/system/nexuskvm-client.service
install -Dm755 "$ROOT/scripts/nexuskvm-enable-boot.sh" /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh
install -Dm755 "$ROOT/scripts/nexuskvm-runtime-apply.sh" /usr/libexec/nexuskvm/nexuskvm-runtime-apply.sh
# Keep switch chord from leaking modifiers to either PC.
if [ -f /var/lib/nexuskvm/daemon.toml ]; then
  if ! grep -q '^propagate-switch-keys' /var/lib/nexuskvm/daemon.toml; then
    sed -i '/^switch-keys/a propagate-switch-keys = false' /var/lib/nexuskvm/daemon.toml || true
  else
    sed -i 's/^propagate-switch-keys.*/propagate-switch-keys = false/' /var/lib/nexuskvm/daemon.toml || true
  fi
fi
/bin/sh /usr/libexec/nexuskvm/nexuskvm-runtime-apply.sh
echo "hotfix installed; reopen NexusKVM on both PCs"
