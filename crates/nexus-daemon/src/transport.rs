use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nexus_common::{EntryPoint, Peer, PeerId, PeerStatus};
use rkvm_server::server::PeerLatencies;
use rkvm_server::target::TargetHandle;
use std::collections::HashMap;

#[async_trait]
pub trait InputTransport: Send + Sync {
    async fn peers(&self) -> Result<Vec<Peer>>;
    fn latencies(&self) -> HashMap<PeerId, u32>;
    async fn prepare(&self, peer: &PeerId, entry: &EntryPoint) -> Result<()>;
    async fn activate(&self, peer: &PeerId) -> Result<()>;
    async fn activate_local(&self) -> Result<()>;
    async fn release_all(&self, peer: Option<&PeerId>) -> Result<()>;
}

pub struct RkvmAdapter {
    handle: TargetHandle,
    latencies: PeerLatencies,
}

impl RkvmAdapter {
    pub fn new(handle: TargetHandle, latencies: PeerLatencies) -> Self {
        Self { handle, latencies }
    }

    fn latency_map(&self) -> HashMap<PeerId, u32> {
        self.latencies.lock().unwrap().clone()
    }
}

#[async_trait]
impl InputTransport for RkvmAdapter {
    async fn peers(&self) -> Result<Vec<Peer>> {
        let rtt = self.latency_map();
        Ok(self
            .handle
            .snapshot()
            .peers
            .into_iter()
            .map(|p| Peer {
                latency_ms: rtt.get(&p.id).copied(),
                id: p.id.clone(),
                name: p.id.clone(),
                address: p.address,
                status: if p.connected {
                    PeerStatus::Connected
                } else {
                    PeerStatus::Disconnected
                },
                protocol_version: 1,
            })
            .collect())
    }

    fn latencies(&self) -> HashMap<PeerId, u32> {
        self.latency_map()
    }

    async fn prepare(&self, peer: &PeerId, _: &EntryPoint) -> Result<()> {
        self.handle.prepare(peer).await.map_err(|e| anyhow!(e))
    }

    async fn activate(&self, peer: &PeerId) -> Result<()> {
        self.handle
            .activate(peer)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    }

    async fn activate_local(&self) -> Result<()> {
        self.handle
            .local()
            .await
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    }

    async fn release_all(&self, peer: Option<&PeerId>) -> Result<()> {
        self.handle
            .release_all(peer.map(String::as_str))
            .await
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    }
}
