use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use hard_sync_core::{
    add_pair, get_drive_id, get_pair, list_connected_drives, list_pairs, play_event_sound,
    remove_pair, same_drive, set_source, sync_pair, watch_pair, watcher_log_file, watcher_pid,
    ConnectedDrive, DeleteBehavior, PairConfig, SoundConfig, SoundEvent, SourceSide, SyncOptions,
    SyncReport, WatchEvent, WatchHandle,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// Pair commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn cmd_list_pairs() -> Result<Vec<PairConfig>, String> {
    list_pairs()
}

#[tauri::command]
pub fn cmd_add_pair(
    name: String,
    base: String,
    target: String,
    source: String,
) -> Result<PairConfig, String> {
    let base_path = std::path::Path::new(&base);
    let target_path = std::path::Path::new(&target);

    let source_side = match source.as_str() {
        "target" => SourceSide::Target,
        _ => SourceSide::Base,
    };

    // Detect whether the two paths are on the same drive
    let drive_id = if same_drive(base_path, target_path) {
        None
    } else {
        get_drive_id(target_path)
    };

    let pair = PairConfig {
        name: name.clone(),
        base: base_path.to_path_buf(),
        target: target_path.to_path_buf(),
        source: source_side,
        drive_id,
        ignore: vec![
            "node_modules".into(),
            ".git".into(),
            "target".into(),
            "dist".into(),
        ],
        delete_behavior: DeleteBehavior::Trash,
        sounds: SoundConfig { sync_start: None, sync_done: None, sync_error: None },
        created_at: chrono::Utc::now(),
    };

    add_pair(pair.clone())?;
    Ok(pair)
}

#[tauri::command]
pub fn cmd_remove_pair(name: String) -> Result<(), String> {
    remove_pair(&name)
}

#[tauri::command]
pub fn cmd_set_source(name: String, source: String) -> Result<(), String> {
    let side = match source.as_str() {
        "base" => SourceSide::Base,
        "target" => SourceSide::Target,
        other => return Err(format!("Invalid source side: \"{}\". Use \"base\" or \"target\"", other)),
    };
    set_source(&name, side)
}

// ---------------------------------------------------------------------------
// Sync command
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn cmd_trigger_sync(name: String, dry_run: bool) -> Result<SyncReport, String> {
    let sounds = get_pair(&name).map(|p| p.sounds).ok();

    if let Some(ref s) = sounds {
        if !dry_run { play_event_sound(s, SoundEvent::SyncStart); }
    }

    let result = sync_pair(&name, SyncOptions { dry_run, verify: false });

    if let Some(ref s) = sounds {
        if !dry_run {
            match &result {
                // "done" means "safe to unplug" — not if any file failed
                Ok(r) if r.errors.is_empty() => play_event_sound(s, SoundEvent::SyncDone),
                Ok(_)  => play_event_sound(s, SoundEvent::SyncError),
                Err(_) => play_event_sound(s, SoundEvent::SyncError),
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Drive commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn cmd_list_drives() -> Vec<ConnectedDrive> {
    list_connected_drives()
}

// ---------------------------------------------------------------------------
// Watcher commands
//
// Watchers run inside the app process (core::watch_pair), so the app needs no
// separate hsync binary and gets every event live. Watchers started from a
// terminal (`hsync watch`) are still shown and can be stopped; core's
// WatchLock guarantees a pair is never watched twice.
// ---------------------------------------------------------------------------

/// Watchers running in this app, by pair name.
#[derive(Default)]
pub struct AppWatchers(Mutex<HashMap<String, WatchHandle>>);

#[derive(Debug, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
    /// "app" = running in this window's process, "external" = an `hsync watch`
    /// process (terminal / --detach), None = not running.
    pub owner: Option<String>,
}

/// Payload of the "watch-event" event sent to the window.
#[derive(Debug, Clone, Serialize)]
pub struct WatchUpdate {
    pub name: String,
    /// watching | drive_detected | drive_removed | sync_started | sync_completed | sync_error
    pub kind: &'static str,
    pub message: String,
    pub report: Option<SyncReport>,
}

#[tauri::command]
pub fn cmd_watcher_status(name: String, watchers: State<'_, AppWatchers>) -> WatcherStatus {
    if watchers.0.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&name) {
        return WatcherStatus { name, running: true, pid: Some(std::process::id()), owner: Some("app".into()) };
    }
    match watcher_pid(&name) {
        Some(pid) => WatcherStatus { name, running: true, pid: Some(pid), owner: Some("external".into()) },
        None => WatcherStatus { name, running: false, pid: None, owner: None },
    }
}

#[tauri::command]
pub fn cmd_start_watcher(
    name: String,
    app: AppHandle,
    watchers: State<'_, AppWatchers>,
) -> Result<(), String> {
    let mut map = watchers.0.lock().unwrap_or_else(|e| e.into_inner());
    if map.contains_key(&name) {
        return Err(format!("\"{}\" is already being watched", name));
    }

    let sounds = get_pair(&name).map(|p| p.sounds).ok();
    let pair_name = name.clone();
    let handle = watch_pair(&name, move |event| {
        let update = to_update(&pair_name, event);
        if let Some(s) = &sounds {
            match update.kind {
                "sync_started" => play_event_sound(s, SoundEvent::SyncStart),
                // "done" means "safe to unplug" — not if any file failed
                "sync_completed" if update.report.as_ref().is_some_and(|r| r.errors.is_empty()) => {
                    play_event_sound(s, SoundEvent::SyncDone)
                }
                "sync_completed" | "sync_error" => play_event_sound(s, SoundEvent::SyncError),
                _ => {}
            }
        }
        append_log(&pair_name, &update.message);
        let _ = app.emit("watch-event", update);
    })?;

    map.insert(name, handle);
    Ok(())
}

#[tauri::command]
pub fn cmd_stop_watcher(name: String, watchers: State<'_, AppWatchers>) -> Result<(), String> {
    if let Some(handle) = watchers.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&name) {
        handle.stop();
        append_log(&name, "Stopped from the app.");
        return Ok(());
    }

    // Not ours: a watcher started from a terminal.
    let pid = watcher_pid(&name)
        .ok_or_else(|| format!("No running watcher found for \"{}\"", name))?;
    kill_process(pid)
}

/// Last `lines` lines of a pair's watcher log (app and terminal watchers both write it).
#[tauri::command]
pub fn cmd_watcher_log(name: String, lines: usize) -> Result<Vec<String>, String> {
    let path = watcher_log_file(&name)?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("Cannot read log: {}", e)),
    };
    let all: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = all.len().saturating_sub(lines);
    Ok(all[from..].iter().map(|l| l.to_string()).collect())
}

