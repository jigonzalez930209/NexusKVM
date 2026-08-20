use clap::{Parser, Subcommand};
use nexus_common::*;
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

fn default_socket() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("nexuskvm/control.sock"))
        .unwrap_or_else(|_| PathBuf::from("/run/nexuskvm/control.sock"))
}

#[derive(Parser)]
struct Args {
    #[arg(long)]
    socket: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Status,
    Peers,
    Switch { target: String },
    Local,
    ReleaseAll,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a = Args::parse();
    let socket = a.socket.unwrap_or_else(default_socket);
    let cmd = match a.cmd {
        Cmd::Status => ControlCommand::Status,
        Cmd::Peers => ControlCommand::Peers,
        Cmd::Switch { target } => ControlCommand::Switch {
            target,
            entry: None,
        },
        Cmd::Local => ControlCommand::Local,
        Cmd::ReleaseAll => ControlCommand::ReleaseAll,
    };
    let req = ControlRequest {
        id: uuid::Uuid::new_v4().to_string(),
        command: cmd,
    };
    let mut s = UnixStream::connect(&socket).await?;
    s.write_all(serde_json::to_string(&req)?.as_bytes()).await?;
    s.write_all(
        b"
",
    )
    .await?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).await?;
    let r: ControlResponse = serde_json::from_str(&line)?;
    println!("{}", serde_json::to_string_pretty(&r)?);
    if !r.ok {
        std::process::exit(2)
    }
    Ok(())
}
