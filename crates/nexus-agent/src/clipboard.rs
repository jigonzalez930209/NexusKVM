use crate::peer_channel::{self, PeerMessage};
use arboard::Clipboard;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tracing::{debug, warn};

/// Plain-text clipboard sync between peers. Does not log content.
pub struct ClipboardBridge {
    last_local: Arc<Mutex<String>>,
    last_applied_seq: Arc<AtomicU64>,
    out_seq: Arc<AtomicU64>,
    peer: Arc<Mutex<Option<SocketAddr>>>,
}

impl ClipboardBridge {
    pub fn new() -> Self {
        Self {
            last_local: Arc::new(Mutex::new(String::new())),
            last_applied_seq: Arc::new(AtomicU64::new(0)),
            out_seq: Arc::new(AtomicU64::new(1)),
            peer: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_peer(&self, addr: Option<SocketAddr>) {
        *self.peer.lock().unwrap() = addr;
    }

    pub fn apply_remote(&self, seq: u64, text: String) {
        let prev = self.last_applied_seq.load(Ordering::SeqCst);
        if seq <= prev {
            return;
        }
        self.last_applied_seq.store(seq, Ordering::SeqCst);
        if let Ok(mut clip) = Clipboard::new() {
            if clip.set_text(text.clone()).is_ok() {
                *self.last_local.lock().unwrap() = text;
            }
        }
    }

    pub fn spawn_watch(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut clip = match Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    warn!("local clipboard unavailable: {e}");
                    return;
                }
            };
            loop {
                tokio::time::sleep(Duration::from_millis(400)).await;
                let Ok(text) = clip.get_text() else {
                    continue;
                };
                if text.is_empty() {
                    continue;
                }
                {
                    let mut last = this.last_local.lock().unwrap();
                    if *last == text {
                        continue;
                    }
                    *last = text.clone();
                }
                let peer = *this.peer.lock().unwrap();
                let Some(addr) = peer else {
                    continue;
                };
                let seq = this.out_seq.fetch_add(1, Ordering::SeqCst);
                let msg = PeerMessage::Clipboard { seq, text };
                if let Err(e) = peer_channel::send_to(addr, &msg).await {
                    debug!("clipboard send failed: {e}");
                }
            }
        });
    }
}

impl Default for ClipboardBridge {
    fn default() -> Self {
        Self::new()
    }
}

pub fn clipboard_ok() -> bool {
    Clipboard::new().is_ok()
}
