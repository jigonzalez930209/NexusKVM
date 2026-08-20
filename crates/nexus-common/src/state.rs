use crate::PeerId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeState {
    #[default]
    Local,
    PreparingRemote {
        peer: PeerId,
        transition_id: Uuid,
    },
    Remote {
        peer: PeerId,
        transition_id: Uuid,
    },
    ReturningLocal {
        transition_id: Uuid,
    },
    Recovering {
        reason: String,
    },
    Degraded {
        reason: String,
    },
    Stopped,
}
