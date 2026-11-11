use colored::Colorize;
use fli::option_parser::{Value, ValueTypes};
use hard_sync_core::{get_pair, play_event_sound_handle, sync_pair, SoundEvent, SyncOptions};

use crate::output::{mark_failed, print_err, print_sync_report, require_name};

pub fn sync_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let dry_run = matches!(data.get_option_value("dry-run"), Some(ValueTypes::OptionalSingle(Some(Value::Bool(true)))));
    let verify  = matches!(data.get_option_value("verify"),  Some(ValueTypes::OptionalSingle(Some(Value::Bool(true)))));

    // Load sounds config (silently ignore if pair not found — sync will surface the error)
    let sounds = get_pair(&name).map(|p| p.sounds).ok();

    println!("Syncing {}...", format!("\"{}\"", name).cyan().bold());

    // Sounds play on background threads; keep the handles and wait for them
    // below, otherwise this one-shot process exits and cuts them off.
    let mut playing = Vec::new();
    let mut play = |event| {
        if let Some(ref s) = sounds {
            if !dry_run { playing.push(play_event_sound_handle(s, event)); }
        }
    };

    play(SoundEvent::SyncStart);

    match sync_pair(&name, SyncOptions { dry_run, verify }) {
        Ok(report) => {
            // A "done" chime means "safe to unplug" — never play it if files failed.
            let failed = !report.errors.is_empty();
            play(if failed { SoundEvent::SyncError } else { SoundEvent::SyncDone });
            print_sync_report(&report, dry_run);
            if failed { mark_failed(); }
        }
        Err(e) => {
            play(SoundEvent::SyncError);
            print_err(&e);
        }
    }

    for handle in playing {
        let _ = handle.join();
    }
}
