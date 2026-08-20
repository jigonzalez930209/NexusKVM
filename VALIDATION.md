# Package validation

- Structure and files: verified.
- Rust tests (Nexus + `rkvm-master` target router): yes.
- Frontend tests: yes.
- Production transport: `RkvmAdapter` + `TargetHandle` (no MockTransport).
- Physical GDM/uinput/Wayland validation: requires two Ubuntu machines.

Before enabling the daemon, run through `docs/TESTING.md` and complete TLS certificates.
