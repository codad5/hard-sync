use colored::Colorize;
use hard_sync_core::{watch_pair, WatchEvent};

use crate::output::{print_err, print_sync_report, require_name};

pub fn watch_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    println!("Watching {}...", format!("\"{}\"", name).cyan().bold());
    println!("{}", "Press Ctrl+C to stop.\n".dimmed());

    let handle = match watch_pair(&name, move |event| {
        match event {
            WatchEvent::Watching => {
                println!("{}", "  Ready. Watching for changes...".dimmed());
            }
            WatchEvent::DriveDetected { mount_point } => {
                println!("  {} Drive detected at {} — syncing...",
                    chrono_now(),
                    mount_point.display().to_string().cyan(),
                );
            }
            WatchEvent::DriveRemoved => {
                println!("  {} {}", chrono_now(), "Drive removed. Waiting...".yellow());
            }
            WatchEvent::SyncStarted => {
                // silent — DriveDetected or file-change message already shown
            }
            WatchEvent::SyncCompleted(report) => {
                print_sync_report(&report, false);
                println!("{}", "  Watching for changes...".dimmed());
            }
            WatchEvent::SyncError(e) => {
                eprintln!("  {} {} {}", chrono_now(), "Sync error:".bright_red(), e);
            }
        }
    }) {
        Ok(h)  => h,
        Err(e) => { print_err(&e); return; }
    };

    handle.wait();
}

fn chrono_now() -> String {
    format!("[{}]", chrono::Local::now().format("%H:%M:%S")).dimmed().to_string()
}
