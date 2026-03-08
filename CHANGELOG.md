# Changelog

## [Unreleased] — rewrite/workspace-scaffold

Complete ground-up rewrite. New architecture, new config format, new CLI.

### Added

**Core library (`hard-sync-core`)**
- Config system — named pairs stored in `~/.config/hard-sync/config.json`
- Two pair types: cross-drive (drive_id by UUID + label) and same-drive (drive_id null)
- Drive detection — identifies drives by UUID and volume label, never by mount path
- Windows fix: strip `\\?\` UNC prefix from canonicalized paths before mount-point comparison
- Sync engine — mtime+size comparison by default; SHA256 opt-in via `--verify`
- Delete behaviors: `trash` (default), `delete`, `ignore` — per pair
- Trash system — orphaned files moved to `.hard-sync-trash/` on target with timestamps
- Ignore patterns — built-ins + per-pair config list + `.hardsyncignore` file (gitignore-style)
- File watcher — `notify`-based, 500ms debounce
- Drive poller — 3s interval, used in watch mode for cross-drive pairs
- Notification sounds — `rodio`-based, opt-in, per-event per-pair (sync_start, sync_done, sync_error)
- `list_connected_drives()` — enumerate all mounted drives with removable flag and space info
- `get_config_path()` and `reset_config()` — config introspection and reset

**CLI (`hsync`)**
- `hsync init` — set up a pair; auto-detects drive type from paths
- `hsync sync` — one-shot sync with `--dry-run` and `--verify` flags
- `hsync watch` — blocking watch mode; handles both pair types
- `hsync list` — show all configured pairs
- `hsync drives` — list connected drives, annotated with pair matches
- `hsync set-source` — flip which side is the source of truth
- `hsync remove` — remove a pair from config (no files deleted)
- `hsync trash list` — inspect trash for a pair
- `hsync trash clear` — delete trash for one pair or all pairs
- `hsync config path` — print config file location
- `hsync config reset` — delete config file and remove all pairs

### Changed
- Replaced single-command `hard-sync-cli` with named-pair model — set up once, run by name forever
- Replaced per-directory `.hard_sync_cli/` state files with a single global config
- Replaced path-based drive identification with UUID/label-based identification

### Removed
- Old `--src` / `--dest` / `--reverse` / `--exclude` flags (replaced by named pair config)
- Per-directory state tracking (`.hard_sync_cli/` directories)
- `_ref/` directory (old codebase reference files)
