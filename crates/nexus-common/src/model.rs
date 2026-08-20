use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub type PeerId = String;
pub const LOCAL_TARGET: &str = "local";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerStatus {
    Connected,
    Disconnected,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Peer {
    pub id: PeerId,
    pub name: String,
    pub address: String,
    pub status: PeerStatus,
    pub latency_ms: Option<u32>,
    pub protocol_version: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}
impl Edge {
    pub fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Barrier {
    pub id: u32,
    pub from_peer: PeerId,
    pub display_id: String,
    pub edge: Edge,
    pub range_start: f32,
    pub range_end: f32,
    pub destination: PeerId,
    pub activation_delay_ms: u32,
    pub cooldown_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayNode {
    pub peer_id: PeerId,
    pub display_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub version: u16,
    pub local_peer: PeerId,
    pub nodes: Vec<DisplayNode>,
    pub barriers: Vec<Barrier>,
}
impl Layout {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported layout version".into());
        }
        for b in &self.barriers {
            if !(0.0..=1.0).contains(&b.range_start)
                || !(0.0..=1.0).contains(&b.range_end)
                || b.range_start >= b.range_end
            {
                return Err(format!("barrier {} has invalid range", b.id));
            }
            if !self.nodes.iter().any(|n| n.peer_id == b.destination) {
                return Err(format!("destination {} does not exist", b.destination));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntryPoint {
    pub edge: Edge,
    pub normalized_position: f32,
    pub inset_px: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transition {
    pub id: Uuid,
    pub source: PeerId,
    pub target: PeerId,
    pub entry: EntryPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppStatus {
    pub state: RuntimeState,
    pub active_target: PeerId,
    pub peers: BTreeMap<PeerId, Peer>,
    pub agent_connected: bool,
    pub portal_available: bool,
    pub emergency_shortcut: String,
}

use crate::RuntimeState;
