# rkvm 0.6.1 Integration & Fork Architecture

<RkvmCitation />

---

## 1. Formal Attribution & Citation

NexusKVM expresses deep gratitude to **Florian Larysch** ([@htrefil](https://github.com/htrefil)) and the open-source contributors of the **[rkvm](https://github.com/htrefil/rkvm)** project.

The original rkvm project established a groundbreaking foundation for fast, lightweight input sharing in Linux under the terms of the **MIT License**:

```bibtex
@software{rkvm2023,
  author = {Florian Larysch},
  title = {rkvm: Virtual KVM switch for Linux},
  year = {2023},
  publisher = {GitHub},
  journal = {GitHub repository},
  howpublished = {\url{https://github.com/htrefil/rkvm}},
  license = {MIT}
}
```

---

## 2. Why a Dedicated Fork for NexusKVM?

While upstream rkvm provides outstanding raw input performance and low latency, it was designed primarily as a monolithic terminal utility controlled solely by hardcoded physical hotkeys.

To integrate rkvm smoothly into a modern desktop platform featuring a graphical user interface (**Tauri 2**), edge-of-screen cursor transitions (**Wayland InputCapture**), dynamic certificate pairing, and a robust Unix socket API, NexusKVM maintains an advanced fork located in `rkvm-master/`.

---

## 3. Key Upstream Deviations & Architectural Enhancements

```
                         [rkvm 0.6.1 Upstream]
                                   |
                +------------------+------------------+
                |                                     |
      [rkvm Original]                         [NexusKVM Fork]
- Monolithic CLI binary              - Dual `lib + bin` crate structure
- Switching by Slab index slot       - Stable `SocketAddr` identity
- No local IPC control plane         - Asynchronous `TargetRouter` & `TargetHandle`
- Disconnect can stick keys          - Atomic `fail_local()` & keystroke purge
- Static keyboard hotkeys            - Dynamic Wayland barriers & JSONL API
```

### A. Conversion of `rkvm-server` to a Dual `lib + bin` Crate
In upstream, `rkvm-server` is solely an executable binary (`main.rs`). In NexusKVM, it was refactored into a dual library (`lib.rs`) and binary, enabling the `nexus-kvmd` daemon to embed the server loop directly without managing external child processes.

### B. Decoupled Routing: `TargetRouter` & `TargetHandle`
Target selection was extracted from the monolithic `current` state variable and redesigned as a clean, deterministic state machine in `rkvm-server/src/target.rs`:

```rust
pub trait TargetController {
    fn peers(&self) -> Vec<PeerSnapshot>;
    async fn prepare(&self, id: PeerId, entry: EntryPoint) -> Result<()>;
    async fn activate(&self, id: PeerId) -> Result<TransitionId>;
    async fn local(&self) -> Result<TransitionId>;
    async fn release_all(&self, id: Option<PeerId>) -> Result<Vec<Key>>;
}
```

### C. Stable Peer Identity via `SocketAddr`
Rather than tracking client nodes by volatile internal array indices (`Slab`), NexusKVM identifies peers by their normalized network endpoint address (`SocketAddr`, e.g., `192.168.1.50:5258`), providing deterministic behavior across connect/disconnect events.

### D. Automatic Fail-Safe to `local`
If an active client drops its TCP connection or shuts down while holding the input focus:
```rust
fn fail_local(&mut self) {
    self.previous = self.active.clone();
    self.active = LOCAL_TARGET.to_string();
    self.chord_changed = false;
    self.busy = false;
    self.held_keys.clear(); // Atomic keystroke purge prevents ghost inputs
}
```
This guarantees the user is **never locked out of their local workstation**.

### E. Full Protocol Compatibility (`rkvm-net`)
The underlying binary event serialization format (`rkvm-net`) is **100% preserved**, ensuring rock-solid compatibility and maximum transport speed.
