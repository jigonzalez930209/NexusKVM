use anyhow::Result;
use nexus_common::*;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
#[derive(Clone)]
pub struct DaemonClient {
    pub socket: String,
}
impl DaemonClient {
    pub async fn send(&self, command: ControlCommand) -> Result<ControlResponse> {
        let mut s = UnixStream::connect(&self.socket).await?;
        let req = ControlRequest {
            id: uuid::Uuid::new_v4().to_string(),
            command,
        };
        s.write_all(serde_json::to_string(&req)?.as_bytes()).await?;
        s.write_all(
            b"
",
        )
        .await?;
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).await?;
        Ok(serde_json::from_str(&line)?)
    }
}
