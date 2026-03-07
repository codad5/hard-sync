# Complete rewrite: named-pair sync with drive detection

## Summary

Full ground-up rewrite of `hard-sync-cli`. The old codebase required typing source and destination paths on every run. The new version uses named pairs — set up once, reference forever.

**The killer use case:** plug in a drive, hear a start sound, hear a done sound, unplug. Zero typing after setup.

---

## What changed

### Architecture

| Before | After |
|--------|-------|
| Single `hsync sync --src X --dest Y` every time | `hsync init --name backup ...` once, then `hsync watch --name backup` |
| Per-directory `.hard_sync_cli/` state files | Single global config at `~/.config/hard-sync/config.json` |
| Drive identified by path | Drive identified by UUID + volume label (survives remounts) |
| `clap` for CLI | `fli` (user-authored, lightweight) |

### New workspace structure

```
core/   → hard-sync-core  (published library — all business logic)
cli/    → hsync binary    (thin CLI wrapper)
ui/     → deferred
```

`hard-sync-core` is a standalone library. Anyone building a GUI, TUI, or daemon can depend on it directly without going through the CLI.

### Commands

```
hsync init          Set up a named sync pair
hsync sync          One-shot sync
hsync watch         Auto-sync on drive detect / file change
hsync list          Show configured pairs
hsync drives        Show connected drives (annotated with pair matches)
hsync set-source    Flip which side is the source of truth
hsync remove        Remove a pair
hsync trash list    Inspect trash
hsync trash clear   Delete trash
hsync config path   Show config file location
hsync config reset  Delete all config
```

### Notable fixes

- **Windows drive detection bug:** `Path::canonicalize()` on Windows prepends `\\?\` to paths, which broke `starts_with()` comparisons against plain drive-letter paths from `sysinfo`. Fixed by stripping the prefix before comparison. Covered by unit tests.

---

## Test plan

- [ ] `cargo test` — 6 passing (4 ignore pattern tests, 2 UNC prefix tests)
- [ ] `hsync init` with same-drive paths → shows "same drive"
- [ ] `hsync init` with cross-drive paths (drive plugged in) → shows drive label + UUID
- [ ] `hsync sync --dry-run` → shows diff without touching files
- [ ] `hsync sync` → files copied to target
- [ ] `hsync drives` → lists connected drives
- [ ] `hsync config path` → prints config file path
- [ ] `hsync watch` → blocks and syncs on file change (same-drive pair)
