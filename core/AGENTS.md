# hard-sync-core — Agent Context

## What This Crate Is

The published library crate. All business logic lives here. `cli/` and `ui/` are consumers only.

Crate name: `hard-sync-core`
Folder: `core/`

---

## Module Structure

```
core/src/
├── lib.rs           ← public API surface only — re-exports, no logic
├── config.rs        ← read/write config.json, PairConfig struct, CRUD ops
├── sync_engine.rs   ← diff + copy + trash/delete/ignore logic, SyncReport
├── watcher.rs       ← notify-based file watching, WatchHandle
├── drive.rs         ← drive detection, UUID/label matching, OS abstractions
├── ignore.rs        ← .hardsyncignore parsing + config list merging
├── sounds.rs        ← rodio audio notifications per event
└── logging.rs       ← structured operation log (what was copied/trashed/skipped)
```

---

## Public API (what lib.rs exposes)

```rust
// Pair management
pub fn init_pair(config: PairConfig) -> Result<()>
pub fn list_pairs() -> Result<Vec<PairConfig>>
pub fn remove_pair(name: &str) -> Result<()>
pub fn set_source(name: &str, source: SourceSide) -> Result<()>
pub fn get_pair(name: &str) -> Result<PairConfig>

// Sync
pub fn sync_pair(name: &str, options: SyncOptions) -> Result<SyncReport>

// Watch
pub fn watch_pair(name: &str, callbacks: WatchCallbacks) -> Result<WatchHandle>

// Trash
pub fn list_trash(name: &str) -> Result<Vec<TrashEntry>>
pub fn clear_trash(name: Option<&str>) -> Result<()>   // None = clear all
```

Keep internals `pub(crate)`. Only expose what callers genuinely need.

---

## Key Types

```rust
pub struct PairConfig {
    pub name: String,
    pub base: PathBuf,
    pub target: PathBuf,
    pub source: SourceSide,        // SourceSide::Base | SourceSide::Target
    pub drive_id: Option<DriveId>,
    pub ignore: Vec<String>,
    pub delete_behavior: DeleteBehavior,
    pub sounds: SoundConfig,
    pub created_at: DateTime<Utc>,
}

pub enum SourceSide { Base, Target }

pub struct DriveId {
    pub label: Option<String>,
    pub uuid: Option<String>,
}

pub enum DeleteBehavior { Trash, Delete, Ignore }

pub struct SyncOptions {
    pub dry_run: bool,
    pub verify: bool,        // SHA256 comparison instead of mtime+size
}

pub struct SyncReport {
    pub copied: usize,
    pub updated: usize,
    pub trashed: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub ignored: usize,
    pub errors: Vec<SyncError>,
}

pub struct SoundConfig {
    pub sync_start: Option<PathBuf>,
    pub sync_done: Option<PathBuf>,
    pub sync_error: Option<PathBuf>,
}

pub struct TrashEntry {
    pub original_name: String,
    pub trashed_at: DateTime<Utc>,
    pub size: u64,
    pub path: PathBuf,
}
```

---

## Sync Engine Logic

```
1. Resolve which path is source and which is target from PairConfig
2. Walk source → HashMap<relative_path: String, DirEntry { size: u64, mtime: u64 }>
3. Walk target → same (apply ignore patterns during walk)
4. Diff:
   - In source, not target            → COPY to target
   - In target, not source            → apply delete_behavior
   - In both, source mtime newer      → OVERWRITE on target
   - In both, same mtime + size       → SKIP
   - --verify: also check SHA256 before overwrite decision
5. Create parent dirs as needed before copying
6. Log every operation with outcome
7. Fire sound notification: sync_start before loop, sync_done or sync_error after
8. Return SyncReport
```

**Never load file content into memory.** Use `fs::metadata()` for mtime/size. Only read bytes when computing SHA256 under `--verify`.

---

## Trash System

- Trash folder: `<target_path>/.hard-sync-trash/`
- Trash filename format: `<ISO8601_timestamp>_<original_filename>`
  - Example: `2026-03-07T14-23-01Z_old_notes.txt`
- On `clear_trash`: delete all files in `.hard-sync-trash/` for that pair
- Trash folder itself is always ignored during sync walks

---

## Pair Types

Two valid configurations — both fully supported:

**Cross-drive pair** (typical USB use case)
- `base` and `target` are on different physical drives
- `drive_id` is populated on `init`
- Watch mode polls for drive AND watches for file changes
- Drive may not always be mounted — tool waits for it

**Same-drive pair** (two folders on the same machine)
- `base` and `target` are on the same physical drive (including two different local repos)
- `drive_id` is `null` in config
- Watch mode only watches source for file changes — no drive polling
- Target is always assumed to be accessible — error immediately if missing

On `init`, detect automatically: if both paths resolve to the same mount point / disk, set `drive_id: null`. Do not ask the user to specify.

---

## Drive Detection

- On `init`: given the target path, detect what drive it lives on. Compare with the drive that `base` lives on.
  - If same drive → `drive_id: null`, skip drive detection
  - If different drive → extract UUID and label from target drive, store in `PairConfig.drive_id`
- On `watch` (cross-drive pairs only): poll mounted drives every N seconds. Compare UUID/label against stored `drive_id`. When match found, resolve current mount path and use as target.
- `sysinfo` crate provides disk info cross-platform. May need OS-specific calls for UUID on Windows vs Linux.
- Abstract behind a trait:
  ```rust
  pub(crate) trait DriveDetector {
      fn get_drive_id(path: &Path) -> Result<DriveId>;
      fn find_mounted_drive(id: &DriveId) -> Option<PathBuf>;
      fn same_drive(a: &Path, b: &Path) -> bool;
  }
  ```

---

## Ignore Pattern Matching

Priority order (later overrides earlier is NOT how this works — union of all patterns):
1. Built-in defaults: `.hard-sync-trash/`, `.hard_sync_cli/`, `.hardsyncignore`
2. Per-pair list from config JSON
3. `.hardsyncignore` file in base directory (gitignore line format, `#` = comment)

A file is ignored if it matches ANY pattern from any source.

---

## Config File Location

```rust
// Cross-platform:
dirs::config_dir()  // ~/.config on Linux, %APPDATA% on Windows
    .join("hard-sync")
    .join("config.json")
```

Use the `dirs` crate for this. Create the directory if it doesn't exist on first write.

---

## Crates for This Module

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
walkdir = "2"
chrono = { version = "0.4", features = ["serde"] }
sysinfo = "0.29"
notify = "6"
rodio = "0.17"
sha2 = "0.10"
regex = "1"
dirs = "5"
```
