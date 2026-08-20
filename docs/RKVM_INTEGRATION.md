# Integration with rkvm 0.6.1

The fork lives in `rkvm-master/` (rkvm 0.6.1, MIT). The `rkvm-net` event protocol is not modified.

## Deviations from upstream

1. `rkvm-server` is a **lib + bin** crate.
2. Destination selection lives in `rkvm-server/src/target.rs` (`TargetRouter` / `TargetHandle`).
3. Peers are identified by `SocketAddr` (`127.0.0.1:port`), not by `Slab` index.
4. `switch-keys` shortcuts call `switch_next()` on that same API.
5. If the active peer disconnects, return to `local` runs and retained keys are cleared.
6. `nexus-kvmd` starts the `rkvm-server` loop and exposes the Unix control socket via `RkvmAdapter`.

## Internal API

```rust
pub trait TargetController {
    fn peers(&self) -> Vec<PeerSnapshot>;
    async fn prepare(&self, id: PeerId, entry: EntryPoint) -> Result<()>;
    async fn activate(&self, id: PeerId) -> Result<()>;
    async fn local(&self) -> Result<()>;
    async fn release_all(&self, id: Option<PeerId>) -> Result<()>;
}
```

In this fork that is `TargetHandle` (channel into the loop + snapshot `watch`).

rkvm is not controlled by injecting shortcuts or running shell commands.
