use colored::Colorize;
use fli::option_parser::{Value, ValueTypes};
use hard_sync_core::{get_pair, play_event_sound, watch_pair, SoundEvent, WatchEvent};

use crate::daemon::spawn_detached;
use crate::output::{print_err, print_sync_report, require_name};

pub fn watch_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let detach = matches!(
        data.get_option_value("detach"),
        Some(ValueTypes::OptionalSingle(Some(Value::Bool(true))))
    );

    if detach {
        match spawn_detached(&name) {
            Ok(pid) => {
                println!(
                    "Watcher {} started in background (PID {}).",
                    format!("\"{}\"", name).cyan().bold(),
                    pid.to_string().dimmed()
                );
                println!("{}", "  Run `hsync watch attach --name <name>` to view output.".dimmed());
                println!("{}", "  Run `hsync watch stop --name <name>` to stop it.".dimmed());
            }
            Err(e) => print_err(&e),
        }
        return;
    }

    // Foreground mode
    println!("Watching {}...", format!("\"{}\"", name).cyan().bold());
    println!("{}", "Press Ctrl+C to stop.\n".dimmed());

    let sounds = get_pair(&name).map(|p| p.sounds).ok();

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
                if let Some(ref s) = sounds {
                    play_event_sound(s, SoundEvent::SyncStart);
                }
            }
            WatchEvent::SyncCompleted(report) => {
                if let Some(ref s) = sounds {
                    play_event_sound(s, SoundEvent::SyncDone);
                }
                print_sync_report(&report, false);
                println!("{}", "  Watching for changes...".dimmed());
            }
            WatchEvent::SyncError(e) => {
                if let Some(ref s) = sounds {
                    play_event_sound(s, SoundEvent::SyncError);
                }
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
