use clap::{Parser, ValueEnum};
use nexus_agent::{
    backend::{EdgeCaptureBackend, PortalBackend},
    clipboard::{self, ClipboardBridge},
    daemon_client::DaemonClient,
    engine::EdgeEngine,
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

    let clipboard = Arc::new(ClipboardBridge::new());
    clipboard.spawn_watch();

    let backend = PortalBackend::connect().await?;
    let portal_probe = backend.available();
    write_status(
        &args.data_dir,
        portal_probe,
        backend.portal_error(),
        &layout_file,
        clipboard::clipboard_ok(),
    );

    match args.role {
        Role::Host => run_host(args, layout_file, backend, clipboard).await,
        Role::Client => run_client(args, layout_file, backend, clipboard).await,
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
    backend: PortalBackend,
    clipboard: Arc<ClipboardBridge>,
) -> anyhow::Result<()> {
    let daemon = DaemonClient {
        socket: args.socket.clone(),
    };
    // Wait for the daemon socket.
    for _ in 0..50 {
        if daemon.send(ControlCommand::Status).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut engine = EdgeEngine::new(backend, daemon.clone(), layout_file.layout.clone());
    if let Err(e) = engine.configure().await {
        warn!("portal register: {e}");
        write_status(
            &args.data_dir,
            false,
            Some(&e.to_string()),
            &layout_file,
            clipboard::clipboard_ok(),
        );
        let _ = daemon
            .send(ControlCommand::AgentHeartbeat {
                portal_available: false,
            })
            .await;
    } else {
        write_status(
            &args.data_dir,
            true,
            None,
            &layout_file,
            clipboard::clipboard_ok(),
        );
        let _ = daemon
            .send(ControlCommand::AgentHeartbeat {
                portal_available: true,
            })
            .await;
    }

    let bind: SocketAddr = format!("0.0.0.0:{CONTROL_PORT}").parse()?;
    let daemon_listen = daemon.clone();
    let clip_listen = clipboard.clone();
    tokio::spawn(async move {
        let _ = peer_channel::listen(bind, move |msg, _| {
            let daemon = daemon_listen.clone();
            let clip = clip_listen.clone();
            async move {
                match msg {
                    PeerMessage::SwitchLocal => {
                        let _ = daemon.send(ControlCommand::Local).await;
                    }
                    PeerMessage::Clipboard { seq, text } => clip.apply_remote(seq, text),
                    PeerMessage::Ping => {}
                }
            }
        })
        .await;
    });

    let mut layout_mtime = std::fs::metadata(layout_store::layout_path(&args.data_dir))
        .and_then(|m| m.modified())
        .ok();
    let mut suspended = false;

    loop {
        // Layout UI reload
        if let Ok(meta) = std::fs::metadata(layout_store::layout_path(&args.data_dir)) {
            if let Ok(modified) = meta.modified() {
                if layout_mtime.map(|t| modified > t).unwrap_or(true) {
                    layout_mtime = Some(modified);
                    if let Ok(f) = layout_store::load_or_default(&args.data_dir) {
                        layout_file = f;
                        engine.set_layout(layout_file.layout.clone());
                        if let Err(e) = engine.configure().await {
                            warn!("reload layout: {e}");
                        } else {
                            info!("layout reloaded ({:?})", layout_file.peer_side);
                        }
                        write_status(
                            &args.data_dir,
                            engine.backend_mut().available(),
                            None,
                            &layout_file,
                            clipboard::clipboard_ok(),
                        );
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
                        layout_store::save(&args.data_dir, &layout_file)?;
                        engine.set_layout(layout_file.layout.clone());
                        if let Err(e) = engine.configure().await {
                            warn!("layout peer update: {e}");
                        }
                    }
                } else {
                    clipboard.set_peer(None);
                }

                let remote = status.active_target != LOCAL_TARGET;
                if remote && !suspended {
                    let _ = engine.backend_mut().suspend().await;
                    suspended = true;
                } else if !remote && suspended {
                    let _ = engine.backend_mut().resume().await;
                    suspended = false;
                }

                let _ = daemon
                    .send(ControlCommand::AgentHeartbeat {
                        portal_available: engine.backend_mut().available(),
                    })
                    .await;
            }
        }

        // Edge step with timeout so we can poll status
        let step = engine.step();
        tokio::select! {
            r = step => {
                if let Err(e) = r {
                    warn!("edge step: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
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
    mut backend: PortalBackend,
    clipboard: Arc<ClipboardBridge>,
) -> anyhow::Result<()> {
    let host_control = args
        .server
        .as_deref()
        .and_then(peer_channel::control_addr_from_peer);
    clipboard.set_peer(host_control);

    let local_barriers: Vec<_> = layout_file
        .layout
        .barriers
        .iter()
        .filter(|b| b.from_peer == layout_file.layout.local_peer)
        .cloned()
        .collect();

    // On the client: local barriers point to "local" via switch_local to the host.
    // Rewrite destination to local for the filter; the edge is peer_side.
    let barriers = if local_barriers.is_empty() {
        layout_file
            .layout
            .barriers
            .iter()
            .filter(|b| b.edge == layout_file.peer_side.as_edge())
            .cloned()
            .collect::<Vec<_>>()
    } else {
        local_barriers
    };

    if let Err(e) = backend.register(barriers).await {
        warn!("portal register (client): {e}");
        write_status(
            &args.data_dir,
            false,
            Some(&e.to_string()),
            &layout_file,
            clipboard::clipboard_ok(),
        );
    } else {
        write_status(
            &args.data_dir,
            true,
            None,
            &layout_file,
            clipboard::clipboard_ok(),
        );
    }

    let bind: SocketAddr = format!("0.0.0.0:{CONTROL_PORT}").parse()?;
    let clip_listen = clipboard.clone();
    tokio::spawn(async move {
        let _ = peer_channel::listen(bind, move |msg, _| {
            let clip = clip_listen.clone();
            async move {
                if let PeerMessage::Clipboard { seq, text } = msg {
                    clip.apply_remote(seq, text);
                }
            }
        })
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
                        let barriers: Vec<_> = layout_file
                            .layout
                            .barriers
                            .iter()
                            .filter(|b| b.from_peer == layout_file.layout.local_peer)
                            .cloned()
                            .collect();
                        if let Err(e) = backend.register(barriers).await {
                            warn!("client reload layout: {e}");
                        }
                        write_status(
                            &args.data_dir,
                            backend.available(),
                            backend.portal_error(),
                            &layout_file,
                            clipboard::clipboard_ok(),
                        );
                    }
                }
            }
        }

        let next = backend.next();
        tokio::select! {
            r = next => {
                match r {
                    Ok(ev) => {
                        info!("client edge {:?} → switch_local", ev.edge);
                        if let Some(addr) = host_control {
                            if let Err(e) =
                                peer_channel::send_to(addr, &PeerMessage::SwitchLocal).await
                            {
                                warn!("switch_local: {e}");
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(350)).await;
                    }
                    Err(e) => {
                        warn!("client edge: {e}");
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}
