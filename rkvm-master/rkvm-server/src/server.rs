use crate::target::{TargetControl, TargetRouter, LOCAL_TARGET};
use rkvm_input::abs::{AbsAxis, AbsEvent, AbsInfo};
use rkvm_input::event::Event;
use rkvm_input::key::{Key, KeyEvent};
use rkvm_input::monitor::Monitor;
use rkvm_input::rel::RelAxis;
use rkvm_input::sync::SyncEvent;
use rkvm_net::auth::{AuthChallenge, AuthResponse, AuthStatus};
use rkvm_net::message::Message;
use rkvm_net::version::Version;
use rkvm_net::{Pong, Update};
use slab::Slab;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CString;
use std::io::{self, ErrorKind};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Shared registry of per-peer round-trip times in milliseconds, keyed by peer id.
pub type PeerLatencies = Arc<Mutex<HashMap<String, u32>>>;

pub fn new_peer_latencies() -> PeerLatencies {
    Arc::new(Mutex::new(HashMap::new()))
}
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{self, Sender};
use tokio::time;
use tokio_rustls::TlsAcceptor;
use tracing::Instrument;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(io::Error),
    #[error("Input error: {0}")]
    Input(io::Error),
    #[error("Event queue overflow")]
    Overflow,
}

struct ClientSlot {
    sender: Sender<Update>,
    #[allow(dead_code)]
    addr: SocketAddr,
    id: String,
}

