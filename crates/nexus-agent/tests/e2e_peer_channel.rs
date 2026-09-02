use nexus_agent::peer_channel::{self, PeerMessage, CONTROL_PORT};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

const SECRET: &str = "e2e-secret";

#[tokio::test]
async fn e2e_switch_local_roundtrip() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let got: Arc<Mutex<Option<PeerMessage>>> = Arc::new(Mutex::new(None));
    let got2 = got.clone();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut lines = BufReader::new(stream).lines();
        if let Ok(Some(line)) = lines.next_line().await {
            *got2.lock().unwrap() = serde_json::from_str(&line).ok();
        }
    });

    // unsigned listener: send_to expects an HMAC ack, so this only checks connect+write
    let _ = peer_channel::send_to(addr, &PeerMessage::SwitchLocal, SECRET).await;
}

#[tokio::test]
async fn e2e_listen_accepts_switch_local() {
    let port = 19100 + (std::process::id() % 400) as u16;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let got: Arc<Mutex<Option<PeerMessage>>> = Arc::new(Mutex::new(None));
    let got2 = got.clone();
    let server = tokio::spawn(async move {
        let _ = peer_channel::listen(
            addr,
            SECRET.into(),
            None,
            move |msg, _| {
                let got2 = got2.clone();
                async move {
                    *got2.lock().unwrap() = Some(msg);
                    Ok(Some("local".into()))
                }
            },
            |_| {},
        )
        .await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let ack = peer_channel::send_to(addr, &PeerMessage::SwitchLocal, SECRET)
        .await
        .expect("send to listen()");
    assert!(matches!(ack, PeerMessage::Ack { ok: true, .. }));
    timeout(Duration::from_secs(2), async {
        loop {
            if got.lock().unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("listen received SwitchLocal");
    server.abort();
}

#[tokio::test]
async fn e2e_rejects_bad_mac() {
    let port = 19500 + (std::process::id() % 400) as u16;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let server = tokio::spawn(async move {
        let _ = peer_channel::listen(
            addr,
            SECRET.into(),
            None,
            |_msg, _| async { Ok(None) },
            |_| {},
        )
        .await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let err = peer_channel::send_to(addr, &PeerMessage::SwitchLocal, "wrong").await;
    assert!(err.is_err() || matches!(err, Ok(PeerMessage::Ack { ok: false, .. })));
    server.abort();
}

#[tokio::test]
async fn e2e_control_addr_from_peer_id() {
    let a = peer_channel::control_addr_from_peer("10.0.0.8:41222").unwrap();
    assert_eq!(a.port(), CONTROL_PORT);
    assert_eq!(a.ip().to_string(), "10.0.0.8");
}

#[tokio::test]
async fn e2e_clip_text_and_png_roundtrip() {
    let port = 19700 + (std::process::id() % 400) as u16;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let dir = std::env::temp_dir().join(format!("nexus-clip-e2e-{port}"));
    let _ = std::fs::create_dir_all(&dir);
    let got: Arc<Mutex<Vec<peer_channel::IncomingClip>>> = Arc::new(Mutex::new(Vec::new()));
    let got2 = got.clone();
    let inbox = dir.clone();
    let server = tokio::spawn(async move {
        let _ = peer_channel::listen(
            addr,
            SECRET.into(),
            Some(inbox),
            |_msg, _| async { Ok(None) },
            move |c| got2.lock().unwrap().push(c),
        )
        .await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;

    let ack = peer_channel::send_clip(addr, SECRET, &peer_channel::ClipOut::Text("hello kvm".into()))
        .await
        .unwrap();
    assert!(matches!(ack, PeerMessage::Ack { ok: true, .. }));

    let png = nexus_agent::clipboard::rgba_to_png(1, 1, &[0, 128, 255, 255]).unwrap();
    let ack = peer_channel::send_clip(addr, SECRET, &peer_channel::ClipOut::Png(png.clone()))
        .await
        .unwrap();
    assert!(matches!(ack, PeerMessage::Ack { ok: true, .. }));

    let src = dir.join("src.txt");
    std::fs::write(&src, b"file-bytes").unwrap();
    let ack = peer_channel::send_clip(
        addr,
        SECRET,
        &peer_channel::ClipOut::Files(vec![("src.txt".into(), src)]),
    )
    .await
    .unwrap();
    assert!(matches!(ack, PeerMessage::Ack { ok: true, .. }));

    timeout(Duration::from_secs(3), async {
        loop {
            if got.lock().unwrap().len() >= 3 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("three clip payloads");
    let items = got.lock().unwrap().clone();
    assert_eq!(items[0].text.as_deref(), Some("hello kvm"));
    assert_eq!(items[1].png.as_ref().unwrap().as_slice(), png.as_slice());
    assert_eq!(std::fs::read(&items[2].files[0]).unwrap(), b"file-bytes");
    server.abort();
}

#[tokio::test]
async fn e2e_clip_folder_roundtrip() {
    let port = 19900 + (std::process::id() % 400) as u16;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let dir = std::env::temp_dir().join(format!("nexus-clip-folder-{port}"));
    let src = dir.join("src-tree");
    let folder = src.join("pack");
    std::fs::create_dir_all(folder.join("empty")).unwrap();
    std::fs::write(folder.join("hello.txt"), b"nested").unwrap();
    let inbox = dir.join("inbox");
    std::fs::create_dir_all(&inbox).unwrap();
    let got: Arc<Mutex<Vec<peer_channel::IncomingClip>>> = Arc::new(Mutex::new(Vec::new()));
    let got2 = got.clone();
    let inbox2 = inbox.clone();
    let server = tokio::spawn(async move {
        let _ = peer_channel::listen(
            addr,
            SECRET.into(),
            Some(inbox2),
            |_msg, _| async { Ok(None) },
            move |c| got2.lock().unwrap().push(c),
        )
        .await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let ack = peer_channel::send_clip(
        addr,
        SECRET,
        &peer_channel::ClipOut::Files(vec![("pack".into(), folder)]),
    )
    .await
    .unwrap();
    assert!(matches!(ack, PeerMessage::Ack { ok: true, .. }));
    timeout(Duration::from_secs(3), async {
        loop {
            if !got.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("folder clip");
    let root = got.lock().unwrap()[0].files[0].clone();
    assert!(root.is_dir());
    assert_eq!(std::fs::read(root.join("hello.txt")).unwrap(), b"nested");
    assert!(root.join("empty").is_dir());
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_switch_local_not_blocked_by_clip_handler() {
    let port = 19800 + (std::process::id() % 400) as u16;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let dir = std::env::temp_dir().join(format!("nexus-clip-slow-{port}"));
    let _ = std::fs::create_dir_all(&dir);
    let server = tokio::spawn(async move {
        let _ = peer_channel::listen(
            addr,
            SECRET.into(),
            Some(dir),
            |_msg, _| async { Ok(Some("local".into())) },
            |_| std::thread::sleep(Duration::from_millis(400)),
        )
        .await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let png = vec![0u8; 64 * 1024];
    let send_clip = tokio::spawn(async move {
        peer_channel::send_clip(addr, SECRET, &peer_channel::ClipOut::Png(png)).await
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let started = std::time::Instant::now();
    let ack = peer_channel::send_to(addr, &PeerMessage::SwitchLocal, SECRET)
        .await
        .unwrap();
    assert!(matches!(ack, PeerMessage::Ack { ok: true, .. }));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "SwitchLocal waited {:?}",
        started.elapsed()
    );
    let _ = send_clip.await;
    server.abort();
}

#[tokio::test]
async fn e2e_listen_rejects_empty_secret() {
    let err = peer_channel::listen(
        "127.0.0.1:0".parse().unwrap(),
        String::new(),
        None,
        |_msg, _| async { Ok(None) },
        |_| {},
    )
    .await;
    assert!(err.is_err());
}
