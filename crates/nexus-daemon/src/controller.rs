use crate::transport::InputTransport;
use anyhow::{bail, Result};
use nexus_common::*;
use parking_lot::RwLock;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

const AGENT_TTL: Duration = Duration::from_secs(5);

/// A duplicated edge event (portal bounce, slow UI, retried IPC) must never
/// flip control a second time. Transitions within this window are dropped.
const EDGE_CONTAINMENT: Duration = Duration::from_millis(750);

pub struct Controller<T: InputTransport> {
    transport: Arc<T>,
    state: RwLock<RuntimeState>,
    active_target: RwLock<PeerId>,
    peers: RwLock<BTreeMap<PeerId, Peer>>,
    last_heartbeat: RwLock<Option<Instant>>,
    portal_available: RwLock<bool>,
    last_transition_at: RwLock<Option<Instant>>,
    transition: Mutex<()>,
}
impl<T: InputTransport> Controller<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(transport),
            state: RwLock::new(RuntimeState::Local),
            active_target: RwLock::new(LOCAL_TARGET.into()),
            peers: RwLock::new(BTreeMap::new()),
            last_heartbeat: RwLock::new(None),
            portal_available: RwLock::new(false),
            last_transition_at: RwLock::new(None),
            transition: Mutex::new(()),
        }
    }
    pub async fn refresh_peers(&self) -> Result<()> {
        let peers = self.transport.peers().await?;
        *self.peers.write() = peers.into_iter().map(|p| (p.id.clone(), p)).collect();
        Ok(())
    }
    fn in_transition(state: &RuntimeState) -> bool {
        matches!(
            state,
            RuntimeState::PreparingRemote { .. }
                | RuntimeState::ReturningLocal { .. }
                | RuntimeState::Recovering { .. }
        )
    }
    fn state_for_active(active: &str) -> RuntimeState {
        if active == LOCAL_TARGET {
            RuntimeState::Local
        } else {
            RuntimeState::Remote {
                peer: active.to_string(),
                transition_id: Uuid::nil(),
            }
        }
    }

    /// Drop stale Preparing/Returning/Recovering when the transport already settled.
    fn heal_transition_locked(&self, active: &str) {
        let current = self.state.read().clone();
        let next = match &current {
            RuntimeState::ReturningLocal { .. } | RuntimeState::Recovering { .. } => {
                // Intent is always local; callers force the transport if needed.
                Some(RuntimeState::Local)
            }
            RuntimeState::PreparingRemote {
                peer,
                transition_id,
            } if active == peer.as_str() => Some(RuntimeState::Remote {
                peer: peer.clone(),
                transition_id: *transition_id,
            }),
            RuntimeState::PreparingRemote { .. } if active == LOCAL_TARGET => {
                Some(RuntimeState::Local)
            }
            RuntimeState::Remote { peer, .. }
                if active == LOCAL_TARGET || active != peer.as_str() =>
            {
                Some(Self::state_for_active(active))
            }
            _ if !Self::in_transition(&current) => Some(Self::state_for_active(active)),
            _ => None,
        };
        if let Some(s) = next {
            *self.state.write() = s;
        }
    }

    async fn ensure_local_transport(&self) {
        let active = self.transport.active_target();
        if active != LOCAL_TARGET {
            let _ = self.transport.release_all(None).await;
            let _ = self.transport.activate_local().await;
        }
        *self.active_target.write() = LOCAL_TARGET.into();
        *self.state.write() = RuntimeState::Local;
    }

    pub async fn sync_target(&self) -> Result<()> {
        let active = self.transport.active_target();
        *self.active_target.write() = active.clone();
        let was_returning = matches!(
            *self.state.read(),
            RuntimeState::ReturningLocal { .. } | RuntimeState::Recovering { .. }
        );
        self.heal_transition_locked(&active);
        self.refresh_peers().await?;
        if was_returning && active != LOCAL_TARGET {
            self.ensure_local_transport().await;
            return Ok(());
        }
        // Dead active peer (reconnect changed id): force local.
        if active != LOCAL_TARGET {
            let alive = self
                .peers
                .read()
                .get(&active)
                .map(|p| p.status == PeerStatus::Connected)
                .unwrap_or(false);
            if !alive {
                self.ensure_local_transport().await;
            }
        }
        Ok(())
    }
    pub fn status(&self) -> AppStatus {
        let active = self.transport.active_target();
        *self.active_target.write() = active.clone();
        self.heal_transition_locked(&active);
        let state = self.state.read().clone();
        let rtt = self.transport.latencies();
        let mut peers = self.peers.read().clone();
        for (id, peer) in peers.iter_mut() {
            if let Some(ms) = rtt.get(id) {
                peer.latency_ms = Some(*ms);
            }
        }
        AppStatus {
            state,
            active_target: active,
            peers,
            agent_connected: (*self.last_heartbeat.read())
                .map(|t| t.elapsed() < AGENT_TTL)
                .unwrap_or(false),
            portal_available: *self.portal_available.read(),
            emergency_shortcut: "Left Alt + Left Ctrl".into(),
        }
    }
    pub fn heartbeat(&self, portal: bool) {
        *self.last_heartbeat.write() = Some(Instant::now());
        *self.portal_available.write() = portal;
    }

    fn mark_transition(&self) {
        *self.last_transition_at.write() = Some(Instant::now());
    }

    async fn switch_locked(&self, peer: PeerId, entry: EntryPoint) -> Result<Uuid> {
        let id = Uuid::new_v4();
        *self.state.write() = RuntimeState::PreparingRemote {
            peer: peer.clone(),
            transition_id: id,
        };
        if let Err(e) = self.transport.prepare(&peer, &entry).await {
            *self.state.write() = Self::state_for_active(&self.transport.active_target());
            return Err(e);
        }
        if let Err(e) = self.transport.activate(&peer).await {
            let _ = self.transport.activate_local().await;
            *self.state.write() = RuntimeState::Local;
            return Err(e);
        }
        self.mark_transition();
        *self.active_target.write() = peer.clone();
        *self.state.write() = RuntimeState::Remote {
            peer,
            transition_id: id,
        };
        Ok(id)
    }

    /// Edge crossing from this machine's portal.
    ///
    /// Containment rules, in order: while already remote the crossing is
    /// ignored (return must be deliberate: chord, remote request or UI), and
    /// while inside the containment window any duplicate is ignored. The
    /// target is the connected peer, never a cycle.
    pub async fn switch_edge(&self, side: Edge, position: f32) -> Result<Uuid> {
        let _guard = self.transition.lock().await;
        self.sync_target().await?;
        let active = self.transport.active_target();
        if active != LOCAL_TARGET {
            tracing::info!(active = %active, side = ?side, "edge crossing contained: already remote");
            return Ok(Uuid::nil());
        }
        if let Some(at) = *self.last_transition_at.read() {
            if at.elapsed() < EDGE_CONTAINMENT {
                tracing::info!(
                    elapsed_ms = at.elapsed().as_millis(),
                    side = ?side,
                    "edge crossing contained: within debounce window"
                );
                return Ok(Uuid::nil());
            }
        }
        let peer = self
            .peers
            .read()
            .values()
            .find(|p| p.status == PeerStatus::Connected)
            .map(|p| p.id.clone());
        let Some(peer) = peer else {
            bail!("no connected peer for edge switch");
        };
        self.switch_locked(peer, entry_for(side, position)).await
    }

    pub async fn switch_to(&self, peer: PeerId, entry: EntryPoint) -> Result<Uuid> {
        let _guard = self.transition.lock().await;
        let _ = self.refresh_peers().await;
        let mut active = self.transport.active_target();
        *self.active_target.write() = active.clone();
        let was_returning = matches!(
            *self.state.read(),
            RuntimeState::ReturningLocal { .. } | RuntimeState::Recovering { .. }
        );
        self.heal_transition_locked(&active);
        let active_peer_alive = self
            .peers
            .read()
            .get(&active)
            .map(|p| p.status == PeerStatus::Connected)
            .unwrap_or(false);
        if was_returning || (active != LOCAL_TARGET && !active_peer_alive) {
            self.ensure_local_transport().await;
            active = LOCAL_TARGET.into();
        }
        let _ = active;
        let connected = self
            .peers
            .read()
            .get(&peer)
            .map(|p| p.status == PeerStatus::Connected)
            .unwrap_or(false);
        if !connected {
            bail!("peer unavailable: {peer}");
        }
        if Self::in_transition(&self.state.read()) {
            bail!("transition in progress");
        }
        self.switch_locked(peer, entry).await
    }
    /// Cycle to the next connected target, exactly like the Ctrl+Alt chord.
    pub async fn next(&self) -> Result<Uuid> {
        let _guard = self.transition.lock().await;
        self.sync_target().await?;
        let before = self.transport.active_target();
        let connected = self
            .peers
            .read()
            .values()
            .filter(|p| p.status == PeerStatus::Connected)
            .count();
        self.transport.next().await?;
        let active = self.transport.active_target();
        self.mark_transition();
        *self.active_target.write() = active.clone();
        *self.state.write() = Self::state_for_active(&active);
        if active == before && active == LOCAL_TARGET {
            tracing::warn!(connected, "next: no connected peer to switch to");
            bail!("no connected peer to switch to");
        }
        tracing::info!(from = %before, to = %active, connected, "next: target cycled");
        Ok(Uuid::new_v4())
    }
    pub async fn local(&self) -> Result<Uuid> {
        let _guard = self.transition.lock().await;
        let id = Uuid::new_v4();
        *self.state.write() = RuntimeState::ReturningLocal { transition_id: id };
        let current = self.transport.active_target();
        if current != LOCAL_TARGET {
            if let Err(e) = self.transport.release_all(Some(&current)).await {
                tracing::warn!("release_all({current}): {e}; draining all");
                let _ = self.transport.release_all(None).await;
            }
        }
        match self.transport.activate_local().await {
            Ok(()) => {
                self.mark_transition();
                *self.active_target.write() = LOCAL_TARGET.into();
                *self.state.write() = RuntimeState::Local;
                let _ = self.refresh_peers().await;
                tracing::info!(from = %current, "local: control returned");
                Ok(id)
            }
            Err(e) => {
                let active = self.transport.active_target();
                *self.active_target.write() = active.clone();
                *self.state.write() = Self::state_for_active(&active);
                Err(e)
            }
        }
    }
    pub async fn recover(&self, reason: impl Into<String>) -> Result<()> {
        let _guard = self.transition.lock().await;
        *self.state.write() = RuntimeState::Recovering {
            reason: reason.into(),
        };
        let _ = self.transport.release_all(None).await;
        match self.transport.activate_local().await {
            Ok(()) => {
                self.mark_transition();
                *self.active_target.write() = LOCAL_TARGET.into();
                *self.state.write() = RuntimeState::Local;
                Ok(())
            }
            Err(e) => {
                let active = self.transport.active_target();
                *self.active_target.write() = active.clone();
                *self.state.write() = Self::state_for_active(&active);
                Err(e)
            }
        }
    }
    pub async fn release_all(&self) -> Result<()> {
        self.recover("release_all").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::RkvmAdapter;
    use rkvm_server::target::{control_pair_with, drive_with, TargetRouter};

    fn handle_with_peer(id: &str, connected: bool) -> RkvmAdapter {
        let mut router = TargetRouter::new();
        router.insert_peer(id.into(), "127.0.0.1:5258".into());
        if !connected {
            router.mark_disconnected(id);
        }
        let (handle, control) = control_pair_with(router.snapshot());
        tokio::spawn(drive_with(control, router));
        RkvmAdapter::new(handle, rkvm_server::server::new_peer_latencies())
    }

    #[tokio::test]
    async fn changes_and_returns() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        c.switch_to(
            "b".into(),
            EntryPoint {
                edge: Edge::Left,
                normalized_position: 0.5,
                inset_px: 6,
            },
        )
        .await
        .unwrap();
        assert_eq!(c.status().active_target, "b");
        c.local().await.unwrap();
        assert_eq!(c.status().active_target, LOCAL_TARGET);
    }
    #[tokio::test]
    async fn rejects_disconnected() {
        let c = Controller::new(handle_with_peer("b", false));
        c.refresh_peers().await.unwrap();
        assert!(c
            .switch_to(
                "b".into(),
                EntryPoint {
                    edge: Edge::Left,
                    normalized_position: 0.5,
                    inset_px: 6
                }
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn status_matches_transport_target() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        c.switch_to(
            "b".into(),
            EntryPoint {
                edge: Edge::Left,
                normalized_position: 0.5,
                inset_px: 6,
            },
        )
        .await
        .unwrap();
        let s = c.status();
        assert_eq!(s.active_target, "b");
        assert!(matches!(s.state, RuntimeState::Remote { peer, .. } if peer == "b"));
    }

    #[tokio::test]
    async fn release_all_returns_local() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        c.switch_to(
            "b".into(),
            EntryPoint {
                edge: Edge::Left,
                normalized_position: 0.5,
                inset_px: 6,
            },
        )
        .await
        .unwrap();
        c.release_all().await.unwrap();
        assert_eq!(c.status().active_target, LOCAL_TARGET);
        assert!(matches!(c.status().state, RuntimeState::Local));
    }

    #[tokio::test]
    async fn next_cycles_local_and_peer() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        c.next().await.unwrap();
        assert_eq!(c.status().active_target, "b");
        c.next().await.unwrap();
        assert_eq!(c.status().active_target, LOCAL_TARGET);
    }

    #[tokio::test]
    async fn next_after_local_works_again() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        // full edge loop: local -> peer -> local -> peer
        c.next().await.unwrap();
        assert_eq!(c.status().active_target, "b");
        c.local().await.unwrap();
        assert_eq!(c.status().active_target, LOCAL_TARGET);
        c.next().await.unwrap();
        assert_eq!(c.status().active_target, "b");
    }

    #[tokio::test]
    async fn edge_switch_contains_duplicates_while_remote() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        let first = c.switch_edge(Edge::Right, 0.5).await.unwrap();
        assert!(!first.is_nil());
        assert_eq!(c.status().active_target, "b");
        // A second crossing (portal bounce) must NOT cycle back to local.
        let second = c.switch_edge(Edge::Right, 0.5).await.unwrap();
        assert!(second.is_nil());
        assert_eq!(c.status().active_target, "b");
    }

    #[tokio::test]
    async fn edge_switch_contained_right_after_return() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        c.switch_edge(Edge::Right, 0.5).await.unwrap();
        c.local().await.unwrap();
        assert_eq!(c.status().active_target, LOCAL_TARGET);
        // Instant re-trigger (cursor parked on the portal pixel): contained.
        let bounce = c.switch_edge(Edge::Right, 0.5).await.unwrap();
        assert!(bounce.is_nil());
        assert_eq!(c.status().active_target, LOCAL_TARGET);
        // After the containment window a deliberate crossing works again.
        tokio::time::sleep(EDGE_CONTAINMENT + Duration::from_millis(20)).await;
        let later = c.switch_edge(Edge::Right, 0.5).await.unwrap();
        assert!(!later.is_nil());
        assert_eq!(c.status().active_target, "b");
    }

    #[tokio::test]
    async fn sync_target_heals_stuck_returning_local() {
        let c = Controller::new(handle_with_peer("b", true));
        *c.state.write() = RuntimeState::ReturningLocal {
            transition_id: Uuid::nil(),
        };
        c.sync_target().await.unwrap();
        assert!(matches!(*c.state.read(), RuntimeState::Local));
        assert_eq!(c.status().active_target, LOCAL_TARGET);
    }

    #[tokio::test]
    async fn switch_heals_stuck_returning_local() {
        let c = Controller::new(handle_with_peer("b", true));
        c.refresh_peers().await.unwrap();
        *c.state.write() = RuntimeState::ReturningLocal {
            transition_id: Uuid::nil(),
        };
        c.switch_to(
            "b".into(),
            EntryPoint {
                edge: Edge::Left,
                normalized_position: 0.5,
                inset_px: 6,
            },
        )
        .await
        .unwrap();
        assert_eq!(c.status().active_target, "b");
    }
}
