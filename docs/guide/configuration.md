# Configuration & Pairing

Most people use the GUI. Files below are what the app actually writes.

---

## 1. File locations

| Mode | Directory | Typical files |
| :--- | :--- | :--- |
| **GUI / session (default)** | `$XDG_DATA_HOME/nexuskvm` (usually `~/.local/share/nexuskvm`) | `state.json`, `password`, `certificate.pem`, `key.pem`, `client-cert.pem`, `client-key.pem`, `daemon.toml` or `client.toml`, `layout.json` |
| **Boot / GDM** | `/etc/nexuskvm/` (copied by `nexuskvm-enable-boot.sh`) | Same names, owned by the system service user |

Private material (`password`, `key.pem`, `client-key.pem`, TOML that embeds the password) is created with mode **0600**.

Unix control socket:

- Session daemon: `$XDG_RUNTIME_DIR/nexuskvm/control.sock`
- System host unit: `/run/nexuskvm/control.sock`

---

## 2. Host: `daemon.toml`

Flattened rkvm-server fields plus `socket`. Example (`config/daemon.example.toml`):

```toml
socket = "/run/nexuskvm/control.sock"
listen = "0.0.0.0:5258"
switch-keys = ["left-alt", "left-ctrl"]
certificate = "/path/certificate.pem"
key = "/path/key.pem"
password = "change-me"
```

`password` must be non-empty. The daemon uses it for the TLS challenge **and** as the Unix-socket token.

---

## 3. Client: `client.toml`

```toml
server = "192.168.0.10:5258"
certificate = "/path/certificate.pem"
client-certificate = "/path/client-cert.pem"
client-key = "/path/client-key.pem"
password = "change-me"
```

---

## 4. Layout: `layout.json`

Written by the spatial editor (`LayoutFile`): `peer_side`, optional `remote_peer`, and a `layout` object (`version`, `local_peer`, `nodes[]`, `barriers[]` with normalized `range_start` / `range_end` 0–1, `cooldown_ms`, `activation_delay_ms`).

---

## 5. Pairing invite

**Copy pairing code** places JSON on the clipboard (not Base64). Fields:

- `server` — advertised `IP:5258`
- `password` — pairing secret (also HMAC/AEAD key for `:5259` and IPC token)
- `certificate` — host CA PEM
- `client_certificate` / `client_key` — mTLS identity for `rkvm-client`

Paste that JSON on the other machine (**Connect to another**). Re-pair after changing CA or mTLS policy.

The dashboard and tray never display the raw password; they only show that pairing is configured. Copy invite is the supported way to move the secret.
