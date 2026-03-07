use std::path::{Path, PathBuf};

use sysinfo::Disks;

use crate::config::DriveId;

// ── Public types ──────────────────────────────────────────────────────────────

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
    let canonical = path.canonicalize().ok()?;

    let disk = disks
        .iter()
        .filter(|d| canonical.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())?;

    let raw_name = disk.name().to_string_lossy().to_string();
    let label = if raw_name.is_empty() { None } else { Some(raw_name) };
    let uuid = get_volume_uuid(disk.mount_point());

    // If we can't identify the drive at all, don't store a DriveId
    if label.is_none() && uuid.is_none() {
        return None;
    }

    Some(DriveId { label, uuid })
}

/// Poll currently mounted drives and return the mount point of the first drive
/// that matches the stored DriveId (by label OR uuid).
/// Called in watch mode for cross-drive pairs.
pub fn find_mounted_drive(id: &DriveId) -> Option<PathBuf> {
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.iter() {
        let name = disk.name().to_string_lossy().to_string();
        let disk_uuid = get_volume_uuid(disk.mount_point());

        let label_match = id
            .label
            .as_ref()
            .map(|l| !l.is_empty() && l == &name)
            .unwrap_or(false);

        let uuid_match = id
            .uuid
            .as_ref()
            .zip(disk_uuid.as_ref())
            .map(|(a, b)| a == b)
            .unwrap_or(false);

        if label_match || uuid_match {
            return Some(disk.mount_point().to_path_buf());
        }
    }

    None
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn find_mount_point(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|d| canonical.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.mount_point().to_path_buf())
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

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn get_volume_uuid(_mount_point: &Path) -> Option<String> {
    None
}
