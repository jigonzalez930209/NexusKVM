use crate::{AppStatus, EntryPoint, PeerId};
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
    Local,
    ReleaseAll,
    AgentHeartbeat {
        portal_available: bool,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    pub id: String,
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
