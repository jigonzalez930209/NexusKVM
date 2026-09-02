# Package validation

- Structure and files: verified.
- Rust tests (Nexus + `rkvm-master` target router, IPC token, peer AEAD): yes.
- Frontend tests: yes.
- Production transport: `RkvmAdapter` + `TargetHandle` (no MockTransport).
- Physical GDM/uinput/Wayland validation: requires two Ubuntu machines.

Before enabling the daemon: TLS certs, non-empty `password`, and `docs/TESTING.md`. `nexusctl` needs `NEXUSKVM_TOKEN`.
