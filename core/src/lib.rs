pub mod config;
pub mod drive;
pub mod ignore;
pub mod sounds;
pub mod sync_engine;
pub mod watcher;

// Config API
pub use config::{
    add_pair, get_pair, list_pairs, remove_pair, set_source, update_pair,
    AppConfig, DeleteBehavior, DriveId, PairConfig, SoundConfig, SourceSide,
};

// Sounds API
pub use sounds::{play_event_sound, SoundEvent};

// Drive API
pub use drive::{find_mounted_drive, get_drive_id, same_drive};

// Watch API
pub use watcher::{watch_pair, WatchEvent, WatchHandle};

// Sync API
pub use sync_engine::{
    clear_trash, list_trash, sync_pair, SyncError, SyncOptions, SyncReport, TrashEntry,
};
