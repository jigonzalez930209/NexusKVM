# Technical roadmap: software KVM based on rkvm with visual edge transitions

**Document version:** 1.0  
**Date:** August 19, 2026  
**Status:** technical proposal for MVP and evolution to a stable product  
**Initial target platform:** Ubuntu 26.04, GNOME, Wayland, and GDM  
**Recommended language for the core:** Rust  
**Technology base:** fork of `rkvm`, licensed under MIT

---

## 1. Executive summary

The goal is to build a free and open-source application that shares keyboard and mouse across two or more Linux computers and offers two complementary behaviors:

1. **Before login:** use the `rkvm` engine, running as a system service, to switch computers with a key combination and type the password on the real GDM screen.
2. **Inside a graphical session:** detect that the cursor reached an edge and automatically transfer control to the neighboring computer, simulating a multi-monitor layout attached to a single machine.

`rkvm` is a strong base because it does not need a display server, uses a client-server architecture, encrypts communication with TLS, works with Linux input devices, and provides `systemd` service examples. The project currently switches destinations via configurable shortcuts, so an explicit control API will be needed to integrate it with the visual agent. citeturn7search140turn7search144

Linux can create virtual keyboard and mouse devices via `uinput`. Events sent to `/dev/uinput` are treated by the system as input device events, which makes it possible to control GDM without depending on a user session, X11, or Wayland. citeturn6search118turn6search120

Under Wayland, a conventional application must not rely on freely reading the global cursor position. The modern approach is to use input capture portals supported by modern GNOME. Deskflow uses this approach and documents Wayland support via portals on recent GNOME versions. citeturn3search48

---

## 2. Goals, scope, and constraints

### 2.1. Functional goals

#### 2.1.1. Pre-login control

- The receiver service must start during system boot.
- It must create virtual keyboard and mouse devices before GDM prompts for the password.
- Machine selection must work via a global emergency shortcut.
- It must not require automatic login.
- It must not require the graphical desktop to be available.

#### 2.1.2. Visual transition inside the session

- The cursor must move to another computer when it reaches a configured edge.
- Vertical entry must keep a proportional position across screens.
- Return must be possible from the opposite edge.
- The transition must avoid unintended bounce-back.
- Shortcut switching must remain available as a recovery mechanism.

#### 2.1.3. Visual configuration

- Discover authorized machines on the local network.
- Show machines as draggable rectangles.
- Allow placing a machine left, right, above, or below another.
- Allow configuring partial edge zones.
- Show connection status, version, latency, and active destination.

#### 2.1.4. Security and recovery

- End-to-end encryption between nodes.
- Explicit and revocable pairing.
- Local control via a protected Unix socket.
- Automatic return to the local machine on network loss.
- Automatic release of held keys and buttons.
- Logs that never store keystroke content.

### 2.2. Out of scope for the MVP

#### 2.2.1. Features excluded initially

- Video sharing or remote desktop.
- Control of BIOS, UEFI, GRUB, or pre-kernel unlock.
- File transfer.
- Clipboard synchronization.
- Initial Windows or macOS compatibility.
- Dragging files between machines.
- Compatibility with graphics tablets, multi-touch gestures, or gamepads.

#### 2.2.2. Features reserved for later phases

- Topologies of more than four computers.
- Different networks via relay.
- Automatic mDNS discovery.
- Signed automatic updates.
- Flatpak portal for the graphical interface.
- Packages for Fedora, Arch Linux, and NixOS.

### 2.3. Technical constraints

#### 2.3.1. Wayland

- Do not assume direct access to the global pointer position.
- Do not depend on private Mutter APIs for the stable version.
- Use `InputCapture Portal` or an equivalent supported integration.
- Keep an optional X11 backend for development and diagnostics.

#### 2.3.2. Privileges

- The frontend must never run as `root`.
- The daemon must have only read permissions on the required input devices and write permission on `uinput`.
- The session agent must communicate with the daemon via authenticated IPC.
- Private keys must live outside the user's home directory when the service starts before the session.

#### 2.3.3. Compatibility with rkvm

`rkvm` is open source under the MIT license, is display-server agnostic, and includes separate components such as server, client, input, network, certificate generator, and `systemd` units. citeturn6search116turn7search140

- Keep a fork with small, well-isolated changes.
- Avoid modifying the event protocol during the first MVP.
- Add a local control plane first.
- Document every deviation from upstream.

---

## 3. Target architecture

### 3.1. Main components

#### 3.1.1. `kvm-core-daemon`

Privileged process based on `rkvm`:

- Detects physical devices via `evdev`.
- Captures keyboard and mouse events from the primary computer.
- Sends events to the active node.
- Receives events on secondary nodes.
- Injects events via `uinput`.
- Maintains TLS, authentication, and connections.
- Exposes a Unix control socket.
- Works before, during, and after a graphical session.

#### 3.1.2. `kvm-session-agent`

Unprivileged process started with the graphical session:

- Detects the graphical environment.
- Obtains monitor topology.
- Registers edge barriers.
- Asks the daemon to change destination.
- Receives state events.
- Coordinates return from the remote machine.
- Triggers error notifications.

#### 3.1.3. `kvm-ui`

Unprivileged graphical interface:

- Configures machines and layout.
- Manages pairing.
- Shows diagnostics and connectivity.
- Writes configuration via a secure API.
- Never reads raw keyboard events.

#### 3.1.4. `kvmctl`

CLI client for administration and recovery:

