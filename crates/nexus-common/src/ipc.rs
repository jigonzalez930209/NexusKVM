use crate::{AppStatus, Edge, EntryPoint, PeerId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Status,
    Peers,
    Switch {
        target: PeerId,
        entry: Option<EntryPoint>,
    },
    /// Pointer crossed this machine's layout edge. Unlike `Next` this never
    /// cycles: it is a no-op while the machine is already remote or during the
    /// containment window, so a duplicated edge event cannot bounce control.
    SwitchEdge {
        side: Edge,
        position: f32,
    },
    /// Same as the Ctrl+Alt chord: cycle to the next connected target.
    Next,
    Local,
    /// Return requested by the peer's edge portal. Contained until the remote
    /// pointer has moved away from its entry edge (anti-bounce).
    PeerLocal,
    ReleaseAll,
    AgentHeartbeat {
        portal_available: bool,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    pub id: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(flatten)]
    pub command: ControlCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
    pub status: Option<AppStatus>,
    pub transition_id: Option<Uuid>,
}
impl ControlResponse {
    pub fn ok(id: String, status: Option<AppStatus>) -> Self {
        Self {
            id,
            ok: true,
            error: None,
            status,
            transition_id: None,
        }
    }
    pub fn error(id: String, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            error: Some(error.into()),
            status: None,
            transition_id: None,
        }
    }
}
