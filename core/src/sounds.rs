use std::path::{Path, PathBuf};
use std::thread;

use crate::config::SoundConfig;

// ── Public types ──────────────────────────────────────────────────────────────

pub enum SoundEvent {
    SyncStart,
    SyncDone,
    SyncError,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Play the configured sound for a given event, if one is set.
/// Playback runs on a background thread — this call returns immediately.
/// Errors are silently ignored (sound is optional, never blocks sync).
pub fn play_event_sound(config: &SoundConfig, event: SoundEvent) {
    let path = match event {
        SoundEvent::SyncStart => config.sync_start.as_ref(),
        SoundEvent::SyncDone => config.sync_done.as_ref(),
        SoundEvent::SyncError => config.sync_error.as_ref(),
    };

    if let Some(p) = path {
        play_async(p.clone());
    }
}

// ── Internal ──────────────────────────────────────────────────────────────────

/// Spawn a thread to play the file. The thread owns the OutputStream so it
/// stays alive for the full duration of playback.
fn play_async(path: PathBuf) {
    thread::spawn(move || {
        let _ = play_blocking(&path);
    });
}

fn play_blocking(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // DeviceSinkBuilder opens the OS audio device. Must stay alive during playback.
    let handle = rodio::DeviceSinkBuilder::open_default_sink()?;

    let file = std::fs::File::open(path)?;
    // rodio::play queues the file on the device's mixer and returns a Player
    let player = rodio::play(&handle.mixer(), std::io::BufReader::new(file))?;

    // Block this thread until playback finishes. handle stays alive in scope.
    player.sleep_until_end();
    Ok(())
}
