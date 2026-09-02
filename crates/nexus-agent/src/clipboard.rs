use crate::peer_channel::{self, ClipKind, ClipOut, IncomingClip};
use arboard::{Clipboard, ImageData};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tracing::{debug, warn};

enum LocalSnap {
    Empty,
    Text(String),
    Image {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
    Files(Vec<PathBuf>),
}

enum Cmd {
    Apply(IncomingClip),
}

/// Bidirectional clipboard: text, PNG, and file lists. Linux ownership is held
/// on a dedicated thread so paste still works after apply.
pub struct ClipboardBridge {
    last_fp: Arc<Mutex<String>>,
    peer: Arc<Mutex<Option<std::net::SocketAddr>>>,
    secret: Arc<Mutex<Option<String>>>,
    apply_tx: Mutex<Option<mpsc::Sender<Cmd>>>,
    sending: Arc<AtomicBool>,
    inbox: PathBuf,
}

impl ClipboardBridge {
    pub fn new(inbox: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&inbox);
        Self {
            last_fp: Arc::new(Mutex::new(String::new())),
            peer: Arc::new(Mutex::new(None)),
            secret: Arc::new(Mutex::new(None)),
            apply_tx: Mutex::new(None),
            sending: Arc::new(AtomicBool::new(false)),
            inbox,
        }
    }

    pub fn set_secret(&self, password: String) {
        *self.secret.lock().unwrap() = if password.is_empty() {
            None
        } else {
            Some(password)
        };
    }

    pub fn set_peer(&self, addr: Option<std::net::SocketAddr>) {
        *self.peer.lock().unwrap() = addr;
    }

    pub fn ingest(&self, clip: IncomingClip) {
        if let Some(tx) = self.apply_tx.lock().unwrap().as_ref() {
            let _ = tx.send(Cmd::Apply(clip));
        }
    }

