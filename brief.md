# hard-sync — Project Brief

## What It Is

A Rust CLI tool that syncs files between two paths (typically a local folder and a removable drive). One path is designated the **source** (truth), the other the **target** (follows the source). Supports watch mode for auto-syncing when a drive is detected.

## Core Design Decisions (Already Made)

- **Naming**: source/target (not master/slave, not primary/replica)
- **Direction**: configurable per sync pair — either side can be source
- **Both directions supported in v1**: user can flip which side is source at any time
- **Config format**: JSON at `~/.config/hard-sync/config.json`
- **Delete behavior**: configurable per sync pair — `"trash"` | `"delete"` | `"ignore"`
  - `trash` = move to `.hard-sync-trash/` on target with timestamp
  - `delete` = actually remove the file
  - `ignore` = leave orphaned files, just log them
- **Comparison method (v1)**: file size + modified timestamp (no checksums yet)
- **Build the sync engine first**, then CLI/config, then watcher, then drive detection

## CLI Design

```bash
# First-time setup — pairs two paths and names the pair
hard-sync init --base "./projects" --target "/media/usb/projects" --source base

# One-shot sync
hard-sync sync --name "my-backup"

# Watch mode — auto-syncs on file change + drive detection
hard-sync watch --name "my-backup"

# Flip which side is the source of truth
hard-sync set-source --name "my-backup" --source target

# List all configured sync pairs
hard-sync list

# Remove a sync pair
hard-sync remove --name "my-backup"
```

**Note**: `--base` = the local/primary path, `--target` = the other path (often a removable drive). The `--re` flag from early brainstorming was renamed to `--target` for clarity. `--base` could also be renamed to `--local` if preferred.

## Config Structure

```json
{
  "version": 1,
  "pairs": [
    {
      "name": "my-backup",
      "base": "/home/user/projects",
      "target": "/media/usb/projects",
      "source": "base",
      "drive_id": {
        "label": "MY_USB",
        "uuid": "xxxx-xxxx"
      },
      "ignore": ["node_modules", ".git", "target", "dist"],
      "delete_behavior": "trash",
      "created_at": "2026-03-07T12:00:00Z"
    }
  ]
}
```

## Sync Engine Logic

```
1. Walk source directory → build file list (relative path, size, modified time)
2. Walk target directory → same
3. Compare:
   - In source but not target → COPY to target
   - In target but not source → handle per delete_behavior (trash/delete/ignore)
   - In both, source is newer → OVERWRITE on target
   - In both, same timestamp & size → SKIP
4. Log all operations
```

## Module Structure

```
src/
├── main.rs          # CLI entry point (clap)
├── config.rs        # Read/write JSON config
├── sync_engine.rs   # Core diff + copy + trash logic
├── watcher.rs       # notify-based file watching
├── drive.rs         # Drive detection & identification by UUID/label
├── ignore.rs        # Pattern matching for ignored paths
└── logging.rs       # Sync operation logging
```

## Key Crates

- `clap` — CLI argument parsing
- `notify` — cross-platform file watching (wraps inotify/FSEvents/ReadDirectoryChangesW)
- `serde` + `serde_json` — config serialization
- `walkdir` — recursive directory traversal
- `filetime` — cross-platform file timestamp comparison
- `chrono` — timestamps for logging and trash naming

## Important Technical Considerations

### Drive Identity Problem
Drive letters (Windows) and mount points (Linux) can change depending on plug order. The tool should identify drives by **volume label** or **filesystem UUID**, not by path. On first `init`, detect and store the drive's stable identifier. In watch mode, periodically check if a drive matching that identifier is mounted, regardless of where it mounted.

### Ignore Patterns
Support a per-pair ignore list in config AND an optional `.hardsyncignore` file in the base directory (like `.gitignore`). Always ignore `.hard-sync-trash/` by default.

### Watch Mode Architecture
Combines two systems:
1. `notify` crate watches the base path for file changes
2. Periodic polling (or OS events) checks if the target drive is present
When both conditions align (files changed AND drive is mounted), trigger sync.

### Cross-Platform
- Linux: systemd for auto-start, udev or mount polling for drive detection
- Windows: Task Scheduler for auto-start, drive letter polling for detection
- File paths must handle both `/` and `\` separators

## What's NOT in v1 Scope

- Checksum-based comparison (add later as `--verify` flag)
- Encryption
- True bidirectional sync with no declared source (complex conflict resolution)
- Network/remote sync
- GUI

## Build Order

1. **Sync engine** — the core diff/copy/trash logic (start here)
2. **Config module** — read/write JSON config
3. **CLI** — clap commands wired to sync engine + config
4. **Ignore patterns** — filtering during directory walks
5. **File watcher** — notify integration
6. **Drive detection** — stable drive identification
7. **Watch mode** — combining watcher + drive detection

## Background Context

This project originated from a conversation about file watchers (how nodemon works), OS filesystem APIs (inotify, FSEvents, ReadDirectoryChangesW), Rust concurrency primitives (mpsc channels, Arc, Mutex, RwLock, atomics), process signals (SIGINT, SIGTERM, SIGHUP), IPC mechanisms, and systemd services. The developer has strong systems-level understanding and is building this as both a learning project and a practical tool.