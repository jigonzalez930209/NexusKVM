# NexusKVM

Rust + Tauri 2 codebase for a software KVM built on a fork of rkvm 0.6.1 (`rkvm-master/`).

## What it implements

- `nexus-kvmd` daemon that runs the `rkvm-server` loop and a local control plane.
- `TargetHandle`: destination by stable ID (`SocketAddr`), shortcuts, and fail-safe when a peer is lost.
- Local IPC over a Unix socket with JSON Lines and `0660` permissions.
- `nexusctl` CLI for status, peers, switch, return local, and input release.
- Session agent with an `EdgeCaptureBackend` contract and a decoupled Wayland portal.
- Barrier engine, hysteresis, partial zones, and proportional transformation.
- Tauri 2 + React + TypeScript UI.
- systemd services, udev rules, and example configuration.
- Unit tests for the controller and the rkvm router (without `/dev/uinput`).

## Real transport

`RkvmAdapter` talks to the fork's `TargetHandle`. Clients remain `rkvm-client` with TLS. Patch details: `docs/RKVM_INTEGRATION.md`.

The InputCapture/EIS portal for Wayland edges still needs the wiring described in `docs/WAYLAND_PORTAL.md`.

## Usage (all from the app)

1. Open NexusKVM on the machine with keyboard and mouse → **This is the host**.
2. **Copy pairing code**.
3. On the other PC, open NexusKVM → **Connect to another** and paste the code.

The app creates certificates, configures TLS, and starts `nexus-kvmd` or `rkvm-client` on its own. Closing the window sends the app to the **system tray** (service keeps running). Use tray → **Quit…** to exit. After pairing, the app can enable a system service (password prompt once) so the role stays active at the GDM login screen. You need `openssl` and, for input capture, membership in the `input` group (or permissions on `/dev/uinput`).

## Quick development

```bash
cargo test --workspace
(cd rkvm-master && cargo test --workspace)
npm install
npm test
npm run tauri dev
```

`tauri dev` builds `nexus-kvmd`, `nexus-agent`, and `rkvm-client` before opening the UI. You need `libevdev-dev` (or a `libevdev.pc` in `~/.local/pkgconfig` if only the runtime package is installed). For `npm run package:linux`, install `libayatana-appindicator3-1` (tray); `libayatana-appindicator3-dev` is preferred, otherwise the packaging script synthesizes a stub `.pc`. The openssl `++++` when generating certificates should not appear in the console.

## Security

Do not run the GUI as root. The daemon should use a system user, minimal udev rules, and a `0660` socket. Never apply `chmod 666 /dev/uinput`.

## Install on another PC

On this machine:

```bash
npm run package:linux
```

Copy the `.deb` from `target/release/bundle/deb/` to the other PC (Ubuntu/Debian) and there:

```bash
sudo apt install ./NexusKVM_0.1.0_amd64.deb
```

The installer configures permissions (`input`), udev, the `uinput` module, and firewall (ufw). **Log out** after installing. On the host: **This is the host** → copy pairing code. On the other: **Connect to another**. Hotkey on the host: **Left Alt + Left Ctrl**.

You need `openssl` (installed by the package) and WebKitGTK. Do not run the GUI as root.
