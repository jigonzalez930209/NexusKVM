# Process Architecture Overview

NexusKVM is built as a distributed, modular suite of decoupled components in **Rust**, designed for extreme responsiveness, memory safety, and minimal system footprint.

---

## 1. Core Processes & System Topology

The NexusKVM ecosystem consists of four independent processes:

```mermaid
graph TD
    subgraph Host["Host PC (Workstation)"]
        UI["🖥️ NexusKVM GUI (Tauri 2 + React)"]
        Agent["📐 nexus-agent (Wayland InputCapture / EIS)"]
        Daemon["⚙️ nexus-kvmd (rkvm-server + TargetRouter)"]
        Evdev["⌨️ /dev/input/event* (Physical Peripherals)"]
        
        UI <-->|Unix Socket 0660 / JSONL| Daemon
        Agent <-->|Unix Socket 0660 / JSONL| Daemon
        Daemon <-->|evdev read (input group)| Evdev
    end

    subgraph Client["Client PC (Target)"]
        RkvmClient["📥 rkvm-client (TLS Receiver)"]
        Uinput["🖱️ /dev/uinput (Virtual Devices)"]
        Desktop["🖥️ Desktop Session / GDM"]

        RkvmClient -->|Inject Input Events| Uinput
        Uinput --> Desktop
    end

    Daemon <===>|TLS 1.3 - 5258/tcp (rkvm-net stream)| RkvmClient
    Agent <===>|TLS 1.3 - 5259/tcp (Control/Clipboard)| RkvmClient
```

### 1. `nexus-kvmd` (Host Daemon)
- Drives the server loop, accepts TLS connections from clients, and coordinates active target routing.
- Hosts the **`TargetRouter`** state machine to dynamically direct events between local input devices and remote peers.
- Exposes a local Unix domain socket control plane (`0660` permissions) using JSON Lines.

### 2. `nexus-agent` (Session Agent)
- Runs inside the graphical user desktop session (GNOME, KDE Plasma, Sway, Hyprland).
- Registers pointer barriers along display edges via `org.freedesktop.portal.InputCapture` and `reis`/`libei`.
- Converts physical mouse collisions into seamless screen transitions.

### 3. `NexusKVM` (Desktop UI)
- Built with **Tauri 2**, **React**, and **TypeScript**.
- Runs completely unprivileged under the desktop user account.
- Provides 1-click pairing, visual display drag-and-drop layout configuration, and system tray management.

### 4. `nexusctl` (CLI Tool)
- Command-line utility for automation, scripting, status inspection, and emergency input recovery.

---

## 2. Core Design & Security Invariants

NexusKVM strictly enforces the following architectural invariants:

1. **Uncertainty Fallback to `local`:** On any network timeout, dropped socket, or unexpected client termination, input focus **immediately falls back to `local`** without locking up the user's desktop.
2. **Key Press/Release Destination Coherence:** Every key release event is guaranteed to be dispatched to the destination that received the corresponding key press, preventing modifier keys like <kbd>Shift</kbd>, <kbd>Ctrl</kbd>, or <kbd>Alt</kbd> from getting stuck in remote sessions.
3. **Zero Keylogging Guarantee:** By design, neither `nexus-kvmd` nor `nexus-agent` ever logs, prints, or persists raw scancodes or key character data.
4. **GUI Process Isolation:** The desktop UI does not hold file descriptors to raw `evdev` or `/dev/uinput` devices; all actions flow through the strictly governed Unix socket IPC.
5. **Monotonic Transition Identifiers:** Every focus change generates a unique monotonic transition ID to prevent race conditions during rapid pointer movements.
