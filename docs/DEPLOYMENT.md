# Ubuntu 26.04 deployment

## Desktop PC (recommended)

1. On the development machine: `npm run package:linux`.
2. Copy `target/release/bundle/deb/*.deb` to the other PC.
3. `sudo apt install ./NexusKVM_0.1.0_amd64.deb` — the postinst:
   - adds your user to the `input` group
   - loads `uinput` and reloads udev
   - opens port **5258/tcp** if `ufw` is active
4. **Log out** (required the first time) and open NexusKVM.
5. Primary: **This is the primary**. Remote: **Connect to another** with the copied code.

The GUI generates certificates, writes config, and launches `nexus-kvmd` or `rkvm-client`. Do not use `chmod 666 /dev/uinput`.

## System service (GDM / boot)

1. Install Rust/Tauri/WebKitGTK dependencies and `libevdev-dev`.
2. Build the Nexus workspace and `rkvm-master`.
3. Generate certificates and copy them outside the home directory if the service starts before the session.
4. Create the `nexuskvm` user/group (`scripts/install-dev.sh`).
5. Install systemd units and udev rules.
6. Test `nexusctl status` with the `0660` socket.
7. Enable the daemon only after you have SSH or a recovery keyboard available.
8. Install the agent as a user service.
9. Authorize InputCapture in the session.
