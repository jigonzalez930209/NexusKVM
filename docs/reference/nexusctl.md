# CLI Reference: `nexusctl`

`nexusctl` is the command-line interface for NexusKVM. It enables inspecting daemon state, managing connected peer nodes, manually triggering target transitions, and running automation or emergency input recovery.

---

## 1. Basic Syntax & Options

```bash
nexusctl [OPTIONS] <SUBCOMMAND>
```

### Global Options:
- `--socket <PATH>`: Custom path to the Unix domain socket (default: `$XDG_RUNTIME_DIR/nexuskvm.sock` or `/run/nexuskvm.sock`).
- `-h, --help`: Print command help.
- `-V, --version`: Print version information.

---

## 2. Subcommands

### A. `status`
Displays current daemon state, active input destination, and list of known clients.

```bash
nexusctl status
```
**Example Output:**
```text
Active Target: 192.168.1.50:5258
Connected Peers:
  - id: client-laptop (192.168.1.50:5258) [ONLINE]
  - id: client-desktop (192.168.1.60:5258) [OFFLINE]
```

---

### B. `peers`
Prints detailed information on all paired endpoints and their connection health.

```bash
nexusctl peers
```

---

### C. `switch`
Manually directs mouse and keyboard input to a specific peer endpoint address.

```bash
# Switch to remote client
nexusctl switch 192.168.1.50:5258
```

---

### D. `local`
Immediately returns mouse and keyboard control back to the **local Host machine**.

```bash
nexusctl local
```

---

### E. `release`
Atomically releases all keys that might be held down in the active destination. Useful for recovering from stuck keys.

```bash
nexusctl release
```

---

## 3. Automation Script Example

Integrate `nexusctl` into window manager hotkeys (i3, bspwm, Hyprland, Sway) or Stream Deck macros:

```bash
#!/usr/bin/env bash
# Quick toggle between Host and Primary Client

ACTIVE=$(nexusctl status | grep "Active Target" | awk '{print $3}')

if [ "$ACTIVE" = "local" ]; then
    nexusctl switch 192.168.1.50:5258
else
    nexusctl local
fi
```
