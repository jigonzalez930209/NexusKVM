use nexus_common::PeerId;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct InputState {
    keys: HashMap<u16, PeerId>,
    buttons: HashMap<u16, PeerId>,
}
impl InputState {
    pub fn press_key(&mut self, code: u16, dest: PeerId) {
        self.keys.insert(code, dest);
    }
    pub fn release_key(&mut self, code: u16) -> Option<PeerId> {
        self.keys.remove(&code)
    }
    pub fn press_button(&mut self, code: u16, dest: PeerId) {
        self.buttons.insert(code, dest);
    }
    pub fn release_button(&mut self, code: u16) -> Option<PeerId> {
        self.buttons.remove(&code)
    }
    pub fn drain(&mut self) -> Vec<(u16, PeerId, bool)> {
        let mut out = Vec::new();
        out.extend(self.keys.drain().map(|(c, p)| (c, p, false)));
        out.extend(self.buttons.drain().map(|(c, p)| (c, p, true)));
        out
    }
    pub fn is_clear(&self) -> bool {
        self.keys.is_empty() && self.buttons.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_goes_to_press_destination() {
        let mut s = InputState::default();
        s.press_key(42, "b".into());
        assert_eq!(s.release_key(42), Some("b".into()));
        assert!(s.is_clear());
    }
}
