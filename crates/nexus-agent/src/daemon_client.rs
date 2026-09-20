use anyhow::{anyhow, Result};
use nexus_common::*;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    time,
};

/// A hung daemon must not block the agent's status loop forever (which would
/// let the agent heartbeat expire and mark the UI connection dead).
const DAEMON_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct DaemonClient {
    pub socket: String,
    pub token: Option<String>,
}
impl DaemonClient {
    pub async fn send(&self, command: ControlCommand) -> Result<ControlResponse> {
        let fut = async {
            let mut s = UnixStream::connect(&self.socket).await?;
            let req = ControlRequest {
                id: uuid::Uuid::new_v4().to_string(),
                token: self.token.clone(),
                command,
            };
            s.write_all(serde_json::to_string(&req)?.as_bytes()).await?;
            s.write_all(b"\n").await?;
            let mut line = String::new();
            BufReader::new(s).read_line(&mut line).await?;
            Ok::<_, anyhow::Error>(serde_json::from_str(&line)?)
        };
        time::timeout(DAEMON_TIMEOUT, fut)
            .await
            .map_err(|_| anyhow!("daemon request timed out after {DAEMON_TIMEOUT:?}"))?
    }
}
