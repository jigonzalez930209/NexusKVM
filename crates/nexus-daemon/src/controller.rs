use crate::transport::InputTransport;
use anyhow::{bail, Result};
use nexus_common::*;
use parking_lot::RwLock;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

pub struct Controller<T: InputTransport> {
    transport: Arc<T>,
    state: RwLock<RuntimeState>,
    active_target: RwLock<PeerId>,
    peers: RwLock<BTreeMap<PeerId, Peer>>,
    agent_connected: RwLock<bool>,
    portal_available: RwLock<bool>,
}
impl<T: InputTransport> Controller<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(transport),
            state: RwLock::new(RuntimeState::Local),
            active_target: RwLock::new(LOCAL_TARGET.into()),
            peers: RwLock::new(BTreeMap::new()),
            agent_connected: RwLock::new(false),
            portal_available: RwLock::new(false),
        }
    }
    pub async fn refresh_peers(&self) -> Result<()> {
        let peers = self.transport.peers().await?;
        *self.peers.write() = peers.into_iter().map(|p| (p.id.clone(), p)).collect();
        Ok(())
    }
    pub fn status(&self) -> AppStatus {
        // Merge latest measured RTTs so polling clients see fresh latency values.
        let rtt = self.transport.latencies();
        let mut peers = self.peers.read().clone();
        for (id, peer) in peers.iter_mut() {
            if let Some(ms) = rtt.get(id) {
                peer.latency_ms = Some(*ms);
            }
        }
        AppStatus {
            state: self.state.read().clone(),
            active_target: self.active_target.read().clone(),
            peers,
            agent_connected: *self.agent_connected.read(),
            portal_available: *self.portal_available.read(),
            emergency_shortcut: "Left Alt + Left Ctrl".into(),
        }
    }
    pub fn heartbeat(&self, portal: bool) {
        *self.agent_connected.write() = true;
        *self.portal_available.write() = portal;
    }
    pub async fn switch_to(&self, peer: PeerId, entry: EntryPoint) -> Result<Uuid> {
        let connected = self
            .peers
            .read()
            .get(&peer)
            .map(|p| p.status == PeerStatus::Connected)
            .unwrap_or(false);
        if !connected {
            bail!("peer unavailable: {peer}");
        }
        if matches!(
            *self.state.read(),
            RuntimeState::PreparingRemote { .. } | RuntimeState::ReturningLocal { .. }
        ) {
            bail!("transition in progress");
        }
        let id = Uuid::new_v4();
        *self.state.write() = RuntimeState::PreparingRemote {
            peer: peer.clone(),
            transition_id: id,
        };
        if let Err(e) = self.transport.prepare(&peer, &entry).await {
            *self.state.write() = RuntimeState::Local;
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
        let id = Uuid::new_v4();
        *self.state.write() = RuntimeState::ReturningLocal { transition_id: id };
        let current = self.active_target.read().clone();
        if current != LOCAL_TARGET {
            self.transport.release_all(Some(&current)).await?;
        }
        self.transport.activate_local().await?;
        *self.active_target.write() = LOCAL_TARGET.into();
        *self.state.write() = RuntimeState::Local;
        Ok(id)
    }
    pub async fn recover(&self, reason: impl Into<String>) -> Result<()> {
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
        self.transport.release_all(None).await
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
}
