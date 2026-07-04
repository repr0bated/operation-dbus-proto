//! Procfs event feed integration using inotify and procfs crate

use anyhow::Result;
use inotify::{Inotify, WatchMask};
use procfs::{Current, CurrentSI, LoadAverage};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;
use tracing::info;

use crate::event::MirrorEvent;

/// Spawn procfs inotify watchers for meminfo and stat
pub async fn spawn_procfs_inotify_watchers(
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning procfs inotify watchers");

    let mut inotify = Inotify::init()?;

    let wd_meminfo = inotify.watches().add("/proc/meminfo", WatchMask::ACCESS)?;
    let wd_stat = inotify.watches().add("/proc/stat", WatchMask::ACCESS)?;

    tokio::spawn(async move {
        let mut buffer = [0; 4096];
        loop {
            let events = inotify.read_events(&mut buffer).ok();
            if let Some(events) = events {
                for event in events {
                    let path = if event.wd == wd_meminfo {
                        Some("/proc/meminfo")
                    } else if event.wd == wd_stat {
                        Some("/proc/stat")
                    } else {
                        None
                    };

                    match path {
                        Some("/proc/meminfo") => {
                            if let Ok(meminfo) = procfs::Meminfo::current() {
                                let event = MirrorEvent::ProcMem {
                                    delta: serde_json::to_value(meminfo).unwrap_or_default(),
                                    sequence: 0,
                                };
                                let _ = broadcast_tx.send(event);
                            }
                        }
                        Some("/proc/stat") => {
                            if let Ok(stat) = procfs::KernelStats::current() {
                                let event = MirrorEvent::ProcStatic {
                                    section: "stat".to_string(),
                                    data: serde_json::to_value(stat).unwrap_or_default(),
                                    sequence: 0,
                                };
                                let _ = broadcast_tx.send(event);
                            }
                        }
                        _ => {}
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    Ok(())
}

/// Spawn procfs timer for /proc/loadavg
pub async fn spawn_procfs_loadavg_timer(
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning procfs loadavg timer");

    let mut interval = time::interval(Duration::from_secs(5));

    tokio::spawn(async move {
        loop {
            interval.tick().await;
            if let Ok(loadavg) = LoadAverage::current() {
                let event = MirrorEvent::ProcLoad {
                    delta: serde_json::to_value(loadavg).unwrap_or_default(),
                    sequence: 0,
                };
                let _ = broadcast_tx.send(event);
            }
        }
    });

    Ok(())
}
