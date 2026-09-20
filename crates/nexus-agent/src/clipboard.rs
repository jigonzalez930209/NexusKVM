use crate::peer_channel::{self, ClipKind, ClipOut, IncomingClip, PeerMessage};
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
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

const SEND_TIMEOUT: Duration = Duration::from_secs(30);
/// After a failed send the same clipboard content is retried with exponential
/// backoff instead of hammering the peer every poll tick.
const RETRY_BASE: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(60);

struct SendGate {
    fp: String,
    fails: u32,
    next_try: Instant,
}

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

fn snap_desc(snap: &LocalSnap) -> String {
    match snap {
        LocalSnap::Empty => "empty".into(),
        LocalSnap::Text(t) => format!("text {} bytes", t.len()),
        LocalSnap::Image {
            width,
            height,
            rgba,
        } => format!("image {width}x{height} ({} bytes)", rgba.len()),
        LocalSnap::Files(paths) => format!("files {} items", paths.len()),
    }
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
        let mut cur = self.peer.lock().unwrap();
        if *cur != addr {
            match addr {
                Some(a) => info!("clipboard peer → {a}"),
                None => info!("clipboard peer → none"),
            }
            *cur = addr;
        }
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
        let gate: Arc<Mutex<Option<SendGate>>> = Arc::new(Mutex::new(None));
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
                info!("clipboard watcher started (inbox {})", inbox.display());
                loop {
                    match rx.recv_timeout(Duration::from_millis(350)) {
                        Ok(Cmd::Apply(incoming)) => {
                            if let Err(e) = apply_incoming(&mut clip, incoming, &last_fp) {
                                warn!("clipboard apply failed: {e}");
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
                    let snap = read_snap(&mut clip, &inbox);
                    let fp = fingerprint(&snap, &inbox);
                    if fp.is_empty() {
                        continue;
                    }
                    // Same payload failing repeatedly backs off instead of
                    // retrying every poll tick (which spammed the peer and the
                    // log when the peer ran an incompatible agent).
                    if let Some(g) = gate.lock().unwrap().as_ref() {
                        if g.fp == fp && Instant::now() < g.next_try {
                            continue;
                        }
                    }
                    let prev = {
                        let mut last = last_fp.lock().unwrap();
                        if *last == fp {
                            continue;
                        }
                        let prev = last.clone();
                        *last = fp.clone();
                        prev
                    };
                    let desc = snap_desc(&snap);
                    let Some(out) = snap_to_out(snap) else {
                        continue;
                    };
                    info!("clipboard → {addr}: {desc}");
                    sending.store(true, Ordering::SeqCst);
                    let sending2 = Arc::clone(&sending);
                    let last_fp2 = Arc::clone(&last_fp);
                    let gate2 = Arc::clone(&gate);
                    handle.spawn(async move {
                        let send = peer_channel::send_clip(addr, &pw, &out);
                        let mut failure: Option<String> = None;
                        match tokio::time::timeout(SEND_TIMEOUT, send).await {
                            Ok(Ok(PeerMessage::Ack { ok: true, .. })) => {
                                info!("clipboard sent → {addr}: {desc}");
                            }
                            Ok(Ok(PeerMessage::Ack {
                                ok: false, error, ..
                            })) => {
                                failure = Some(format!(
                                    "peer rejected clipboard: {}",
                                    error.unwrap_or_else(|| "unknown".into())
                                ));
                            }
                            Ok(Ok(other)) => {
                                failure = Some(format!("unexpected clip reply: {other:?}"));
                            }
                            Ok(Err(e)) => failure = Some(e.to_string()),
                            Err(_) => failure = Some("send timed out".into()),
                        }
                        if let Some(reason) = failure {
                            warn!("clipboard send failed → {addr}: {reason}");
                            let mut g = last_fp2.lock().unwrap();
                            if *g == fp {
                                *g = prev;
                            }
                            let mut gate = gate2.lock().unwrap();
                            let fails = gate
                                .as_ref()
                                .filter(|g| g.fp == fp)
                                .map(|g| g.fails + 1)
                                .unwrap_or(1);
                            let backoff =
                                (RETRY_BASE * 2u32.saturating_pow(fails - 1)).min(RETRY_MAX);
                            *gate = Some(SendGate {
                                fp,
                                fails,
                                next_try: Instant::now() + backoff,
                            });
                        } else {
                            *gate2.lock().unwrap() = None;
                        }
                        sending2.store(false, Ordering::SeqCst);
                    });
                }
            })
            .expect("clipboard thread");
    }
}

