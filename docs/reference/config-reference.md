# Configuration Reference

This page provides the formal specification for configuration files used by NexusKVM.

---

## 1. `daemon.toml`

Primary configuration file for the `nexus-kvmd` server daemon.

| Field | Type | Required | Description | Default Value |
| :--- | :--- | :--- | :--- | :--- |
| `listen_address` | `String` | Yes | Network interface and TCP port for incoming client connections. | `"0.0.0.0:5258"` |
| `socket_path` | `String` | No | Path to the local Unix domain IPC control socket. | `"$XDG_RUNTIME_DIR/nexuskvm.sock"` |
| `switch_keys` | `Array<String>` | No | Physical keyboard key names used for the cycling shortcut. | `["KEY_LEFTALT", "KEY_LEFTCTRL"]` |
| `certificate_path` | `String` | Yes | Path to the X.509 server certificate (`.crt` or `.pem`). | `~/.config/nexuskvm/server.crt` |
| `key_path` | `String` | Yes | Path to the TLS private key (`.key`). | `~/.config/nexuskvm/server.key` |

---

## 2. `layout.json`

Defines screen positions and border transition boundaries.

### Schema Definition:

```typescript
interface LayoutConfig {
  screens: ScreenDefinition[];
  links: EdgeLinkDefinition[];
}

interface ScreenDefinition {
  id: string;          // Unique identifier (e.g. "host-main", "laptop")
  name: string;        // Human-readable display label
  is_host: boolean;    // true if this is the primary host, false for clients
  address?: string;    // IP:port of the client (if is_host is false)
  width: number;       // Pixel resolution width (e.g. 2560)
  height: number;      // Pixel resolution height (e.g. 1440)
}

interface EdgeLinkDefinition {
  source_id: string;                       // Source display ID
  edge: "left" | "right" | "top" | "bottom"; // Trigger edge where pointer leaves
  target_id: string;                       // Destination display ID
  target_edge: "left" | "right" | "top" | "bottom"; // Entry edge where pointer lands
  start_percent?: number;                  // Sub-boundary start percent (0-100, default 0)
  end_percent?: number;                    // Sub-boundary end percent (0-100, default 100)
}
```

---

## 3. Environment Variables

NexusKVM recognizes the following environment variables:

- `RUST_LOG`: Logging verbosity filter (`error`, `warn`, `info`, `debug`, `trace`). Example: `RUST_LOG=nexus_daemon=debug,rkvm=debug`.
- `NEXUSKVM_CONFIG_DIR`: Overrides the base configuration and certificate search directory.
- `NEXUSKVM_SOCKET`: Overrides the Unix domain control socket path.
