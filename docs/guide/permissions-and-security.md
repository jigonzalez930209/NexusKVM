# Linux Permissions, udev & Security

For NexusKVM to capture physical keyboard and mouse events on the **Host** and inject synthetic events on the **Client**, it interacts with specific Linux kernel subsystems (`evdev` and `uinput`).

This guide details all required permissions, how to configure them safely, and essential security practices.

---

## Permission Matrix Overview

<PermissionMatrix />

---

## 1. Kernel Subsystems: `evdev` and `uinput`

Linux manages physical input hardware and virtual devices through two core subsystems:

### A. `evdev` (`/dev/input/event*`)
- **Purpose:** Provides a direct interface to read raw hardware events from keyboards, mice, trackpads, and other input peripherals.
- **Role in NexusKVM:** On the **Host**, `nexus-kvmd` reads physical input events from your keyboard and mouse to transmit them across the network when a remote client is active.
- **Safe Permission:** Read access restricted to the system `input` group (`0660` mode).

### B. `uinput` (`/dev/uinput`)
- **Purpose:** A kernel driver allowing user-space processes to instantiate virtual input devices that the operating system treats as genuine physical hardware.
- **Role in NexusKVM:** On the **Client**, `rkvm-client` creates a virtual keyboard and mouse to emulate keystrokes and cursor movements received from the Host.
- **Safe Permission:** Read/write access restricted to the system `input` group with static node configuration in udev (`0660` mode).

---

## 2. Step-by-Step Permission Configuration

### Step 1: Load the `uinput` Kernel Module

The `uinput` module must be loaded into the running kernel. You can load it immediately or ensure persistent loading across system reboots:

```bash
# Load module immediately
sudo modprobe uinput

# Ensure module loads automatically at system boot
echo "uinput" | sudo tee /etc/modules-load.d/uinput.conf
```

---

### Step 2: Install `udev` Rules

NexusKVM provides two essential udev rules that belong in `/etc/udev/rules.d/`:

#### 📄 `/etc/udev/rules.d/70-nexuskvm-uinput.rules`
Ensures the `/dev/uinput` node is owned by the `input` group with `0660` permissions:
```ini
KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input"
```

#### 📄 `/etc/udev/rules.d/70-nexuskvm-desktop.rules`
Assigns event nodes to the `input` group and applies the `uaccess` tag so that `systemd-logind` dynamically grants access to the active desktop session:
```ini
# Desktop user session (GUI) and members of group `input` can open /dev/uinput and evdev
KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", MODE="0660", GROUP="input", TAG+="uaccess"
```

#### Apply the udev Rules:
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=misc --action=add
sudo udevadm trigger --subsystem-match=input --action=add
sudo udevadm trigger --name-match=uinput
```

---

### Step 3: Add Your User to the `input` Group

Your desktop user account must belong to the `input` group:

```bash
sudo usermod -aG input $USER
```

::: danger ⚠️ MANDATORY: Log Out to Apply Group Changes
In Linux, group membership updates **are not inherited by processes that are already running**.
You must **log out of your desktop session (Log Out)** and log back in for your terminal and graphical applications to gain `input` group privileges.

Verify your membership by running:
```bash
groups
# Should include 'input' in the output list
```
:::

---

## 3. The Golden Rule: The `chmod 666 /dev/uinput` Anti-Pattern

::: danger 🛑 WHY YOU MUST NEVER USE `chmod 666`
Several outdated tutorials online suggest running:
```bash
# ❌ DANGEROUS SECURITY FLAW - NEVER DO THIS:
sudo chmod 666 /dev/uinput
sudo chmod 666 /dev/input/event*
```

### Why is this a severe security vulnerability?
1. **System-Wide Keylogging:** If `/dev/input/event*` has universal read permissions (`666`), **any unprivileged background process, compromised npm package, or malicious browser exploit** can read every key you type, including master passwords, private SSH keys, and credit card credentials.
2. **Arbitrary Input Injection:** If `/dev/uinput` is world-writable (`666`), any background application can inject keystrokes to spawn a shell and execute arbitrary commands.

NexusKVM **strictly rejects this practice** and relies solely on controlled `0660` permissions via group membership and dynamic desktop session tokens.
:::

---

## 4. Architectural Privilege Separation

NexusKVM is built on the principle of **least privilege**:

```
+-------------------------------------------------------------+
|                     User Desktop Session                    |
|                                                             |
|  [NexusKVM GUI (Tauri)] ---- (Unix Socket 0660)             |
|     (Unprivileged)                    |                     |
|                                       v                     |
|  [nexus-agent (Wayland Portal)] -> [nexus-kvmd (Daemon)]    |
|                                       |                     |
+---------------------------------------|---------------------+
                                        v
                            [/dev/input/event* & uinput]
                             (Access via `input` group)
```

- **GUI Never Runs as Root:** The Tauri 2 window runs as your normal, unprivileged user.
- **Secure Local IPC:** The frontend and session agent talk to the daemon over a local Unix domain socket (`/run/nexuskvm/control.sock` or `$XDG_RUNTIME_DIR/nexuskvm/control.sock`) with `0660` permissions, a required pairing **token**, and `SO_PEERCRED`.
- **Network planes:** `:5258` is TLS 1.3 + mTLS + password (input). `:5259` is ChaCha20-Poly1305 AEAD (edge return + clipboard). Never log key codes.
- **Secrets on disk:** `password`, `key.pem`, `client-key.pem`, and daemon/client TOML are written mode `0600`. The GUI status poll does **not** include the pairing password (`has_password` only). The agent receives the secret via `NEXUSKVM_PASSWORD`, not process argv.
- **Zero Keylogging Invariant:** `nexus-kvmd` never logs or persists scancodes or key events.

---

## 5. Boot & Login Screen Permissions (`nexuskvm-enable-boot.sh`)

If you want NexusKVM active on the Linux display manager (**GDM / SDDM**) before logging in:

NexusKVM includes a secure setup script requiring one-time elevation:

```bash
sudo /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh enable-host
# Or for the client:
sudo /usr/libexec/nexuskvm/nexuskvm-enable-boot.sh enable-client
```

This helper script:
1. Copies validated TLS certificates to the secure system directory `/etc/nexuskvm/`.
2. Sets proper ownership to the system user and group `nexuskvm`.
3. Enables the system-level systemd service (`systemctl enable --now nexuskvm-host.service`).