```text
kvmctl status
kvmctl peers
kvmctl switch <peer-id>
kvmctl local
kvmctl release-all
kvmctl diagnostics
```

### 3.2. Plane separation

#### 3.2.1. Data plane

Carries low-latency input events:

```text
Physical device -> evdev -> server daemon -> TLS -> client daemon -> uinput
```

#### 3.2.2. Control plane

Carries commands and state:

```text
Agent/UI/CLI -> Unix socket -> daemon -> state machine -> active destination
```

#### 3.2.3. Inter-agent coordination plane

Carries visual events, not keystrokes:

```text
Agent A -> edge activated -> daemon A -> authenticated channel -> agent B
```

Expected messages:

```json
{
  "type": "prepare_entry",
  "target": "peer-b",
  "edge": "left",
  "normalized_position": 0.52,
  "transition_id": "uuid"
}
```

#### 3.2.4. Configuration plane

- System configuration: `/etc/app-name/daemon.toml`
- Private keys: `/var/lib/app-name/keys/`
- Pairing state: `/var/lib/app-name/peers.json`
- User configuration: `~/.config/app-name/layout.json`
- Control socket: `/run/app-name/control.sock`
- Logs: `journald`, without logging keystrokes.

---

## 4. State model

### 4.1. Global server state

#### 4.1.1. States

```text
LOCAL
PREPARING_REMOTE
REMOTE
RETURNING_LOCAL
DEGRADED
RECOVERING
STOPPED
```

#### 4.1.2. Transitions

```text
LOCAL --edge/shortcut--> PREPARING_REMOTE
PREPARING_REMOTE --peer_ready--> REMOTE
PREPARING_REMOTE --timeout/error--> LOCAL
REMOTE --return_edge/shortcut--> RETURNING_LOCAL
RETURNING_LOCAL --input_released--> LOCAL
REMOTE --network_lost--> RECOVERING
RECOVERING --cleanup_complete--> LOCAL
ANY --fatal_error--> DEGRADED
DEGRADED --admin_repair--> LOCAL
```

#### 4.1.3. Pseudocode

```rust
loop {
    event = next_control_or_network_event().await;

    match (state, event) {
        (LOCAL, SwitchRequested(peer)) => {
            release_all_local_transient_state();
            send_prepare(peer).await?;
            state = PREPARING_REMOTE(peer, deadline_after(500_ms));
        }

        (PREPARING_REMOTE(peer, _), PeerReady(peer)) => {
            active_target = peer;
            state = REMOTE(peer);
        }

        (PREPARING_REMOTE(_, deadline), Tick) if now() > deadline => {
            active_target = LOCAL_TARGET;
            notify("Could not switch machines");
            state = LOCAL;
        }

        (REMOTE(peer), ReturnRequested) => {
            send_release_all(peer).await;
            active_target = LOCAL_TARGET;
            state = RETURNING_LOCAL;
        }

        (REMOTE(_), ConnectionLost) => {
            force_release_all_everywhere();
            active_target = LOCAL_TARGET;
            state = RECOVERING;
        }

        (RECOVERING, CleanupComplete) => {
            state = LOCAL;
        }

        _ => log_ignored_transition(state, event),
    }
}
```

### 4.2. Key and button state

#### 4.2.1. Minimum registry

- Physically pressed keys.
- Physically pressed buttons.
- Active modifiers.
- Destination that received each press event.
- Monotonic event sequence.

#### 4.2.2. Consistency rule

A release must be sent to the same destination that received the press, unless a global recovery runs.

#### 4.2.3. Pseudocode

```rust
on_key_event(code, value):
    if value == PRESSED:
        destination = active_target
        pressed_keys[code] = destination
        send(destination, key_press(code))

    if value == RELEASED:
        destination = pressed_keys.remove(code).unwrap_or(active_target)
        send(destination, key_release(code))

release_all():
    for (key, destination) in pressed_keys:
        send(destination, key_release(key))
    for (button, destination) in pressed_buttons:
        send(destination, button_release(button))
    pressed_keys.clear()
    pressed_buttons.clear()
```

### 4.3. Edge transition state

#### 4.3.1. Parameters

- `cooldown_ms`: initial 200 ms.
- `inset_px`: 4 to 10 pixels.
- `minimum_velocity`: configurable.
- `edge_activation_delay_ms`: 0 to 500 ms.
- `allow_drag_transition`: false in MVP.

#### 4.3.2. Bounce prevention

```text
ARMED -> EDGE_HIT -> SWITCHING -> COOLDOWN -> ARMED
```

#### 4.3.3. Pseudocode

```rust
on_barrier_hit(edge, position, pointer_state):
    if transition_state != ARMED:
        return

    if pointer_state.any_button_pressed and not allow_drag_transition:
        return

    if not edge_is_enabled(edge):
        return

    transition_state = EDGE_HIT
    peer = layout.neighbor_for(edge, position)

    if peer is None:
        transition_state = ARMED
        return

    request_switch(peer, normalize(position))
    transition_state = SWITCHING

on_switch_confirmed(peer):
    transition_state = COOLDOWN(until = now + cooldown_ms)

on_tick():
    if transition_state is COOLDOWN and now >= until:
        transition_state = ARMED
```

---

## 5. Implementation roadmap to depth 4

# Phase 0. Discovery, governance, and reproducible base

## 0.1. Technical audit of rkvm

### 0.1.1. Map the repository

#### 0.1.1.1. Tasks

