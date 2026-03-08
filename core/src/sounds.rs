use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::config::SoundConfig;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum SoundEvent {
    SyncStart,
    SyncDone,
    SyncError,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Play the configured sound for a given event.
/// Falls back to a synthesized tone when no custom path is configured.
/// Playback runs on a background thread — this call returns immediately.
/// Audio errors are silently ignored (sound never blocks sync).
pub fn play_event_sound(config: &SoundConfig, event: SoundEvent) {
    let path = match event {
        SoundEvent::SyncStart => config.sync_start.as_ref(),
        SoundEvent::SyncDone  => config.sync_done.as_ref(),
        SoundEvent::SyncError => config.sync_error.as_ref(),
    };

    match path {
        Some(p) => play_file_async(p.clone()),
        None    => play_default_async(event),
    }
}

// ── Internal — file playback ──────────────────────────────────────────────────

fn play_file_async(path: PathBuf) {
    thread::spawn(move || {
        let _ = play_file_blocking(&path);
    });
}

fn play_file_blocking(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let handle = rodio::DeviceSinkBuilder::open_default_sink()?;
    let file   = std::fs::File::open(path)?;
    let player = rodio::play(&handle.mixer(), std::io::BufReader::new(file))?;
    player.sleep_until_end();
    Ok(())
}

// ── Internal — synthesized default tones ─────────────────────────────────────

fn play_default_async(event: SoundEvent) {
    thread::spawn(move || {
        let _ = play_default_blocking(event);
    });
}

fn play_default_blocking(event: SoundEvent) -> Result<(), Box<dyn std::error::Error>> {
    use rodio::source::{SineWave, Source};

    let handle = rodio::DeviceSinkBuilder::open_default_sink()?;

    // Sink::append accepts any Source — no Read+Seek required
    let play_tone = |freq: f32, ms: u64, vol: f32| {
        let src = SineWave::new(freq).take_duration(Duration::from_millis(ms)).amplify(vol);
        let sink = rodio::Sink::connect_new(&handle.mixer());
        sink.append(src);
        sink.sleep_until_end();
    };

    match event {
        SoundEvent::SyncStart => play_tone(660.0, 180, 0.4),
        SoundEvent::SyncDone => {
            play_tone(523.0, 120, 0.4);
            play_tone(784.0, 200, 0.4);
        }
        SoundEvent::SyncError => {
            play_tone(440.0, 120, 0.4);
            play_tone(220.0, 220, 0.4);
        }
    }
    Ok(())
}
