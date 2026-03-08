use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::{PairConfig, SourceSide};
use crate::drive::find_mounted_drive;
use crate::sync_engine::{sync_pair, SyncOptions, SyncReport};

// ── Timing constants ───────────────────────────────────────────────────────────

/// How often the drive poll thread checks for the target drive.
const DRIVE_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Wait this long after the last file-change event before triggering a sync.
/// Prevents thrashing when an editor writes many small events on save.
const DEBOUNCE_DELAY: Duration = Duration::from_millis(500);

// ── Public types ──────────────────────────────────────────────────────────────

/// Events emitted by the watch loop — consumed by the caller (CLI / UI).
pub enum WatchEvent {
    /// Drive-aware pair: target drive was just detected at this mount point.
    DriveDetected { mount_point: PathBuf },
    /// Drive-aware pair: target drive was unplugged / no longer found.
    DriveRemoved,
    /// A sync is about to start.
    SyncStarted,
    /// A sync completed successfully.
    SyncCompleted(SyncReport),
    /// A sync failed with this error string.
    SyncError(String),
    /// Emitted once at startup when the watcher is ready.
    Watching,
}

/// Handle returned to the caller. Keeps the watch loop alive.
/// Drop it or call `stop()` to shut down the watcher.
pub struct WatchHandle {
    stop_tx: mpsc::Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
}

impl WatchHandle {
    /// Block the current thread until the watch loop exits.
    pub fn wait(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Signal the watch loop to stop (non-blocking).
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }
}

// ── Internal channel messages ─────────────────────────────────────────────────

enum Msg {
    FileChanged,
    DriveCheck,
    Stop,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Start watching a named pair. Returns a `WatchHandle` immediately.
/// The `on_event` callback is called from the watch thread for each event.
pub fn watch_pair(
    name: &str,
    on_event: impl Fn(WatchEvent) + Send + 'static,
) -> Result<WatchHandle, String> {
    let pair = crate::config::get_pair(name)?;
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let name = name.to_string();

    let thread = thread::spawn(move || {
        run_watch_loop(name, pair, on_event, stop_rx);
    });

    Ok(WatchHandle {
        stop_tx,
        thread: Some(thread),
    })
}

// ── Watch loop ────────────────────────────────────────────────────────────────

fn run_watch_loop(
    name: String,
    pair: PairConfig,
    on_event: impl Fn(WatchEvent),
    stop_rx: mpsc::Receiver<()>,
) {
    let (msg_tx, msg_rx) = mpsc::channel::<Msg>();

    // Resolve the source path (always local — always present)
    let source_path = match pair.source {
        SourceSide::Base => pair.base.clone(),
        SourceSide::Target => pair.target.clone(),
    };

    // Set up the file watcher on source
    let file_tx = msg_tx.clone();
    let mut watcher: RecommendedWatcher =
        match notify::recommended_watcher(move |res: notify::Result<Event>| {
            if res.is_ok() {
                let _ = file_tx.send(Msg::FileChanged);
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                on_event(WatchEvent::SyncError(format!("Failed to start watcher: {}", e)));
                return;
            }
        };

    if let Err(e) = watcher.watch(&source_path, RecursiveMode::Recursive) {
        on_event(WatchEvent::SyncError(format!("Failed to watch path: {}", e)));
        return;
    }

    // For cross-drive pairs, spawn a drive poll thread
    let is_cross_drive = pair.drive_id.is_some();
    if is_cross_drive {
        let poll_tx = msg_tx.clone();
        thread::spawn(move || loop {
            thread::sleep(DRIVE_POLL_INTERVAL);
            if poll_tx.send(Msg::DriveCheck).is_err() {
                break;
            }
        });
    }

    // Forward stop signal into the message channel
    {
        let stop_tx = msg_tx.clone();
        thread::spawn(move || {
            if stop_rx.recv().is_ok() {
                let _ = stop_tx.send(Msg::Stop);
            }
        });
    }

    on_event(WatchEvent::Watching);

    // State
    let mut drive_mounted = if is_cross_drive {
        // Check once immediately on startup
        pair.drive_id
            .as_ref()
            .and_then(|id| find_mounted_drive(id))
            .is_some()
    } else {
        true // same-drive pair — target always accessible
    };

    let mut last_change: Option<Instant> = None;
    let mut pending_sync = false;

    // If drive is already mounted at startup, sync immediately
    if drive_mounted && is_cross_drive {
        if let Some(mount) = pair.drive_id.as_ref().and_then(|id| find_mounted_drive(id)) {
            on_event(WatchEvent::DriveDetected { mount_point: mount });
            do_sync(&name, &on_event);
        }
    }

    loop {
        // Timeout so we can check debounce even if no messages arrive
        let timeout = if pending_sync {
            DEBOUNCE_DELAY
        } else {
            Duration::from_secs(60)
        };

        let msg = msg_rx.recv_timeout(timeout);

        match msg {
            Ok(Msg::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,

            Ok(Msg::FileChanged) => {
                if drive_mounted {
                    last_change = Some(Instant::now());
                    pending_sync = true;
                }
            }

            Ok(Msg::DriveCheck) => {
                let now_mounted = pair
                    .drive_id
                    .as_ref()
                    .and_then(|id| find_mounted_drive(id))
                    .is_some();

                match (drive_mounted, now_mounted) {
                    (false, true) => {
                        // Drive just appeared
                        drive_mounted = true;
                        if let Some(mount) =
                            pair.drive_id.as_ref().and_then(|id| find_mounted_drive(id))
                        {
                            on_event(WatchEvent::DriveDetected { mount_point: mount });
                        }
                        do_sync(&name, &on_event);
                    }
                    (true, false) => {
                        // Drive just disappeared
                        drive_mounted = false;
                        pending_sync = false;
                        on_event(WatchEvent::DriveRemoved);
                    }
                    _ => {}
                }
            }

            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Debounce: fire sync if enough time has passed since last change
                if pending_sync {
                    if let Some(last) = last_change {
                        if last.elapsed() >= DEBOUNCE_DELAY {
                            pending_sync = false;
                            last_change = None;
                            do_sync(&name, &on_event);
                        }
                    }
                }
            }
        }
    }
}

// ── Sync helper ───────────────────────────────────────────────────────────────

fn do_sync(name: &str, on_event: &impl Fn(WatchEvent)) {
    on_event(WatchEvent::SyncStarted);
    match sync_pair(name, SyncOptions { dry_run: false, verify: false }) {
        Ok(report) => on_event(WatchEvent::SyncCompleted(report)),
        Err(e) => on_event(WatchEvent::SyncError(e)),
    }
}
