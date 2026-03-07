use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::Utc;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::config::{DeleteBehavior, PairConfig, SourceSide};
use crate::ignore::IgnoreList;

// ── Public types ──────────────────────────────────────────────────────────────

pub struct SyncOptions {
    pub dry_run: bool,
    pub verify: bool,
}

pub struct SyncError {
    pub path: String,
    pub message: String,
}

pub enum SyncOutcome {
    Copied,
    Updated,
    Trashed,
    Deleted,
    Skipped,
    Ignored,
}

pub struct SyncOperation {
    pub rel_path: String,
    pub outcome: SyncOutcome,
}

pub struct SyncReport {
    pub copied: usize,
    pub updated: usize,
    pub trashed: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub ignored: usize,
    pub errors: Vec<SyncError>,
    pub ops: Vec<SyncOperation>,
}

pub struct TrashEntry {
    pub original_name: String,
    pub trashed_at: chrono::DateTime<Utc>,
    pub size: u64,
    pub path: PathBuf,
}

// ── Internal dir entry ────────────────────────────────────────────────────────

struct DirEntry {
    size: u64,
    mtime: u64,
    abs_path: PathBuf,
}

// ── Directory walking ─────────────────────────────────────────────────────────

/// Walk a directory, applying the ignore list. Returns relative paths with
/// forward slashes as keys.
fn walk_dir(root: &Path, ignore: &IgnoreList) -> HashMap<String, DirEntry> {
    let mut map = HashMap::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let abs = entry.path().to_path_buf();
        let rel = abs
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");

        if ignore.is_ignored(&rel) {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        map.insert(rel, DirEntry { size: meta.len(), mtime, abs_path: abs });
    }
    map
}

// ── SHA256 comparison (--verify mode only) ────────────────────────────────────

fn sha256_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536]; // 64 KB chunks — never loads file into memory
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_vec())
}

fn sha256_changed(src: &Path, tgt: &Path) -> bool {
    match (sha256_file(src), sha256_file(tgt)) {
        (Ok(a), Ok(b)) => a != b,
        _ => true, // if we can't hash, assume changed to be safe
    }
}

// ── File operations ───────────────────────────────────────────────────────────

fn copy_file(src: &Path, dest: &Path, errors: &mut Vec<SyncError>) {
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(SyncError {
                    path: dest.to_string_lossy().to_string(),
                    message: format!("Failed to create directory: {}", e),
                });
                return;
            }
        }
    }
    if let Err(e) = std::fs::copy(src, dest) {
        errors.push(SyncError {
            path: dest.to_string_lossy().to_string(),
            message: format!("Failed to copy: {}", e),
        });
    }
}

fn trash_file(
    abs_path: &Path,
    target_root: &Path,
    pair: &PairConfig,
    errors: &mut Vec<SyncError>,
) {
    let trash_dir = target_root.join(".hard-sync-trash");
    if !trash_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&trash_dir) {
            errors.push(SyncError {
                path: abs_path.to_string_lossy().to_string(),
                message: format!("Failed to create trash dir: {}", e),
            });
            return;
        }
    }

    let filename = abs_path.file_name().unwrap_or_default().to_string_lossy();
    // Timestamp safe for all filesystems (no colons)
    let ts = Utc::now().format("%Y-%m-%dT%H-%M-%SZ");
    let trash_name = format!("{}_{}_{}", ts, pair.name, filename);
    let trash_dest = trash_dir.join(&trash_name);

    if let Err(e) = std::fs::rename(abs_path, &trash_dest) {
        // rename fails across drives — fall back to copy + delete
        if std::fs::copy(abs_path, &trash_dest).is_ok() {
            let _ = std::fs::remove_file(abs_path);
        } else {
            errors.push(SyncError {
                path: abs_path.to_string_lossy().to_string(),
                message: format!("Failed to trash file: {}", e),
            });
        }
    }
}

// ── Public sync API ───────────────────────────────────────────────────────────