/// Tray "Stop all watchers": stop the app's own watchers and any `hsync watch`
/// processes started from a terminal (like `hsync watch stop --all`).
/// The app keeps running; the window is told to refresh.
pub fn stop_every_watcher(app: &AppHandle) {
    use tauri::Manager;
    stop_all(&app.state::<AppWatchers>());

    let me = std::process::id();
    if let Ok(entries) = hard_sync_core::watcher_pids_dir().and_then(|d| std::fs::read_dir(d).map_err(|e| e.to_string())) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("pid") {
                continue;
            }
            let Some(name) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
            if let Some(pid) = watcher_pid(&name) {
                if pid != me && kill_process(pid).is_ok() {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    let _ = app.emit("watchers-changed", ());
}

/// Stop every watcher this app started (called on quit).
pub fn stop_all(watchers: &AppWatchers) {
    for (_, handle) in watchers.0.lock().unwrap_or_else(|e| e.into_inner()).drain() {
        handle.stop();
    }
}

fn to_update(name: &str, event: WatchEvent) -> WatchUpdate {
    let (kind, message, report) = match event {
        WatchEvent::Watching => ("watching", "Watching for changes.".to_string(), None),
        WatchEvent::DriveDetected { mount_point } => (
            "drive_detected",
            format!("Drive detected at {} — syncing.", mount_point.display()),
            None,
        ),
        WatchEvent::DriveRemoved => ("drive_removed", "Drive removed. Waiting for it.".to_string(), None),
        WatchEvent::SyncStarted => ("sync_started", "Syncing…".to_string(), None),
        WatchEvent::SyncCompleted(r) => ("sync_completed", summarize(&r), Some(r)),
        WatchEvent::SyncError(e) => ("sync_error", format!("Sync failed: {}", e), None),
    };
    WatchUpdate { name: name.to_string(), kind, message, report }
}

fn summarize(r: &SyncReport) -> String {
    let mut msg = format!(
        "Done. {} copied, {} updated, {} trashed, {} skipped, {} errors",
        r.copied, r.updated, r.trashed, r.skipped, r.errors.len()
    );
    for e in &r.errors {
        msg.push_str(&format!("\n  ! {} — {}", e.path, e.message));
    }
    msg
}

fn append_log(name: &str, message: &str) {
    let Ok(path) = watcher_log_file(name) else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), message);
    }
}

fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    let status = Command::new("kill").args(["-TERM", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    #[cfg(windows)]
    let status = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(format!("Could not stop the watcher process (PID {})", pid)),
        Err(e) => Err(format!("Failed to stop watcher: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Platform
// ---------------------------------------------------------------------------

/// The OS the app runs on: "macos", "windows", "linux", ...
/// Lets the UI show paths in the style people expect on their OS.
#[tauri::command]
pub fn cmd_platform() -> &'static str {
    std::env::consts::OS
}
