use anyhow::{anyhow, Context, Result};
use ashpd::desktop::input_capture::{
    ActivatedBarrier, Barrier as PortalBarrier, BarrierID, BarrierPosition, Capabilities,
    CreateSessionOptions, InputCapture, Region, ReleaseOptions, StartOptions,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use nexus_common::{Barrier, Edge};
use reis::{
    ei::{self, keyboard::KeyState},
    event::{DeviceCapability, EiEvent, KeyboardKey},
};
use std::{
    collections::HashMap,
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct EdgeEvent {
    pub display_id: String,
    pub edge: Edge,
    pub normalized_position: f32,
    pub any_button_pressed: bool,
}

#[async_trait]
pub trait EdgeCaptureBackend: Send {
    async fn register(&mut self, barriers: Vec<Barrier>) -> Result<()>;
    async fn next(&mut self) -> Result<EdgeEvent>;
    async fn suspend(&mut self) -> Result<()>;
    async fn resume(&mut self) -> Result<()>;
    fn available(&self) -> bool;
}

#[derive(Clone)]
struct BarrierMeta {
    display_id: String,
    edge: Edge,
}

#[derive(Clone)]
struct RegionInfo {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

struct LiveSession {
    input_capture: InputCapture,
    session: ashpd::desktop::Session<InputCapture>,
}

/// Production backend: InputCapture portal v2 + EIS (reis).
pub struct PortalBackend {
    available: bool,
    portal_error: Option<String>,
    events: Option<mpsc::Receiver<EdgeEvent>>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    live: Option<Arc<Mutex<LiveSession>>>,
    suspended: Arc<AtomicBool>,
}

impl PortalBackend {
    pub async fn connect() -> Result<Self> {
        match InputCapture::new().await {
            Ok(ic) => {
                let version = ic.version();
                let available = version >= 2;
                if !available {
                    warn!("InputCapture version {version} < 2");
                }
                Ok(Self {
                    available,
                    portal_error: if available {
                        None
                    } else {
                        Some(format!("InputCapture v{version} (v2 required)"))
                    },
                    events: None,
                    cancel: None,
                    live: None,
                    suspended: Arc::new(AtomicBool::new(false)),
                })
            }
            Err(e) => {
                warn!("portal InputCapture unavailable: {e}");
                Ok(Self {
                    available: false,
                    portal_error: Some(e.to_string()),
                    events: None,
                    cancel: None,
                    live: None,
                    suspended: Arc::new(AtomicBool::new(false)),
                })
            }
        }
    }

    pub fn portal_error(&self) -> Option<&str> {
        self.portal_error.as_deref()
    }

    async fn shutdown_session(&mut self) {
        if let Some(tx) = self.cancel.take() {
            let _ = tx.send(true);
        }
        self.events = None;
        if let Some(live) = self.live.take() {
            let g = live.lock().await;
            let _ = g
                .input_capture
                .disable(&g.session, Default::default())
                .await;
            let _ = g.session.close().await;
        }
    }

    async fn start_session(ic: &InputCapture) -> Result<ashpd::desktop::Session<InputCapture>> {
        let capabilities = Capabilities::Keyboard | Capabilities::Pointer;
        match ic.create_session2(Default::default()).await {
            Ok(session) => {
                let start = ic
                    .start(
                        &session,
                        None,
                        StartOptions::default().set_capabilities(capabilities),
                    )
                    .await
                    .context("InputCapture.Start")?;
                let _ = start.response().context("Start response")?;
                Ok(session)
            }
            Err(ashpd::Error::RequiresVersion(_, _)) => {
                let (session, _) = ic
                    .create_session(
                        None,
                        CreateSessionOptions::default().set_capabilities(capabilities),
                    )
                    .await
                    .context("InputCapture.CreateSession")?;
                Ok(session)
            }
            Err(e) => Err(e.into()),
        }
    }
}

fn portal_position(edge: Edge, r: &Region) -> BarrierPosition {
    let x = r.x_offset();
    let y = r.y_offset();
    let w = r.width() as i32;
    let h = r.height() as i32;
    match edge {
        Edge::Left => BarrierPosition::new(x, y, x, y + h - 1),
        Edge::Right => BarrierPosition::new(x + w, y, x + w, y + h - 1),
        Edge::Top => BarrierPosition::new(x, y, x + w - 1, y),
        Edge::Bottom => BarrierPosition::new(x, y + h, x + w - 1, y + h),
    }
}

fn normalized_at(edge: Edge, cursor: (f32, f32), regions: &[RegionInfo]) -> f32 {
    let (cx, cy) = cursor;
    let region = regions
        .iter()
        .find(|r| {
            cx >= r.x as f32
                && cy >= r.y as f32
                && cx <= (r.x as f32 + r.w as f32)
                && cy <= (r.y as f32 + r.h as f32)
        })
        .or_else(|| regions.first());
    let Some(r) = region else {
        return 0.5;
    };
    match edge {
        Edge::Left | Edge::Right => {
            if r.h == 0 {
                0.5
            } else {
                ((cy - r.y as f32) / r.h as f32).clamp(0.0, 1.0)
            }
        }
        Edge::Top | Edge::Bottom => {
            if r.w == 0 {
                0.5
            } else {
                ((cx - r.x as f32) / r.w as f32).clamp(0.0, 1.0)
            }
        }
    }
}

fn release_cursor(edge: Edge, cursor: (f32, f32)) -> (f64, f64) {
    let (x, y) = (cursor.0 as f64, cursor.1 as f64);
    match edge {
        Edge::Left => (x + 2.0, y),
        Edge::Right => (x - 2.0, y),
        Edge::Top => (x, y + 2.0),
        Edge::Bottom => (x, y - 2.0),
    }
}

fn spawn_eis_drain(fd: std::os::fd::OwnedFd, cancel: tokio::sync::watch::Receiver<bool>) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                warn!("eis runtime: {e}");
                return;
            }
        };
        rt.block_on(async move {
            let stream = UnixStream::from(fd);
            if stream.set_nonblocking(true).is_err() {
                return;
            }
            let Ok(context) = ei::Context::new(stream) else {
                return;
            };
            let _ = context.flush();
            let Ok((_conn, mut events)) = context
                .handshake_tokio("nexus-agent", ei::handshake::ContextType::Receiver)
                .await
            else {
                warn!("eis handshake failed");
                return;
            };
            let mut cancel = cancel;
            loop {
                tokio::select! {
                    _ = cancel.changed() => {
                        if *cancel.borrow() {
                            break;
                        }
                    }
                    ev = events.next() => {
                        match ev {
                            Some(Ok(EiEvent::SeatAdded(seat))) => {
                                seat.seat.bind_capabilities(&[
                                    DeviceCapability::Pointer,
                                    DeviceCapability::PointerAbsolute,
                                    DeviceCapability::Keyboard,
                                    DeviceCapability::Touch,
                                    DeviceCapability::Scroll,
                                    DeviceCapability::Button,
                                ]);
                                let _ = context.flush();
                            }
                            Some(Ok(EiEvent::KeyboardKey(KeyboardKey { key, state, .. }))) => {
                                if key == 1 && state == KeyState::Press {
                                    debug!("eis ESC");
                                }
                            }
                            Some(Ok(_)) => {}
                            Some(Err(_)) | None => break,
                        }
                    }
                }
            }
        });
    });
}

