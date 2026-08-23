# Troubleshooting & Diagnostics

A step-by-step diagnostic guide to resolving common setup, permission, and networking issues.

---

## 1. Permission Denied on `/dev/uinput` or `evdev`

### Symptom:
The app reports an error stating that `/dev/uinput` or `/dev/input/event*` cannot be opened, or the background service fails immediately upon launch.

### Solution:
1. Verify that your user is a member of the `input` group:
   ```bash
   id -nG | grep -q input && echo "OK" || echo "Missing input group membership"
   ```
2. If you are not in the group, add your account and **log out of your desktop session**:
   ```bash
   sudo usermod -aG input $USER
   ```
3. Ensure udev rules are loaded and triggered:
   ```bash
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```
4. Verify `/dev/uinput` device node permissions:
   ```bash
   ls -la /dev/uinput
   # Expected output: crw-rw---- 1 root input 10, 223 ... /dev/uinput
   ```

---

## 2. Machines Cannot Connect (Connection Timed Out)

### Symptom:
After pasting the pairing code, the status remains on *"Connecting..."* or fails with a network timeout.

### Solution:
1. **Test Basic Connectivity (Ping):**
   Ensure both machines are on the same local subnet and can reach each other:
   ```bash
   ping -c 3 HOST_IP_ADDRESS
   ```
2. **Check Host Firewall:**
   Ensure ports `5258/tcp` and `5259/tcp` are open on the Host:
   ```bash
   sudo ufw status
   # If active, allow traffic:
   sudo ufw allow 5258/tcp
   sudo ufw allow 5259/tcp
   ```
3. **Verify Listening Sockets on the Host:**
   ```bash
   ss -tulpn | grep 5258
   ```

---

## 3. Pointer Does Not Cross Edge in Wayland (GNOME / KDE)

### Symptom:
The keyboard hotkey switches machines fine, but moving the mouse cursor against the display boundary does not trigger a transition.

### Solution:
1. **Verify Desktop Portal Backend:**
   Ensure your desktop environment has its portal package installed:
   - On GNOME: `xdg-desktop-portal-gnome`
   - On KDE Plasma: `xdg-desktop-portal-kde`
2. **Verify User Session Agent:**
   ```bash
   systemctl --user status nexus-agent.service
   ```
3. **Portal Permission Dialog:**
   When first triggered, Wayland desktop compositors show an authorization prompt asking to permit pointer capture. Make sure to accept and check *"Remember this choice"*.

---

## 4. Enabling Verbose Debug Logs (`RUST_LOG=debug`)

To inspect detailed internal traces:

```bash
# Launch daemon with debug logging
RUST_LOG=nexus_daemon=debug,nexus_agent=debug,rkvm=debug nexus-kvmd --config ~/.config/nexuskvm/daemon.toml

# Or inspect systemd logs
journalctl -u nexuskvm-host.service -f --output=cat
```