- Identify crates and responsibilities.
- Locate client selection logic.
- Locate switch-key registration.
- Locate disconnect handling.
- Review existing `systemd` units.
- Review protocol serialization.
- Document where to introduce the control socket.

#### 0.1.1.2. Deliverables

- `docs/upstream-architecture.md`
- Dependency diagram.
- Extension point list.
- Fork risk register.

#### 0.1.1.3. Testing

- Build the base commit without modifications.
- Run all upstream tests.
- Record initial coverage.
- Test connection between two clean machines.

#### 0.1.1.4. Acceptance criteria

The team can explain and demonstrate the full path of an event from `evdev` to `uinput`, including authentication, serialization, and recovery.

## 0.2. Define fork governance

### 0.2.1. Branch policy

#### 0.2.1.1. Branches

```text
main                stable product branch
upstream-sync       clean rkvm mirror
develop             feature integration
feature/*           isolated changes
release/*           stabilization
hotfix/*            urgent fixes
```

#### 0.2.1.2. Rules

- Every change must include tests.
- Do not rewrite `main` history.
- Keep upstream sync commits separate.
- Use feature flags for experimental changes.

#### 0.2.1.3. Testing

- Simulate an upstream update.
- Resolve conflicts on a temporary branch.
- Confirm the protocol remains compatible.

#### 0.2.1.4. Acceptance criteria

An upstream commit can be incorporated without mixing it with local changes and without breaking the build.

## 0.3. Reproducible lab

### 0.3.1. Physical environment

#### 0.3.1.1. Minimum matrix

- PC A: Ubuntu 26.04, physical keyboard and mouse.
- PC B: Ubuntu 26.04, no dedicated keyboard or mouse.
- Two monitors.
- Preferred Ethernet connection.
- Wi-Fi as a secondary scenario.
- Recovery SSH access.

#### 0.3.1.2. Instrumentation

- `journalctl` for logs.
- `evtest` to validate physical events.
- `uinput` device inspection tool.
- Latency captures without logging keys.
- Network disconnect monitor.

#### 0.3.1.3. Testing

- Reboot both machines.
- Start the receiver before GDM.
- Network loss and return.
- Suspend and resume.
- Resolution change.

#### 0.3.1.4. Acceptance criteria

The lab can be reproduced from documentation in under one hour and always has a recovery mechanism.

---

# Phase 1. Minimal fork and local control plane

## 1.1. Introduce a destination abstraction

### 1.1.1. Extract selection logic

#### 1.1.1.1. Design

Create an internal interface:

```rust
trait TargetController {
    fn active_target(&self) -> TargetId;
    async fn switch_to(&mut self, target: TargetId) -> Result<TransitionId>;
    async fn switch_local(&mut self) -> Result<TransitionId>;
    async fn release_all(&mut self) -> Result<()>;
}
```

#### 1.1.1.2. Implementation

- Adapt existing shortcuts to call `TargetController`.
- Do not duplicate switch logic.
- Emit internal state events.
- Use stable identifiers, not list indices.

#### 1.1.1.3. Testing

**Unit**

- Switch from local to a connected client.
- Reject a nonexistent client.
- Reject a disconnected client.
- Make `switch_local` idempotent.
- Verify that two simultaneous commands are serialized.

**Pseudotest**

```rust
#[test]
async fn disconnected_peer_cannot_be_selected() {
    let mut controller = fixture_with_peer("b", connected=false);
    let result = controller.switch_to("b").await;

    assert_error(result, PeerUnavailable);
    assert_eq!(controller.active_target(), LOCAL_TARGET);
}
```

#### 1.1.1.4. Acceptance criteria

Original shortcuts keep working and destination selection can be invoked from a single internal API.

## 1.2. Unix control socket

### 1.2.1. IPC protocol

#### 1.2.1.1. Format

Use JSON Lines for the MVP:

```json
{"id":"1","command":"status"}
{"id":"2","command":"switch","target":"peer-b"}
{"id":"3","command":"release_all"}
```

Response:

```json
{"id":"2","ok":true,"transition_id":"...","active_target":"peer-b"}
```

#### 1.2.1.2. Security

- Socket at `/run/app-name/control.sock`.
- Owner `root:app-name`.
- Mode `0660`.
- Validate process credentials via `SO_PEERCRED`.
- Limit maximum message size.
- Apply a read timeout.
- Do not accept arbitrary paths or commands.

#### 1.2.1.3. Testing

- Authorized user can query and switch.
- Unauthorized user receives `Permission denied`.
- Invalid JSON does not stop the daemon.
- Oversized message is rejected.
- Client disconnecting mid-message does not block the server.
- Stress test of 1,000 concurrent `status` commands.

#### 1.2.1.4. Acceptance criteria

GUI and CLI can control the destination without simulating shortcuts and without administrative privileges.

## 1.3. Create `kvmctl`

### 1.3.1. Initial commands

#### 1.3.1.1. Interface

```text
kvmctl status --json
kvmctl peers
kvmctl switch peer-b
kvmctl local
kvmctl release-all
```

#### 1.3.1.2. Behavior

- Human-readable output by default.
- Stable JSON with `--json`.
- Defined exit codes.
- Configurable timeout.

#### 1.3.1.3. Testing

- Human output snapshot.
- JSON schema validation.
- Missing socket.
- Stopped daemon.
- Disconnected peer.
- Command canceled with `Ctrl+C`.

#### 1.3.1.4. Acceptance criteria

An operator can always recover local control using `kvmctl local` over SSH.

## 1.4. Safe operational telemetry

