use crate::{Barrier, Edge, EntryPoint, Layout};

pub fn normalize_axis(value: f64, length: f64) -> f32 {
    if length <= 0.0 {
        return 0.5;
    }
    (value / length).clamp(0.0, 1.0) as f32
}

pub fn map_axis(normalized: f32, remote_length: u32, inset: u32) -> u32 {
    if remote_length == 0 {
        return 0;
    }
    let max = remote_length.saturating_sub(inset.max(1));
    let min = inset.min(max);
    ((normalized.clamp(0.0, 1.0) * remote_length as f32).round() as u32).clamp(min, max)
}

pub fn barrier_for(
    layout: &Layout,
    peer: &str,
    display: &str,
    edge: Edge,
    position: f32,
) -> Option<Barrier> {
    layout
        .barriers
        .iter()
        .find(|b| {
            b.from_peer == peer
                && b.display_id == display
                && b.edge == edge
                && position >= b.range_start
                && position <= b.range_end
        })
        .cloned()
}

pub fn entry_for(edge: Edge, normalized_position: f32) -> EntryPoint {
    EntryPoint {
        edge: edge.opposite(),
        normalized_position: normalized_position.clamp(0.0, 1.0),
        inset_px: 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_half() {
        assert_eq!(map_axis(0.5, 1440, 6), 720);
    }
    #[test]
    fn clamps() {
        assert_eq!(normalize_axis(200.0, 100.0), 1.0);
    }
}