pub async fn run(
    listen: SocketAddr,
    acceptor: TlsAcceptor,
    password: &str,
    switch_keys: &HashSet<Key>,
    propagate_switch_keys: bool,
    mut control: TargetControl,
    latencies: PeerLatencies,
) -> Result<(), Error> {
    let listener = TcpListener::bind(&listen).await.map_err(Error::Network)?;
    tracing::info!("Listening on {}", listen);

    let mut monitor = Monitor::new();
    let mut devices = Slab::<Device>::new();
    let mut clients = Slab::<ClientSlot>::new();
    let mut router = TargetRouter::new();
    let mut pressed_keys = HashSet::new();
    let mut chord_active = false;

    // High capacity: the mouse produces REL_X/REL_Y/SYN at a high rate; cap 1
    // blocked the interceptor until each TLS flush and caused SYN_DROPPED.
    let (events_sender, mut events_receiver) = mpsc::channel(1024);
    let (reg_tx, mut reg_rx) = mpsc::channel::<(String, SocketAddr, Sender<Update>)>(16);
    control.publish(router.snapshot());
    let mut prune_tick = time::interval(Duration::from_millis(250));
    prune_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        let event = async { events_receiver.recv().await.unwrap() };

        tokio::select! {
            Some(cmd) = control.recv() => {
                let keys = cmd.apply(&mut router);
                emit_releases(&mut devices, &mut clients, &mut router, keys).await?;
                if let Some((edge, pos)) = router.take_warp() {
                    let active = router.active_target().to_string();
                    emit_warp(&mut devices, &mut clients, &mut router, active, edge, pos).await?;
                }
                control.publish(router.snapshot());
            }
            _ = prune_tick.tick() => {
                prune_and_release(
                    &mut devices,
                    &mut clients,
                    &mut router,
                    &latencies,
                    &control,
                )
                .await?;
            }
            Some((id, addr, sender)) = reg_rx.recv() => {
                prune_and_release(
                    &mut devices,
                    &mut clients,
                    &mut router,
                    &latencies,
                    &control,
                )
                .await?;
                clients.retain(|_, client| client.id != id);
                router.insert_peer(id.clone(), addr.to_string());
                let notify = sender.clone();
                clients.insert(ClientSlot {
                    sender,
                    addr,
                    id: id.clone(),
                });
                for (dev_id, device) in devices.iter() {
                    let _ = notify
                        .send(Update::CreateDevice {
                            id: dev_id,
                            name: device.name.clone(),
                            version: device.version,
                            vendor: device.vendor,
                            product: device.product,
                            rel: device.rel.clone(),
                            abs: device.abs.clone(),
                            keys: device.keys.clone(),
                            delay: device.delay,
                            period: device.period,
                        })
                        .await;
                }
                control.publish(router.snapshot());
            }
            result = listener.accept() => {
                let (stream, addr) = result.map_err(Error::Network)?;
                if let Err(err) = stream.set_nodelay(true) {
                    tracing::warn!(%err, "TCP_NODELAY failed on accept");
                }
                let acceptor = acceptor.clone();
                let password = password.to_owned();
                let id = peer_id(addr);
                let span = tracing::info_span!("connection", addr = %addr);
                let client_latencies = latencies.clone();
                let reg_tx = reg_tx.clone();
                tokio::spawn(
                    async move {
                        tracing::info!("Connected");
                        match client(
                            stream,
                            acceptor,
                            &password,
                            &id,
                            addr,
                            client_latencies.clone(),
                            reg_tx,
                        )
                        .await
                        {
                            Ok(()) => tracing::info!("Disconnected"),
                            Err(err) => tracing::error!("Disconnected: {}", err),
                        }
                        client_latencies.lock().unwrap().remove(&id);
                    }
                    .instrument(span),
                );
            }
            result = monitor.read() => {
                let mut interceptor = result.map_err(Error::Input)?;

                let name = interceptor.name().to_owned();
                let id = devices.vacant_key();
                let version = interceptor.version();
                let vendor = interceptor.vendor();
                let product = interceptor.product();
                let rel = interceptor.rel().collect::<HashSet<_>>();
                let abs = interceptor.abs().collect::<HashMap<_,_>>();
                let keys = interceptor.key().collect::<HashSet<_>>();
                let repeat = interceptor.repeat();

                for (_, client) in &clients {
                    let update = Update::CreateDevice {
                        id,
                        name: name.clone(),
                        version,
                        vendor,
                        product,
                        rel: rel.clone(),
                        abs: abs.clone(),
                        keys: keys.clone(),
                        delay: repeat.delay,
                        period: repeat.period,
                    };

                    let _ = client.sender.send(update).await;
                }

                let (interceptor_sender, mut interceptor_receiver) = mpsc::channel(32);
                devices.insert(Device {
                    name,
                    version,
                    vendor,
                    product,
                    rel,
                    abs,
                    keys,
                    delay: repeat.delay,
                    period: repeat.period,
                    sender: interceptor_sender,
                });

                let events_sender = events_sender.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            event = interceptor.read() => {
                                if event.is_err() | events_sender.send((id, event)).await.is_err() {
                                    break;
                                }
                            }
                            event = interceptor_receiver.recv() => {
                                let event = match event {
                                    Some(event) => event,
                                    None => break,
                                };

                                match interceptor.write(&event).await {
                                    Ok(()) => {},
                                    Err(err) => {
                                        let _ = events_sender.send((id, Err(err))).await;
                                        break;
                                    }
                                }

                                tracing::trace!(id = %id, "Wrote an event to device");
                            }
                        }
                    }
                });

                let device = &devices[id];

                tracing::info!(
                    id = %id,
                    name = ?device.name,
                    vendor = %device.vendor,
                    product = %device.product,
                    version = %device.version,
                    "Registered new device"
                );
            }
            (id, result) = event => match result {
                Ok(event) => {
                    let mut press = false;

                    if let Event::Key(KeyEvent { key, down }) = event {
                        router.note_key(key, down);
                        if switch_keys.contains(&key) {
                            press = true;

                            match down {
                                true => pressed_keys.insert(key),
                                false => pressed_keys.remove(&key),
                            };
                        }
                    }

                    // Finish the chord only AFTER routing this event so key-ups
                    // still go to the pre-switch target (prevents sticky modifiers).
                    let chord_complete = press && pressed_keys.is_empty();

                    if press && pressed_keys.len() == switch_keys.len() {
                        chord_active = true;
                        // Release everything the previous target still thinks is
                        // held (including the switch chord) before flipping.
                        let held = router.drain_held();
                        match router.switch_next() {
                            Ok(_) => {
                                tracing::info!(target = %router.active_target(), "Switched target");
                                emit_releases(
                                    &mut devices,
                                    &mut clients,
                                    &mut router,
                                    held,
                                )
                                .await?;
                                control.publish(router.snapshot());
                            }
                            Err(err) => {
                                // Put held keys back so later ups still track.
                                for (key, dest) in held {
                                    router.restore_held(key, dest);
                                }
                                tracing::warn!(%err, "Switch shortcut ignored");
                            }
                        }
                    }

                    // Withhold only the engaged shortcut chord. Lone switch keys
                    // (Ctrl/Alt) must still reach the target, otherwise Ctrl+C,
                    // Alt+Tab, etc. arrive as the bare key on either machine.
                    let suppress_chord = press && !propagate_switch_keys && chord_active;
                    if chord_complete {
                        chord_active = false;
                    }
                    if suppress_chord {
                        if chord_complete {
                            router.finish_chord();
                        }
                        continue;
                    }

                    let events = [event]
                        .into_iter()
                        .chain(press.then_some(Event::Sync(SyncEvent::All)));

                    let dest = router.event_target().to_string();
                    route_events(&mut devices, &mut clients, &mut router, id, dest, events).await?;
                    if chord_complete {
                        router.finish_chord();
                    }
                    control.publish(router.snapshot());
                }
                Err(err)
                    if err.kind() == ErrorKind::BrokenPipe
                        || err.kind() == ErrorKind::PermissionDenied =>
                {
                    tracing::warn!(id = %id, %err, "Dropping input device");
                    for (_, client) in &clients {
                        let _ = client.sender.send(Update::DestroyDevice { id }).await;
                    }
                    devices.remove(id);
                    tracing::info!(id = %id, "Destroyed device");
                }
                Err(err) => return Err(Error::Input(err)),
            }
        }
    }
}