### 1.4.1. Allowed events

#### 1.4.1.1. Log

- Connection and disconnection.
- Destination change.
- Aggregated latency.
- Protocol version.
- `uinput` and network errors.

#### 1.4.1.2. Do not log

- Individual key codes.
- Typed text.
- Permanent detailed coordinates.
- Passwords or tokens.

#### 1.4.1.3. Testing

- Run a sequence with known text.
- Search that text and key codes in logs.
- Fail the test if sensitive information appears.

#### 1.4.1.4. Acceptance criteria

Logs allow diagnosing connections without reconstructing what the user typed.

---

# Phase 2. Input robustness and GDM operation

## 2.1. Pre-session receiver service

### 2.1.1. `systemd` unit

#### 2.1.1.1. Dependencies

```ini
[Unit]
After=network-online.target
Wants=network-online.target
Before=display-manager.service

[Service]
ExecStart=/usr/libexec/app-name/kvm-client --config /etc/app-name/client.toml
Restart=always
RestartSec=2
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

The final unit must match real `uinput`, network, and key-file permissions. Hardening options must not be copied blindly if they block the minimum required access.

#### 2.1.1.2. Boot order

- Load `uinput` via `modules-load.d`.
- Create permissions via `udev` rules.
- Wait for usable network.
- Start client.
- Create virtual keyboard and mouse.
- Start GDM.

#### 2.1.1.3. Testing

- Cold boot through GDM.
- Service restart with GDM active.
- Network delayed 30 seconds.
- Primary server off during boot.
- Server available after 60 seconds.
- Type and erase password characters.
- Verify configured keyboard layout.

#### 2.1.1.4. Acceptance criteria

The secondary PC reaches GDM without automatic login and can receive keyboard and mouse after the server connects.

## 2.2. Held-key recovery

### 2.2.1. Per-destination tracking

#### 2.2.1.1. Critical cases

- Press `Shift`, switch destination, and release.
- Press left button during network drop.
- Hold a key across suspend.
- Physically disconnect the keyboard.

#### 2.2.1.2. Algorithm

```rust
on_target_change(old, new):
    release_all_for(old)
    clear_transient_state()
    activate(new)

on_connection_lost(peer):
    release_all_for(peer)
    active_target = LOCAL_TARGET
    synthesize_local_releases_if_required()
```

#### 2.2.1.3. Testing

- Unit tests for each modifier.
- Tests with buttons 1 through 5.
- Property-based testing with random press/release/switch sequences.
- Invariant: after a sequence finishes, no keys remain active.

#### 2.2.1.4. Acceptance criteria

No disconnect test leaves keys or buttons logically pressed.

## 2.3. Local fail-safe

### 2.3.1. Escape mechanisms

#### 2.3.1.1. Options

- Reserved shortcut to return local.
- Configurable timeout without remote activity.
- `kvmctl local` over SSH.
- Safe daemon restart.
- Optional watchdog.

#### 2.3.1.2. Primary rule

Under state uncertainty, prioritize return to the local machine and release all inputs.

#### 2.3.1.3. Testing

- Unplug network cable during remote control.
- Kill the client process.
- Kill the server process.
- Freeze packets with temporary network rules.
- Close the lid or suspend the secondary.

#### 2.3.1.4. Acceptance criteria

Control returns to the primary within a defined maximum time, initially 1 second for confirmed connection loss.

## 2.4. GDM-specific tests

### 2.4.1. Authentication matrix

#### 2.4.1.1. Cases

- Correct password.
- Incorrect password.
- Caps Lock.
- Spanish keyboard and special characters.
- Different user.
- Locked screen after login.

#### 2.4.1.2. Privacy

Automated tests must not use real passwords. Create lab users with temporary credentials.

#### 2.4.1.3. Testing

- Automate only lab session events and states.
- Confirm the daemon survives the GDM -> user handoff.
- Confirm virtual devices are not duplicated.

#### 2.4.1.4. Acceptance criteria

Control continues without restarting the daemon when GDM hands the session to the user.

---

# Phase 3. Session agent and edge backend

## 3.1. Agent internal API

### 3.1.1. Modules

#### 3.1.1.1. Interfaces

```rust
trait DisplayTopologyProvider {
    async fn topology(&self) -> Result<DisplayTopology>;
    fn watch_changes(&self) -> EventStream<DisplayTopology>;
}

trait EdgeCaptureBackend {
    async fn register(&mut self, barriers: Vec<Barrier>) -> Result<()>;
    fn events(&self) -> EventStream<EdgeEvent>;
    async fn suspend(&mut self);
    async fn resume(&mut self);
}

trait DaemonControl {
    async fn switch_to(&self, peer: PeerId, entry: EntryPoint) -> Result<TransitionId>;
    async fn local(&self) -> Result<()>;
}
```

#### 3.1.1.2. Backends

- `portal-wayland`
- `x11-xinput2`, compatibility and tests only.
- `mock`, for CI.

#### 3.1.1.3. Testing

- Mock backend emits deterministic edges.
- Topology changes invalidate old barriers.
- Portal restart does not stop the agent.
- Unavailable daemon socket produces degraded mode.

#### 3.1.1.4. Acceptance criteria

Business logic does not depend directly on GNOME, D-Bus, or X11.

## 3.2. Wayland integration via portal

### 3.2.1. Permission lifecycle

#### 3.2.1.1. Flow

```text
Session start
 -> create portal session
 -> request devices/capabilities
 -> define barriers
 -> show consent if needed
 -> enable capture
 -> receive events
 -> close session on exit
