use anyhow::{bail, Context, Result};
use nexus_common::{
    now_unix, open, open_chunk, seal, seal_chunk, secret_ok, AeadEnvelope, ReplayGuard,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, info, warn};

pub const CONTROL_PORT: u16 = 5259;
pub const CLIP_MAX_TEXT: u64 = 2 * 1024 * 1024;
pub const CLIP_MAX_PNG: u64 = 16 * 1024 * 1024;
pub const CLIP_MAX_FILES: u64 = 64 * 1024 * 1024;
pub const CLIP_INBOX_CAP: u64 = 256 * 1024 * 1024;
pub const CLIP_MAX_ENTRIES: usize = 2048;
pub const CLIP_MAX_DEPTH: u32 = 16;
const LINE_MAX: usize = 512 * 1024;
const CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Text,
    Png,
    Files,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipFileMeta {
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PeerMessage {
    Clipboard {
        seq: u64,
        text: String,
    },
    ClipOffer {
        id: String,
        kind: ClipKind,
        byte_len: u64,
        #[serde(default)]
        files: Vec<ClipFileMeta>,
    },
    ClipDone {
        id: String,
        sha256: String,
    },
    SwitchLocal,
    Ping,
    Ack {
        ok: bool,
        #[serde(default)]
        active_target: Option<String>,
        #[serde(default)]
        error: Option<String>,
    },
}

#[derive(Debug)]
pub enum ClipOut {
    Text(String),
    Png(Vec<u8>),
    Files(Vec<(String, PathBuf)>),
}

#[derive(Debug, Clone)]
pub struct IncomingClip {
    pub kind: ClipKind,
    pub text: Option<String>,
    pub png: Option<Vec<u8>>,
    pub files: Vec<PathBuf>,
}

fn decode_msg(password: &str, env: &AeadEnvelope) -> Result<PeerMessage> {
    if !secret_ok(password) {
        bail!("empty peer secret");
    }
    let pt = open(password, env).map_err(|e| anyhow::anyhow!(e))?;
    Ok(serde_json::from_slice(&pt)?)
}

fn kind_max(kind: ClipKind) -> u64 {
    match kind {
        ClipKind::Text => CLIP_MAX_TEXT,
        ClipKind::Png => CLIP_MAX_PNG,
        ClipKind::Files => CLIP_MAX_FILES,
    }
}

pub fn safe_file_name(name: &str) -> Option<String> {
    let base = Path::new(name).file_name()?.to_string_lossy();
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .take(128)
        .collect();
    if cleaned.trim().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Relative path with `/` separators. Rejects `..` and absolute paths.
pub fn safe_rel_path(name: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        parts.push(safe_file_name(part)?);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn confined(base: &Path, rel: &str) -> Option<PathBuf> {
    let rel = safe_rel_path(rel)?;
    let mut p = base.to_path_buf();
    for c in rel.split('/') {
        p.push(c);
    }
    Some(p)
}

#[derive(Debug, Clone)]
pub struct ClipEntry {
    pub rel: String,
    pub path: PathBuf,
    pub dir: bool,
    pub size: u64,
}

/// Flatten copied files and folders. Skips symlinks. Empty dirs are kept.
pub fn flatten_clip_paths(items: &[(String, PathBuf)]) -> Result<Vec<ClipEntry>> {
    let mut out = Vec::new();
    let mut total = 0u64;
    for (name, path) in items {
        let root = safe_file_name(name).unwrap_or_else(|| "item".into());
        push_tree(&mut out, &mut total, path, &root, 0)?;
    }
    Ok(out)
}

fn push_tree(
    out: &mut Vec<ClipEntry>,
    total: &mut u64,
    path: &Path,
    rel: &str,
    depth: u32,
) -> Result<()> {
    if out.len() >= CLIP_MAX_ENTRIES {
        bail!("too many clipboard files");
    }
    if depth > CLIP_MAX_DEPTH {
        bail!("clipboard folder too deep");
    }
    let meta = std::fs::symlink_metadata(path).with_context(|| path.display().to_string())?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    let rel = safe_rel_path(rel).ok_or_else(|| anyhow::anyhow!("bad clip path"))?;
    if meta.is_dir() {
        out.push(ClipEntry {
            rel: rel.clone(),
            path: path.to_path_buf(),
            dir: true,
            size: 0,
        });
        let mut children: Vec<_> = std::fs::read_dir(path)?.filter_map(|e| e.ok()).collect();
        children.sort_by_key(|e| e.file_name());
        for child in children {
            let ft = child.file_type()?;
            if ft.is_symlink() {
                continue;
            }
            let name = child.file_name();
            let name = name.to_string_lossy();
            let Some(seg) = safe_file_name(&name) else {
                continue;
            };
            let child_rel = format!("{rel}/{seg}");
            push_tree(out, total, &child.path(), &child_rel, depth + 1)?;
        }
        return Ok(());
    }
    if !meta.is_file() {
        return Ok(());
    }
    *total = total.saturating_add(meta.len());
    if *total > CLIP_MAX_FILES {
        bail!("clipboard files too large");
    }
    out.push(ClipEntry {
        rel,
        path: path.to_path_buf(),
        dir: false,
        size: meta.len(),
    });
    Ok(())
}

async fn write_signed(
    w: &mut (impl AsyncWriteExt + Unpin),
    password: &str,
    msg: &PeerMessage,
) -> Result<()> {
    if !secret_ok(password) {
        bail!("empty peer secret");
    }
    let body = serde_json::to_vec(msg)?;
    let env = seal(password, &body, now_unix()).map_err(|e| anyhow::anyhow!(e))?;
    w.write_all(serde_json::to_string(&env)?.as_bytes()).await?;
    w.write_all(b"\n").await?;
    Ok(())
}

async fn read_signed_line(
    lines: &mut BufReader<impl AsyncRead + Unpin>,
    password: &str,
) -> Result<PeerMessage> {
    let mut reply = String::new();
    lines.read_line(&mut reply).await?;
    if reply.is_empty() {
        bail!("peer control closed without ack");
    }
    if reply.len() > LINE_MAX {
        bail!("peer line too large");
    }
    let env: AeadEnvelope = serde_json::from_str(&reply)?;
    decode_msg(password, &env)
}

async fn write_aead_bytes(
    w: &mut (impl AsyncWriteExt + Unpin),
    password: &str,
    data: &[u8],
) -> Result<()> {
    for chunk in data.chunks(CHUNK) {
        let frame = seal_chunk(password, chunk).map_err(|e| anyhow::anyhow!(e))?;
        w.write_all(&(frame.len() as u32).to_be_bytes()).await?;
        w.write_all(&frame).await?;
    }
    Ok(())
}

async fn read_aead_bytes(
    r: &mut (impl AsyncRead + Unpin),
    password: &str,
    mut remain: u64,
    mut sink: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    while remain > 0 {
        let mut lenb = [0u8; 4];
        r.read_exact(&mut lenb).await?;
        let n = u32::from_be_bytes(lenb) as usize;
        if !(16..=CHUNK + 64).contains(&n) {
            bail!("clip frame size");
        }
        let mut frame = vec![0u8; n];
        r.read_exact(&mut frame).await?;
        let pt = open_chunk(password, &frame).map_err(|e| anyhow::anyhow!(e))?;
        if pt.len() as u64 > remain {
            bail!("clip frame overflow");
        }
        sink(&pt)?;
        remain -= pt.len() as u64;
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for e in entries.flatten() {
        if let Ok(m) = e.metadata() {
            if m.is_dir() {
                total = total.saturating_add(dir_size(&e.path()));
            } else {
                total = total.saturating_add(m.len());
            }
        }
    }
    total
}

fn prune_inbox(clip_dir: &Path) {
    let Ok(mut entries): Result<Vec<_>, _> =
        std::fs::read_dir(clip_dir).map(|i| i.flatten().collect())
    else {
        return;
    };
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    while dir_size(clip_dir) > CLIP_INBOX_CAP {
        let Some(oldest) = entries.first() else {
            break;
        };
        let path = oldest.path();
        entries.remove(0);
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::remove_file(&path);
    }
}

pub async fn send_to(addr: SocketAddr, msg: &PeerMessage, password: &str) -> Result<PeerMessage> {
    let mut stream = TcpStream::connect(addr).await?;
    write_signed(&mut stream, password, msg).await?;
    let mut lines = BufReader::new(stream);
    read_signed_line(&mut lines, password).await
}

pub async fn send_clip(addr: SocketAddr, password: &str, payload: &ClipOut) -> Result<PeerMessage> {
    let id = uuid::Uuid::new_v4().to_string();
    let (kind, files_meta, byte_len) = match payload {
        ClipOut::Text(t) => {
            let n = t.len() as u64;
            if n > CLIP_MAX_TEXT {
                bail!("clipboard text too large");
            }
            (ClipKind::Text, Vec::new(), n)
        }
        ClipOut::Png(p) => {
            let n = p.len() as u64;
            if n > CLIP_MAX_PNG {
                bail!("clipboard image too large");
            }
            (ClipKind::Png, Vec::new(), n)
        }
        ClipOut::Files(files) => {
            let entries = flatten_clip_paths(files)?;
            if entries.is_empty() {
                bail!("no clipboard files");
            }
            let mut meta = Vec::new();
            let mut total = 0u64;
            for e in &entries {
                total = total.saturating_add(e.size);
                meta.push(ClipFileMeta {
                    name: e.rel.clone(),
                    size: e.size,
                    dir: e.dir,
                });
            }
            if total > CLIP_MAX_FILES {
                bail!("clipboard files too large");
            }
            (ClipKind::Files, meta, total)
        }
    };

    let mut stream = TcpStream::connect(addr).await?;
    write_signed(
        &mut stream,
        password,
        &PeerMessage::ClipOffer {
            id: id.clone(),
            kind,
            byte_len,
            files: files_meta,
        },
    )
    .await?;

    let mut hasher = Sha256::new();
    match payload {
        ClipOut::Text(t) => {
            hasher.update(t.as_bytes());
            write_aead_bytes(&mut stream, password, t.as_bytes()).await?;
        }
        ClipOut::Png(p) => {
            hasher.update(p);
            write_aead_bytes(&mut stream, password, p).await?;
        }
        ClipOut::Files(files) => {
            let entries = flatten_clip_paths(files)?;
            for e in entries {
                if e.dir {
                    continue;
                }
                let mut f = File::open(&e.path).await?;
                let mut buf = vec![0u8; CHUNK];
                loop {
                    let n = f.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buf[..n]);
                    write_aead_bytes(&mut stream, password, &buf[..n]).await?;
                }
            }
        }
    }
    stream.flush().await?;
    let sha256 = hex::encode(hasher.finalize());
    write_signed(&mut stream, password, &PeerMessage::ClipDone { id, sha256 }).await?;
    let mut lines = BufReader::new(stream);
    read_signed_line(&mut lines, password).await
}

async fn hash_copy<R: AsyncRead + Unpin>(
    r: &mut R,
    remain: u64,
    password: &str,
    mut sink: impl FnMut(&[u8]) -> Result<()>,
) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    read_aead_bytes(r, password, remain, |c| {
        hasher.update(c);
        sink(c)
    })
    .await?;
    Ok(hasher.finalize().into())
}

async fn recv_clip<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    password: &str,
    clip_dir: &Path,
    offer: PeerMessage,
) -> Result<IncomingClip> {
    let PeerMessage::ClipOffer {
        id,
        kind,
        byte_len,
        files,
    } = offer
    else {
        bail!("expected clip_offer");
    };
    if byte_len > kind_max(kind) {
        bail!("clip payload exceeds limit");
    }
    if files.len() > CLIP_MAX_ENTRIES {
        bail!("too many clipboard files");
    }
    prune_inbox(clip_dir);
    if dir_size(clip_dir) + byte_len > CLIP_INBOX_CAP {
        bail!("clipboard inbox full");
    }

    let dest = clip_dir.join(&id);
    let mut incoming = IncomingClip {
        kind,
        text: None,
        png: None,
        files: Vec::new(),
    };

    match kind {
        ClipKind::Text => {
            let mut body = Vec::new();
            let digest = hash_copy(reader, byte_len, password, |c| {
                body.extend_from_slice(c);
                Ok(())
            })
            .await?;
            verify_done(reader, password, &id, &digest).await?;
            incoming.text = Some(String::from_utf8(body).context("clipboard text is not utf-8")?);
        }
        ClipKind::Png => {
            let mut body = Vec::new();
            let digest = hash_copy(reader, byte_len, password, |c| {
                body.extend_from_slice(c);
                Ok(())
            })
            .await?;
            verify_done(reader, password, &id, &digest).await?;
            incoming.png = Some(body);
        }
        ClipKind::Files => {
            tokio::fs::create_dir_all(&dest).await?;
            let expected: u64 = files.iter().filter(|f| !f.dir).map(|f| f.size).sum();
            if expected != byte_len {
                bail!("file sizes do not match offer");
            }
            let mut hasher = Sha256::new();
            let mut roots: Vec<PathBuf> = Vec::new();
            let mut seen_root = std::collections::HashSet::new();
            for meta in &files {
                let rel =
                    safe_rel_path(&meta.name).ok_or_else(|| anyhow::anyhow!("bad file name"))?;
                let path = confined(&dest, &rel).ok_or_else(|| anyhow::anyhow!("bad file name"))?;
                if let Some(first) = rel.split('/').next() {
                    if seen_root.insert(first.to_string()) {
                        roots.push(dest.join(first));
                    }
                }
                if meta.dir {
                    tokio::fs::create_dir_all(&path).await?;
                    continue;
                }
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let mut out = File::create(&path).await?;
                let mut body = Vec::new();
                read_aead_bytes(reader, password, meta.size, |c| {
                    hasher.update(c);
                    body.extend_from_slice(c);
                    Ok(())
                })
                .await?;
                if body.len() as u64 != meta.size {
                    bail!("clip file size mismatch");
                }
                out.write_all(&body).await?;
                out.flush().await?;
            }
            incoming.files = roots;
            let digest: [u8; 32] = hasher.finalize().into();
            verify_done(reader, password, &id, &digest).await?;
        }
    }
    Ok(incoming)
}

async fn verify_done(
    reader: &mut BufReader<impl AsyncRead + Unpin>,
    password: &str,
    id: &str,
    digest: &[u8; 32],
) -> Result<()> {
    let msg = read_signed_line(reader, password).await?;
    let PeerMessage::ClipDone {
        id: done_id,
        sha256,
    } = msg
    else {
        bail!("expected clip_done");
    };
    if done_id != id {
        bail!("clip id mismatch");
    }
    let got = hex::encode(digest);
    if got != sha256 {
        bail!("clip sha256 mismatch");
    }
    Ok(())
}

pub async fn listen<F, Fut, C>(
    bind: SocketAddr,
    password: String,
    clip_dir: Option<PathBuf>,
    on_msg: F,
    on_clip: C,
) -> Result<()>
where
    F: Fn(PeerMessage, SocketAddr) -> Fut + Send + Clone + 'static,
    Fut: std::future::Future<Output = Result<Option<String>, String>> + Send,
    C: Fn(IncomingClip) + Send + Clone + 'static,
{
    if !secret_ok(&password) {
        bail!("empty peer secret");
    }
    let listener = TcpListener::bind(bind).await?;
    info!("peer control listening on {bind} (aead)");
    let replay = Arc::new(Mutex::new(ReplayGuard::default()));
    loop {
        let (stream, peer) = listener.accept().await?;
        let password = password.clone();
        let on_msg = on_msg.clone();
        let on_clip = on_clip.clone();
        let clip_dir = clip_dir.clone();
        let replay = replay.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_conn(stream, peer, password, clip_dir, replay, on_msg, on_clip).await
            {
                debug!("peer control {peer}: {e}");
            }
        });
    }
}