/// Stable across reconnects: the client ephemeral TCP port must not be part of the id.
fn peer_id(addr: SocketAddr) -> String {
    addr.ip().to_string()
}

fn prune_clients(
    clients: &mut Slab<ClientSlot>,
    router: &mut TargetRouter,
    latencies: &PeerLatencies,
) -> Vec<(Key, String)> {
    let mut dead = Vec::new();
    clients.retain(|_, client| {
        if client.sender.is_closed() {
            dead.push(client.id.clone());
            false
        } else {
            true
        }
    });
    let mut keys = Vec::new();
    for id in dead {
        keys.extend(router.remove_peer(&id));
        latencies.lock().unwrap().remove(&id);
    }
    keys
}

async fn prune_and_release(
    devices: &mut Slab<Device>,
    clients: &mut Slab<ClientSlot>,
    router: &mut TargetRouter,
    latencies: &PeerLatencies,
    control: &TargetControl,
) -> Result<(), Error> {
    let before = router.snapshot();
    let keys = prune_clients(clients, router, latencies);
    if !keys.is_empty() {
        emit_releases(devices, clients, router, keys).await?;
    }
    let after = router.snapshot();
    if before != after {
        control.publish(after);
    }
    Ok(())
}

async fn route_events(
    devices: &mut Slab<Device>,
    clients: &mut Slab<ClientSlot>,
    router: &mut TargetRouter,
    device_id: usize,
    dest: String,
    events: impl Iterator<Item = Event>,
) -> Result<(), Error> {
    if dest == LOCAL_TARGET {
        for event in events {
            match devices[device_id].sender.try_send(event) {
                Ok(()) | Err(TrySendError::Closed(_)) => {}
                Err(TrySendError::Full(_)) => return Err(Error::Overflow),
            }
        }
        return Ok(());
    }

    let slot = clients.iter().find(|(_, c)| c.id == dest).map(|(k, _)| k);
    let Some(key) = slot else {
        router.remove_peer(&dest);
        return Ok(());
    };

    for event in events {
        if clients[key]
            .sender
            .send(Update::Event {
                id: device_id,
                event,
            })
            .await
            .is_err()
        {
            let id = clients[key].id.clone();
            clients.remove(key);
            router.remove_peer(&id);
            break;
        }
    }
    Ok(())
}

