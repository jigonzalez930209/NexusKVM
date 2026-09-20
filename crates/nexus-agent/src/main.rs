use clap::{Parser, ValueEnum};
use nexus_agent::{
    clipboard::{self, ClipboardBridge},
    daemon_client::DaemonClient,
    layout_store::{self, AgentStatusFile},
    peer_channel::{self, PeerMessage, CONTROL_PORT},
};
use nexus_common::{ControlCommand, LayoutFile, PeerSide, PeerStatus, LOCAL_TARGET};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tracing::{info, warn};

#[derive(Clone, Copy, ValueEnum)]
enum Role {
    Host,
    Client,
}

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/run/nexuskvm/control.sock")]
    socket: String,
    #[arg(long)]
    data_dir: PathBuf,
    #[arg(long, value_enum, default_value_t = Role::Host)]
    role: Role,
    /// Host address (client only), e.g. 192.168.0.10:5258
    #[arg(long)]
    server: Option<String>,
    /// Shared pairing password (HMAC/AEAD). Prefer env NEXUSKVM_PASSWORD.
    #[arg(long)]
    password: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexus_agent=info".into()),
        )
        .init();
    let args = Args::parse();
    std::fs::create_dir_all(&args.data_dir)?;

    let mut layout_file = layout_store::load_or_default(&args.data_dir)?;
    if matches!(args.role, Role::Client) {
        // On the client, the return edge is the opposite of the host default.
        if layout_file.peer_side == PeerSide::Right
            && layout_file.remote_peer.as_deref() == Some("peer")
        {
            layout_file = layout_file.with_side(PeerSide::Left);
            layout_store::save(&args.data_dir, &layout_file)?;
        }
    }

    let password = args
        .password
        .clone()
        .or_else(|| std::env::var("NEXUSKVM_PASSWORD").ok())
        .or_else(|| {
            std::fs::read_to_string(args.data_dir.join("password"))
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default();
    if password.is_empty() {
        anyhow::bail!("pairing password required (NEXUSKVM_PASSWORD or data-dir/password)");
    }
    let inbox = args.data_dir.join("clip-inbox");
    let clipboard = Arc::new(ClipboardBridge::new(inbox));
    clipboard.set_secret(password.clone());
    clipboard.spawn_watch();
    // Probed once: creating a Clipboard every poll tick is expensive and the
    // answer only changes when the session itself changes.
    let clipboard_ok = clipboard::clipboard_ok();
    // Deploy marker: compare this line on both machines before chasing
    // clipboard bugs — a mismatch restarts nothing and silently drops clips.
    info!("peer protocol: aead-v1 (envelope n/t/c, clip chunks, ready ack)");

    // The InputCapture portal is intentionally not used. On GNOME/Mutter it is
    // part of the same "remote access" machinery as screen casting, so any
    // active session makes GNOME show its screen-capture indicator. Both roles
    // switch through the X11 edge strip in the UI instead.
    write_status(&args.data_dir, false, None, &layout_file, clipboard_ok);

    info!("edge-strip mode: InputCapture portal disabled");

    match args.role {
        Role::Host => run_host(args, layout_file, clipboard, password, clipboard_ok).await,
        Role::Client => run_client(args, layout_file, clipboard, password, clipboard_ok).await,
    }
}

fn write_status(
    data_dir: &std::path::Path,
    portal_available: bool,
    portal_error: Option<&str>,
    layout: &LayoutFile,
    clipboard_ok: bool,
) {
    let side = match layout.peer_side {
        PeerSide::Left => "left",
        PeerSide::Right => "right",
        PeerSide::Top => "top",
        PeerSide::Bottom => "bottom",
    };
    let _ = layout_store::write_agent_status(
        data_dir,
        &AgentStatusFile {
            portal_available,
            portal_error: portal_error.map(str::to_string),
            peer_side: side.into(),
            clipboard_ok,
        },
    );
}

async fn run_host(
    args: Args,
    mut layout_file: LayoutFile,
    clipboard: Arc<ClipboardBridge>,
    password: String,
    clipboard_ok: bool,
) -> anyhow::Result<()> {
    let daemon = DaemonClient {
        socket: args.socket.clone(),
        token: Some(password.clone()),
    };
    // Wait for the daemon socket.
    for _ in 0..50 {
        if daemon.send(ControlCommand::Status).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Host edge switching goes through the UI's X11 edge strip (switch_edge →
    // daemon). No InputCapture portal session is registered: GNOME/Mutter ties
    // it to the screen-capture remote-access indicator.
    let _ = daemon
        .send(ControlCommand::AgentHeartbeat {
            portal_available: false,
        })
        .await;

    let bind: SocketAddr = format!("0.0.0.0:{CONTROL_PORT}").parse()?;
    let daemon_listen = daemon.clone();
    let clip_listen = clipboard.clone();
    let pw = password.clone();
    let inbox = args.data_dir.join("clip-inbox");
    tokio::spawn(async move {
        let _ = peer_channel::listen(
            bind,
            pw,
            Some(inbox),
            move |msg, _| {
                let daemon = daemon_listen.clone();
                async move {
                    match msg {
                        PeerMessage::SwitchLocal => {
                            // Peer edge portal: contained by the daemon until
                            // the remote pointer has left its entry edge.
                            let r = daemon.send(ControlCommand::PeerLocal).await;
                            match r {
                                Ok(resp) if resp.ok => Ok(resp
                                    .status
                                    .map(|s| s.active_target)
                                    .or(Some(LOCAL_TARGET.into()))),
                                Ok(resp) => {
                                    Err(resp.error.unwrap_or_else(|| "local failed".into()))
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        }
                        PeerMessage::Ping => Ok(None),
                        _ => Ok(None),
                    }
                }
            },
            move |incoming| clip_listen.ingest(incoming),
        )
        .await;
    });

    let mut layout_mtime = std::fs::metadata(layout_store::layout_path(&args.data_dir))
        .and_then(|m| m.modified())
        .ok();

    loop {
        // Layout UI reload
        if let Ok(meta) = std::fs::metadata(layout_store::layout_path(&args.data_dir)) {
            if let Ok(modified) = meta.modified() {
                if layout_mtime.map(|t| modified > t).unwrap_or(true) {
                    layout_mtime = Some(modified);
                    if let Ok(f) = layout_store::load_or_default(&args.data_dir) {
                        layout_file = f;
                        info!("layout reloaded ({:?})", layout_file.peer_side);
                        write_status(&args.data_dir, false, None, &layout_file, clipboard_ok);
                    }
                }
            }
        }

        // Peer + active target
        if let Ok(st) = daemon.send(ControlCommand::Status).await {
            if let Some(status) = st.status {
                let peer = status
                    .peers
                    .values()
                    .find(|p| p.status == PeerStatus::Connected)
                    .cloned();
                if let Some(p) = peer {
                    clipboard.set_peer(peer_channel::control_addr_from_peer(&p.address));
                    if layout_file.remote_peer.as_deref() != Some(p.id.as_str()) {
                        layout_file = layout_file.with_remote(&p.id);
                        // A transient write failure must not kill the agent.
                        if let Err(e) = layout_store::save(&args.data_dir, &layout_file) {
                            warn!("layout save failed: {e}");
                        }
                    }
                } else {
                    clipboard.set_peer(None);
                }

                let _ = daemon
                    .send(ControlCommand::AgentHeartbeat {
                        portal_available: false,
                    })
                    .await;
            }
        }
        // Keep the UI's copy fresh (peer_side / clipboard availability).
        write_status(&args.data_dir, false, None, &layout_file, clipboard_ok);

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = tokio::signal::ctrl_c() => {
                info!("agent shutdown");
                break;
            }
        }
    }
    Ok(())
}

async fn run_client(
    args: Args,
    mut layout_file: LayoutFile,
    clipboard: Arc<ClipboardBridge>,
    password: String,
    clipboard_ok: bool,
) -> anyhow::Result<()> {
    let host_control = args
        .server
        .as_deref()
        .and_then(peer_channel::control_addr_from_peer);
    clipboard.set_peer(host_control);

    // Return-to-host switching goes through the UI's X11 edge strip
    // (switch_edge → peer SwitchLocal). No InputCapture portal session is
    // registered: GNOME/Mutter ties it to the screen-capture indicator.
    write_status(&args.data_dir, false, None, &layout_file, clipboard_ok);

    let bind: SocketAddr = format!("0.0.0.0:{CONTROL_PORT}").parse()?;
    let clip_listen = clipboard.clone();
    let pw = password.clone();
    let inbox = args.data_dir.join("clip-inbox");
    tokio::spawn(async move {
        let _ = peer_channel::listen(
            bind,
            pw,
            Some(inbox),
            |_msg, _| async { Ok(None) },
            move |incoming| clip_listen.ingest(incoming),
        )
        .await;
    });

    let mut layout_mtime = std::fs::metadata(layout_store::layout_path(&args.data_dir))
        .and_then(|m| m.modified())
        .ok();

    loop {
        if let Ok(meta) = std::fs::metadata(layout_store::layout_path(&args.data_dir)) {
            if let Ok(modified) = meta.modified() {
                if layout_mtime.map(|t| modified > t).unwrap_or(true) {
                    layout_mtime = Some(modified);
                    if let Ok(f) = layout_store::load_or_default(&args.data_dir) {
                        layout_file = f;
                        write_status(&args.data_dir, false, None, &layout_file, clipboard_ok);
                    }
                }
            }
        }

        write_status(&args.data_dir, false, None, &layout_file, clipboard_ok);

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}
