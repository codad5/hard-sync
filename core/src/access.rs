//! Pre-flight access checks.
//!
//! A sync reads from the source side and writes to the other side. Before any
//! file is touched we make sure the OS actually allows that, so an unusable
//! drive fails once, up front, with a reason a person can act on — instead of
//! producing one cryptic error per file halfway through a sync.
//!
//! The classic case: an NTFS drive on macOS is mounted read-only, so it can be
//! a source but never a target.

use std::fmt;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::config::{PairConfig, SourceSide};

/// File created (and immediately removed) in the target to prove it is writable.
/// Listed in the ignore built-ins so a leftover is never synced or trashed.
pub(crate) const WRITE_PROBE_PREFIX: &str = ".hard-sync-write-test";

/// Why a path can't be used for the role it has in a pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    /// The path does not exist (e.g. drive unplugged, folder deleted).
    Missing { path: PathBuf },
    /// The path exists but is a file, not a folder.
    NotADirectory { path: PathBuf },
    /// The source can't be read (permissions, macOS privacy protection, ...).
    NotReadable { path: PathBuf, reason: String },
    /// The target lives on a filesystem mounted read-only (e.g. NTFS on macOS).
    ReadOnlyFilesystem { path: PathBuf },
    /// The target is on a writable filesystem but this folder can't be written.
    NotWritable { path: PathBuf, reason: String },
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccessError::Missing { path } => {
                write!(f, "{} does not exist. If it's on a removable drive, make sure the drive is plugged in.", path.display())
            }
            AccessError::NotADirectory { path } => {
                write!(f, "{} is a file, not a folder.", path.display())
            }
            AccessError::NotReadable { path, reason } => {
                write!(f, "Can't read from {}: {}.", path.display(), reason)?;
                if cfg!(target_os = "macos") {
                    write!(f, " If this folder is in Desktop, Documents, Downloads or on an external drive, allow access in System Settings → Privacy & Security → Files and Folders.")?;
                }
                Ok(())
            }
            AccessError::ReadOnlyFilesystem { path } => {
                write!(f, "Can't write to {}: the drive is mounted read-only.", path.display())?;
                if cfg!(target_os = "macos") {
                    write!(f, " macOS can read NTFS (Windows-formatted) drives but not write to them. Use an exFAT or APFS drive, install an NTFS driver, or make this drive the source instead.")?;
                } else {
                    write!(f, " Remount it read-write, or make this drive the source instead.")?;
                }
                Ok(())
            }
            AccessError::NotWritable { path, reason } => {
                write!(f, "Can't write to {}: {}.", path.display(), reason)
            }
        }
    }
}

impl std::error::Error for AccessError {}

/// How thorough the writability check is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteCheck {
    /// Only ask the OS whether the filesystem is mounted read-only.
    /// Touches nothing — safe for dry runs.
    MountOnly,
    /// Also create and delete a small probe file. Catches permission problems
    /// too, but writes to the target.
    Probe,
}

/// Check that `path` is an existing folder the OS lets us list.
pub fn check_readable(path: &Path) -> Result<(), AccessError> {
    check_is_dir(path)?;
    match std::fs::read_dir(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(AccessError::NotReadable { path: path.to_path_buf(), reason: describe(&e) }),
    }
}

/// Check that `path` is an existing folder the OS lets us write into.
pub fn check_writable(path: &Path, mode: WriteCheck) -> Result<(), AccessError> {
    check_is_dir(path)?;

    if is_read_only_mount(path) {
        return Err(AccessError::ReadOnlyFilesystem { path: path.to_path_buf() });
    }

    if mode == WriteCheck::Probe {
        let probe = path.join(format!("{}-{}", WRITE_PROBE_PREFIX, std::process::id()));
        let result = std::fs::OpenOptions::new().write(true).create_new(true).open(&probe);
        match result {
            Ok(file) => {
                drop(file);
                let _ = std::fs::remove_file(&probe);
            }
            // Leftover from a crashed run with the same pid: the folder is writable.
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) if e.kind() == ErrorKind::ReadOnlyFilesystem => {
                return Err(AccessError::ReadOnlyFilesystem { path: path.to_path_buf() });
            }
            Err(e) => {
                return Err(AccessError::NotWritable { path: path.to_path_buf(), reason: describe(&e) });
            }
        }
    }

    Ok(())
}

