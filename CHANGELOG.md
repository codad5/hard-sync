# Changelog

All notable changes to this project are documented here.

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
