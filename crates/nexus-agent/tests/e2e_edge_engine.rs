use async_trait::async_trait;
use nexus_agent::backend::{EdgeCaptureBackend, EdgeEvent};
use nexus_agent::daemon_client::DaemonClient;
use nexus_agent::engine::EdgeEngine;
use nexus_common::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

struct MockBackend {
    events: mpsc::Receiver<EdgeEvent>,
    registered: Arc<Mutex<Vec<Barrier>>>,
}

#[async_trait]
impl EdgeCaptureBackend for MockBackend {
    async fn register(&mut self, barriers: Vec<Barrier>) -> anyhow::Result<()> {
        *self.registered.lock().unwrap() = barriers;
        Ok(())
    }
    async fn next(&mut self) -> anyhow::Result<EdgeEvent> {
        self.events
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("event channel closed"))
    }
    async fn suspend(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn resume(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn available(&self) -> bool {
        true
    }
}

fn sock_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "nexuskvm-e2e-engine-{}-{}-{label}.sock",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

async fn fake_daemon(path: PathBuf, seen: Arc<Mutex<Vec<String>>>) {
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("bind fake daemon");
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => break,
        };
        let (r, mut w) = stream.into_split();
        let mut lines = BufReader::new(r).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let req: ControlRequest = serde_json::from_str(&line).expect("req");
            let kind = match &req.command {
                ControlCommand::Switch { target, entry } => {
                    let edge = entry.as_ref().map(|e| format!("{:?}", e.edge));
                    format!("switch:{target}:{}", edge.unwrap_or_default())
                }
                ControlCommand::Local => "local".into(),
                other => format!("{other:?}"),
            };
            seen.lock().unwrap().push(kind);
            let status = AppStatus {
                state: RuntimeState::Remote {
                    peer: "peer-b".into(),
                    transition_id: uuid::Uuid::nil(),
                },
                active_target: "peer-b".into(),
                peers: Default::default(),
                agent_connected: true,
                portal_available: true,
                emergency_shortcut: String::new(),
            };
            let mut resp = ControlResponse::ok(req.id, Some(status));
            resp.transition_id = Some(uuid::Uuid::nil());
            w.write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                .await
                .unwrap();
            w.write_all(b"\n").await.unwrap();
        }
    }
}

#[tokio::test]
async fn e2e_edge_crossing_sends_switch_with_opposite_entry() {
    let sock = sock_path("edge");
    let seen = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(fake_daemon(sock.clone(), seen.clone()));
    for _ in 0..50 {
        if sock.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let layout = LayoutFile::default_right(Some("peer-b")).layout;
    let (tx, rx) = mpsc::channel(4);
    let registered = Arc::new(Mutex::new(Vec::new()));
    let mut engine = EdgeEngine::new(
        MockBackend {
            events: rx,
            registered: registered.clone(),
        },
        DaemonClient {
            socket: sock.to_string_lossy().into(),
            token: None,
        },
        layout,
    );
    engine.configure().await.unwrap();
    assert_eq!(registered.lock().unwrap().len(), 1);

    tx.send(EdgeEvent {
        display_id: "main".into(),
        edge: Edge::Right,
        normalized_position: 0.25,
        any_button_pressed: false,
    })
    .await
    .unwrap();
    engine.step().await.unwrap();

    let cmds = seen.lock().unwrap().clone();
    assert!(
        cmds.iter().any(|c| c.starts_with("switch:peer-b:Left")),
        "expected switch to peer-b entering Left, got {cmds:?}"
    );

    let _ = std::fs::remove_file(sock);
}
