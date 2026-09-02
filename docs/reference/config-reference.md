# Configuration Reference

## 1. `daemon.toml` (`nexus-kvmd --config`)

Nexus fields plus flattened rkvm-server keys (`kebab-case`):

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `socket` | path | no | Unix control socket. Default `/run/nexuskvm/control.sock`. |
| `listen` | `IP:port` | yes | Input TLS listener. Typical `0.0.0.0:5258`. |
| `certificate` | path | yes | Server / CA certificate PEM. |
| `key` | path | yes | Server private key PEM. |
| `password` | string | yes | Must be non-empty. TLS challenge, IPC token, and `:5259` AEAD. |
| `switch-keys` | array | yes | Chord, e.g. `["left-alt", "left-ctrl"]`. |
| `propagate-switch-keys` | bool | no | Default true. |

---

## 2. `layout.json`

```typescript
interface LayoutFile {
  peer_side: "left" | "right" | "top" | "bottom";
  remote_peer?: string;
  layout: {
    version: 1;
    local_peer: string;
    nodes: {
      peer_id: string;
      display_id: string;
      x: number; y: number; width: number; height: number; scale: number;
    }[];
    barriers: {
      id: number;
      from_peer: string;
      display_id: string;
      edge: "left" | "right" | "top" | "bottom";
      range_start: number; // 0..1
      range_end: number;   // 0..1
      destination: string;
      activation_delay_ms: number;
      cooldown_ms: number;
    }[];
  };
}
```

---

## 3. Environment variables

| Variable | Used by | Meaning |
| :--- | :--- | :--- |
| `NEXUSKVM_PASSWORD` | `nexus-agent` | Pairing secret (preferred over `--password` / argv). |
| `NEXUSKVM_TOKEN` | `nexusctl` | Same secret, sent as IPC `token`. Required. |
| `RUST_LOG` | all Rust bins | e.g. `nexus_agent=debug,nexus=info,rkvm_server=info`. |
| `XDG_RUNTIME_DIR` | socket default | Session socket parent. |

There is no `NEXUSKVM_CONFIG_DIR` override in the current binaries; the GUI uses the Tauri app data directory.
