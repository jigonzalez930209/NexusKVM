# Architecture

## Processes

1. The daemon can be started from the Tauri app (generates certificates, config, and process) or as a system service with `nexus-kvmd --config`.
2. `nexus-agent`: session agent that registers barriers.
3. `NexusKVM`: unprivileged Tauri UI.
4. `nexusctl`: recovery and automation.

## Invariants

- Under uncertainty, the destination returns to `local`.
- Each release is sent to the destination that received the press.
- The daemon never logs key codes.
- The GUI does not access evdev/uinput.
- A transition is identified by UUID and is not reused.

## Real integration

The `InputTransport` trait decouples UI and internal state from the fork. `RkvmAdapter` wraps `TargetHandle` from `rkvm-server`.
