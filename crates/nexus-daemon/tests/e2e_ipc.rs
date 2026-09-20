use nexus_common::*;
use nexus_daemon::controller::Controller;
use nexus_daemon::ipc_server;
use nexus_daemon::transport::RkvmAdapter;
use rkvm_server::target::{control_pair_with, drive_with, TargetRouter};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

fn unique_sock(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "nexuskvm-e2e-ipc-{}-{}-{label}.sock",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

fn adapter_with_peer(id: &str) -> RkvmAdapter {
    let mut router = TargetRouter::new();
    router.insert_peer(id.into(), "192.168.1.20:5258".into());
    let (handle, control) = control_pair_with(router.snapshot());
    tokio::spawn(drive_with(control, router));
    RkvmAdapter::new(handle, rkvm_server::server::new_peer_latencies())
}

const TOKEN: &str = "e2e-token";

async fn rpc(sock: &std::path::Path, command: ControlCommand) -> ControlResponse {
    rpc_with_token(sock, command, Some(TOKEN)).await
}

async fn rpc_with_token(
    sock: &std::path::Path,
    command: ControlCommand,
    token: Option<&str>,
) -> ControlResponse {
    let mut s = UnixStream::connect(sock)
        .await
        .expect("connect control socket");
    let req = ControlRequest {
        id: uuid::Uuid::new_v4().to_string(),
        token: token.map(str::to_string),
        command,
    };
    s.write_all(serde_json::to_string(&req).unwrap().as_bytes())
        .await
        .unwrap();
    s.write_all(b"\n").await.unwrap();
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).expect("control response json")
}

async fn spawn_ipc(label: &str) -> (PathBuf, Arc<Controller<RkvmAdapter>>) {
    let sock = unique_sock(label);
    let controller = Arc::new(Controller::new(adapter_with_peer("192.168.1.20")));
    controller.refresh_peers().await.unwrap();
    let serve_sock = sock.clone();
    let c = controller.clone();
    tokio::spawn(async move {
        let _ = ipc_server::serve(&serve_sock, c, Some(TOKEN.into())).await;
    });
    for _ in 0..50 {
        if sock.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(sock.exists(), "ipc socket should appear");
    (sock, controller)
}

#[tokio::test]
async fn e2e_status_switch_local_release() {
    let (sock, _) = spawn_ipc("switch").await;

    let st = rpc(&sock, ControlCommand::Status).await;
    assert!(st.ok);
    let status = st.status.expect("status");
    assert_eq!(status.active_target, LOCAL_TARGET);

    let sw = rpc(
        &sock,
        ControlCommand::Switch {
            target: "192.168.1.20".into(),
            entry: Some(entry_for(Edge::Right, 0.4)),
        },
    )
    .await;
    assert!(sw.ok, "{}", sw.error.unwrap_or_default());
    let after = sw.status.expect("status after switch");
    assert_eq!(after.active_target, "192.168.1.20");
    assert!(matches!(
        after.state,
        RuntimeState::Remote { ref peer, .. } if peer == "192.168.1.20"
    ));

    let local = rpc(&sock, ControlCommand::Local).await;
    assert!(local.ok);
    assert_eq!(local.status.expect("local").active_target, LOCAL_TARGET);

    let rel = rpc(&sock, ControlCommand::ReleaseAll).await;
    assert!(rel.ok);
    assert_eq!(rel.status.expect("release").active_target, LOCAL_TARGET);

    let _ = std::fs::remove_file(sock);
}

#[tokio::test]
async fn e2e_next_loops_local_peer_local_peer() {
    let (sock, _) = spawn_ipc("next").await;
    for expect in ["192.168.1.20", LOCAL_TARGET, "192.168.1.20"] {
        let r = rpc(&sock, ControlCommand::Next).await;
        assert!(r.ok, "{}", r.error.unwrap_or_default());
        assert_eq!(r.status.expect("status").active_target, expect);
    }
    let _ = std::fs::remove_file(sock);
}

#[tokio::test]
async fn e2e_switch_rejects_unknown_peer() {
    let (sock, _) = spawn_ipc("ghost").await;
    let sw = rpc(
        &sock,
        ControlCommand::Switch {
            target: "ghost".into(),
            entry: None,
        },
    )
    .await;
    assert!(!sw.ok);
    let _ = std::fs::remove_file(sock);
}

#[tokio::test]
async fn e2e_heartbeat_marks_agent() {
    let (sock, _) = spawn_ipc("hb").await;
    let r = rpc(
        &sock,
        ControlCommand::AgentHeartbeat {
            portal_available: true,
        },
    )
    .await;
    assert!(r.ok);
    let st = r.status.expect("status");
    assert!(st.agent_connected);
    assert!(st.portal_available);
    let _ = std::fs::remove_file(sock);
}

#[tokio::test]
async fn e2e_rejects_missing_token() {
    let (sock, _) = spawn_ipc("notoken").await;
    let r = rpc_with_token(&sock, ControlCommand::Status, None).await;
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("unauthorized"));
    let _ = std::fs::remove_file(sock);
}