```

#### 3.2.1.2. Considerations

- The portal may require interaction the first time.
- The session may invalidate on lock, suspend, or user switch.
- The agent must re-request capabilities without entering a dialog loop.
- There must be a clear indicator if permission was denied.

Deskflow documents that Wayland support depends on input capture and remote desktop portals available in the desktop environment. citeturn3search48

#### 3.2.1.3. Testing

- Permission granted.
- Permission denied.
- Portal unavailable.
- `xdg-desktop-portal` restart.
- Session lock and unlock.
- Logout and new login.
- Two local monitors with different scales.

#### 3.2.1.4. Acceptance criteria

With permission granted, a configured edge generates a single transition event per intentional crossing.

## 3.3. X11 backend for development

### 3.3.1. Goal

#### 3.3.1.1. Scope

- Quickly validate topology and UX.
- Enable CI with virtual X.
- Do not make X11 a product requirement.

#### 3.3.1.2. Pseudocode

```rust
loop every 8_ms:
    position = query_pointer()
    screen = topology.screen_at(position)

    if position touches enabled_edge(screen):
        emit EdgeEvent(screen, edge, normalized_axis_position)
```

#### 3.3.1.3. Testing

- Xvfb with synthetic resolutions.
- Left, right, top, and bottom edges.
- Corners with two possible neighbors.
- Fast movement that skips several pixels.

#### 3.3.1.4. Acceptance criteria

The transition machine is validated before completing the portal backend.

## 3.4. Hysteresis and edge zones

### 3.4.1. Barrier model

#### 3.4.1.1. Data

```rust
struct Barrier {
    display_id: DisplayId,
    edge: Edge,
    range_start: f32,
    range_end: f32,
    destination: PeerId,
    activation_delay_ms: u32,
    cooldown_ms: u32,
}
```

#### 3.4.1.2. Rules

- Normalized ranges from `0.0` to `1.0`.
- Avoid non-deterministic overlap.
- Define explicit corner priority.
- Do not activate during a drag in the MVP.

#### 3.4.1.3. Testing

- Full range.
- Upper and lower halves with different destinations.
- Coordinate exactly on the boundary.
- Overlapping barriers rejected by validation.
- Repeated bounce within under 200 ms.

#### 3.4.1.4. Acceptance criteria

A barrier produces deterministic transitions even with several configured neighbors.

---

# Phase 4. Symmetric coordination and visual continuity

## 4.1. Inter-agent transition protocol

### 4.1.1. Entry handshake

#### 4.1.1.1. Sequence

```text
A detects edge
A -> B: PREPARE_ENTRY(position, edge, transition_id)
B validates session and edge
B -> A: READY(transition_id)
A switches destination in daemon
A -> B: COMMIT(transition_id)
B activates cooldown and entry point
```

#### 4.1.1.2. Pseudocode

```rust
async fn transition_to(peer, edge, position):
    id = uuid()
    normalized = normalize(position)

    ready = send_prepare(peer, id, opposite(edge), normalized)
        .timeout(300_ms)
        .await?

    if not ready:
        return error("peer agent not ready")

    daemon.switch_to(peer, EntryPoint(opposite(edge), normalized)).await?
    send_commit(peer, id).await?
```

#### 4.1.1.3. Testing

- Normal handshake.
- Lost `READY`.
- Duplicate `COMMIT`.
- Stale or repeated IDs.
- Remote agent absent but daemon connected.
- Incompatible versions.

#### 4.1.1.4. Acceptance criteria

Never switch to a remote machine whose agent has not confirmed it can manage return, unless the user forces the switch via shortcut.

## 4.2. Coordinate transformation

### 4.2.1. Normalized position

#### 4.2.1.1. Formula

```text
normalized = clamp(local_axis / local_axis_length, 0.0, 1.0)
remote_axis = round(normalized * remote_axis_length)
remote_axis = clamp(remote_axis, inset, remote_axis_length - inset)
```

#### 4.2.1.2. Modes

- Proportional.
- Top alignment.
- Center alignment.
- Bottom alignment.
- Approximate physical scale using size and DPI.

#### 4.2.1.3. Testing

- 1080p -> 1440p.
- 1440p -> 1080p.
- Portrait -> landscape.
- 100% -> 150% scale.
- Coordinates 0, 0.5, and 1.
- Out-of-range values.

#### 4.2.1.4. Acceptance criteria

Deviation from the expected proportional position is under two logical pixels in lab scenarios.

## 4.3. Return from remote

### 4.3.1. Symmetric detection

#### 4.3.1.1. Flow

- Agent B knows entry came from the left edge.
- Temporarily registers the return edge.
- On hit, requests `RETURN` to A.
- A releases remote inputs and activates local.
- Both agents enter cooldown.

#### 4.3.1.2. Pseudocode

```rust
on_remote_return_edge(transition_context):
    send_return_request(owner, transition_context.id)

on_return_request(id):
    if id != active_transition.id:
        reject_stale_request()
        return

    daemon.release_all()
    daemon.switch_local()
    send_return_confirmed(id)
