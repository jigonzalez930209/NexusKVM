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

pub struct Controller<T: InputTransport> {
    transport: Arc<T>,
    state: RwLock<RuntimeState>,
    active_target: RwLock<PeerId>,
    peers: RwLock<BTreeMap<PeerId, Peer>>,
    last_heartbeat: RwLock<Option<Instant>>,
    portal_available: RwLock<bool>,
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
    pub async fn sync_target(&self) -> Result<()> {
        let active = self.transport.active_target();
        *self.active_target.write() = active.clone();
        let current_state = self.state.read().clone();
        if !Self::in_transition(&current_state) {
            *self.state.write() = Self::state_for_active(&active);
        }
        self.refresh_peers().await?;
        Ok(())
    }
    pub fn status(&self) -> AppStatus {
        let active = self.transport.active_target();
        *self.active_target.write() = active.clone();
        let current = self.state.read().clone();
        let state = if Self::in_transition(&current) {
            current
        } else {
            let reconciled = Self::state_for_active(&active);
            *self.state.write() = reconciled.clone();
            reconciled
        };
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
    pub async fn switch_to(&self, peer: PeerId, entry: EntryPoint) -> Result<Uuid> {
        let _guard = self.transition.lock().await;
        let _ = self.refresh_peers().await;
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
        *self.active_target.write() = peer.clone();
        *self.state.write() = RuntimeState::Remote {
            peer,
            transition_id: id,
        };
        Ok(id)
    }
    pub async fn local(&self) -> Result<Uuid> {
        let _guard = self.transition.lock().await;
        let id = Uuid::new_v4();
        *self.state.write() = RuntimeState::ReturningLocal { transition_id: id };
        let current = self.active_target.read().clone();
        if current != LOCAL_TARGET {
            let _ = self.transport.release_all(Some(&current)).await;
        }
        self.transport.activate_local().await?;
        *self.active_target.write() = LOCAL_TARGET.into();
        *self.state.write() = RuntimeState::Local;
        let _ = self.refresh_peers().await;
        Ok(id)
    }
    pub async fn recover(&self, reason: impl Into<String>) -> Result<()> {
        let _guard = self.transition.lock().await;
        *self.state.write() = RuntimeState::Recovering {
            reason: reason.into(),
        };
        let _ = self.transport.release_all(None).await;
        self.transport.activate_local().await?;
        *self.active_target.write() = LOCAL_TARGET.into();
        *self.state.write() = RuntimeState::Local;
        Ok(())
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
    async fn sync_target_preserves_returning_local() {
        let c = Controller::new(handle_with_peer("b", true));
        *c.state.write() = RuntimeState::ReturningLocal {
            transition_id: Uuid::nil(),
        };
        c.sync_target().await.unwrap();
        assert!(matches!(
            *c.state.read(),
            RuntimeState::ReturningLocal { .. }
        ));
    }
}