async fn emit_releases(
    devices: &mut Slab<Device>,
    clients: &mut Slab<ClientSlot>,
    router: &mut TargetRouter,
    keys: Vec<(Key, String)>,
) -> Result<(), Error> {
    for (key, dest) in keys {
        if dest == LOCAL_TARGET {
            for (_, device) in devices.iter() {
                for event in [
                    Event::Key(KeyEvent { key, down: false }),
                    Event::Sync(SyncEvent::All),
                ] {
                    match device.sender.try_send(event) {
                        Ok(()) | Err(TrySendError::Closed(_)) => {}
                        Err(TrySendError::Full(_)) => return Err(Error::Overflow),
                    }
                }
            }
        } else {
            let device_ids: Vec<usize> = devices.iter().map(|(id, _)| id).collect();
            for device_id in device_ids {
                let events = [
                    Event::Key(KeyEvent { key, down: false }),
                    Event::Sync(SyncEvent::All),
                ];
                route_events(
                    devices,
                    clients,
                    router,
                    device_id,
                    dest.clone(),
                    events.into_iter(),
                )
                .await?;
            }
        }
    }
    Ok(())
}

fn lerp_abs(normalized: f32, min: i32, max: i32) -> i32 {
    if max <= min {
        return min;
    }
    let t = normalized.clamp(0.0, 1.0);
    min + ((max - min) as f32 * t).round() as i32
}

async fn emit_warp(
    devices: &mut Slab<Device>,
    clients: &mut Slab<ClientSlot>,
    router: &mut TargetRouter,
    dest: String,
    edge: u8,
    pos: f32,
) -> Result<(), Error> {
    let Some((device_id, xmin, xmax, ymin, ymax)) = devices.iter().find_map(|(id, d)| {
        let x = d.abs.get(&AbsAxis::X)?;
        let y = d.abs.get(&AbsAxis::Y)?;
        Some((id, x.min, x.max, y.min, y.max))
    }) else {
        return Ok(());
    };
    let inset = 2;
    let (xv, yv) = match edge {
        0 => (xmin + inset, lerp_abs(pos, ymin, ymax)),
        1 => (xmax.saturating_sub(inset), lerp_abs(pos, ymin, ymax)),
        2 => (lerp_abs(pos, xmin, xmax), ymin + inset),
        _ => (lerp_abs(pos, xmin, xmax), ymax.saturating_sub(inset)),
    };
    let events = [
        Event::Abs(AbsEvent::Axis {
            axis: AbsAxis::X,
            value: xv,
        }),
        Event::Abs(AbsEvent::Axis {
            axis: AbsAxis::Y,
            value: yv,
        }),
        Event::Sync(SyncEvent::All),
    ];
    route_events(
        devices,
        clients,
        router,
        device_id,
        dest,
        events.into_iter(),
    )
    .await
}

struct Device {
    name: CString,
    vendor: u16,
    product: u16,
    version: u16,
    rel: HashSet<RelAxis>,
    abs: HashMap<AbsAxis, AbsInfo>,
    keys: HashSet<Key>,
    delay: Option<i32>,
    period: Option<i32>,
    sender: Sender<Event>,
}

#[derive(Error, Debug)]
enum ClientError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Incompatible client version (got {client}, expected {server})")]
    Version { server: Version, client: Version },
    #[error("Invalid password")]
    Auth,
    #[error(transparent)]
    Rand(#[from] rand::Error),
}

