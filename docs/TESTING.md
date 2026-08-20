# Testing

## Automated

```bash
cargo fmt --all -- --check
(cd rkvm-master && cargo fmt --all -- --check)
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo test --workspace
(cd rkvm-master && cargo test --workspace)
npm test
npm run build
npm run format:check
```

`rkvm-input` needs `libevdev` (>= 1.9) and a C compiler for bindgen (`libclang-dev`).

## Real path (no mocks)

From the Tauri app: on the host PC tap «This is the host», copy the pairing code, and on the other paste it. The GUI starts `nexus-kvmd` or `rkvm-client`.

Via CLI:

1. Complete `config/daemon.example.toml` (TLS, password, `switch-keys`).
2. Start `nexus-kvmd --config /path/daemon.toml`.
3. On the other machine, `rkvm-client` with the same password and certificate.
4. `nexusctl status` / `nexusctl switch <addr>` / `nexusctl local`.

## Physical matrix (two Ubuntu machines)

Not run in CI. Includes:

- Cold boot through GDM.
- Typing with a lab user.
- GDM to session switch without recreating devices.
- Crossing A -> B -> A.
- Network disconnect while remote.
- Peer suspend.
- Agent and portal restart.
- Resolution and scale change.
- Modifier keys during transition.

## Chaos

Use `tc netem` on a lab network for latency and loss. Never apply rules to a critical connection without physical access or alternate SSH.