async fn handle_conn<F, Fut, C>(
    stream: TcpStream,
    peer: SocketAddr,
    password: String,
    clip_dir: Option<PathBuf>,
    replay: Arc<Mutex<ReplayGuard>>,
    on_msg: F,
    on_clip: C,
) -> Result<()>
where
    F: Fn(PeerMessage, SocketAddr) -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>, String>>,
    C: Fn(IncomingClip),
{
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        if line.len() > LINE_MAX {
            warn!("peer message too large from {peer}");
            break;
        }
        let env: AeadEnvelope = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                warn!("peer parse error from {peer}: {e}");
                break;
            }
        };
        if replay.lock().unwrap().check(&env).is_err() {
            warn!("peer replay from {peer}");
            break;
        }
        let msg = match decode_msg(&password, &env) {
            Ok(m) => m,
            Err(e) => {
                warn!("peer auth failed from {peer}: {e}");
                let _ = write_signed(
                    &mut w,
                    &password,
                    &PeerMessage::Ack {
                        ok: false,
                        active_target: None,
                        error: Some("unauthorized".into()),
                    },
                )
                .await;
                break;
            }
        };
        if matches!(msg, PeerMessage::Ack { .. }) {
            continue;
        }
        debug!("peer msg from {peer}: {}", msg_kind(&msg));

        if matches!(msg, PeerMessage::ClipOffer { .. }) {
            let outcome = match &clip_dir {
                Some(dir) => match recv_clip(&mut reader, &password, dir, msg).await {
                    Ok(clip) => {
                        on_clip(clip);
                        Ok(None)
                    }
                    Err(e) => Err(e.to_string()),
                },
                None => Err("clipboard inbox unavailable".into()),
            };
            write_outcome(&mut w, &password, outcome).await?;
            continue;
        }

        if let PeerMessage::Clipboard { seq: _, text } = msg {
            on_clip(IncomingClip {
                kind: ClipKind::Text,
                text: Some(text),
                png: None,
                files: Vec::new(),
            });
            write_outcome(&mut w, &password, Ok(None)).await?;
            continue;
        }

        let outcome = on_msg(msg, peer).await;
        write_outcome(&mut w, &password, outcome).await?;
    }
    Ok(())
}

