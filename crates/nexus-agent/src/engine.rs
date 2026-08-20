use crate::{backend::EdgeCaptureBackend, daemon_client::DaemonClient};
use anyhow::Result;
use nexus_common::*;
use std::time::{Duration, Instant};

#[derive(Debug)]
enum Gate {
    Armed,
    Switching,
    Cooldown(Instant),
}

pub struct EdgeEngine<B: EdgeCaptureBackend> {
    backend: B,
    daemon: DaemonClient,
    layout: Layout,
    local_peer: String,
    gate: Gate,
    allow_drag: bool,
}

impl<B: EdgeCaptureBackend> EdgeEngine<B> {
    pub fn new(backend: B, daemon: DaemonClient, layout: Layout) -> Self {
        let local_peer = layout.local_peer.clone();
        Self {
            backend,
            daemon,
            layout,
            local_peer,
            gate: Gate::Armed,
            allow_drag: false,
        }
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.local_peer = layout.local_peer.clone();
        self.layout = layout;
    }

    pub async fn configure(&mut self) -> Result<()> {
        self.layout.validate().map_err(anyhow::Error::msg)?;
        let local_barriers: Vec<_> = self
            .layout
            .barriers
            .iter()
            .filter(|b| b.from_peer == self.local_peer)
            .cloned()
            .collect();
        self.backend.register(local_barriers).await
    }

    pub async fn step(&mut self) -> Result<()> {
        if let Gate::Cooldown(t) = self.gate {
            if Instant::now() >= t {
                self.gate = Gate::Armed;
            } else {
                return Ok(());
            }
        }
        let e = self.backend.next().await?;
        if !matches!(self.gate, Gate::Armed) || (!self.allow_drag && e.any_button_pressed) {
            return Ok(());
        }
        let Some(b) = barrier_for(
            &self.layout,
            &self.local_peer,
            &e.display_id,
            e.edge,
            e.normalized_position,
        ) else {
            return Ok(());
        };
        self.gate = Gate::Switching;
        let entry = entry_for(e.edge, e.normalized_position);
        let r = if b.destination == LOCAL_TARGET {
            self.daemon.send(ControlCommand::Local).await?
        } else {
            self.daemon
                .send(ControlCommand::Switch {
                    target: b.destination.clone(),
                    entry: Some(entry),
                })
                .await?
        };
        if r.ok {
            self.gate =
                Gate::Cooldown(Instant::now() + Duration::from_millis(b.cooldown_ms as u64));
        } else {
            self.gate = Gate::Armed;
        }
        Ok(())
    }
}