/// Check both sides of a pair for the roles they have *right now*:
/// the source must be readable, the other side must be writable.
pub fn check_pair_access(pair: &PairConfig, mode: WriteCheck) -> Result<(), AccessError> {
    let (source, target) = match pair.source {
        SourceSide::Base => (&pair.base, &pair.target),
        SourceSide::Target => (&pair.target, &pair.base),
    };
    check_readable(source)?;
    check_writable(target, mode)
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn check_is_dir(path: &Path) -> Result<(), AccessError> {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(AccessError::NotADirectory { path: path.to_path_buf() }),
        Err(e) if e.kind() == ErrorKind::NotFound => Err(AccessError::Missing { path: path.to_path_buf() }),
        Err(e) => Err(AccessError::NotReadable { path: path.to_path_buf(), reason: describe(&e) }),
    }
}

/// Human-readable io::Error without the "(os error N)" suffix.
fn describe(e: &io::Error) -> String {
    match e.kind() {
        ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => {
            let s = e.to_string();
            match s.find(" (os error") {
                Some(i) => s[..i].to_lowercase(),
                None => s,
            }
        }
    }
}

/// Ask the OS whether the filesystem holding `path` is mounted read-only.
/// Returns false when it can't tell — the write probe is the fallback.
#[cfg(unix)]
fn is_read_only_mount(path: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: c_path is a valid NUL-terminated string and stat is a valid out-pointer.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    rc == 0 && (stat.f_flag & libc::ST_RDONLY) != 0
}

/// Windows: no cheap read-only-mount query without more FFI; rely on the write
/// probe (always used for real syncs, add_pair and set_source).
#[cfg(not(unix))]
fn is_read_only_mount(_path: &Path) -> bool {
    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DeleteBehavior, SoundConfig};

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hsync-access-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn pair(base: &Path, target: &Path, source: SourceSide) -> PairConfig {
        PairConfig {
            name: "t".into(),
            base: base.to_path_buf(),
            target: target.to_path_buf(),
            source,
            drive_id: None,
            ignore: vec![],
            delete_behavior: DeleteBehavior::Trash,
            sounds: SoundConfig::default(),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn writable_folder_passes_and_leaves_no_probe() {
        let d = temp_dir("ok");
        assert_eq!(check_writable(&d, WriteCheck::Probe), Ok(()));
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0, "probe file must be removed");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_and_file_paths_are_rejected() {
        let d = temp_dir("missing");
        let missing = d.join("nope");
        assert!(matches!(check_readable(&missing), Err(AccessError::Missing { .. })));
        let file = d.join("f.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(matches!(check_writable(&file, WriteCheck::MountOnly), Err(AccessError::NotADirectory { .. })));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn read_only_folder_is_not_writable_but_still_readable() {
        use std::os::unix::fs::PermissionsExt;
        let d = temp_dir("ro");
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root ignores permission bits; nothing meaningful to assert then.
        if unsafe { libc::geteuid() } != 0 {
            assert!(matches!(check_writable(&d, WriteCheck::Probe), Err(AccessError::NotWritable { .. })));
            // MountOnly doesn't write, so it can't see permission problems.
            assert_eq!(check_writable(&d, WriteCheck::MountOnly), Ok(()));
            assert_eq!(check_readable(&d), Ok(()));
        }
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn pair_roles_follow_the_source_side() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let base = temp_dir("pair-base");
        let target = temp_dir("pair-target");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o555)).unwrap();

        // target is read-only: can't be written to...
        assert!(check_pair_access(&pair(&base, &target, SourceSide::Base), WriteCheck::Probe).is_err());
        // ...but is fine as the source, with base receiving the files.
        assert_eq!(check_pair_access(&pair(&base, &target, SourceSide::Target), WriteCheck::Probe), Ok(()));

        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&target);
    }

    #[test]
    fn read_only_message_explains_the_cause() {
        let msg = AccessError::ReadOnlyFilesystem { path: "/Volumes/USB".into() }.to_string();
        assert!(msg.contains("mounted read-only"), "{msg}");
    }
}
