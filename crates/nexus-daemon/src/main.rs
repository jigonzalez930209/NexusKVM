use clap::Parser;
use nexus_daemon::{controller::Controller, ipc_server, transport::RkvmAdapter};
use rkvm_server::config::Config as RkvmConfig;
use rkvm_server::{server, target, tls};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/etc/nexuskvm/daemon.toml")]
    config: PathBuf,
}

#[derive(Deserialize)]
struct DaemonConfig {
    #[serde(default = "default_socket")]
    socket: PathBuf,
    #[serde(flatten)]
    rkvm: RkvmConfig,
}

fn default_socket() -> PathBuf {
    PathBuf::from("/run/nexuskvm/control.sock")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("nexus=info,rkvm_server=info,rkvm_input=info")),
        )
        .with_ansi(false)
        .with_target(true)
        .init();
    let boost = rkvm_input::priority::boost_cpu();
    match boost.rt_prio {
        Some(prio) => tracing::info!(
            prio,
            memlocked = boost.memlocked,
            "SCHED_FIFO realtime scheduling active (kernel-level input latency)"
        ),
        None => tracing::info!(
            nice = boost.nice,
            memlocked = boost.memlocked,
            "realtime unavailable; raised CPU priority via nice"
        ),
    }
    let args = Args::parse();
    tracing::info!(config = %args.config.display(), "nexus-kvmd 0.1.0-input2");
    // Deploy marker: postinstall verifies this string so a stale daemon that
    // does not understand `ControlCommand::Next` can never be paired with the
    // current UI.
    tracing::info!("ipc features: ipc-next");
    let raw = tokio::fs::read_to_string(&args.config).await?;
    let cfg: DaemonConfig = toml::from_str(&raw)?;

    let acceptor = tls::configure(&cfg.rkvm.certificate, &cfg.rkvm.key).await?;
    let switch_keys = cfg
        .rkvm
        .switch_keys
        .iter()
        .copied()
        .map(Into::into)
        .collect();
    let propagate = cfg.rkvm.propagate_switch_keys.unwrap_or(false);

    let (handle, control) = target::control_pair();
    let latencies = rkvm_server::server::new_peer_latencies();
    let controller = Arc::new(Controller::new(RkvmAdapter::new(
        handle.clone(),
        latencies.clone(),
    )));
    controller.refresh_peers().await?;

    // Resync on every transport change plus a periodic safety tick so a stuck
    // transition, a dead peer or a lost snapshot cannot leave the daemon
    // routing input to nowhere.
    let mut snap = handle.subscribe();
    let watcher = controller.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                changed = snap.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                _ = tick.tick() => {}
            }
            let _ = watcher.sync_target().await;
        }
    });

    if let Some(parent) = cfg.socket.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listen = cfg.rkvm.listen;
    let password = cfg.rkvm.password.clone();
    if password.is_empty() {
        anyhow::bail!("daemon password must not be empty");
    }
    let socket = cfg.socket.clone();

    tokio::select! {
        result = server::run(listen, acceptor, &password, &switch_keys, propagate, control, latencies) => {
            result.map_err(|e| anyhow::anyhow!(e))?;
        }
        result = ipc_server::serve(&socket, controller, Some(password.clone())) => {
            result?;
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("exiting on signal");
        }
    }
    Ok(())
}
