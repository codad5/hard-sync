# hard-sync — Agent Guidelines

## What This Project Is

`hard-sync` is a Rust CLI tool that syncs files between two paths — typically a local folder and a removable drive. One path is the **source** (truth), the other the **target** (follows source). Supports watch mode that auto-syncs when a specific drive is detected by UUID/label — no path typing after setup.

**The core use case:** user sets up a pair once, plugs in drive, tool auto-detects it, syncs, plays a sound, user unplugs. Zero interaction after setup.

**The killer scenario:** time-critical one-way transfer — plug drive, hear start sound, hear done sound, unplug. Works even if you can't be at a keyboard.

---

## Workspace Structure

Cargo workspace. Three members — folder name vs published crate name:

```
hard-sync/
├── Cargo.toml              ← [workspace] members = ["core", "cli", "ui"]
├── CLAUDE.md               ← this file
├── core/                   → published crate: hard-sync-core
│   ├── Cargo.toml
│   ├── AGENTS.md           ← core-specific agent context
│   └── src/
├── cli/                    → published crate: hard-sync-cli  (binary: hsync)
│   ├── Cargo.toml
│   ├── AGENTS.md           ← cli-specific agent context
│   └── src/
└── ui/                     → Tauri app (not published yet, deferred)
    └── AGENTS.md
```

**Rule:** `core/` contains all business logic. `cli/` and `ui/` are thin wrappers that call core and format output. Never put sync logic, drive detection, or config logic in cli/ or ui/.

---

## Architecture Decisions

### Naming Convention
- `base` = the local/primary path
- `target` = the other path (often a removable drive)
- `source` = which side is the truth (either `"base"` or `"target"`)
- Either side can be source — configurable per pair, swappable at any time

### Config
- Location: `~/.config/hard-sync/config.json`
- Format: JSON
- Named pairs — user inits once, references by name forever
- No per-directory state files (`.hard_sync_cli/` approach is dropped)

### Config Schema

Two pair types — both supported. Drive type is auto-detected on `init`, never specified by user.

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
      "sounds": {
        "sync_start": null,
        "sync_done": null,
        "sync_error": null
      },
      "created_at": "2026-03-07T12:00:00Z"
    },
    {
      "name": "local-mirror",
      "base": "/home/user/project-a",
      "target": "/home/user/project-b",
      "source": "base",
      "drive_id": null,
      "ignore": ["node_modules", ".git"],
      "delete_behavior": "trash",
      "sounds": {
        "sync_start": null,
        "sync_done": null,
        "sync_error": null
      },
      "created_at": "2026-03-07T12:00:00Z"
    }
  ]
}
```

`drive_id: null` = same-drive pair. No drive polling in watch mode.

### File Comparison
- **v1 default:** mtime + file size — fast, no memory overhead
- **`--verify` flag:** SHA256 checksum — opt-in only, used before overwrite decision
- **Never** load file content into memory for comparison

### Delete Behavior (per pair, configurable)
- `"trash"` — move to `.hard-sync-trash/<timestamp>_<filename>` on target (recommended default)
- `"delete"` — permanently remove from target
- `"ignore"` — leave orphaned files on target, log them only

### Drive Detection
- Identify drives by **UUID and/or volume label** — never by path
- Drive letters (Windows) and mount points (Linux) shift depending on plug order
- On `init`: detect and store the drive's stable identifier from the target path
- On `watch`: poll for a drive matching the stored UUID/label regardless of where it mounted
- Abstract behind a trait — Windows and Linux implementations are separate

### Ignore Patterns
- Per-pair list in config JSON
- Optional `.hardsyncignore` file in base dir (gitignore-style lines, `#` comments)
- Always auto-ignore: `.hard-sync-trash/`, `.hard_sync_cli/`, `.hardsyncignore`

### Notification Sounds
- **Opt-in** — null by default (sound in CLI is unusual, should never surprise)
- Events: `sync_start`, `sync_done`, `sync_error`
- User provides custom WAV/MP3 path per event, per pair, in config
- Crate: `rodio` (pure Rust, cross-platform)

---

## Key Crates

| Crate | Purpose | Notes |
|-------|---------|-------|
| `fli` | CLI argument parsing | User-authored — use latest version |
| `notify` | Cross-platform file watching | Wraps inotify/FSEvents/ReadDirectoryChangesW |
| `serde` + `serde_json` | Config serialization | |
| `walkdir` | Recursive directory traversal | |
| `chrono` | Timestamps for logging and trash naming | |
| `sysinfo` | Drive detection and disk info | |
| `rodio` | Cross-platform audio notifications | |
| `sha2` | SHA256 for `--verify` mode | Optional, not default comparison |
| `regex` | Ignore pattern matching | |

Do NOT use `clap`. Use `fli`.

---

## Build Order

1. Workspace scaffold (Cargo.toml, folder structure)
2. `core/` — config module
3. `core/` — sync engine
4. `core/` — ignore patterns
5. `core/` — drive detection
6. `core/` — watcher
7. `core/` — sounds
8. `cli/` — all commands wired to core
9. `ui/` — deferred until cli is stable

---

## Agent Workflow Rules

### Before Starting Any Fresh Task
1. Check for uncommitted changes. If any: request the user run the commit command via their **local CLI** — do not use MCP or any tool to commit.
2. Checkout a new branch named after the task before touching code.
3. Check memory files (`~/.claude/projects/.../memory/`) for prior context.

### Planning Protocol (mandatory)
4. Break the task into a todo list first — before writing any code.
5. Draft an implementation plan and present it to the user. **Wait for explicit go-ahead.**
6. After go-ahead: finalize the todo, then start implementation.

### During Implementation
7. Mark todo items complete immediately when done — never batch.
8. Request a commit (via user's local CLI) after each major milestone.
9. A "major milestone" = a module is complete and compiles cleanly.

### Code Rules
10. `core/` is a pure library — no CLI I/O, no printing, no user interaction.
11. `cli/` calls core functions and formats output. Nothing else.
12. Never load file content into memory for comparison.
13. Default comparison is mtime + size. SHA256 only on `--verify`.
14. Drive paths are never the stable identifier — always UUID/label.
15. **Always use `cargo add <crate>` to install dependencies** — never write dependency entries into `Cargo.toml` manually, and never pin specific versions in docs or code. `cargo add` picks the latest compatible version. Run it from inside the relevant workspace member directory (`core/` or `cli/`).