#[async_trait]
impl EdgeCaptureBackend for PortalBackend {
    async fn register(&mut self, barriers: Vec<Barrier>) -> Result<()> {
        if !self.available {
            anyhow::bail!(
                "InputCapture v2 unavailable: {}",
                self.portal_error.as_deref().unwrap_or("unknown")
            );
        }
        self.shutdown_session().await;

        let ic = InputCapture::new().await.context("InputCapture::new")?;
        let session = Self::start_session(&ic).await?;
        let zones = ic
            .zones(&session, Default::default())
            .await
            .context("GetZones")?
            .response()
            .context("zones response")?;

        let regions: Vec<RegionInfo> = zones
            .regions()
            .iter()
            .map(|r| RegionInfo {
                x: r.x_offset(),
                y: r.y_offset(),
                w: r.width(),
                h: r.height(),
            })
            .collect();

        let mut portal_barriers = Vec::new();
        let mut meta: HashMap<u32, BarrierMeta> = HashMap::new();
        for b in &barriers {
            for (zi, region) in zones.regions().iter().enumerate() {
                let id_num = b.id.saturating_mul(100).saturating_add(zi as u32 + 1);
                let Some(id) = BarrierID::new(id_num) else {
                    continue;
                };
                portal_barriers.push(PortalBarrier::new(id, portal_position(b.edge, region)));
                meta.insert(
                    id_num,
                    BarrierMeta {
                        display_id: b.display_id.clone(),
                        edge: b.edge,
                    },
                );
            }
        }
        if portal_barriers.is_empty() {
            anyhow::bail!("no barriers to register");
        }

        let resp = ic
            .set_pointer_barriers(
                &session,
                &portal_barriers,
                zones.zone_set(),
                Default::default(),
            )
            .await
            .context("SetPointerBarriers")?
            .response()
            .context("barriers response")?;
        if !resp.failed_barriers().is_empty() {
            warn!("barriers rejected: {:?}", resp.failed_barriers());
        }

        let fd = ic
            .connect_to_eis(&session, Default::default())
            .await
            .context("ConnectToEIS")?;
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        spawn_eis_drain(fd, cancel_rx.clone());

        ic.enable(&session, Default::default())
            .await
            .context("Enable")?;

        let live = Arc::new(Mutex::new(LiveSession {
            input_capture: ic,
            session,
        }));
        let (tx, rx) = mpsc::channel(8);
        let suspended = self.suspended.clone();
        let mut cancel_listen = cancel_rx;
        let live_h = live.clone();

        // Separate proxy only for signals (same bus).
        let signal_ic = InputCapture::new().await.context("InputCapture signals")?;

        tokio::spawn(async move {
            let mut activated = match signal_ic.receive_activated().await {
                Ok(s) => s,
                Err(e) => {
                    warn!("receive_activated: {e}");
                    return;
                }
            };
            loop {
                tokio::select! {
                    _ = cancel_listen.changed() => {
                        if *cancel_listen.borrow() {
                            break;
                        }
                    }
                    a = activated.next() => {
                        let Some(activated) = a else { break };
                        if suspended.load(Ordering::SeqCst) {
                            let g = live_h.lock().await;
                            let mut opts = ReleaseOptions::default()
                                .set_activation_id(activated.activation_id());
                            if let Some(pos) = activated.cursor_position() {
                                opts = opts.set_cursor_position((pos.0 as f64, pos.1 as f64));
                            }
                            let _ = g.input_capture.release(&g.session, opts).await;
                            continue;
                        }

                        let barrier_id = match activated.barrier_id() {
                            Some(ActivatedBarrier::Barrier(id)) => id.get(),
                            _ => {
                                let g = live_h.lock().await;
                                let _ = g
                                    .input_capture
                                    .release(
                                        &g.session,
                                        ReleaseOptions::default()
                                            .set_activation_id(activated.activation_id()),
                                    )
                                    .await;
                                continue;
                            }
                        };
                        let Some(m) = meta.get(&barrier_id) else {
                            let g = live_h.lock().await;
                            let _ = g
                                .input_capture
                                .release(
                                    &g.session,
                                    ReleaseOptions::default()
                                        .set_activation_id(activated.activation_id()),
                                )
                                .await;
                            continue;
                        };
                        let cursor = activated.cursor_position().unwrap_or((0.0, 0.0));
                        let event = EdgeEvent {
                            display_id: m.display_id.clone(),
                            edge: m.edge,
                            normalized_position: normalized_at(m.edge, cursor, &regions),
                            any_button_pressed: false,
                        };
                        {
                            let g = live_h.lock().await;
                            let opts = ReleaseOptions::default()
                                .set_activation_id(activated.activation_id())
                                .set_cursor_position(release_cursor(m.edge, cursor));
                            let _ = g.input_capture.release(&g.session, opts).await;
                        }
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        self.cancel = Some(cancel_tx);
        self.live = Some(live);
        self.events = Some(rx);
        self.portal_error = None;
        info!("portal barriers active ({})", portal_barriers.len());
        Ok(())
    }

    async fn next(&mut self) -> Result<EdgeEvent> {
        let rx = self
            .events
            .as_mut()
            .ok_or_else(|| anyhow!("portal not registered"))?;
        rx.recv()
            .await
            .ok_or_else(|| anyhow!("portal event channel closed"))
    }

    async fn suspend(&mut self) -> Result<()> {
        // Soft-suspend: ignore activations (GNOME may fail to re-enable after Disable).
        self.suspended.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        self.suspended.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn available(&self) -> bool {
        self.available && self.portal_error.is_none()
    }
}
