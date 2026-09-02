# CLI Reference: `nexusctl`

Talks to `nexus-kvmd` over the Unix control socket. Output is JSON (`ControlResponse`).

---

## 1. Syntax

```bash
export NEXUSKVM_TOKEN='your-pairing-password'
nexusctl [--socket PATH] <SUBCOMMAND>
```

### Options

- `--socket <PATH>`: default `$XDG_RUNTIME_DIR/nexuskvm/control.sock`, else `/run/nexuskvm/control.sock`. Against a boot host unit use `/run/nexuskvm/control.sock`.
- The pairing password **must** be in `NEXUSKVM_TOKEN`. Without it the daemon returns `unauthorized`. Exit code `2` if `ok` is false.

---

## 2. Subcommands

| Command | IPC | Notes |
| :--- | :--- | :--- |
| `nexusctl status` | `status` | State, active target, peers, agent heartbeat. |
| `nexusctl peers` | `peers` | Same status payload (peer map). |
| `nexusctl switch <id>` | `switch` | Peer id is the client **IP** (stable), not the ephemeral TCP port. |
| `nexusctl local` | `local` | Return input to the host. |
| `nexusctl release-all` | `release_all` | Release held keys and return local. |

There is no `shutdown` command; the daemon rejects remote shutdown.

Example:

```bash
NEXUSKVM_TOKEN=$(cat ~/.local/share/nexuskvm/password) \
  nexusctl --socket /run/nexuskvm/control.sock local
```

---

## 3. Recovery over SSH

If the pointer is stuck on a remote machine:

```bash
ssh host
export NEXUSKVM_TOKEN=...
nexusctl local
# or
nexusctl release-all
```
