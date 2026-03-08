pub mod config;
pub mod drive;
pub mod ignore;
pub mod sounds;
pub mod sync_engine;
pub mod watcher;

// Config API
pub use config::{
    add_pair, get_config_path, get_pair, list_pairs, remove_pair, reset_config,
    set_source, update_pair, AppConfig, DeleteBehavior, DriveId, PairConfig, SoundConfig,
    SourceSide,
};

// Sounds API
pub use sounds::{play_event_sound, SoundEvent};

// Drive API
pub use drive::{find_mounted_drive, get_drive_id, list_connected_drives, same_drive, ConnectedDrive};

// Watch API
pub use watcher::{watch_pair, WatchEvent, WatchHandle};

// Sync API
pub use sync_engine::{
    clear_trash, list_trash, sync_pair, SyncError, SyncOperation, SyncOptions, SyncOutcome,
    SyncReport, TrashEntry,
};