async fn client(
    stream: TcpStream,
    acceptor: TlsAcceptor,
    password: &str,
    id: &str,
    addr: SocketAddr,
    latencies: PeerLatencies,
    reg_tx: Sender<(String, SocketAddr, Sender<Update>)>,
) -> Result<(), ClientError> {
    let stream = rkvm_net::timeout(rkvm_net::TLS_TIMEOUT, acceptor.accept(stream)).await?;
    tracing::info!("TLS connected");

    // Larger write buffer to batch several Update::Event before flush.
    let mut stream = BufStream::with_capacity(1024, 16 * 1024, stream);

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        Version::CURRENT.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await?;

    let version = rkvm_net::timeout(rkvm_net::READ_TIMEOUT, Version::decode(&mut stream)).await?;
    if version != Version::CURRENT {
        return Err(ClientError::Version {
            server: Version::CURRENT,
            client: version,
        });
    }

    let challenge = AuthChallenge::generate().await?;

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        challenge.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await?;

    let response =
        rkvm_net::timeout(rkvm_net::READ_TIMEOUT, AuthResponse::decode(&mut stream)).await?;
    let status = match response.verify(&challenge, password) {
        true => AuthStatus::Passed,
        false => AuthStatus::Failed,
    };

    rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
        status.encode(&mut stream).await?;
        stream.flush().await?;

        Ok(())
    })
    .await?;

    if status == AuthStatus::Failed {
        return Err(ClientError::Auth);
    }

    tracing::info!("Authenticated successfully");

    let (sender, mut receiver) = mpsc::channel(1024);
    if reg_tx.send((id.to_string(), addr, sender)).await.is_err() {
        return Err(ClientError::Io(io::Error::new(
            ErrorKind::BrokenPipe,
            "control closed before register",
        )));
    }
    let mut init_updates = VecDeque::new();

    let mut interval = time::interval(rkvm_net::PING_INTERVAL);
    let mut awaiting_pong = false;
    let mut ping_sent_at = Instant::now();
    let pong_limit = rkvm_net::PING_INTERVAL + rkvm_net::READ_TIMEOUT;

    loop {
        let mut batch = Vec::new();

        let first = async {
            match init_updates.pop_front() {
                Some(update) => Some(update),
                None => receiver.recv().await,
            }
        };

        let pong_timeout = async {
            let elapsed = ping_sent_at.elapsed();
            if elapsed < pong_limit {
                time::sleep(pong_limit - elapsed).await;
            }
        };

        tokio::select! {
            biased;

            result = Pong::decode(&mut stream), if awaiting_pong => {
                result?;
                let duration = ping_sent_at.elapsed();
                tracing::debug!(duration = ?duration, "Received pong");
                latencies
                    .lock()
                    .unwrap()
                    .insert(id.to_string(), duration.as_millis().min(u32::MAX as u128) as u32);
                awaiting_pong = false;
                continue;
            }
            _ = pong_timeout, if awaiting_pong => {
                return Err(ClientError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Pong timed out",
                )));
            }
            _ = interval.tick() => {
                batch.push(Update::Ping);
            }
            update = first => {
                match update {
                    Some(update) => batch.push(update),
                    None => break,
                }
            }
        }

        while let Some(update) = init_updates.pop_front() {
            batch.push(update);
        }
        while let Ok(update) = receiver.try_recv() {
            batch.push(update);
        }

        let sent_ping = batch.iter().any(|u| matches!(u, Update::Ping));
        let start = Instant::now();
        rkvm_net::timeout(rkvm_net::WRITE_TIMEOUT, async {
            for update in &batch {
                update.encode(&mut stream).await?;
            }
            stream.flush().await?;
            Ok(())
        })
        .await?;
        let duration = start.elapsed();

        if sent_ping {
            tracing::debug!(duration = ?duration, count = batch.len(), "Sent ping (batched)");
            awaiting_pong = true;
            ping_sent_at = Instant::now();
        } else {
            tracing::trace!(count = batch.len(), "Wrote updates");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_id_ignores_ephemeral_port() {
        let a: SocketAddr = "192.168.1.5:54321".parse().unwrap();
        let b: SocketAddr = "192.168.1.5:11111".parse().unwrap();
        assert_eq!(peer_id(a), peer_id(b));
        assert_eq!(peer_id(a), "192.168.1.5");
    }

    #[test]
    fn peer_id_ipv6_is_address_only() {
        let a: SocketAddr = "[2001:db8::1]:5258".parse().unwrap();
        assert_eq!(peer_id(a), "2001:db8::1");
    }
}