    pub fn spawn_watch(self: &Arc<Self>) {
        let (tx, rx) = mpsc::channel();
        *self.apply_tx.lock().unwrap() = Some(tx);

        let last_fp = Arc::clone(&self.last_fp);
        let peer = Arc::clone(&self.peer);
        let secret = Arc::clone(&self.secret);
        let sending = Arc::clone(&self.sending);
        let inbox = self.inbox.clone();
        let handle = tokio::runtime::Handle::current();

        thread::Builder::new()
            .name("nexus-clipboard".into())
            .spawn(move || {
                let mut clip = match Clipboard::new() {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("local clipboard unavailable: {e}");
                        return;
                    }
                };
                loop {
                    match rx.recv_timeout(Duration::from_millis(350)) {
                        Ok(Cmd::Apply(incoming)) => {
                            if apply_incoming(&mut clip, incoming, &last_fp).is_err() {
                                debug!("clipboard apply failed");
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }

                    if sending.load(Ordering::SeqCst) {
                        continue;
                    }
                    let Some(addr) = *peer.lock().unwrap() else {
                        continue;
                    };
                    let Some(pw) = secret.lock().unwrap().clone() else {
                        continue;
                    };
                    let snap = read_snap(&mut clip);
                    let fp = fingerprint(&snap, &inbox);
                    let prev = {
                        let mut last = last_fp.lock().unwrap();
                        if fp.is_empty() || *last == fp {
                            continue;
                        }
                        let prev = last.clone();
                        *last = fp.clone();
                        prev
                    };
                    let Some(out) = snap_to_out(snap) else {
                        continue;
                    };
                    sending.store(true, Ordering::SeqCst);
                    let sending2 = Arc::clone(&sending);
                    let last_fp2 = Arc::clone(&last_fp);
                    handle.spawn(async move {
                        if let Err(e) = peer_channel::send_clip(addr, &pw, &out).await {
                            debug!("clipboard send failed: {e}");
                            let mut g = last_fp2.lock().unwrap();
                            if *g == fp {
                                *g = prev;
                            }
                        }
                        sending2.store(false, Ordering::SeqCst);
                    });
                }
            })
            .expect("clipboard thread");
    }
}

fn read_snap(clip: &mut Clipboard) -> LocalSnap {
    if let Ok(files) = clip.get().file_list() {
        if !files.is_empty() {
            return LocalSnap::Files(files);
        }
    }
    if let Ok(img) = clip.get().image() {
        if img.width > 0 && img.height > 0 && !img.bytes.is_empty() {
            return LocalSnap::Image {
                width: img.width,
                height: img.height,
                rgba: img.bytes.into_owned(),
            };
        }
    }
    if let Ok(text) = clip.get_text() {
        if !text.is_empty() {
            return LocalSnap::Text(text);
        }
    }
    LocalSnap::Empty
}

fn fingerprint(snap: &LocalSnap, inbox: &Path) -> String {
    match snap {
        LocalSnap::Empty => String::new(),
        LocalSnap::Text(t) => format!("t:{}:{}", t.len(), simple_hash(t.as_bytes())),
        LocalSnap::Image {
            width,
            height,
            rgba,
        } => format!("i:{width}x{height}:{}:{}", rgba.len(), simple_hash(rgba)),
        LocalSnap::Files(paths) => {
            if !paths.is_empty() && paths.iter().all(|p| p.starts_with(inbox)) {
                return format!("inbox:{}", paths.len());
            }
            let items: Vec<_> = paths
                .iter()
                .filter_map(|p| {
                    let name = p.file_name()?.to_str()?.to_string();
                    Some((name, p.clone()))
                })
                .collect();
            let entries = peer_channel::flatten_clip_paths(&items).unwrap_or_default();
            let mut parts: Vec<_> = entries
                .iter()
                .map(|e| format!("{}:{}:{}", e.rel, e.size, e.dir as u8))
                .collect();
            parts.sort();
            format!("f:{}", parts.join("|"))
        }
    }
}

fn simple_hash(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn snap_to_out(snap: LocalSnap) -> Option<ClipOut> {
    match snap {
        LocalSnap::Empty => None,
        LocalSnap::Text(t) => {
            if t.len() as u64 > peer_channel::CLIP_MAX_TEXT {
                warn!("skip clipboard text: too large");
                return None;
            }
            Some(ClipOut::Text(t))
        }
        LocalSnap::Image {
            width,
            height,
            rgba,
        } => match rgba_to_png(width as u32, height as u32, &rgba) {
            Ok(png) if png.len() as u64 <= peer_channel::CLIP_MAX_PNG => Some(ClipOut::Png(png)),
            Ok(_) => {
                warn!("skip clipboard image: too large");
                None
            }
            Err(e) => {
                debug!("png encode: {e}");
                None
            }
        },
        LocalSnap::Files(paths) => {
            let mut files = Vec::new();
            for p in paths {
                let Ok(meta) = std::fs::symlink_metadata(&p) else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if !meta.is_file() && !meta.is_dir() {
                    continue;
                }
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("item")
                    .to_string();
                files.push((name, p));
            }
            if files.is_empty() {
                return None;
            }
            if peer_channel::flatten_clip_paths(&files).is_err() {
                warn!("skip clipboard files: too large or too many");
                return None;
            }
            Some(ClipOut::Files(files))
        }
    }
}

fn apply_incoming(
    clip: &mut Clipboard,
    incoming: IncomingClip,
    last_fp: &Mutex<String>,
) -> Result<(), arboard::Error> {
    match incoming.kind {
        ClipKind::Text => {
            if let Some(text) = incoming.text {
                clip.set_text(&text)?;
                *last_fp.lock().unwrap() = fingerprint(&LocalSnap::Text(text), Path::new(""));
            }
        }
        ClipKind::Png => {
            if let Some(png) = incoming.png {
                let (w, h, rgba) =
                    png_to_rgba(&png).map_err(|_| arboard::Error::ConversionFailure)?;
                clip.set_image(ImageData {
                    width: w,
                    height: h,
                    bytes: Cow::Owned(rgba.clone()),
                })?;
                *last_fp.lock().unwrap() = fingerprint(
                    &LocalSnap::Image {
                        width: w,
                        height: h,
                        rgba,
                    },
                    Path::new(""),
                );
            }
        }
        ClipKind::Files => {
            if incoming.files.is_empty() {
                return Ok(());
            }
            clip.set().file_list(&incoming.files)?;
            *last_fp.lock().unwrap() = format!("inbox:{}", incoming.files.len());
        }
    }
    Ok(())
}

pub fn rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> anyhow::Result<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    encoder.write_image(rgba, width, height, image::ExtendedColorType::Rgba8)?;
    Ok(out)
}

pub fn png_to_rgba(png: &[u8]) -> anyhow::Result<(usize, usize, Vec<u8>)> {
    let img = image::load_from_memory(png)?.to_rgba8();
    let w = img.width() as usize;
    let h = img.height() as usize;
    Ok((w, h, img.into_raw()))
}

impl Default for ClipboardBridge {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("nexuskvm-clip"))
    }
}

pub fn clipboard_ok() -> bool {
    Clipboard::new().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_roundtrip_rgba() {
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let png = rgba_to_png(2, 1, &rgba).unwrap();
        let (w, h, back) = png_to_rgba(&png).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(back, rgba);
    }

    #[test]
    fn file_fp_ignores_inbox_echo() {
        let inbox = PathBuf::from("/tmp/nexuskvm-clip-test");
        let snap = LocalSnap::Files(vec![inbox.join("a.txt")]);
        assert!(fingerprint(&snap, &inbox).starts_with("inbox:"));
    }
}
