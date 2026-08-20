#!/usr/bin/env bash
set -euo pipefail
cargo build --workspace --release
sudo groupadd -f nexuskvm
id -u nexuskvm >/dev/null 2>&1 || sudo useradd --system --gid nexuskvm --home /var/lib/nexuskvm --shell /usr/sbin/nologin nexuskvm
sudo install -Dm755 target/release/nexus-kvmd /usr/libexec/nexuskvm/nexus-kvmd
sudo install -Dm755 target/release/nexusctl /usr/bin/nexusctl
sudo install -Dm755 target/release/nexus-agent /usr/bin/nexus-agent
sudo install -Dm644 systemd/nexus-kvmd.service /usr/lib/systemd/system/nexus-kvmd.service
sudo install -Dm644 systemd/nexus-agent.service /usr/lib/systemd/user/nexus-agent.service
sudo install -Dm644 udev/70-nexuskvm-uinput.rules /usr/lib/udev/rules.d/70-nexuskvm-uinput.rules
sudo systemctl daemon-reload
sudo udevadm control --reload-rules
echo 'Installed. Enable the daemon only after testing mock mode.'
