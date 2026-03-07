# hard-sync

A fast, zero-interaction file sync tool for removable drives. Set up a pair once — plug in your drive, hear the start sound, hear the done sound, unplug. No typing required after setup.

```
hsync init --name backup --base ~/projects --target /media/usb/projects
hsync watch --name backup
```

---

## How It Works

You define a **sync pair**: a `base` path and a `target` path, with one side designated as the source of truth. For drive pairs, `hard-sync` identifies the drive by UUID and volume label — not by mount path — so it finds it wherever the OS mounts it.

In watch mode:
- **Same-drive pairs** — file watcher triggers sync on change (debounced 500ms)
- **Cross-drive pairs** — drive poller detects plug-in, syncs immediately, then watches for changes until drive is removed

---

## Install

### From crates.io

```bash
cargo install hard-sync-cli
```

### From source

```bash
git clone https://github.com/codad5/hard-sync-cli
cd hard-sync-cli
cargo install --path cli
```

Confirm:

```bash
hsync --help
```

---

## Commands

### `hsync init` — Set up a new sync pair

```bash
hsync init --name <name> --base <path> --target <path> [--source base|target]
```

Detects whether base and target are on the same drive automatically. For cross-drive pairs, stores the drive's UUID and volume label so it can be found in watch mode regardless of mount path.

```
Pair "backup" initialized.
  base:    /home/user/projects
  target:  /media/usb/projects
  source:  base
  drive:   MY_USB (uuid: a1b2-c3d4)
  delete:  trash
```

---

### `hsync sync` — One-shot sync

```bash
hsync sync --name <name> [--dry-run] [--verify]
```

| Flag | Description |
|------|-------------|
| `--dry-run` / `-d` | Show what would change, touch nothing |
| `--verify` / `-v` | Use SHA256 comparison instead of mtime+size |

```
Syncing "backup"...
  + copied   src/main.rs
  ~ updated  src/config.rs
  - trashed  old_notes.txt

Done.  2 copied  1 updated  1 trashed  143 skipped  0 errors
```

---

### `hsync watch` — Auto-sync on drive detect / file change

```bash
hsync watch --name <name>
```

Blocks until Ctrl+C. For cross-drive pairs, waits for the drive to appear, syncs immediately on plug-in, and re-syncs on file changes while the drive is connected.

```
Watching "backup"...
Press Ctrl+C to stop.

  Ready. Watching for changes...
  [14:23:01] Drive detected at E:\ — syncing...
  [14:23:04] Done.  5 copied  0 updated  0 trashed  210 skipped
  Watching for changes...
```

---

### `hsync list` — Show all configured pairs

```bash
hsync list
```

---

### `hsync drives` — Show connected drives

```bash
hsync drives
```

Lists all currently mounted drives. Annotates any drive that matches a configured pair.

---

### `hsync set-source` — Flip source of truth

```bash
hsync set-source --name <name> --source base|target
```

---

### `hsync remove` — Remove a pair

```bash
hsync remove --name <name>
```

Does not delete any files — only removes the pair from config.

---

### `hsync trash list` / `hsync trash clear` — Manage the trash

Files deleted from target (when `delete_behavior = trash`) go to `.hard-sync-trash/` on the target. You can inspect and clear it:

```bash
hsync trash list --name <name>
hsync trash clear --name <name>
hsync trash clear --all
```

---

## Config

Config is stored at `~/.config/hard-sync/config.json`. You should not need to edit it directly — all settings are managed through commands.

**Ignore patterns:** Add a `.hardsyncignore` file to your base directory (gitignore syntax). Patterns can also be added per-pair in config. Built-in ignores: `.hard-sync-trash/`, `.hardsyncignore`.

**Delete behavior** (per pair, edit config directly): `"trash"` (default), `"delete"`, or `"ignore"`.

**Notification sounds** (per pair, edit config directly): set `sounds.sync_start`, `sounds.sync_done`, or `sounds.sync_error` to a path to a WAV or MP3 file. Null by default.

---

## Roadmap

### SSH sync (planned)

Support for syncing with remote paths over SSH, so you can keep a local folder in sync with a cloud server or NAS:

```bash
# One-time: store SSH connection details
hsync ssh add --name myserver --host user@192.168.1.10 --key ~/.ssh/id_rsa

# Init with ssh:// prefix on either side (not both)
hsync init --name cloud-backup --base ~/projects --target ssh://myserver/home/user/backup
```

Rules:
- Only one side of a pair can be an SSH path — not both
- The SSH connection is set up once via `hsync ssh add` and referenced by name
- Works with `hsync sync` and `hsync watch` like any other pair
- Watch mode will poll connectivity instead of drive detection

---

## Crates

| Crate | Role |
|-------|------|
| `hard-sync-core` | Library — all sync logic, drive detection, config, watcher |
| `hard-sync-cli` (`hsync`) | Binary — thin CLI wrapper over core |

Both are published on [crates.io](https://crates.io). If you want to build your own frontend (GUI, TUI, daemon), depend on `hard-sync-core` directly.

---

## License

MIT
