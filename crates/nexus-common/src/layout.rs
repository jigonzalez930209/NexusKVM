use crate::{Barrier, DisplayNode, Edge, Layout, LOCAL_TARGET, PeerId};
use serde::{Deserialize, Serialize};

/// Preferencia de lado del peer remoto respecto de este equipo.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PeerSide {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

impl PeerSide {
    pub fn as_edge(self) -> Edge {
        match self {
            Self::Left => Edge::Left,
            Self::Right => Edge::Right,
            Self::Top => Edge::Top,
            Self::Bottom => Edge::Bottom,
        }
    }

    pub fn from_edge(edge: Edge) -> Self {
        match edge {
            Edge::Left => Self::Left,
            Edge::Right => Self::Right,
            Edge::Top => Self::Top,
            Edge::Bottom => Self::Bottom,
        }
    }

    pub fn opposite(self) -> Self {
        Self::from_edge(self.as_edge().opposite())
    }
}

/// Layout persistido + lado del peer (UI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutFile {
    pub peer_side: PeerSide,
    #[serde(default)]
    pub remote_peer: Option<PeerId>,
    pub layout: Layout,
}

impl LayoutFile {
    pub fn default_right(remote: Option<&str>) -> Self {
        let remote = remote.unwrap_or("peer").to_string();
        Self {
            peer_side: PeerSide::Right,
            remote_peer: Some(remote.clone()),
            layout: two_node_layout(LOCAL_TARGET, &remote, PeerSide::Right),
        }
    }

    pub fn with_side(mut self, side: PeerSide) -> Self {
        let remote = self
            .remote_peer
            .clone()
            .unwrap_or_else(|| "peer".into());
        self.peer_side = side;
        self.layout = two_node_layout(LOCAL_TARGET, &remote, side);
        self.remote_peer = Some(remote);
        self
    }

    pub fn with_remote(mut self, remote: &str) -> Self {
        self.remote_peer = Some(remote.to_string());
        self.layout = two_node_layout(LOCAL_TARGET, remote, self.peer_side);
        self
    }
}

/// Layout de dos pantallas: el remoto queda en `side` de este equipo.
pub fn two_node_layout(local: &str, remote: &str, side: PeerSide) -> Layout {
    let edge = side.as_edge();
    let display = "main";
    let (rx, ry) = match edge {
        Edge::Right => (1920, 0),
        Edge::Left => (-1920, 0),
        Edge::Top => (0, -1080),
        Edge::Bottom => (0, 1080),
    };
    Layout {
        version: 1,
        local_peer: local.into(),
        nodes: vec![
            DisplayNode {
                peer_id: local.into(),
                display_id: display.into(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            DisplayNode {
                peer_id: remote.into(),
                display_id: display.into(),
                x: rx,
                y: ry,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
        ],
        barriers: vec![
            Barrier {
                id: 1,
                from_peer: local.into(),
                display_id: display.into(),
                edge,
                range_start: 0.0,
                range_end: 1.0,
                destination: remote.into(),
                activation_delay_ms: 0,
                cooldown_ms: 350,
            },
            Barrier {
                id: 2,
                from_peer: remote.into(),
                display_id: display.into(),
                edge: edge.opposite(),
                range_start: 0.0,
                range_end: 1.0,
                destination: local.into(),
                activation_delay_ms: 0,
                cooldown_ms: 350,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_right_validates() {
        let f = LayoutFile::default_right(Some("studio-b"));
        f.layout.validate().unwrap();
        assert_eq!(f.layout.barriers[0].edge, Edge::Right);
        assert_eq!(f.layout.barriers[1].edge, Edge::Left);
    }

    #[test]
    fn side_rewrite() {
        let f = LayoutFile::default_right(Some("b")).with_side(PeerSide::Left);
        assert_eq!(f.peer_side, PeerSide::Left);
        assert_eq!(f.layout.barriers[0].edge, Edge::Left);
        assert_eq!(f.layout.barriers[0].destination, "b");
    }
}
