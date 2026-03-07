# hard-sync-cli — Agent Context

## What This Crate Is

The published binary crate. A thin wrapper over `hard-sync-core`.

- Crate name: `hard-sync-cli`
- Folder: `cli/`
- Binary name: `hsync`
- CLI framework: `fli` (user-authored — use latest version on crates.io)

**Rule:** No business logic here. Parse args with `fli`, call a `core` function, format and print the result. That's it.

---

## All Commands — Full Spec

### `hsync init`
Set up a new sync pair. Detects drive ID from the target path automatically.

```
hsync init --base <path> --target <path> --name <name> [--source <base|target>]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--base` / `-b` | yes | Local/primary path (absolute or relative) |
| `--target` / `-t` | yes | Other path, often a removable drive |
| `--name` / `-n` | yes | Name for this pair (used in all other commands) |
| `--source` / `-s` | no | Which side is truth: `base` or `target`. Default: `base` |

Expected output (cross-drive pair — different drives):
```
Pair "my-backup" initialized.
  base:    /home/user/projects
  target:  /media/usb/projects
  source:  base
  drive:   MY_USB (uuid: xxxx-xxxx)
  delete:  trash
```

Expected output (same-drive pair — both paths on same machine/drive):
```
Pair "local-mirror" initialized.
  base:    /home/user/project-a
  target:  /home/user/project-b
  source:  base
  drive:   same drive (no detection needed)
  delete:  trash
```

Drive type is detected automatically — user does not specify it.

---

### `hsync sync`
One-shot sync for a named pair.

```
hsync sync --name <name> [--dry-run] [--verify]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name` / `-n` | yes | Name of the pair to sync |
| `--dry-run` / `-d` | no | Show what would happen — no files are touched |
| `--verify` / `-v` | no | Use SHA256 checksum comparison instead of mtime+size |

Expected output:
```
Syncing "my-backup"...
  source:  /home/user/projects
  target:  /media/usb/projects

  + copied    src/main.rs
  ~ updated   src/config.rs
  - trashed   old_notes.txt
  · skipped   README.md  (+142 more)

Done.  3 copied  1 updated  1 trashed  143 skipped  0 errors
```

Dry-run banner shown above file list:
```
[DRY RUN] No files will be modified.
```

Error line format:
```
  ! error     some/file.rs  — permission denied
```

---

### `hsync watch`
Watch mode — auto-syncs when the paired drive is detected and files change.

```
hsync watch --name <name>
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name` / `-n` | yes | Name of the pair to watch |

Expected output:
```
Watching "my-backup"...
  source:  /home/user/projects
  drive:   MY_USB (uuid: xxxx-xxxx)

  Waiting for drive...
  [14:23:01] Drive detected at /media/MY_USB — syncing...
  [14:23:04] Done.  5 copied  0 updated  0 trashed  210 skipped
  Watching for changes...
  [14:25:17] Changes detected — syncing...
  [14:25:18] Done.  1 copied  0 updated  0 trashed  214 skipped

Press Ctrl+C to stop.
```

Same-drive pair (no drive polling, syncs immediately on file change):
```
Watching "local-mirror"...
  source:  /home/user/project-a
  target:  /home/user/project-b

  Ready. Watching for changes...
  [14:25:17] Changes detected — syncing...
  [14:25:18] Done.  1 copied  0 updated  0 trashed  214 skipped

Press Ctrl+C to stop.
```

---

### `hsync set-source`
Flip which side is the source of truth for a pair.

```
hsync set-source --name <name> --source <base|target>
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name` / `-n` | yes | Pair name |
| `--source` / `-s` | yes | New source side: `base` or `target` |

Expected output:
```
"my-backup" source updated.
  source is now: target (/media/usb/projects)
```

---

### `hsync list`
List all configured sync pairs.

```
hsync list
```

Expected output:
```
Sync pairs (2)

  my-backup
    base:    /home/user/projects
    target:  /media/usb/projects
    source:  base
    drive:   MY_USB (xxxx-xxxx)
    delete:  trash

  work-files
    base:    /home/user/work
    target:  /media/usb2/work
    source:  base
    drive:   WORK_USB (yyyy-yyyy)
    delete:  delete
```

If no pairs configured:
```
No sync pairs configured. Run `hsync init` to add one.
```

---

### `hsync remove`
Remove a named sync pair from config (does not delete any files).

```
hsync remove --name <name>
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name` / `-n` | yes | Pair to remove |

Expected output:
```
Pair "my-backup" removed.
```

---

### `hsync trash list`
Show what's in the trash folder for a pair.

```
hsync trash list --name <name>
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name` / `-n` | yes | Pair name |

Expected output:
```
Trash — "my-backup"  (/media/usb/projects/.hard-sync-trash/)

  2026-03-07T14-23-01Z_old_notes.txt     4.2 KB
  2026-03-05T09-11-44Z_archive.zip      12.1 MB

Total: 2 files, 12.3 MB
```

If empty:
```
Trash is empty for "my-backup".
```

---

### `hsync trash clear`
Delete everything in the trash folder for a pair (or all pairs).

```
hsync trash clear --name <name>
hsync trash clear --all
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name` / `-n` | one of these | Clear trash for one pair |
| `--all` | one of these | Clear trash for all pairs |

Expected output:
```
Trash cleared for "my-backup".  2 files removed (12.3 MB freed)
```

---

## Output Formatting Rules

- Use `colored` crate for terminal color
- `+` copied = green
- `~` updated = yellow
- `-` trashed/deleted = red
- `·` skipped = dimmed
- `!` error = bright red
- Counts line always shown, even if all zeros
- File list: if more than 10 files, show first 10 then `(+N more)`
- Dry-run: same output but with `[DRY RUN]` banner, all colors dimmed

---

## Error Handling

- If `--name` references a pair that doesn't exist: print clear error and exit 1
  ```
  Error: no pair named "my-backup". Run `hsync list` to see configured pairs.
  ```
- If target drive is not mounted during `hsync sync`: print error and exit 1
  ```
  Error: drive for "my-backup" is not mounted (MY_USB / xxxx-xxxx).
         Use `hsync watch` to wait for the drive automatically.
  ```
- Never panic in cli/ — all errors are caught and printed

---

## Crates for This Module

Install via `cargo add` from inside `cli/` — do not pin versions manually:

```bash
cargo add fli
cargo add colored
```

`hard-sync-core` is added as a path dependency in `cli/Cargo.toml` by the workspace scaffold — not via `cargo add`.
