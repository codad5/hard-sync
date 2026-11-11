use std::path::{Path, PathBuf};

use sysinfo::Disks;

use crate::config::DriveId;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectedDrive {
    pub name: String,
    pub mount_point: PathBuf,
    pub is_removable: bool,
    pub total_space: u64,
    pub available_space: u64,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// List all currently connected drives on the system.
pub fn list_connected_drives() -> Vec<ConnectedDrive> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .map(|d| ConnectedDrive {
            name: d.name().to_string_lossy().to_string(),
            mount_point: d.mount_point().to_path_buf(),
            is_removable: d.is_removable(),
            total_space: d.total_space(),
            available_space: d.available_space(),
        })
        .collect()
}

/// Returns true if both paths live on the same physical drive / mount point.
/// Used on init to decide whether drive_id should be stored.
pub fn same_drive(a: &Path, b: &Path) -> bool {
    let mount_a = find_mount_point(a);
    let mount_b = find_mount_point(b);
    match (mount_a, mount_b) {
        (Some(ma), Some(mb)) => ma == mb,
        // Fallback: compare root prefixes (drive letter on Windows, "/" on Linux)
        _ => root_of(a) == root_of(b),
    }
}

/// Given a path, detect the drive it lives on and return its DriveId.
/// Returns None if the drive cannot be identified (e.g. network share).
pub fn get_drive_id(path: &Path) -> Option<DriveId> {
    let disks = Disks::new_with_refreshed_list();
    let canonical = strip_unc_prefix(&path.canonicalize().ok()?);

    let disk = disks
        .iter()
        .filter(|d| canonical.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())?;

    let label = volume_label(disk);
    let uuid = get_volume_uuid(disk.mount_point());

    // If we can't identify the drive at all, don't store a DriveId
    if label.is_none() && uuid.is_none() {
        return None;
    }

    Some(DriveId { label, uuid })
}

/// Poll currently mounted drives and return the mount point of the first drive
/// that matches the stored DriveId. See [`drive_matches`] for the rules.
/// Called in watch mode for cross-drive pairs.
pub fn find_mounted_drive(id: &DriveId) -> Option<PathBuf> {
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.iter() {
        let name = disk.name().to_string_lossy().to_string();
        // Only look up the UUID when the pair has one to compare against —
        // on macOS this shells out to `diskutil`, and watch polls every few seconds.
        let disk_uuid = if id.uuid.is_some() && may_be_external(disk.mount_point()) {
            get_volume_uuid(disk.mount_point())
        } else {
            None
        };

        if drive_matches(id, &name, disk_uuid.as_deref()) {
            return Some(disk.mount_point().to_path_buf());
        }
    }

    None
}

/// True if a drive from [`list_connected_drives`] is the one `id` was set up with.
/// Same rules as watch mode, so `hsync drives` and the UI agree with `watch`.
pub fn drive_matches_connected(id: &DriveId, drive: &ConnectedDrive) -> bool {
    let disk_uuid = if id.uuid.is_some() && may_be_external(&drive.mount_point) {
        get_volume_uuid(&drive.mount_point)
    } else {
        None
    };
    drive_matches(id, &drive.name, disk_uuid.as_deref())
}

/// Decide whether a mounted drive is the one a pair was set up with.
///
/// - If both the pair and the drive have a UUID, the UUID decides on its own.
///   Labels are not unique ("Untitled", "USB DRIVE"), and a drive keeps its UUID
///   when renamed.
/// - Otherwise fall back to an exact, non-empty label match.
fn drive_matches(id: &DriveId, disk_label: &str, disk_uuid: Option<&str>) -> bool {
    if let (Some(want), Some(have)) = (id.uuid.as_deref(), disk_uuid) {
        return want.eq_ignore_ascii_case(have);
    }
    id.label
        .as_deref()
        .map(|l| !l.is_empty() && l == disk_label)
        .unwrap_or(false)
}

