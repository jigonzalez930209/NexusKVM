# Architecture

## Processes

1. The daemon can be started from the Tauri app (generates certificates, config, and process) or as a system service with `nexus-kvmd --config`.
2. `nexus-agent`: session agent that registers barriers.
3. `NexusKVM`: unprivileged Tauri UI.
- `nexus-agent` on the client uses `:5259` to ask the host to return local (`SwitchLocal`) and to sync clipboard.
- `nexusctl`: recovery and automation (`NEXUSKVM_TOKEN` required).

## Invariants

- Under uncertainty, the destination returns to `local`.
- Each release is sent to the destination that received the press.
- The daemon never logs key codes.
- The GUI does not access evdev/uinput.
- A transition is identified by UUID and is not reused.

## Real integration

The `InputTransport` trait decouples UI and internal state from the fork. `RkvmAdapter` wraps `TargetHandle` from `rkvm-server`. Peers are registered on the input plane only **after** TLS and the password challenge succeed. Held keys are tracked **per destination** so a release is sent where the press went.
