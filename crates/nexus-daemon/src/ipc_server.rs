use crate::{controller::Controller, transport::InputTransport};
use anyhow::Result;
use nexus_common::*;
use std::{path::Path, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

pub async fn serve<T: InputTransport + 'static>(
    path: &Path,
    controller: Arc<Controller<T>>,
) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
    }
    loop {
        let (stream, _) = listener.accept().await?;
        let c = controller.clone();
        tokio::spawn(async move {
            let _ = handle(stream, c).await;
        });
    }
}
async fn handle<T: InputTransport>(stream: UnixStream, c: Arc<Controller<T>>) -> Result<()> {
    let (r, mut w) = stream.into_split();
    let mut lines = BufReader::new(r).lines();
    while let Some(line) = lines.next_line().await? {
        if line.len() > 65536 {
            break;
        }
        let req: ControlRequest = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                w.write_all(
                    format!(
                        "{}
",
                        serde_json::to_string(&ControlResponse::error(
                            "unknown".into(),
                            e.to_string()
                        ))?
                    )
                    .as_bytes(),
                )
                .await?;
                continue;
            }
        };
        let id = req.id.clone();
        let resp = match req.command {
            ControlCommand::Status | ControlCommand::Peers => {
                ControlResponse::ok(id, Some(c.status()))
            }
            ControlCommand::Switch { target, entry } => match c
                .switch_to(
                    target,
                    entry.unwrap_or(EntryPoint {
                        edge: Edge::Left,
                        normalized_position: 0.5,
                        inset_px: 6,
                    }),
                )
                .await
            {
                Ok(t) => {
                    let mut r = ControlResponse::ok(id, Some(c.status()));
                    r.transition_id = Some(t);
                    r
                }
                Err(e) => ControlResponse::error(id, e.to_string()),
            },
            ControlCommand::Local => match c.local().await {
                Ok(t) => {
                    let mut r = ControlResponse::ok(id, Some(c.status()));
                    r.transition_id = Some(t);
                    r
                }
                Err(e) => ControlResponse::error(id, e.to_string()),
            },
            ControlCommand::ReleaseAll => match c.release_all().await {
                Ok(_) => ControlResponse::ok(id, Some(c.status())),
                Err(e) => ControlResponse::error(id, e.to_string()),
            },
            ControlCommand::AgentHeartbeat { portal_available } => {
                c.heartbeat(portal_available);
                ControlResponse::ok(id, Some(c.status()))
            }
            ControlCommand::Shutdown => ControlResponse::error(id, "remote shutdown disabled"),
        };
        w.write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        w.write_all(
            b"
",
        )
        .await?;
    }
    Ok(())
}