pub fn sync_pair(name: &str, options: SyncOptions) -> Result<SyncReport, String> {
    let pair = crate::config::get_pair(name)?;

    let (source_path, target_path) = resolve_paths(&pair);

    if !source_path.exists() {
        return Err(format!("Source path does not exist: {}", source_path.display()));
    }
    if !target_path.exists() {
        return Err(format!("Target path does not exist: {}", target_path.display()));
    }

    let ignore = IgnoreList::from_pair(&pair, &source_path);

    let source_files = walk_dir(&source_path, &ignore);
    let target_files = walk_dir(&target_path, &ignore);

    let mut report = SyncReport {
        copied: 0,
        updated: 0,
        trashed: 0,
        deleted: 0,
        skipped: 0,
        ignored: 0,
        errors: vec![],
        ops: vec![],
    };

    // Phase 1: source → target (copy new / overwrite changed)
    for (rel, src) in &source_files {
        match target_files.get(rel) {
            None => {
                // New file on source — copy to target
                if !options.dry_run {
                    let dest = target_path.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
                    copy_file(&src.abs_path, &dest, &mut report.errors);
                }
                report.copied += 1;
                report.ops.push(SyncOperation { rel_path: rel.clone(), outcome: SyncOutcome::Copied });
            }
            Some(tgt) => {
                let changed = if options.verify {
                    sha256_changed(&src.abs_path, &tgt.abs_path)
                } else {
                    src.mtime > tgt.mtime || src.size != tgt.size
                };

                if changed {
                    if !options.dry_run {
                        let dest =
                            target_path.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
                        copy_file(&src.abs_path, &dest, &mut report.errors);
                    }
                    report.updated += 1;
                    report.ops.push(SyncOperation { rel_path: rel.clone(), outcome: SyncOutcome::Updated });
                } else {
                    report.skipped += 1;
                    report.ops.push(SyncOperation { rel_path: rel.clone(), outcome: SyncOutcome::Skipped });
                }
            }
        }
    }

    // Phase 2: orphans on target (in target but not in source)
    for (rel, tgt) in &target_files {
        if source_files.contains_key(rel) {
            continue;
        }
        match pair.delete_behavior {
            DeleteBehavior::Trash => {
                if !options.dry_run {
                    trash_file(&tgt.abs_path, &target_path, &pair, &mut report.errors);
                }
                report.trashed += 1;
                report.ops.push(SyncOperation { rel_path: rel.clone(), outcome: SyncOutcome::Trashed });
            }
            DeleteBehavior::Delete => {
                if !options.dry_run {
                    if let Err(e) = std::fs::remove_file(&tgt.abs_path) {
                        report.errors.push(SyncError {
                            path: rel.clone(),
                            message: format!("Failed to delete: {}", e),
                        });
                    }
                }
                report.deleted += 1;
                report.ops.push(SyncOperation { rel_path: rel.clone(), outcome: SyncOutcome::Deleted });
            }
            DeleteBehavior::Ignore => {
                report.ignored += 1;
                report.ops.push(SyncOperation { rel_path: rel.clone(), outcome: SyncOutcome::Ignored });
            }
        }
    }

    Ok(report)
}

// ── Trash management ──────────────────────────────────────────────────────────

pub fn list_trash(name: &str) -> Result<Vec<TrashEntry>, String> {
    let pair = crate::config::get_pair(name)?;
    let (_, target_path) = resolve_paths(&pair);
    let trash_dir = target_path.join(".hard-sync-trash");

    if !trash_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = vec![];
    for entry in std::fs::read_dir(&trash_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let meta = path.metadata().map_err(|e| e.to_string())?;
        let original_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| {
                let secs = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
                chrono::DateTime::from_timestamp(secs as i64, 0)
            })
            .unwrap_or_else(Utc::now);

        entries.push(TrashEntry {
            original_name,
            trashed_at: mtime,
            size: meta.len(),
            path,
        });
    }

    // Sort newest first
    entries.sort_by(|a, b| b.trashed_at.cmp(&a.trashed_at));
    Ok(entries)
}

pub fn clear_trash(name: Option<&str>) -> Result<(), String> {
    let pairs = match name {
        Some(n) => vec![crate::config::get_pair(n)?],
        None => crate::config::list_pairs()?,
    };

    for pair in pairs {
        let (_, target_path) = resolve_paths(&pair);
        let trash_dir = target_path.join(".hard-sync-trash");
        if trash_dir.exists() {
            std::fs::remove_dir_all(&trash_dir).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn resolve_paths(pair: &PairConfig) -> (PathBuf, PathBuf) {
    match pair.source {
        SourceSide::Base => (pair.base.clone(), pair.target.clone()),
        SourceSide::Target => (pair.target.clone(), pair.base.clone()),
    }
}
