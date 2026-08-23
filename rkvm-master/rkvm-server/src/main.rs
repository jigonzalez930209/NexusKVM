use clap::Parser;
use rkvm_server::config::Config;
use rkvm_server::server;
use rkvm_server::target;
use rkvm_server::tls;
use std::future;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use tokio::{fs, signal, time};
use tracing::subscriber;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "rkvm-server", about = "The rkvm server application")]
struct Args {
    #[arg(help = "Path to configuration file")]
    config_path: PathBuf,
    #[arg(help = "Shutdown after N seconds", long, short)]
    shutdown_after: Option<u64>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().without_time());

    subscriber::set_global_default(registry).unwrap();

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
    let config = match fs::read_to_string(&args.config_path).await {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("Error reading config: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let config = match toml::from_str::<Config>(&config) {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("Error parsing config: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let acceptor = match tls::configure(&config.certificate, &config.key).await {
        Ok(acceptor) => acceptor,
        Err(err) => {
            tracing::error!("Error configuring TLS: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let shutdown = async {
        match args.shutdown_after {
            Some(shutdown_after) => time::sleep(Duration::from_secs(shutdown_after)).await,
            None => future::pending().await,
        }
    };

    let switch_keys = config.switch_keys.into_iter().map(Into::into).collect();
    let propagate_switch_keys = config.propagate_switch_keys.unwrap_or(true);
    let (_handle, control) = target::control_pair();
    let latencies = server::new_peer_latencies();

    tokio::select! {
        result = server::run(config.listen, acceptor, &config.password, &switch_keys, propagate_switch_keys, control, latencies) => {
            if let Err(err) = result {
                tracing::error!("Error: {}", err);
                return ExitCode::FAILURE;
            }
        }
        result = signal::ctrl_c() => {
            if let Err(err) = result {
                tracing::error!("Error setting up signal handler: {}", err);
                return ExitCode::FAILURE;
            }

            tracing::info!("Exiting on signal");
        }
        _ = shutdown => {
            tracing::info!("Shutting down as requested");
        }
    }

    ExitCode::SUCCESS
}
