#!/usr/bin/env bash
# Tauri's Linux bundler panics without an appindicator pkg-config entry
# ("Can't detect any appindicator library"), even when only the runtime .so
# is installed. Prefer the real -dev package; otherwise synthesize a stub .pc.
set -euo pipefail

export TAURI_LINUX_AYATANA_APPINDICATOR="${TAURI_LINUX_AYATANA_APPINDICATOR:-1}"

if pkg-config --exists ayatana-appindicator3-0.1 2>/dev/null \
  || pkg-config --exists appindicator3-0.1 2>/dev/null; then
  return 0 2>/dev/null || exit 0
fi

find_libdir() {
  local so
  for so in \
    /usr/lib/x86_64-linux-gnu/libayatana-appindicator3.so.1 \
    /usr/lib/aarch64-linux-gnu/libayatana-appindicator3.so.1 \
    /usr/lib/libayatana-appindicator3.so.1 \
    /usr/lib/x86_64-linux-gnu/libappindicator3.so.1 \
    /usr/lib/aarch64-linux-gnu/libappindicator3.so.1; do
    if [[ -e "$so" ]]; then
      dirname "$(readlink -f "$so")"
      return 0
    fi
  done
  return 1
}

libdir="$(find_libdir || true)"
if [[ -z "${libdir}" ]]; then
  echo "nexuskvm: no libayatana-appindicator3.so.1 found." >&2
  echo "Install: sudo apt install libayatana-appindicator3-1 libayatana-appindicator3-dev" >&2
  return 1 2>/dev/null || exit 1
fi

pc_dir="${HOME}/.local/pkgconfig"
mkdir -p "${pc_dir}"
pc="${pc_dir}/ayatana-appindicator3-0.1.pc"
cat >"${pc}" <<EOF
prefix=/usr
libdir=${libdir}
includedir=\${prefix}/include
Name: ayatana-appindicator3-0.1
Description: Ayatana Application Indicators (bundler stub for Tauri)
Version: 0.5.94
Libs: -L\${libdir} -layatana-appindicator3
Cflags: -I\${includedir}
EOF

export PKG_CONFIG_PATH="${pc_dir}${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"
export PKG_CONFIG_ALLOW_SYSTEM_LIBS=1

if ! pkg-config --exists ayatana-appindicator3-0.1; then
  echo "nexuskvm: failed to register ayatana-appindicator3-0.1 via ${pc}" >&2
  return 1 2>/dev/null || exit 1
fi

echo "nexuskvm: using stub pkg-config ${pc} (install libayatana-appindicator3-dev for the real one)" >&2
