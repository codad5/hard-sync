use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use hard_sync_core::{
    add_pair, get_drive_id, list_connected_drives, list_pairs, remove_pair, same_drive, set_source,
    sync_pair, ConnectedDrive, DeleteBehavior, DriveId, PairConfig, SoundConfig, SourceSide,
    SyncOptions, SyncReport,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Helpers — shared PID file path logic (mirrors cli/src/daemon.rs)
// ---------------------------------------------------------------------------

fn pid_file(name: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("hsync").join("pids").join(format!("{}.pid", name)))
}

fn read_pid(name: &str) -> Option<u32> {
    let path = pid_file(name)?;
    fs::read_to_string(&path).ok()?.trim().parse().ok()
}

fn is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok();
        let needle = format!(",\"{}\"", pid);
        out.map(|o| String::from_utf8_lossy(&o.stdout).contains(needle.as_str()))
            .unwrap_or(false)
    }
}

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
    sync_pair(&name, SyncOptions { dry_run, verify: false })
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
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
}

#[tauri::command]
pub fn cmd_watcher_status(name: String) -> WatcherStatus {
    match read_pid(&name) {
        Some(pid) if is_alive(pid) => WatcherStatus { name, running: true, pid: Some(pid) },
        _ => WatcherStatus { name, running: false, pid: None },
    }
}

#[tauri::command]
pub fn cmd_start_watcher(name: String) -> Result<u32, String> {
    // Spawn hsync binary with --detach; it writes its own PID file
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate hsync binary: {}", e))?;

    // Walk up from the Tauri binary to find hsync in the same directory
    let dir = exe.parent().ok_or("Cannot find binary directory")?;
    let hsync = dir.join(if cfg!(windows) { "hsync.exe" } else { "hsync" });

    if !hsync.exists() {
        return Err(format!(
            "hsync binary not found at {}. Make sure it is installed.",
            hsync.display()
        ));
    }

    let mut cmd = Command::new(&hsync);
    cmd.args(["watch", "--name", &name, "--detach"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008); // DETACHED_PROCESS
    }

    let child = cmd.spawn().map_err(|e| format!("Failed to spawn watcher: {}", e))?;
    Ok(child.id())
}

#[tauri::command]
pub fn cmd_stop_watcher(name: String) -> Result<(), String> {
    let pid = read_pid(&name)
        .ok_or_else(|| format!("No running watcher found for \"{}\"", name))?;

    if !is_alive(pid) {
        let _ = pid_file(&name).map(|p| fs::remove_file(p));
        return Err(format!("Watcher \"{}\" is not running (stale PID {})", name, pid));
    }

    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("Failed to kill process: {}", e))?;
    }
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("Failed to kill process: {}", e))?;
    }

    let _ = pid_file(&name).map(|p| fs::remove_file(p));
    Ok(())
}
