# Testing

## Automated

```bash
npm run test:all
# or:
cargo fmt --all -- --check
(cd rkvm-master && cargo fmt --all -- --check)
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo test --workspace
(cd rkvm-master && cargo test --workspace)
npm test
npm run build
npm run format:check
```

Coverage includes IPC token rejection, peer-channel AEAD (bad key, clip roundtrip, empty secret), and TargetRouter per-destination key release.

`rkvm-input` needs `libevdev` (>= 1.9) and a C compiler for bindgen (`libclang-dev`).

## Real path (no mocks)

From the Tauri app: on the host tap **This is the host**, copy the pairing JSON, paste it on the other PC. The GUI starts `nexus-kvmd` or `rkvm-client` plus `nexus-agent`.

Via CLI:

1. Fill `config/daemon.example.toml` (TLS paths, non-empty `password`, `switch-keys`).
2. `nexus-kvmd --config /path/daemon.toml`.
3. On the other machine, `rkvm-client` with the same password, CA, and **client** cert/key.
4. `NEXUSKVM_TOKEN=<password> nexusctl status` / `switch <ip>` / `local` / `release-all`.

## Physical matrix (two Ubuntu machines)

Not run in CI. Includes:

- Cold boot through GDM.
- Typing with a lab user.
- GDM to session switch without recreating devices.
- Crossing A → B → A (edges + hotkey).
- Clipboard text / image / folder.
- Network disconnect while remote.
- Peer suspend.
- Agent and portal restart.
- Resolution and scale change.
- Modifier keys during transition (release must follow press destination).

## Chaos

Use `tc netem` on a lab network for latency and loss. Never apply rules to a critical connection without physical access or alternate SSH.
