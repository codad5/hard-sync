//! Who is watching which pair.
//!
//! A pair must only be watched by one watcher at a time — two watchers would
//! sync the same files concurrently. Watchers can live in different processes
//! (`hsync watch` in a terminal, a detached `hsync watch --detach`, the desktop
//! app), so ownership is recorded in a PID file per pair:
//!
//!   <data dir>/hsync/pids/<pair>.pid   → PID of the process watching it
//!   <data dir>/hsync/logs/<pair>.log   → that watcher's activity log
//!
//! Within one process (the desktop app watches several pairs) an in-memory set
//! additionally prevents watching the same pair twice.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// ── Paths ─────────────────────────────────────────────────────────────────────

/// Root folder for watcher state: `~/.local/share/hsync` on Linux,
/// `~/Library/Application Support/hsync` on macOS, `%APPDATA%\hsync` on Windows.
pub fn watcher_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|d| d.join("hsync"))
        .ok_or_else(|| "Cannot determine data directory".to_string())
}

fn subdir(name: &str) -> Result<PathBuf, String> {
    let dir = watcher_data_dir()?.join(name);
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {} dir: {}", name, e))?;
    Ok(dir)
}

/// Folder holding one `<pair>.pid` file per running watcher.
pub fn watcher_pids_dir() -> Result<PathBuf, String> {
    subdir("pids")
}

/// PID file for a pair's watcher.
pub fn watcher_pid_file(name: &str) -> Result<PathBuf, String> {
    Ok(watcher_pids_dir()?.join(format!("{}.pid", name)))
}

/// Activity log for a pair's watcher.
pub fn watcher_log_file(name: &str) -> Result<PathBuf, String> {
    Ok(subdir("logs")?.join(format!("{}.log", name)))
}

// ── Queries ───────────────────────────────────────────────────────────────────

/// PID of the live process watching `name`, if any. Stale PID files (process
/// gone) are removed.
pub fn watcher_pid(name: &str) -> Option<u32> {
    let path = watcher_pid_file(name).ok()?;
    let pid: u32 = fs::read_to_string(&path).ok()?.trim().parse().ok()?;
    if is_process_alive(pid) {
        Some(pid)
    } else {
        let _ = fs::remove_file(&path);
        None
    }
}

/// Is there a process with this PID?
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Signal 0 checks existence without sending anything. EPERM means it
        // exists but belongs to another user — still alive.
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        use std::process::{Command, Stdio};
        // Exact CSV match: `"name","<pid>",...` — avoids 7 matching 7532.
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

// ── Lock ──────────────────────────────────────────────────────────────────────

fn in_process() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Exclusive right to watch one pair. Released on drop.
#[derive(Debug)]
pub struct WatchLock {
    name: String,
}

impl WatchLock {
    /// Claim `name` for this process, or explain who already has it.
    pub fn acquire(name: &str) -> Result<Self, String> {
        let me = std::process::id();
        let mut set = in_process().lock().unwrap_or_else(|e| e.into_inner());

        if set.contains(name) {
            return Err(format!("\"{}\" is already being watched", name));
        }
        if let Some(pid) = watcher_pid(name) {
            // A detached `hsync watch` finds its own PID here, written by the
            // parent that spawned it — that's not a conflict.
            if pid != me {
                return Err(format!(
                    "\"{}\" is already being watched by another hsync process (PID {}). Stop it first.",
                    name, pid
                ));
            }
        }

        fs::write(watcher_pid_file(name)?, me.to_string())
            .map_err(|e| format!("Cannot write watcher PID file: {}", e))?;
        set.insert(name.to_string());
        Ok(WatchLock { name: name.to_string() })
    }
}

impl Drop for WatchLock {
    fn drop(&mut self) {
        let mut set = in_process().lock().unwrap_or_else(|e| e.into_inner());
        set.remove(&self.name);
        // Only remove the PID file if it is still ours.
        if let Ok(path) = watcher_pid_file(&self.name) {
            let ours = fs::read_to_string(&path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                == Some(std::process::id());
            if ours {
                let _ = fs::remove_file(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_alive() {
        assert!(is_process_alive(std::process::id()));
    }

    #[test]
    fn same_pair_cannot_be_watched_twice_in_one_process() {
        let name = format!("lock-test-{}", std::process::id());
        let first = WatchLock::acquire(&name).expect("first acquire");
        assert!(WatchLock::acquire(&name).is_err());
        assert_eq!(watcher_pid(&name), Some(std::process::id()));
        drop(first);
        assert_eq!(watcher_pid(&name), None, "PID file removed on drop");
        let again = WatchLock::acquire(&name).expect("free again after drop");
        drop(again);
    }
}