/// The drive's real volume label, or None if it has none.
///
/// On macOS, unnamed volumes are mounted as "Untitled" and sysinfo reports that
/// placeholder as the name, so ask `diskutil` for the actual label instead.
fn volume_label(disk: &sysinfo::Disk) -> Option<String> {
    #[cfg(target_os = "macos")]
    if let Some(info) = macos::diskutil_info(disk.mount_point()) {
        return info.name;
    }

    let raw = disk.name().to_string_lossy().to_string();
    if raw.is_empty() { None } else { Some(raw) }
}

/// On macOS, external drives mount under /Volumes. Skipping the system volumes
/// ("/", "/System/Volumes/...") avoids pointless `diskutil` calls while polling.
fn may_be_external(mount_point: &Path) -> bool {
    if cfg!(target_os = "macos") {
        mount_point.starts_with("/Volumes")
    } else {
        true
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn find_mount_point(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    // On Windows, canonicalize() prepends \\?\ which breaks starts_with()
    // comparisons against plain drive-letter paths returned by sysinfo.
    let canonical = strip_unc_prefix(&canonical);
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|d| canonical.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.mount_point().to_path_buf())
}

/// Strip the Windows extended-length path prefix `\\?\` if present.
/// On other platforms this is a no-op.
fn strip_unc_prefix(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

fn root_of(path: &Path) -> Option<std::path::Component<'_>> {
    path.components().next()
}

// ── OS-specific UUID extraction ───────────────────────────────────────────────
//
// v1: best-effort UUID detection per platform.
// Label-based matching is the primary mechanism and works on all platforms.
// UUID adds disambiguation when two drives have the same label.
//
// Windows: volume GUID via GetVolumeNameForVolumeMountPointW
// Linux:   symlink resolution in /dev/disk/by-uuid/
// macOS:   "Volume UUID" from `diskutil info`
// Other:   None (label-only matching)

#[cfg(target_os = "windows")]
fn get_volume_uuid(mount_point: &Path) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let mut mount_wide: Vec<u16> = mount_point.as_os_str().encode_wide().collect();
    // Must end with backslash
    if mount_wide.last() != Some(&(b'\\' as u16)) {
        mount_wide.push(b'\\' as u16);
    }
    mount_wide.push(0); // null terminator

    let mut guid_buf = vec![0u16; 64];

    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW(
            mount_wide.as_ptr(),
            guid_buf.as_mut_ptr(),
            guid_buf.len() as u32,
        )
    };

    if ok == 0 {
        return None;
    }

    let end = guid_buf.iter().position(|&c| c == 0).unwrap_or(guid_buf.len());
    let raw = OsString::from_wide(&guid_buf[..end])
        .to_string_lossy()
        .to_string();

    // Format: \\?\Volume{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\
    // Extract the GUID portion only
    let guid = raw
        .trim_start_matches(r"\\?\Volume{")
        .trim_end_matches(r"}\")
        .to_string();

    if guid.is_empty() || guid == raw {
        None
    } else {
        Some(guid)
    }
}

