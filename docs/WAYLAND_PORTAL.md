# Wayland backend

The final backend uses `org.freedesktop.portal.InputCapture` version 2. Flow:

1. Connect to the session bus.
2. Read `SupportedCapabilities` and `version`.
3. Create a v2 session (`CreateSession2` + `Start`) or legacy `CreateSession`.
4. Query zones (`GetZones`).
5. Register barriers with `SetPointerBarriers` on the configured edge.
6. Connect to EIS with `ConnectToEIS` (handshake via `reis` on a dedicated thread).
7. Enable capture (`Enable`).
8. Convert `Activated` → `EdgeEvent` (display, edge, normalized position).
9. Immediate `Release` with the cursor inward so the session is not left hanging.
10. Handle `ZonesChanged` / re-registration when `layout.json` changes.

Implementation: `crates/nexus-agent/src/backend.rs` (`PortalBackend`).

GNOME notes:

- InputCapture uses the same remote-access machinery as screen casting, so any
  active session makes GNOME show its screen-capture indicator when a barrier
  activates (mutter MR !2392). For that reason the agent no longer registers an
  InputCapture session; switching runs through the X11 edge strip (`EdgePortal`).
  `PortalBackend`/`EdgeEngine` stay in the tree for reference or future opt-in.
- Changing barriers after `Disable` can fail on re-enable (GNOME portal bug 46+).
- The agent uses *soft-suspend* (ignores `Activated`) when the active destination is not local.
