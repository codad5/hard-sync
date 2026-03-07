use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use colored::Colorize;

use crate::output::{print_err, require_name};

// ---------------------------------------------------------------------------
// Data directory helpers
// ---------------------------------------------------------------------------

fn data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|d| d.join("hsync"))
        .ok_or_else(|| "Cannot determine data directory".to_string())
}

fn pids_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join("pids");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create pids dir: {}", e))?;
    Ok(dir)
}

fn logs_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join("logs");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create logs dir: {}", e))?;
    Ok(dir)
}

pub fn pid_file(name: &str) -> Result<PathBuf, String> {
    Ok(pids_dir()?.join(format!("{}.pid", name)))
}

pub fn log_file(name: &str) -> Result<PathBuf, String> {
    Ok(logs_dir()?.join(format!("{}.log", name)))
}

// ---------------------------------------------------------------------------
// Process helpers
// ---------------------------------------------------------------------------

fn read_pid(name: &str) -> Option<u32> {
    let path = pid_file(name).ok()?;
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
        out.map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Spawn a background watcher process
// ---------------------------------------------------------------------------

pub fn spawn_detached(name: &str) -> Result<u32, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot get current exe path: {}", e))?;

    let log_path = log_file(name)?;
    let log_f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("Cannot open log file: {}", e))?;
    let log_f2 = log_f.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = Command::new(&exe);
    cmd.args(["watch", "--name", name])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_f))
        .stderr(Stdio::from(log_f2));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS — no console window
        cmd.creation_flags(0x00000008);
    }

    let child = cmd.spawn().map_err(|e| format!("Failed to spawn watcher: {}", e))?;
    let pid = child.id();

    fs::write(pid_file(name)?, pid.to_string())
        .map_err(|e| format!("Failed to write PID file: {}", e))?;

    Ok(pid)
}

// ---------------------------------------------------------------------------
// Callbacks: watch list / watch attach / watch stop
// ---------------------------------------------------------------------------

pub fn watch_list_callback(_data: &fli::command::FliCallbackData) {
    let pids_d = match pids_dir() {
        Ok(d) => d,
        Err(e) => { print_err(&e); return; }
    };

    let entries = match fs::read_dir(&pids_d) {
        Ok(e) => e,
        Err(_) => {
            println!("{}", "No background watchers running.".dimmed());
            return;
        }
    };

    let mut found = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pid") {
            continue;
        }
        let name = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let pid: u32 = match fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            Some(p) => p,
            None => continue,
        };

        if is_alive(pid) {
            println!("  {} {} (PID {})",
                "●".green(),
                name.cyan().bold(),
                pid.to_string().dimmed()
            );
            found = true;
        } else {
            // Clean up stale pid file silently
            let _ = fs::remove_file(&path);
        }
    }

    if !found {
        println!("{}", "No background watchers running.".dimmed());
    }
}

pub fn watch_attach_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let log_path = match log_file(&name) {
        Ok(p) => p,
        Err(e) => { print_err(&e); return; }
    };

    if !log_path.exists() {
        print_err(&format!("No log file found for \"{}\". Is it running? Try: hsync watch list", name));
        return;
    }

    // Check the watcher is actually alive
    if let Some(pid) = read_pid(&name) {
        if !is_alive(pid) {
            println!("{}", format!("Warning: watcher \"{}\" (PID {}) does not appear to be running.", name, pid).yellow());
        }
    }

    let mut file = match fs::File::open(&log_path) {
        Ok(f) => f,
        Err(e) => { print_err(&format!("Cannot open log: {}", e)); return; }
    };

    // Show last 4 KB of existing output, then tail
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len > 4096 {
        let _ = file.seek(SeekFrom::End(-4096));
    }

    println!("{}", format!("Attached to \"{}\". Press Ctrl+C to detach (watcher keeps running).\n", name).dimmed());

    let mut buf = Vec::new();
    loop {
        buf.clear();
        let _ = file.read_to_end(&mut buf);
        if !buf.is_empty() {
            print!("{}", String::from_utf8_lossy(&buf));
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

pub fn watch_stop_callback(data: &fli::command::FliCallbackData) {
    use fli::option_parser::{Value, ValueTypes};

    let stop_all = matches!(
        data.get_option_value("all"),
        Some(ValueTypes::OptionalSingle(Some(Value::Bool(true))))
    );

    if stop_all {
        stop_all_watchers();
        return;
    }

    let name = match require_name(data) { Some(n) => n, None => return };
    stop_one_watcher(&name);
}

fn stop_one_watcher(name: &str) {
    let pid = match read_pid(name) {
        Some(p) => p,
        None => {
            print_err(&format!("No running watcher found for \"{}\"", name));
            return;
        }
    };

    if !is_alive(pid) {
        println!("{}", format!("Watcher \"{}\" is not running (stale PID {}). Cleaning up.", name, pid).yellow());
        let _ = pid_file(name).ok().map(|p| fs::remove_file(p));
        return;
    }

    if kill_pid(pid) {
        let _ = pid_file(name).ok().map(|p| fs::remove_file(p));
        println!("Stopped watcher {} (PID {}).",
            format!("\"{}\"", name).cyan().bold(),
            pid.to_string().dimmed()
        );
    } else {
        print_err(&format!("Failed to stop watcher \"{}\" (PID {})", name, pid));
    }
}

fn stop_all_watchers() {
    let pids_d = match pids_dir() {
        Ok(d) => d,
        Err(e) => { print_err(&e); return; }
    };

    let entries = match fs::read_dir(&pids_d) {
        Ok(e) => e,
        Err(_) => {
            println!("{}", "No background watchers running.".dimmed());
            return;
        }
    };

    let mut stopped = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pid") {
            continue;
        }
        let name = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let pid: u32 = match fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            Some(p) => p,
            None => { let _ = fs::remove_file(&path); continue; }
        };

        if is_alive(pid) && kill_pid(pid) {
            let _ = fs::remove_file(&path);
            println!("Stopped {} (PID {}).",
                name.cyan().bold(),
                pid.to_string().dimmed()
            );
            stopped += 1;
        } else {
            let _ = fs::remove_file(&path);
        }
    }

    if stopped == 0 {
        println!("{}", "No background watchers were running.".dimmed());
    }
}

fn kill_pid(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
