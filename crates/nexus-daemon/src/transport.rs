use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nexus_common::{EntryPoint, Peer, PeerId, PeerStatus};
use rkvm_server::target::TargetHandle;

#[async_trait]
pub trait InputTransport: Send + Sync {
    async fn peers(&self) -> Result<Vec<Peer>>;
    async fn prepare(&self, peer: &PeerId, entry: &EntryPoint) -> Result<()>;
    async fn activate(&self, peer: &PeerId) -> Result<()>;
    async fn activate_local(&self) -> Result<()>;
    async fn release_all(&self, peer: Option<&PeerId>) -> Result<()>;
}

pub struct RkvmAdapter {
    handle: TargetHandle,
}

impl RkvmAdapter {
    pub fn new(handle: TargetHandle) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl InputTransport for RkvmAdapter {
    async fn peers(&self) -> Result<Vec<Peer>> {
        Ok(self
            .handle
            .snapshot()
            .peers
            .into_iter()
            .map(|p| Peer {
                id: p.id.clone(),
                name: p.id.clone(),
                address: p.address,
                status: if p.connected {
                    PeerStatus::Connected
                } else {
                    PeerStatus::Disconnected
                },
                latency_ms: None,
                protocol_version: 1,
            })
            .collect())
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
