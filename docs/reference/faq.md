# Frequently Asked Questions (FAQ)

Direct answers to common questions about NexusKVM.

---

### How does NexusKVM compare to Barrier, Synergy, or Input Leap?

| Feature | Barrier / Input Leap | NexusKVM |
| :--- | :--- | :--- |
| **Language & Safety** | C++ legacy codebase | **Pure Rust** with modern memory safety and concurrency. |
| **Wayland Support** | Emulated XWayland grab | **Native** via `org.freedesktop.portal.InputCapture` & `libei`/`reis`. |
| **Input Engine** | X11 server event injection | **rkvm 0.6.1 fork** interfacing directly with kernel `uinput` & `evdev`. |
| **Security & Setup** | Manual TLS certificates | **Automatic TLS 1.3** with 1-click pairing token generator. |
| **Login Screen Support** | Difficult to setup in systemd | **Native systemd support for GDM/SDDM** via boot helper script. |

---

### Does NexusKVM require a wired Ethernet cable or does it work on Wi-Fi?
NexusKVM works on both Ethernet and Wi-Fi. For the lowest input latency (< 1 ms) and zero packet loss in environments with heavy radio interference, a 5 GHz Wi-Fi network or a Gigabit wired connection is recommended.

---

### Why does my user account need to be in the `input` group?
In Linux, physical device descriptors (`/dev/input/event*`) and virtual device creation nodes (`/dev/uinput`) are owned by the `input` group. Adding your account to this group allows NexusKVM to manage input without requiring elevated `root` permissions, keeping your desktop secure.

---

### What happens if the client computer disconnects or loses power?
Thanks to the `TargetRouter` state machine and the `fail_local()` fail-safe mechanism in our rkvm fork, NexusKVM detects the dropped connection within milliseconds and **instantly returns input focus to your local workstation**, purging any held keys to prevent stuck inputs.

---

### Can I share the clipboard (Copy & Paste) between computers?
Yes. The secondary control channel on port `5259/tcp` automatically synchronizes plaintext clipboard buffers between active paired machines.
