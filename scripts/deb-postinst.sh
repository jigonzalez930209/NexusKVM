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
      log "usuario $user ya está en el grupo input"
    else
      usermod -aG input "$user" && log "usuario $user agregado al grupo input"
    fi
  else
    log "grupo input no existe en este sistema; omitiendo usermod"
  fi
}

setup_uinput() {
  if modprobe uinput 2>/dev/null; then
    log "módulo uinput cargado"
  else
    log "no se pudo cargar uinput (puede estar integrado en el kernel)"
  fi
  if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=misc --action=add || true
    udevadm trigger --subsystem-match=input --action=add || true
    udevadm trigger --name-match=uinput || true
    log "reglas udev recargadas"
  fi
}

setup_firewall() {
  if ! command -v ufw >/dev/null 2>&1; then
    return 0
  fi
  if ufw status 2>/dev/null | grep -qi 'Status: active'; then
    if ufw allow 5258/tcp comment 'NexusKVM peer connections' >/dev/null 2>&1; then
      log "ufw: puerto 5258/tcp permitido"
    else
      log "ufw activo pero no se pudo abrir 5258/tcp (revisá manualmente)"
    fi
    if ufw allow 5259/tcp comment 'NexusKVM clipboard/control' >/dev/null 2>&1; then
      log "ufw: puerto 5259/tcp permitido"
    else
      log "ufw activo pero no se pudo abrir 5259/tcp (revisá manualmente)"
    fi
  fi
}

print_notice() {
  user=${1:-}
  cat <<EOF

============================================================
 NexusKVM instalado correctamente
============================================================
EOF
  if [ -n "$user" ]; then
    cat <<EOF
 • Tu usuario ($user) fue agregado al grupo "input".
 • CERRÁ SESIÓN y volvé a entrar (o reiniciá) antes de usar la app.
EOF
  else
    cat <<EOF
 • Agregá tu usuario al grupo input:
     sudo usermod -aG input TU_USUARIO
   y después cerrá sesión.
EOF
  fi
  cat <<EOF
 • Atajo en el PC principal: Left Alt + Left Ctrl (izquierdas).
 • Guía completa: /usr/share/doc/nexuskvm/POSTINSTALL.txt
============================================================

EOF
}

case "$1" in
  configure)
    user=
    if user=$(install_user); then
      add_input_group "$user"
    else
      log "no se detectó usuario de escritorio; omitiendo usermod"
    fi
    setup_uinput
    setup_firewall
    print_notice "$user"
    ;;
esac

exit 0