```

#### 4.3.1.3. Testing

- Normal return.
- Return under high latency.
- B suspends before returning.
- A restarts its agent but not the daemon.
- Duplicate return request.

#### 4.3.1.4. Acceptance criteria

The user can go and return via edges without using shortcuts during a normal session.

## 4.4. Continuity and latency

### 4.4.1. Initial budget

#### 4.4.1.1. Goals

- Wired LAN transition confirmation: under 100 ms.
- Motion events after the switch: no prolonged perceptible pause.
- Loss recovery: under 1 second.
- No event sent to the wrong destination after commit.

#### 4.4.1.2. Measurement

- Monotonic timestamps.
- Aggregated metrics per transition.
- Do not log keys.
- p50, p95, and p99 percentiles.

#### 4.4.1.3. Testing

- Artificial latency of 10, 50, 100, and 250 ms.
- 1%, 5%, and 10% loss.
- Control-message reordering.
- Congested Wi-Fi.

#### 4.4.1.4. Acceptance criteria

The transition fails safely when it exceeds the timeout and never leaves control in an ambiguous state.

---

# Phase 5. Graphical interface and configuration experience

## 5.1. Visual topology model

### 5.1.1. Layout editor

#### 5.1.1.1. Features

- Draggable rectangles per computer.
- Monitor indicators per computer.
- Magnetic edge snapping.
- Overlap validation.
- Direction arrows.
- Active zone preview.

#### 5.1.1.2. Data model

```json
{
  "layout_version": 1,
  "local_peer": "peer-a",
  "nodes": [
    {"peer":"peer-a","x":0,"y":0,"width":1920,"height":1080},
    {"peer":"peer-b","x":1920,"y":0,"width":2560,"height":1440}
  ],
  "barriers": [
    {"from":"peer-a","edge":"right","to":"peer-b","start":0.0,"end":1.0}
  ]
}
```

#### 5.1.1.3. Testing

- Serialization unit tests.
- `layout_version` migration.
- Drag with UI scaling.
- Full keyboard navigation.
- Screen readers and contrast.

#### 5.1.1.4. Acceptance criteria

A user can configure two machines without editing files manually.

## 5.2. Pairing

### 5.2.1. Secure flow

#### 5.2.1.1. Steps

1. Discover or enter IP.
2. Exchange public identity.
3. Show a short phrase or code on both machines.
4. Confirm physically on both.
5. Save certificate and permissions.
6. Test connection.

#### 5.2.1.2. Security

- Do not rely only on network discovery.
- Replay protection.
- Rotation and revocation.
- Show full fingerprint in advanced mode.
- Do not invent custom cryptography; reuse TLS and existing identities.

#### 5.2.1.3. Testing

- Correct pairing.
- Different code.
- Simulated man-in-the-middle attack.
- Expired or changed certificate.
- Revocation and re-pairing.

#### 5.2.1.4. Acceptance criteria

An unauthorized machine cannot send input or issue switch commands.

## 5.3. Visual diagnostics

### 5.3.1. Status panel

#### 5.3.1.1. Show

- Daemon active.
- Agent and portal active.
- Peer connected.
- Current destination.
- Aggregated latency.
- Missing permissions.
- Emergency shortcut.

#### 5.3.1.2. Actions

- Return local.
- Release all keys.
- Restart agent.
- Export redacted diagnostics.
- Open documentation.

#### 5.3.1.3. Testing

- Simulate each error state.
- Confirm actionable messages.
- Verify export excludes sensitive data.

#### 5.3.1.4. Acceptance criteria

Common failures can be diagnosed without opening a terminal.

---

# Phase 6. Quality, CI, and test automation

## 6.1. Testing pyramid

### 6.1.1. Unit tests

#### 6.1.1.1. Coverage

- State machine.
- Key tracking.
- Hysteresis.
- Coordinate transformation.
- Configuration validation.
- IPC serialization.

#### 6.1.1.2. Tools

- Rust test runner.
- Property-based testing.
- Fuzzing for IPC and network parsers.

#### 6.1.1.3. Criteria

- High coverage on critical logic.
- Zero `panic` on malformed external data.
- Deterministic tests with a simulated clock.

#### 6.1.1.4. CI gate

No merge if a test, formatter, linter, or dependency analysis fails.

## 6.2. Integration tests

### 6.2.1. Virtual devices

#### 6.2.1.1. Scenarios

- Create virtual device.
- Send sequence.
- Verify reception.
- Destroy device.
- Restart daemon.

#### 6.2.1.2. Isolation

Use dedicated privileged Linux runners or virtual machines. Do not assume a common container has safe `uinput` access.

#### 6.2.1.3. Testing

- Stable device names.
- No duplicates after restart.
- Correct permissions.
- Coherent SYN events.

#### 6.2.1.4. Acceptance criteria

The capture -> simulated network -> injection chain is validated automatically.

## 6.3. Physical end-to-end tests

### 6.3.1. Two-machine bench

#### 6.3.1.1. Automation

- Optional remote power control.
- SSH to prepare scenarios.
- Centralized log capture.
- Temporary test user.

#### 6.3.1.2. Mandatory cases

- Start at GDM and lab password.
- Login and agent activation.
- Crossing A -> B -> A.
- Suspend B during control.
- Disconnect network during control.
- Restart the daemon.
- Change resolution.
- Lock session.

#### 6.3.1.3. Privacy

Do not record video of real password entry. Use rotatable lab credentials.

#### 6.3.1.4. Acceptance criteria

The critical suite passes repeatedly overnight without losing local control.

## 6.4. Chaos testing

### 6.4.1. Injected failures

#### 6.4.1.1. Network

- Loss.
- Delay.
- Reordering.
- Brief cuts.

#### 6.4.1.2. Processes

- Kill daemon.
- Kill agent.
- Restart portal.
- Rotate logs.

#### 6.4.1.3. Hardware

- Disconnect mouse.
- Reconnect keyboard on another port.
- Suspend one PC.
- Turn off monitor without turning off PC.

#### 6.4.1.4. Acceptance criteria

Each failure has a defined final state and a documented recovery path.

---

# Phase 7. Security hardening

## 7.1. Threat model

### 7.1.1. Actors

#### 7.1.1.1. Threats

- Malicious machine on the LAN.
- Unauthorized local user.
- Previously authorized peer later compromised.
- Manipulated configuration file.
- Malformed network message.

#### 7.1.1.2. Assets

- Ability to type into GDM.
- Keyboard and mouse control.
- Private keys.
- Peer list.
- Local control availability.

#### 7.1.1.3. Testing

- STRIDE review.
- Protocol fuzzing.
- Filesystem permission tests.
- Dependency audit.

#### 7.1.1.4. Acceptance criteria

There is no known path for an unauthorized local user to control the daemon or read secrets.

## 7.2. Least privilege

### 7.2.1. Design

#### 7.2.1.1. Measures

- Dedicated system user.
- Dedicated group for IPC.
- Specific `udev` rules.
- Directories with restrictive permissions.
- Experimentally validated `systemd` hardening.

#### 7.2.1.2. Prohibitions

- Do not run GUI as root.
- Do not use an unauthenticated local TCP socket.
- Do not `chmod 666 /dev/uinput`.
- Do not store world-readable private keys.

#### 7.2.1.3. Testing

- User outside the group.
- Compromised GUI process.
- Key reading.
- System configuration writing.

#### 7.2.1.4. Acceptance criteria

Compromising the GUI does not automatically grant reading of physical devices or private keys.

## 7.3. Supply chain

### 7.3.1. Dependencies and releases

#### 7.3.1.1. Measures

- Versioned lockfile.
- SBOM per release.
- Artifact signatures.
- Reproducible builds when possible.
- Vulnerability scanning.

#### 7.3.1.2. Testing

- Verify package signatures.
- Compare build hashes.
- Simulate a withdrawn dependency.

#### 7.3.1.3. Acceptance criteria

Each release can be linked to a commit, an SBOM, and signed artifacts.

#### 7.3.1.4. Documentation

Publish a security reporting procedure and response timelines.

---

# Phase 8. Packaging, deployment, and support

## 8.1. Debian package

### 8.1.1. Contents

#### 8.1.1.1. Files

- Binaries in `/usr/bin` and `/usr/libexec`.
- `systemd` units.
- `udev` rules.
- Permission policies.
- Example configuration.
- Configuration migrations.

#### 8.1.1.2. Maintainer scripts

- Create system user and group.
- Do not overwrite existing keys.
- Reload `systemd` and required rules.
- Separate installation from activation.

#### 8.1.1.3. Testing

- Install.
- Upgrade.
- Uninstall while preserving configuration where appropriate.
- Full purge.
- Controlled downgrade.

#### 8.1.1.4. Acceptance criteria

A clean install can complete via package without dangerous manual steps.

## 8.2. Onboarding

### 8.2.1. Initial assistant

#### 8.2.1.1. Steps

1. Verify `uinput`.
2. Verify service.
3. Choose primary or secondary.
4. Pair.
5. Configure emergency shortcut.
6. Test round trip.
7. Authorize portal.
8. Arrange screens.

#### 8.2.1.2. Testing

- User without permissions.
- Portal denied.
- Firewall blocking.
- Changing IP.
- Peer with incompatible version.

#### 8.2.1.3. Acceptance criteria

The assistant does not leave the keyboard captured if a stage fails.

#### 8.2.1.4. Recovery

Always show instructions for `kvmctl local` and disabling the service.

## 8.3. Support observability

### 8.3.1. Diagnostic bundle

#### 8.3.1.1. Include

- Versions.
- Service status.
- Redacted configuration.
- Latest operational logs.
- Topology and graphical backend.

#### 8.3.1.2. Exclude

- Keys.
- Passwords.
- Input content.
- Pairing tokens.

#### 8.3.1.3. Testing

- Automatic secret scanner.
- Review of user paths.
- Configurations with synthetic sensitive values.

#### 8.3.1.4. Acceptance criteria

The bundle can be shared in a public issue without exposing credentials.

---

## 6. Version strategy

### 6.1. `0.1.0`: programmatic control

- Buildable fork.
- Unix socket.
- `kvmctl`.
- Original shortcuts preserved.
- Unit and permission tests.

### 6.2. `0.2.0`: robust GDM

- Boot services.
- Local recovery.
- Key release.
- GDM tests.

### 6.3. `0.3.0`: experimental X11 agent

- Minimal layout editor.
- Edge detection on X11.
- Hysteresis.
- Shortcut return.

### 6.4. `0.4.0`: experimental Wayland

- Portal backend.
- Permissions and lifecycle.
- Two machines.
- Edge return.

### 6.5. `0.5.0`: full coordination

- Inter-agent handshake.
- Normalized coordinates.
- Network recovery.
- Latency metrics.

### 6.6. `0.8.0`: beta

- Configuration GUI.
- Pairing.
- `.deb` package.
- Physical E2E suite.

### 6.7. `1.0.0`: stable

- Ubuntu 26.04 officially supported.
- Wayland and GDM validated.
- Complete documentation.
- Published threat model.
- Signed builds.
- Update and rollback procedure.

---

## 7. Acceptance test matrix

### 7.1. Before login

- [ ] Client starts before GDM.
- [ ] Virtual keyboard appears exactly once.
- [ ] Secondary can be selected via shortcut.
- [ ] A lab password can be typed.
- [ ] Connection loss returns local control.
- [ ] Automatic login is not required.

### 7.2. Inside the session

- [ ] Agent detects Wayland.
- [ ] User can grant portal permission.
- [ ] Right edge switches to secondary.
- [ ] Left edge returns to primary.
- [ ] No visible bounce.
- [ ] Emergency shortcut remains available.

### 7.3. Robustness

- [ ] No held keys remain.
- [ ] Suspend recovers.
- [ ] Resolution change rebuilds barriers.
- [ ] Agent restart does not stop rkvm.
- [ ] Daemon crash has documented recovery.
- [ ] Logs contain no keystrokes.

### 7.4. Security

- [ ] IPC restricted by group and credentials.
- [ ] TLS required.
- [ ] Unauthorized peer rejected.
- [ ] Private keys protected.
- [ ] Unprivileged GUI.
- [ ] Diagnostic bundle without secrets.

---

## 8. Main risks and mitigations

### 8.1. Portal does not provide required behavior

**Risk:** differences across GNOME versions or portal limitations.  
**Mitigation:** abstract backend, keep X11 for diagnostics, retain shortcuts, detect capabilities, and degrade clearly.

### 8.2. Edge bounce

**Risk:** unintended A -> B -> A switches.  
**Mitigation:** cooldown, inset, transition IDs, and minimum movement direction.

### 8.3. Stuck keys

**Risk:** a destination receives press without release.  
**Mitigation:** per-destination tracking, `release_all`, property tests, and cleanup on disconnect.

### 8.4. Total loss of control

**Risk:** rkvm captures devices and an invalid configuration prevents local operation.  
**Mitigation:** SSH, reserved shortcut, watchdog, timeout, and default local return. The rkvm project itself recommends testing before enabling services because of input capture. citeturn6search125turn6search116

### 8.5. Fork maintenance

**Risk:** upstream divergence.  
**Mitigation:** small changes, independent modules, compatibility tests, and periodic sync.

### 8.6. Elevated attack surface

**Risk:** the application can type even into GDM.  
**Mitigation:** least privilege, explicit pairing, TLS, restricted Unix socket, auditing, and no unauthenticated TCP control exposure.

---

## 9. Global Definition of Done

The project can be considered ready for 1.0 when:

1. It works on two clean Ubuntu 26.04 installs.
2. It can type into GDM without automatic login.
3. It can go and return via edges inside GNOME Wayland.
4. It survives disconnects, suspend, and agent restart.
5. It never permanently loses local control in the test suite.
6. It does not log keyboard content.
7. It has a threat model, signed packages, and recovery documentation.
8. All unit, integration, E2E, security, and upgrade tests pass.
9. The emergency shortcut always remains operational.
10. The frontend works without administrative privileges.

---

## 10. Recommended starting work order

### Technical week 1

1. Freeze a stable rkvm commit.
2. Build and test between two machines.
3. Map destination selection.
4. Extract `TargetController`.
5. Create unit tests.

### Technical week 2

1. Implement Unix socket.
2. Implement `kvmctl`.
3. Add permissions and `SO_PEERCRED`.
4. Test recovery over SSH.
5. Run basic IPC parser fuzzing.

### Technical week 3

1. Strengthen key tracking.
2. Implement `release_all`.
3. Test GDM and boot.
4. Run network-loss scenarios.
5. Document the emergency mechanism.

### Technical week 4

1. Create `EdgeCaptureBackend` interface.
2. Implement mock backend.
3. Implement hysteresis machine.
4. Build X11 validation backend.
5. Validate UX in the lab.

### Technical weeks 5 and 6

1. Implement Wayland portal backend.
2. Manage permissions and lifecycle.
3. Coordinate agents.
4. Implement symmetric return.
5. Test scaling and topology changes.

### Technical weeks 7 and 8

1. Build initial GUI.
2. Implement pairing.
3. Create Debian package.
4. Automate physical E2E.
5. Prepare beta release.

Timelines are indicative and depend on prior experience with Rust, D-Bus, Wayland, portals, `evdev`, `uinput`, and Debian packaging.

---

## 11. Technical reference sources

- rkvm repository and general architecture, including display-server independence, TLS, `uinput`, crates, and services: citeturn6search116turn7search140
- Linux kernel documentation on creating virtual devices via `uinput`: citeturn6search118turn6search120
- Kernel documentation on the input subsystem and `evdev`: citeturn6search140
- Wayland support and dependence on capture portals in Deskflow: citeturn3search48
- rkvm service configuration and client/server parameter reference: citeturn7search147
- Topology and flexible-edge design references in modern software KVM: citeturn6search132turn6search133

---

## 12. Final architectural decision

The recommendation is **not to convert rkvm into a library immediately or rewrite its protocol**. The lowest-risk path is:

1. Keep the rkvm daemon as the input and network engine.
2. Extract an internal destination-selection abstraction.
3. Expose that abstraction via a restricted Unix socket.
4. Create a decoupled session agent for edges and topology.
5. Use swappable backends for Wayland, X11, and tests.
6. Keep shortcut switching as a permanent emergency mechanism.
7. Add symmetric inter-agent coordination only after stabilizing GDM and the control plane.

This architecture preserves rkvm's main advantage—operating without a display server—and adds the visual experience only when a session can provide it.
