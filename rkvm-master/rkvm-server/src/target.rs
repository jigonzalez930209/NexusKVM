use rkvm_input::key::Key;
use std::collections::{BTreeMap, HashMap, VecDeque};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};

pub const LOCAL_TARGET: &str = "local";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSnapshot {
    pub id: String,
    pub address: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub active_target: String,
    pub peers: Vec<PeerSnapshot>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            active_target: LOCAL_TARGET.to_string(),
            peers: Vec::new(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TargetError {
    #[error("peer unavailable: {0}")]
    PeerUnavailable(String),
    #[error("transition in progress")]
    TransitionInProgress,
    #[error("control closed")]
    Closed,
}

pub type TransitionId = String;

#[derive(Debug, Clone)]
struct PeerEntry {
    address: String,
    connected: bool,
}

/// Target selection extracted from rkvm's `current` index.
///
/// Shortcut cycle order is: local, then connected peers in insertion order.
/// IDs are stable (client address), not slab indices.
#[derive(Debug)]
pub struct TargetRouter {
    active: String,
    previous: String,
    chord_changed: bool,
    busy: bool,
    order: VecDeque<String>,
    peers: BTreeMap<String, PeerEntry>,
    held_keys: HashMap<Key, String>,
    next_transition: u64,
    pending_warp: Option<(u8, f32)>,
}

impl Default for TargetRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl TargetRouter {
    pub fn new() -> Self {
        Self {
            active: LOCAL_TARGET.to_string(),
            previous: LOCAL_TARGET.to_string(),
            chord_changed: false,
            busy: false,
            order: VecDeque::new(),
            peers: BTreeMap::new(),
            held_keys: HashMap::new(),
            next_transition: 0,
            pending_warp: None,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            active_target: self.active.clone(),
            peers: self.peers(),
        }
    }

    pub fn active_target(&self) -> &str {
        &self.active
    }

    /// Destination for events from the current key chord (shortcut).
    pub fn event_target(&self) -> &str {
        if self.chord_changed {
            &self.previous
        } else {
            &self.active
        }
    }

    pub fn peers(&self) -> Vec<PeerSnapshot> {
        self.order
            .iter()
            .filter_map(|id| {
                self.peers.get(id).map(|p| PeerSnapshot {
                    id: id.clone(),
                    address: p.address.clone(),
                    connected: p.connected,
                })
            })
            .collect()
    }

    pub fn insert_peer(&mut self, id: String, address: String) {
        if !self.order.contains(&id) {
            self.order.push_back(id.clone());
        }
        self.peers.insert(
            id,
            PeerEntry {
                address,
                connected: true,
            },
        );
    }

    /// Remove a peer. If it was the active target, fall back to local and
    /// return held keys so the caller can emit releases.
    pub fn remove_peer(&mut self, id: &str) -> Vec<(Key, String)> {
        self.order.retain(|x| x != id);
        self.peers.remove(id);
        if self.active == id {
            self.fail_local()
        } else {
            Vec::new()
        }
    }

    pub fn mark_disconnected(&mut self, id: &str) -> Vec<(Key, String)> {
        if let Some(peer) = self.peers.get_mut(id) {
            peer.connected = false;
        }
        if self.active == id {
            self.fail_local()
        } else {
            Vec::new()
        }
    }

    fn fail_local(&mut self) -> Vec<(Key, String)> {
        self.previous = self.active.clone();
        self.active = LOCAL_TARGET.to_string();
        self.chord_changed = false;
        self.busy = false;
        self.drain_held()
    }

    fn alloc_transition(&mut self) -> TransitionId {
        self.next_transition += 1;
        format!("{:016x}", self.next_transition)
    }

    fn connected(&self, id: &str) -> bool {
        self.peers.get(id).map(|p| p.connected).unwrap_or(false)
    }

    pub fn prepare(&mut self, id: &str, warp: Option<(u8, f32)>) -> Result<(), TargetError> {
        if self.busy {
            return Err(TargetError::TransitionInProgress);
        }
        if !self.connected(id) {
            return Err(TargetError::PeerUnavailable(id.to_string()));
        }
        self.pending_warp = warp;
        Ok(())
    }

    pub fn take_warp(&mut self) -> Option<(u8, f32)> {
        self.pending_warp.take()
    }

    fn apply_switch(&mut self, id: &str, from_chord: bool) -> Result<TransitionId, TargetError> {
        if self.busy {
            return Err(TargetError::TransitionInProgress);
        }
        if !self.connected(id) {
            return Err(TargetError::PeerUnavailable(id.to_string()));
        }
        self.busy = true;
        self.previous = self.active.clone();
        self.active = id.to_string();
        self.chord_changed = from_chord;
        let t = self.alloc_transition();
        self.busy = false;
        Ok(t)
    }

    pub fn switch_to(&mut self, id: &str) -> Result<TransitionId, TargetError> {
        self.apply_switch(id, false)
    }

    pub fn switch_local(&mut self) -> Result<TransitionId, TargetError> {
        self.apply_local(false)
    }

    fn apply_local(&mut self, from_chord: bool) -> Result<TransitionId, TargetError> {
        if self.busy {
            return Err(TargetError::TransitionInProgress);
        }
        if self.active == LOCAL_TARGET && !from_chord {
            return Ok(self.alloc_transition());
        }
        self.busy = true;
        self.previous = self.active.clone();
        self.active = LOCAL_TARGET.to_string();
        self.chord_changed = from_chord;
        if !from_chord {
            self.held_keys.clear();
        }
        let t = self.alloc_transition();
        self.busy = false;
        Ok(t)
    }

    /// Shortcut cycle: local -> peer0 -> peer1 -> ... -> local.
    pub fn switch_next(&mut self) -> Result<TransitionId, TargetError> {
        let mut cycle = vec![LOCAL_TARGET.to_string()];
        cycle.extend(self.order.iter().filter(|id| self.connected(id)).cloned());
        let i = cycle.iter().position(|id| id == &self.active).unwrap_or(0);
        let next = cycle[(i + 1) % cycle.len()].clone();
        if next == LOCAL_TARGET {
            self.apply_local(true)
        } else {
            self.apply_switch(&next, true)
        }
    }

    pub fn finish_chord(&mut self) {
        self.chord_changed = false;
    }

    pub fn note_key(&mut self, key: Key, down: bool) {
        if down {
            self.held_keys.insert(key, self.event_target().to_string());
        } else {
            self.held_keys.remove(&key);
        }
    }

    pub fn drain_held(&mut self) -> Vec<(Key, String)> {
        self.held_keys.drain().collect()
    }

    pub fn restore_held(&mut self, key: Key, dest: String) {
        self.held_keys.insert(key, dest);
    }

    pub fn release_all(&mut self, peer: Option<&str>) -> Result<Vec<(Key, String)>, TargetError> {
        if let Some(id) = peer {
            if id != LOCAL_TARGET && !self.peers.contains_key(id) {
                return Err(TargetError::PeerUnavailable(id.to_string()));
            }
            let mut out = Vec::new();
            self.held_keys.retain(|k, dest| {
                if dest == id {
                    out.push((*k, dest.clone()));
                    false
                } else {
                    true
                }
            });
            return Ok(out);
        }
        Ok(self.drain_held())
    }
}

enum Command {
    Prepare {
        id: String,
        warp: Option<(u8, f32)>,
        reply: oneshot::Sender<Result<(), TargetError>>,
    },
    Activate {
        id: String,
        reply: oneshot::Sender<Result<TransitionId, TargetError>>,
    },
    Local {
        reply: oneshot::Sender<Result<TransitionId, TargetError>>,
    },
    ReleaseAll {
        peer: Option<String>,
        reply: oneshot::Sender<Result<Vec<(Key, String)>, TargetError>>,
    },
    Next {
        reply: oneshot::Sender<Result<TransitionId, TargetError>>,
    },
}

pub struct TargetControl {
    cmd_rx: mpsc::Receiver<Command>,
    snap_tx: watch::Sender<Snapshot>,
}

impl TargetControl {
    pub async fn recv(&mut self) -> Option<PendingCommand> {
        self.cmd_rx.recv().await.map(PendingCommand)
    }

    pub fn publish(&self, snapshot: Snapshot) {
        let _ = self.snap_tx.send(snapshot);
    }
}

pub struct PendingCommand(Command);

impl PendingCommand {
    pub fn apply(self, router: &mut TargetRouter) -> Vec<(Key, String)> {
        match self.0 {
            Command::Prepare { id, warp, reply } => {
                let _ = reply.send(router.prepare(&id, warp));
                Vec::new()
            }
            Command::Activate { id, reply } => {
                let keys = router.drain_held();
                let _ = reply.send(router.switch_to(&id));
                keys
            }
            Command::Local { reply } => {
                let keys = router.drain_held();
                let _ = reply.send(router.switch_local());
                keys
            }
            Command::ReleaseAll { peer, reply } => {
                let r = router.release_all(peer.as_deref());
                let keys = r.as_ref().ok().cloned().unwrap_or_default();
                let _ = reply.send(r);
                keys
            }
            Command::Next { reply } => {
                let keys = router.drain_held();
                let result = router.switch_next();
                // Command-driven switch is not a held chord: target new events
                // at the new machine immediately.
                if result.is_ok() {
                    router.finish_chord();
                }
                let _ = reply.send(result);
                keys
            }
        }
    }
}

#[derive(Clone)]
pub struct TargetHandle {
    cmd_tx: mpsc::Sender<Command>,
    snap_rx: watch::Receiver<Snapshot>,
}

impl TargetHandle {
    pub fn snapshot(&self) -> Snapshot {
        self.snap_rx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.snap_rx.clone()
    }

    pub async fn prepare(&self, id: &str, warp: Option<(u8, f32)>) -> Result<(), TargetError> {
        let (reply, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Prepare {
                id: id.to_string(),
                warp,
                reply,
            })
            .await
            .map_err(|_| TargetError::Closed)?;
        rx.await.map_err(|_| TargetError::Closed)?
    }

    pub async fn activate(&self, id: &str) -> Result<TransitionId, TargetError> {
        let (reply, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Activate {
                id: id.to_string(),
                reply,
            })
            .await
            .map_err(|_| TargetError::Closed)?;
        rx.await.map_err(|_| TargetError::Closed)?
    }

    pub async fn local(&self) -> Result<TransitionId, TargetError> {
        let (reply, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Local { reply })
            .await
            .map_err(|_| TargetError::Closed)?;
        rx.await.map_err(|_| TargetError::Closed)?
    }

    pub async fn release_all(&self, peer: Option<&str>) -> Result<Vec<(Key, String)>, TargetError> {
        let (reply, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::ReleaseAll {
                peer: peer.map(str::to_string),
                reply,
            })
            .await
            .map_err(|_| TargetError::Closed)?;
        rx.await.map_err(|_| TargetError::Closed)?
    }

    pub async fn switch_next(&self) -> Result<TransitionId, TargetError> {
        let (reply, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Next { reply })
            .await
            .map_err(|_| TargetError::Closed)?;
        rx.await.map_err(|_| TargetError::Closed)?
    }
}

pub fn control_pair() -> (TargetHandle, TargetControl) {
    control_pair_with(Snapshot::default())
}

pub fn control_pair_with(snapshot: Snapshot) -> (TargetHandle, TargetControl) {
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let (snap_tx, snap_rx) = watch::channel(snapshot);
    (
        TargetHandle { cmd_tx, snap_rx },
        TargetControl { cmd_rx, snap_tx },
    )
}

/// Process control commands against a `TargetRouter` (no evdev).
pub async fn drive(control: TargetControl) {
    drive_with(control, TargetRouter::new()).await;
}

pub async fn drive_with(mut control: TargetControl, mut router: TargetRouter) {
    control.publish(router.snapshot());
    while let Some(cmd) = control.recv().await {
        cmd.apply(&mut router);
        control.publish(router.snapshot());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_with_peer(id: &str, connected: bool) -> TargetRouter {
        let mut r = TargetRouter::new();
        r.insert_peer(id.into(), "127.0.0.1:1".into());
        if !connected {
            r.mark_disconnected(id);
        }
        r
    }

    #[test]
    fn switches_local_to_connected_client() {
        let mut c = fixture_with_peer("b", true);
        c.switch_to("b").unwrap();
        assert_eq!(c.active_target(), "b");
    }

    #[test]
    fn disconnected_peer_cannot_be_selected() {
        let mut c = fixture_with_peer("b", false);
        let result = c.switch_to("b");
        assert!(matches!(result, Err(TargetError::PeerUnavailable(_))));
        assert_eq!(c.active_target(), LOCAL_TARGET);
    }

    #[test]
    fn missing_peer_cannot_be_selected() {
        let mut c = TargetRouter::new();
        assert!(c.switch_to("ghost").is_err());
        assert_eq!(c.active_target(), LOCAL_TARGET);
    }

    #[test]
    fn switch_local_is_idempotent() {
        let mut c = TargetRouter::new();
        let first = c.switch_local().unwrap();
        let second = c.switch_local().unwrap();
        assert_ne!(first, second);
        assert_eq!(c.active_target(), LOCAL_TARGET);
    }

    #[test]
    fn lost_active_peer_returns_local() {
        let mut c = fixture_with_peer("b", true);
        c.switch_to("b").unwrap();
        c.remove_peer("b");
        assert_eq!(c.active_target(), LOCAL_TARGET);
    }

    #[test]
    fn lost_active_peer_drains_held_keys() {
        use rkvm_input::key::{Key, Keyboard};
        let mut c = fixture_with_peer("b", true);
        c.switch_to("b").unwrap();
        c.note_key(Key::Key(Keyboard::A), true);
        let released = c.remove_peer("b");
        assert_eq!(released, vec![(Key::Key(Keyboard::A), "b".to_string())]);
        assert_eq!(c.active_target(), LOCAL_TARGET);
        assert!(c.drain_held().is_empty());
    }

    #[test]
    fn release_follows_press_destination() {
        use rkvm_input::key::{Key, Keyboard};
        let mut c = fixture_with_peer("b", true);
        c.note_key(Key::Key(Keyboard::LeftShift), true);
        c.switch_to("b").unwrap();
        let keys = c.release_all(Some("local")).unwrap();
        assert_eq!(
            keys,
            vec![(Key::Key(Keyboard::LeftShift), LOCAL_TARGET.to_string())]
        );
        assert!(c.drain_held().is_empty());
    }

    #[test]
    fn chord_keeps_release_on_previous_until_finished() {
        use rkvm_input::key::{Key, Keyboard};
        let mut c = fixture_with_peer("b", true);
        c.note_key(Key::Key(Keyboard::LeftAlt), true);
        c.note_key(Key::Key(Keyboard::LeftCtrl), true);
        assert_eq!(c.event_target(), LOCAL_TARGET);
        let held = c.drain_held();
        assert_eq!(held.len(), 2);
        c.switch_next().unwrap();
        assert_eq!(c.active_target(), "b");
        // While chord_changed, ups must still target the previous machine.
        assert_eq!(c.event_target(), LOCAL_TARGET);
        c.finish_chord();
        assert_eq!(c.event_target(), "b");
    }

    #[test]
    fn switch_next_cycles_local_and_peers() {
        let mut c = fixture_with_peer("a", true);
        c.insert_peer("b".into(), "127.0.0.1:2".into());
        assert!(!c.switch_next().unwrap().is_empty());
        assert_eq!(c.active_target(), "a");
        c.finish_chord();
        c.switch_next().unwrap();
        assert_eq!(c.active_target(), "b");
        c.finish_chord();
        c.switch_next().unwrap();
        assert_eq!(c.active_target(), LOCAL_TARGET);
    }

    #[tokio::test]
    async fn concurrent_orders_are_serialized() {
        let (handle, control) = control_pair();
        let driver = tokio::spawn(drive(control));
        // Peer must exist in the in-memory driver; drive() starts empty,
        // so we only test local serialization here.
        let a = handle.local();
        let b = handle.local();
        let (ra, rb) = tokio::join!(a, b);
        assert!(ra.is_ok());
        assert!(rb.is_ok());
        drop(handle);
        driver.await.unwrap();
    }

    #[tokio::test]
    async fn handle_rejects_missing_peer() {
        let (handle, control) = control_pair();
        let driver = tokio::spawn(drive(control));
        let err = handle.activate("b").await.unwrap_err();
        assert!(matches!(err, TargetError::PeerUnavailable(_)));
        drop(handle);
        driver.await.unwrap();
    }
}
