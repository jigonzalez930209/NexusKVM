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
    token: Option<String>,
) -> Result<()> {
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        anyhow::bail!("control socket token required");
    };
    if path.exists() {
        // Refuse to steal a socket that a live daemon is serving: two daemons
        // sharing input is a split brain (each has its own router state).
        match UnixStream::connect(path).await {
            Ok(_) => anyhow::bail!(
                "control socket {} is already served by another nexus-kvmd",
                path.display()
            ),
            Err(_) => std::fs::remove_file(path)?,
        }
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
        let token = token.clone();
        tokio::spawn(async move {
            let _ = handle(stream, c, token).await;
        });
    }
}

#[cfg(unix)]
fn peercred_ok(stream: &UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        ) == 0
            && cred.pid > 0
    }
}

async fn handle<T: InputTransport>(
    stream: UnixStream,
    c: Arc<Controller<T>>,
    token: String,
) -> Result<()> {
    #[cfg(unix)]
    if !peercred_ok(&stream) {
        return Ok(());
    }
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);
    loop {
        // Bounded read: the 64 KiB check must happen before the whole line is
        // buffered, otherwise a local peer can OOM the daemon.
        let mut buf = Vec::new();
        let n = {
            use tokio::io::AsyncReadExt;
            let mut limited = (&mut reader).take(65536 + 1);
            limited.read_until(b'\n', &mut buf).await?
        };
        if n == 0 {
            break;
        }
        if buf.len() > 65536 {
            break;
        }
        let line = match std::str::from_utf8(&buf) {
            Ok(line) => line,
            Err(_) => break,
        };
        let req: ControlRequest = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                w.write_all(
                    format!(
                        "{}\n",
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
        if !token_eq(&token, req.token.as_deref()) {
            let resp = ControlResponse::error(id, "unauthorized");
            w.write_all(serde_json::to_string(&resp)?.as_bytes())
                .await?;
            w.write_all(b"\n").await?;
            continue;
        }
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
            ControlCommand::SwitchEdge { side, position } => {
                match c.switch_edge(side, position).await {
                    Ok(t) => {
                        let mut r = ControlResponse::ok(id, Some(c.status()));
                        r.transition_id = Some(t);
                        r
                    }
                    Err(e) => ControlResponse::error(id, e.to_string()),
                }
            }
            ControlCommand::Next => match c.next().await {
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
            ControlCommand::PeerLocal => match c.local_from_peer().await {
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
        w.write_all(b"\n").await?;
    }
    Ok(())
}