#[cfg(target_os = "linux")]
fn get_volume_uuid(mount_point: &Path) -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    let mount_str = mount_point.to_string_lossy();

    // Find the device path for this mount point
    let device = mounts.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let dev = parts.next()?;
        let mp = parts.next()?;
        if mp == mount_str.as_ref() {
            Some(dev.to_string())
        } else {
            None
        }
    })?;

    // Resolve UUID symlinks in /dev/disk/by-uuid/
    let by_uuid = Path::new("/dev/disk/by-uuid");
    if !by_uuid.exists() {
        return None;
    }

    for entry in std::fs::read_dir(by_uuid).ok()?.flatten() {
        if let Ok(link) = std::fs::read_link(entry.path()) {
            let resolved = by_uuid.join(&link);
            if let Ok(canonical) = resolved.canonicalize() {
                if canonical == Path::new(&device) {
                    return Some(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn get_volume_uuid(mount_point: &Path) -> Option<String> {
    macos::diskutil_info(mount_point)?.uuid
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn get_volume_uuid(_mount_point: &Path) -> Option<String> {
    None
}

/// macOS volume info via `diskutil info <mount point>` — ships with every macOS,
/// no extra dependencies or permissions needed.
#[cfg(target_os = "macos")]
mod macos {
    use std::path::Path;
    use std::process::Command;

    pub fn diskutil_info(mount_point: &Path) -> Option<super::VolumeInfo> {
        let out = Command::new("/usr/sbin/diskutil")
            .arg("info")
            .arg(mount_point)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(super::parse_diskutil_info(&String::from_utf8_lossy(&out.stdout)))
    }
}

/// Label and UUID of a volume. `name: None` means the volume has no label.
#[derive(Debug, PartialEq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct VolumeInfo {
    name: Option<String>,
    uuid: Option<String>,
}

/// Parse the "Volume Name:" and "Volume UUID:" lines of `diskutil info` output.
/// Kept platform-independent so it is unit-tested everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_diskutil_info(text: &str) -> VolumeInfo {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.trim_start().strip_prefix(key))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    VolumeInfo {
        name: field("Volume Name:"),
        uuid: field("Volume UUID:"),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{drive_matches, parse_diskutil_info, strip_unc_prefix, VolumeInfo};
    use crate::config::DriveId;
    use std::path::{Path, PathBuf};

    fn id(label: Option<&str>, uuid: Option<&str>) -> DriveId {
        DriveId { label: label.map(String::from), uuid: uuid.map(String::from) }
    }

    #[test]
    fn uuid_decides_when_both_sides_have_one() {
        let pair = id(Some("USB"), Some("AAAA-1111"));
        assert!(drive_matches(&pair, "USB", Some("AAAA-1111")));
        assert!(drive_matches(&pair, "Renamed", Some("aaaa-1111")), "rename keeps the match");
        assert!(!drive_matches(&pair, "USB", Some("BBBB-2222")), "same label, other drive");
    }

    #[test]
    fn label_is_the_fallback_without_uuid() {
        assert!(drive_matches(&id(Some("USB"), None), "USB", Some("BBBB-2222")));
        assert!(drive_matches(&id(Some("USB"), Some("AAAA-1111")), "USB", None));
        assert!(!drive_matches(&id(Some("USB"), None), "OTHER", None));
        assert!(!drive_matches(&id(None, Some("AAAA-1111")), "", None));
        assert!(!drive_matches(&id(Some(""), None), "", None), "empty label never matches");
    }

    #[test]
    fn parses_diskutil_info() {
        let named = "   Device Node:  /dev/disk6s2\n   Volume Name:               MY_USB\n   Mounted:  Yes\n   Volume UUID:               0B0F4412-BC55-4A04-A753-A80B0926489B\n";
        assert_eq!(
            parse_diskutil_info(named),
            VolumeInfo { name: Some("MY_USB".into()), uuid: Some("0B0F4412-BC55-4A04-A753-A80B0926489B".into()) }
        );
        // Unnamed volume (mounted as "Untitled") and no UUID (e.g. FAT32)
        let unnamed = "   Volume Name:               \n   Mount Point:  /Volumes/Untitled\n";
        assert_eq!(parse_diskutil_info(unnamed), VolumeInfo { name: None, uuid: None });
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn strip_unc_removes_windows_prefix() {
        assert_eq!(
            strip_unc_prefix(Path::new(r"\\?\C:\foo\bar")),
            PathBuf::from(r"C:\foo\bar")
        );
        assert_eq!(
            strip_unc_prefix(Path::new(r"\\?\E:\hard-sync-target")),
            PathBuf::from(r"E:\hard-sync-target")
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn strip_unc_is_noop_for_plain_paths() {
        assert_eq!(
            strip_unc_prefix(Path::new(r"C:\foo\bar")),
            PathBuf::from(r"C:\foo\bar")
        );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn strip_unc_is_noop_on_non_windows() {
        assert_eq!(
            strip_unc_prefix(Path::new("/home/user/foo")),
            PathBuf::from("/home/user/foo")
        );
    }
}
