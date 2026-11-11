# Changelog

All notable changes to this project are documented here.

---

## [Unreleased]

### Added
- Access checks before syncing (core, so CLI, app and watch all get them): the source must be readable and the side being written to must be writable. An unusable drive now fails once, up front, with the reason — e.g. "the drive is mounted read-only. macOS can read NTFS drives but not write to them" — instead of one error per file
- `hsync init` / adding a pair in the app and `set-source` refuse a setup that could never sync, such as writing to an NTFS drive on macOS
- `hard-sync-core`: `check_pair_access`, `check_readable`, `check_writable`, `AccessError`, `WriteCheck`
- App: sync errors are shown on the pair's card, with an expandable list of each failed file and why
- App: example paths in the add-pair form match your OS

### Fixed
- App: removing a pair happened immediately, before you answered the "Remove pair?" question

---

## [1.0.2] — 2026-11-12

First release tested on macOS.

### Fixed
- OS metadata files (`.DS_Store`, `._*`, `.Spotlight-V100`, `.fseventsd`, `.Trashes`, `System Volume Information`, `$RECYCLE.BIN`, `Thumbs.db`, `desktop.ini`) are now always ignored — they were copied around and, at a drive root, sync tried to trash system folders
- macOS: drives are now identified by Volume UUID (via `diskutil`), not label only
- macOS: unnamed drives are no longer stored with the placeholder label "Untitled", which matched any unnamed drive
- Drive matching: when both the pair and the drive have a UUID, the UUID decides; label is only a fallback. `hsync drives` now uses the same rules as `watch`
- Files that fail to copy/trash/delete are no longer counted or listed as successful
- `hsync` now exits with code 1 on errors, including syncs with failed files
- `hsync sync` no longer cuts off its own notification sounds
- The "done" sound no longer plays when files failed — you hear the error sound instead (CLI and desktop app)
- Trash fallback no longer leaves a file in two places if deleting the original fails
- Read-only drive errors now explain the cause (e.g. NTFS on macOS)
- README images now render on crates.io; README shows the correct config location per OS

### Added
- `hard-sync-core`: `drive_matches_connected` and `play_event_sound_handle`

---

## [1.0.1] — 2026-11-11

Docs and packaging fix. No behaviour changes.

### Fixed
- README install command: the CLI crate is published as `hsync`, so the command is `cargo install hsync` (was `cargo install hard-sync-cli`, which doesn't exist) — [#4](https://github.com/codad5/hard-sync/issues/4)
- crates.io pages for `hsync` and `hard-sync-core` now show the README
- Crate `repository` links now point to `github.com/codad5/hard-sync`

---

## [1.0.0] — 2026-03-08

First stable release. Complete ground-up rewrite with a new architecture, new config format, desktop app, and background watch support.

### Added

**Core library (`hard-sync-core`)**
- Config system — named pairs stored in `~/.config/hard-sync/config.json`
- Two pair types: cross-drive (identified by UUID + volume label) and same-drive (`drive_id: null`)
- Drive detection — identifies drives by UUID and volume label, never by mount path
- Windows fix: strip `\\?\` UNC prefix from canonicalized paths before mount-point comparison
- Sync engine — mtime+size comparison by default; SHA256 opt-in via `--verify`
- Delete behaviors: `trash` (default), `delete`, `ignore` — configurable per pair
- Trash system — orphaned files moved to `.hard-sync-trash/<timestamp>_<filename>` on target
- Ignore patterns — built-in defaults + per-pair config list + `.hardsyncignore` file (gitignore-style)
- File watcher — `notify`-based, 500ms debounce for same-drive pairs
- Drive poller — 3s interval for cross-drive pairs in watch mode
- Notification sounds — `rodio`-based; built-in synthesized tones play by default (no setup required), custom WAV/MP3/OGG/FLAC paths configurable per event per pair
- `list_connected_drives()` — enumerate all mounted drives with removable flag and space info
- `get_config_path()` and `reset_config()` — config introspection and reset
- All public types implement `serde::Serialize` + `serde::Deserialize` for use across IPC boundaries

**CLI (`hsync`)**
- `hsync init` — set up a named pair; auto-detects drive type from paths
- `hsync sync` — one-shot sync with `--dry-run` and `--verify` flags
- `hsync watch` — blocking foreground watch mode
- `hsync watch --detach` — spawn watcher as a background daemon (PID + logs in `~/.local/share/hsync/`)
- `hsync watch list` — show all running background watchers
- `hsync watch attach` — tail the log of a background watcher (Ctrl+C to detach, watcher keeps running)
- `hsync watch stop` — stop a specific background watcher or all (`--all`)
- `hsync autostart enable/disable/list` — register watchers to start on login via OS-native mechanism
- `hsync list` — show all configured pairs
- `hsync drives` — list connected drives, annotated with pair matches
- `hsync set-source` — flip which side is the source of truth
- `hsync remove` — remove a pair from config (no files deleted)
- `hsync trash list` — inspect trash for a pair
- `hsync trash clear` — delete trash for one pair or all pairs
- `hsync config path` — print config file location
- `hsync config reset` — delete config file and remove all pairs

**Desktop app (Tauri + Svelte)**
- System tray icon — left-click to open window, right-click for Show/Quit menu
- Pair list with status badges (watching / idle), drive pair indicator
- Sync now button — one-shot sync with result summary
- Watch / Stop toggle — spawns or stops the background `hsync` daemon
- Add pair form — name, base path, target path (with native OS folder picker), source side toggle
- Remove pair button
- Refresh button

**CI/CD**
- `release.yml` — builds CLI binaries (`hsync-windows-x86_64.exe`, `hsync-linux-x86_64`, `hsync-macos-x86_64`) and Tauri desktop installers for all three platforms on every GitHub Release
- `publish.yml` — publishes `hard-sync-core` and `hard-sync-cli` to crates.io on release

### Changed
- Replaced single-command `hard-sync-cli` with named-pair model — set up once, run by name forever
- Replaced per-directory `.hard_sync_cli/` state files with a single global config
- Replaced path-based drive identification with UUID/label-based identification

### Removed
- Old `--src` / `--dest` / `--reverse` / `--exclude` flags (replaced by named pair config)
- Per-directory state tracking (`.hard_sync_cli/` directories)
- `_ref/` directory (old codebase reference files)
- Docker setup (incompatible with native OS audio, drive detection, and autostart features)

---

## [0.2.0] — previous

Internal rewrite milestone. Workspace scaffold, core library extraction, basic CLI commands.
Not published.
