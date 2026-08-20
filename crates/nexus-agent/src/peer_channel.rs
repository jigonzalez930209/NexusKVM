use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, info, warn};

pub const CONTROL_PORT: u16 = 5259;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PeerMessage {
    Clipboard { seq: u64, text: String },
    SwitchLocal,
    Ping,
}

pub async fn send_to(addr: SocketAddr, msg: &PeerMessage) -> Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    let line = serde_json::to_string(msg)?;
    stream.write_all(line.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    Ok(())
}

pub async fn listen<F, Fut>(bind: SocketAddr, mut on_msg: F) -> Result<()>
where
    F: FnMut(PeerMessage, SocketAddr) -> Fut + Send,
    Fut: std::future::Future<Output = ()> + Send,
{
    let listener = TcpListener::bind(bind).await?;
    info!("peer control listening on {bind}");
    loop {
        let (stream, peer) = listener.accept().await?;
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.len() > 256 * 1024 {
                warn!("peer message too large from {peer}");
                break;
            }
            match serde_json::from_str::<PeerMessage>(&line) {
                Ok(msg) => {
                    debug!("peer msg from {peer}: {}", msg_kind(&msg));
                    on_msg(msg, peer).await;
                }
                Err(e) => warn!("peer parse error from {peer}: {e}"),
            }
        }
    }
}

fn msg_kind(m: &PeerMessage) -> &'static str {
    match m {
        PeerMessage::Clipboard { .. } => "clipboard",
        PeerMessage::SwitchLocal => "switch_local",
        PeerMessage::Ping => "ping",
    }
}

/// Extract host:port from a peer id like `192.168.0.143:49892` → control on :5259.
pub fn control_addr_from_peer(peer_id_or_addr: &str) -> Option<SocketAddr> {
    let host = peer_id_or_addr.split(':').next()?;
    format!("{host}:{CONTROL_PORT}").parse().ok()
}
