# systemd Services & Boot Integration (GDM)

NexusKVM can run in two operational modes: **Desktop App Mode** (managed dynamically by the Tauri GUI inside your active user session) and **System Service Mode** (managed by `systemd` to allow mouse and keyboard control at the display manager login screen like **GDM / SDDM / LightDM**).

---

## 1. Operational Modes

```
                                [Operational Modes]
                                         |
                   +---------------------+---------------------+
                   |                                           |
            [App Mode]                                  [Service Mode]
     (Managed by Tauri GUI)                         (Managed by systemd)
                   |                                           |
   - Starts when opening NexusKVM               - Starts automatically on Boot
   - Lives in the system tray                   - Works at the login screen (GDM)
   - Config in ~/.config/nexuskvm/              - Config in /etc/nexuskvm/
```

---

## 2. Available Service Units

NexusKVM provides the following systemd units:

| Service Unit | Scope | Purpose |
| :--- | :--- | :--- |
| **`nexus-kvmd.service`** / **`nexuskvm-host.service`** | System (`/etc/systemd/system/`) | Runs the server daemon `nexus-kvmd` on the **Host** under the dedicated `nexuskvm` system user. |
| **`nexuskvm-client.service`** | System (`/etc/systemd/system/`) | Runs the input receiver `rkvm-client` on the **Client** as a persistent background service. |
| **`nexus-agent.service`** | User (`~/.config/systemd/user/`) | Session agent that registers pointer barriers on Wayland via the InputCapture portal. |

---

## 3. Enabling Boot & Login Screen Services

To enable automatic background operation at the login screen:

### On the Host (Workstation with Physical Keyboard/Mouse):
```bash
# Enables and starts the host daemon at system level
sudo /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh enable-host
```

### On the Client (Secondary Machine):
```bash
# Enables and starts the client input receiver at system level
sudo /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh enable-client
```

### To Disable Boot Services:
```bash
sudo /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh disable-host
# Or on the client:
sudo /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh disable-client
```

---

## 4. User Session Agent for Wayland (`nexus-agent`)

In modern Wayland desktop sessions (such as GNOME on Ubuntu or Fedora), the session agent connects to the desktop compositor to detect when the pointer hits the screen edge.

Enable the user service via:

```bash
systemctl --user daemon-reload
systemctl --user enable --now nexus-agent.service
```

Check status:
```bash
systemctl --user status nexus-agent.service
```

---

## 5. Inspecting Logs with `journalctl`

Monitor live service logs:

```bash
# Host system daemon logs
journalctl -u nexuskvm-host.service -f

# Client receiver logs
journalctl -u nexuskvm-client.service -f

# User Wayland agent logs
journalctl --user -u nexus-agent.service -f
```
