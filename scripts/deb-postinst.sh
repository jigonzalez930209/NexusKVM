#!/bin/sh
# Runs as root during: dpkg -i / apt install
set -e

log() {
  echo "nexuskvm: $*"
}

install_user() {
  if [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != root ]; then
    printf '%s\n' "$SUDO_USER"
    return 0
  fi
  if command -v logname >/dev/null 2>&1; then
    u=$(logname 2>/dev/null || true)
    if [ -n "$u" ] && [ "$u" != root ] && id "$u" >/dev/null 2>&1; then
      printf '%s\n' "$u"
      return 0
    fi
  fi
  return 1
}

add_input_group() {
  user=$1
  if getent group input >/dev/null 2>&1; then
    if id -nG "$user" 2>/dev/null | tr ' ' '\n' | grep -qx input; then
      log "user $user is already in the input group"
    else
      usermod -aG input "$user" && log "user $user added to the input group"
    fi
  else
    log "input group does not exist on this system; skipping usermod"
  fi
}

setup_uinput() {
  if modprobe uinput 2>/dev/null; then
    log "uinput module loaded"
  else
    log "could not load uinput (may be built into the kernel)"
  fi
  if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=misc --action=add || true
    udevadm trigger --subsystem-match=input --action=add || true
    udevadm trigger --name-match=uinput || true
    log "udev rules reloaded"
  fi
}

setup_firewall() {
  if ! command -v ufw >/dev/null 2>&1; then
    return 0
  fi
  if ufw status 2>/dev/null | grep -qi 'Status: active'; then
    if ufw allow 5258/tcp comment 'NexusKVM peer connections' >/dev/null 2>&1; then
      log "ufw: port 5258/tcp allowed"
    else
      log "ufw is active but could not open 5258/tcp (check manually)"
    fi
    if ufw allow 5259/tcp comment 'NexusKVM clipboard/control' >/dev/null 2>&1; then
      log "ufw: port 5259/tcp allowed"
    else
      log "ufw is active but could not open 5259/tcp (check manually)"
    fi
  fi
}

print_notice() {
  user=${1:-}
  cat <<EOF

============================================================
 NexusKVM installed successfully
============================================================
EOF
  if [ -n "$user" ]; then
    cat <<EOF
 • Your user ($user) was added to the "input" group.
 • LOG OUT and log back in (or reboot) before using the app.
EOF
  else
    cat <<EOF
 • Add your user to the input group:
     sudo usermod -aG input YOUR_USER
   and then log out.
EOF
  fi
  cat <<EOF
 • Shortcut on the primary PC: Left Alt + Left Ctrl (left keys).
 • Full guide: /usr/share/doc/nexuskvm/POSTINSTALL.txt
============================================================

EOF
}

case "$1" in
  configure)
    user=
    if user=$(install_user); then
      add_input_group "$user"
    else
      log "no desktop user detected; skipping usermod"
    fi
    setup_uinput
    setup_firewall
    print_notice "$user"
    ;;
esac

exit 0
