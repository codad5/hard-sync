pub mod config;
pub mod drive;
pub mod ignore;
pub mod sync_engine;

// Config API
pub use config::{
    add_pair, get_pair, list_pairs, remove_pair, set_source, update_pair,
    AppConfig, DeleteBehavior, DriveId, PairConfig, SoundConfig, SourceSide,
};

// Drive API
pub use drive::{find_mounted_drive, get_drive_id, same_drive};

// Sync API
pub use sync_engine::{
    clear_trash, list_trash, sync_pair, SyncError, SyncOptions, SyncReport, TrashEntry,
};