async fn write_outcome(
    w: &mut (impl AsyncWriteExt + Unpin),
    password: &str,
    outcome: Result<Option<String>, String>,
) -> Result<()> {
    let ack = match outcome {
        Ok(active_target) => PeerMessage::Ack {
            ok: true,
            active_target,
            error: None,
        },
        Err(error) => PeerMessage::Ack {
            ok: false,
            active_target: None,
            error: Some(error),
        },
    };
    write_signed(w, password, &ack).await
}

fn msg_kind(m: &PeerMessage) -> &'static str {
    match m {
        PeerMessage::Clipboard { .. } => "clipboard",
        PeerMessage::ClipOffer { .. } => "clip_offer",
        PeerMessage::ClipDone { .. } => "clip_done",
        PeerMessage::SwitchLocal => "switch_local",
        PeerMessage::Ping => "ping",
        PeerMessage::Ack { .. } => "ack",
    }
}

/// Extract host:port from a peer id like `192.168.0.143:49892` or an IP-only id
/// (`192.168.0.143`) → control on :5259.
pub fn control_addr_from_peer(peer_id_or_addr: &str) -> Option<SocketAddr> {
    if let Ok(addr) = peer_id_or_addr.parse::<SocketAddr>() {
        return Some(SocketAddr::new(addr.ip(), CONTROL_PORT));
    }
    if let Ok(ip) = peer_id_or_addr.parse::<std::net::IpAddr>() {
        return Some(SocketAddr::new(ip, CONTROL_PORT));
    }
    let host = if let Some((h, p)) = peer_id_or_addr.rsplit_once(':') {
        if p.parse::<u16>().is_ok() && !h.is_empty() && !h.contains(']') {
            h
        } else if let Some(stripped) = peer_id_or_addr.strip_prefix('[') {
            stripped.split(']').next().unwrap_or(peer_id_or_addr)
        } else {
            peer_id_or_addr
        }
    } else {
        peer_id_or_addr
    };
    format!("{host}:{CONTROL_PORT}")
        .to_socket_addrs()
        .ok()?
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_socket_and_ip_only() {
        let a = control_addr_from_peer("192.168.0.143:49892").unwrap();
        let b = control_addr_from_peer("192.168.0.143").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.port(), CONTROL_PORT);
    }

    #[test]
    fn ipv6_bracketed_and_bare() {
        let a = control_addr_from_peer("[2001:db8::1]:5258").unwrap();
        let b = control_addr_from_peer("2001:db8::1").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.port(), CONTROL_PORT);
    }

    #[test]
    fn rejects_path_traversal_names() {
        assert!(safe_file_name("../etc/passwd").is_some());
        assert_eq!(safe_file_name("../etc/passwd").unwrap(), "passwd");
        assert!(safe_file_name("..").is_none());
        assert_eq!(safe_rel_path("a/b/c.txt").as_deref(), Some("a/b/c.txt"));
        assert!(safe_rel_path("a/../etc/passwd").is_none());
        assert_eq!(safe_rel_path("/etc/passwd").unwrap(), "etc/passwd");
        assert!(safe_rel_path("/etc/passwd").is_some());
        assert_eq!(safe_rel_path("/etc/passwd").as_deref(), Some("etc/passwd"));
    }

    #[test]
    fn flatten_keeps_nested_and_empty_dirs() {
        let root = std::env::temp_dir().join(format!("nexus-flatten-{}", std::process::id()));
        let folder = root.join("pack");
        let nested = folder.join("sub");
        std::fs::create_dir_all(nested.join("empty")).unwrap();
        std::fs::write(nested.join("a.txt"), b"aa").unwrap();
        let entries = flatten_clip_paths(&[("pack".into(), folder)]).unwrap();
        let rels: Vec<_> = entries.iter().map(|e| (e.rel.as_str(), e.dir)).collect();
        assert!(rels.contains(&("pack", true)));
        assert!(rels.contains(&("pack/sub", true)));
        assert!(rels.contains(&("pack/sub/empty", true)));
        assert!(rels.contains(&("pack/sub/a.txt", false)));
        let _ = std::fs::remove_dir_all(root);
    }
}
