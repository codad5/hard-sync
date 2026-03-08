use colored::Colorize;
use fli::option_parser::{Value, ValueTypes};
use hard_sync_core::{get_pair, play_event_sound, sync_pair, SoundEvent, SyncOptions};

use crate::output::{print_err, print_sync_report, require_name};

pub fn sync_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let dry_run = matches!(data.get_option_value("dry-run"), Some(ValueTypes::OptionalSingle(Some(Value::Bool(true)))));
    let verify  = matches!(data.get_option_value("verify"),  Some(ValueTypes::OptionalSingle(Some(Value::Bool(true)))));

    // Load sounds config (silently ignore if pair not found — sync will surface the error)
    let sounds = get_pair(&name).map(|p| p.sounds).ok();

    println!("Syncing {}...", format!("\"{}\"", name).cyan().bold());

    if let Some(ref s) = sounds {
        if !dry_run { play_event_sound(s, SoundEvent::SyncStart); }
    }

    match sync_pair(&name, SyncOptions { dry_run, verify }) {
        Ok(report) => {
            if let Some(ref s) = sounds {
                if !dry_run { play_event_sound(s, SoundEvent::SyncDone); }
            }
            print_sync_report(&report, dry_run);
        }
        Err(e) => {
            if let Some(ref s) = sounds {
                play_event_sound(s, SoundEvent::SyncError);
            }
            print_err(&e);
        }
    }
}