fn read_snap(clip: &mut Clipboard, inbox: &Path) -> LocalSnap {
    // Prefer real image payloads over a single screenshot file path so paste
    // lands as an image on the peer (Nautilus/GIMP/etc. expect image/png).
    if let Ok(img) = clip.get().image() {
        if img.width > 0 && img.height > 0 && !img.bytes.is_empty() {
            return LocalSnap::Image {
                width: img.width,
                height: img.height,
                rgba: img.bytes.into_owned(),
            };
        }
    }
    if let Ok(files) = clip.get().file_list() {
        if !files.is_empty() {
            // Never convert a received inbox image back into an image payload:
            // that echoed the transferred file back as PNG and replaced the
            // original file-list clipboard on the sender.
            if files.len() == 1 && !files[0].starts_with(inbox) {
                if let Some(img) = image_snap_from_path(&files[0]) {
                    return img;
                }
            }
            return LocalSnap::Files(files);
        }
    }
    if let Ok(text) = clip.get_text() {
        if let Some(paths) = parse_uri_list(&text) {
            if paths.len() == 1 && !paths[0].starts_with(inbox) {
                if let Some(img) = image_snap_from_path(&paths[0]) {
                    return img;
                }
            }
            if !paths.is_empty() {
                return LocalSnap::Files(paths);
            }
        }
        if !text.is_empty() {
            return LocalSnap::Text(text);
        }
    }
    LocalSnap::Empty
}

fn image_snap_from_path(path: &Path) -> Option<LocalSnap> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
    ) {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() as u64 > peer_channel::CLIP_MAX_PNG {
        return None;
    }
    let (w, h, rgba) = png_to_rgba(&bytes)
        .or_else(|_| {
            let img = image::load_from_memory(&bytes).map_err(|e| anyhow::anyhow!(e))?;
            let rgba = img.to_rgba8();
            Ok::<_, anyhow::Error>((
                rgba.width() as usize,
                rgba.height() as usize,
                rgba.into_raw(),
            ))
        })
        .ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some(LocalSnap::Image {
        width: w,
        height: h,
        rgba,
    })
}

fn parse_uri_list(text: &str) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let path = if let Some(rest) = line.strip_prefix("file://") {
            let decoded = percent_decode(rest);
            PathBuf::from(decoded)
        } else if line.starts_with('/') {
            PathBuf::from(line)
        } else {
            continue;
        };
        if path.exists() {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

/// `file:///path/with spaces` → percent-encoded URI, as text/uri-list expects.
fn path_to_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::from("file://");
    for b in raw.bytes() {
        let keep = b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
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
                let len = text.len();
                clip.set_text(&text)?;
                *last_fp.lock().unwrap() = fingerprint(&LocalSnap::Text(text), Path::new(""));
                info!("clipboard ← text applied ({len} bytes)");
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
                info!("clipboard ← image applied ({w}x{h}, {} bytes)", png.len());
            }
        }
        ClipKind::Files => {
            if incoming.files.is_empty() {
                return Ok(());
            }
            let roots: Vec<String> = incoming
                .files
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            // Prefer file_list; if the compositor rejects it, fall back to a
            // text/uri-list so Ctrl+V still has something usable.
            if let Err(e) = clip.set().file_list(&incoming.files) {
                warn!("clipboard file_list apply failed: {e}; falling back to uri-list");
                let uri = incoming
                    .files
                    .iter()
                    .map(|p| path_to_uri(p))
                    .collect::<Vec<_>>()
                    .join("\r\n");
                clip.set_text(uri)?;
            }
            *last_fp.lock().unwrap() = format!("inbox:{}", incoming.files.len());
            info!("clipboard ← files applied: {}", roots.join(", "));
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

    #[test]
    fn path_to_uri_encodes_spaces() {
        assert_eq!(
            path_to_uri(Path::new("/home/u/My Shot.png")),
            "file:///home/u/My%20Shot.png"
        );
    }

    #[test]
    fn uri_list_parses_file_urls() {
        let dir = std::env::temp_dir().join("nexus-uri-list-test");
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("shot.png");
        std::fs::write(&f, b"x").unwrap();
        let text = format!("file://{}\n#comment\n", f.display());
        let paths = parse_uri_list(&text).unwrap();
        assert_eq!(paths, vec![f]);
    }
}
