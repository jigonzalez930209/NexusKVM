use crate::interceptor::{Interceptor, OpenError};
use crate::registry::Registry;

use futures::StreamExt;
use inotify::{Inotify, WatchMask};
use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time;

const EVENT_PATH: &str = "/dev/input";

pub struct Monitor {
    receiver: Receiver<Result<Interceptor, Error>>,
}

impl Monitor {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(1);
        tokio::spawn(monitor(sender));

        Self { receiver }
    }

    pub async fn read(&mut self) -> Result<Interceptor, Error> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::new(ErrorKind::BrokenPipe, "Monitor task exited"))?
    }
}

fn is_event_node(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with("event"))
}

fn is_access_error(err: &Error) -> bool {
    err.kind() == ErrorKind::PermissionDenied
        || err.kind() == ErrorKind::NotFound
        || err.raw_os_error() == Some(libc::EACCES)
        || err.raw_os_error() == Some(libc::EPERM)
}

async fn try_open(path: &Path, registry: &Registry) -> Result<Option<Interceptor>, Error> {
    let mut last_access = None;
    for attempt in 0..8u32 {
        match Interceptor::open(path, registry).await {
            Ok(interceptor) => return Ok(Some(interceptor)),
            Err(OpenError::NotAppliable) => return Ok(None),
            Err(OpenError::Io(err)) if is_access_error(&err) => {
                last_access = Some(err);
                time::sleep(Duration::from_millis(40 + u64::from(attempt) * 20)).await;
            }
            Err(OpenError::Io(err)) => return Err(err),
        }
    }
    if let Some(err) = last_access {
        tracing::debug!(
            path = %path.display(),
            %err,
            "no permission after retries (udev); will retry on next scan"
        );
    }
    Ok(None)
}

async fn scan_dir(
    registry: &Registry,
    sender: &Sender<Result<Interceptor, Error>>,
) -> Result<(), Error> {
    let mut read_dir = fs::read_dir(EVENT_PATH).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();
        if !is_event_node(&path) {
            continue;
        }
        match try_open(&path, registry).await {
            Ok(Some(interceptor)) => {
                if sender.send(Ok(interceptor)).await.is_err() {
                    return Ok(());
                }
            }
            Ok(None) => {}
            // One unreadable/hot-unplugged node must never kill the monitor:
            // losing the whole input server over an ENODEV race is far worse
            // than skipping the device until the next scan.
            Err(err) => {
                tracing::warn!(path = %path.display(), %err, "skipping input device");
            }
        }
    }
    Ok(())
}

async fn monitor(sender: Sender<Result<Interceptor, Error>>) {
    let registry = Registry::new();
    tracing::info!("/dev/input monitor: rescan + skip virtual (does not abort on EACCES)");

    if let Err(err) = scan_dir(&registry, &sender).await {
        tracing::warn!(%err, "initial input scan failed; retrying periodically");
    }

    let mut rescan = time::interval(Duration::from_secs(2));
    rescan.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    let mut stream = match init_inotify() {
        Ok(stream) => stream,
        Err(err) => {
            // No inotify: keep working with the periodic rescan only.
            tracing::warn!(%err, "inotify unavailable; falling back to polling");
            loop {
                rescan.tick().await;
                if let Err(err) = scan_dir(&registry, &sender).await {
                    tracing::warn!(%err, "input rescan failed");
                }
            }
        }
    };

    loop {
        if sender.is_closed() {
            return;
        }
        tokio::select! {
            _ = rescan.tick() => {
                if let Err(err) = scan_dir(&registry, &sender).await {
                    tracing::warn!(%err, "input rescan failed");
                }
            }
            event = stream.next() => {
                let Some(event) = event else { break };
                let event = match event {
                    Ok(event) => event,
                    Err(err) => {
                        tracing::warn!(%err, "inotify event error; continuing");
                        continue;
                    }
                };
                let Some(name) = event.name else { continue };
                let path = PathBuf::from(EVENT_PATH).join(name);
                if !is_event_node(&path) {
                    continue;
                }
                match try_open(&path, &registry).await {
                    Ok(Some(interceptor)) => {
                        if sender.send(Ok(interceptor)).await.is_err() {
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        tracing::warn!(path = %path.display(), %err, "skipping input device");
                    }
                }
            }
        }
    }
}

fn init_inotify() -> Result<inotify::EventStream<[u8; 1024]>, Error> {
    let inotify = Inotify::init()?;
    inotify
        .watches()
        .add(EVENT_PATH, WatchMask::CREATE | WatchMask::ATTRIB)?;
    inotify.into_event_stream([0; 1024])
}
