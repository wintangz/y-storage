use std::collections::HashSet;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
};
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetWindowsDirectoryW};
use windows_sys::Win32::Foundation::SYSTEMTIME;
use y_core::discovery::{normalize_path_key, normalize_windows_path, path_drive_letter};
use y_core::model::DriveInfo;

use crate::registry::read_user_shell_folders_raw;

const DRIVE_FIXED: u32 = 3;

#[must_use]
pub fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[must_use]
pub fn from_wide_slice(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[must_use]
pub fn expand_env_vars(raw: &str) -> String {
    if !raw.contains('%') {
        return raw.to_string();
    }
    let wide = to_wide_null(raw);
    let needed = unsafe { ExpandEnvironmentStringsW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return raw.to_string();
    }
    let mut buf = vec![0u16; needed as usize];
    let written =
        unsafe { ExpandEnvironmentStringsW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
    if written == 0 {
        return raw.to_string();
    }
    from_wide_slice(&buf)
}

#[must_use]
pub fn get_windows_directory() -> String {
    let mut buf = [0u16; 260];
    let len = unsafe { GetWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 || len as usize >= buf.len() {
        return "C:\\Windows".to_string();
    }
    normalize_windows_path(&String::from_utf16_lossy(&buf[..len as usize]))
}

#[must_use]
pub fn enumerate_fixed_drives() -> Vec<DriveInfo> {
    let win_dir = get_windows_directory();
    let sys_letter = path_drive_letter(&win_dir).unwrap_or('C');
    let mask = unsafe { GetLogicalDrives() };
    let mut drives = Vec::new();

    for idx in 0..26u32 {
        if (mask & (1 << idx)) == 0 {
            continue;
        }
        let letter = (b'A' + idx as u8) as char;
        let root = format!("{letter}:\\");
        let root_w = to_wide_null(&root);
        let dtype = unsafe { GetDriveTypeW(root_w.as_ptr()) };
        if dtype != DRIVE_FIXED {
            continue;
        }
        let mut free_avail: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free: u64 = 0;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                root_w.as_ptr(),
                &mut free_avail,
                &mut total_bytes,
                &mut total_free,
            )
        };
        if ok == 0 {
            continue;
        }
        let is_sys = letter.eq_ignore_ascii_case(&sys_letter);
        let exclusions = if is_sys {
            vec![win_dir.clone()]
        } else {
            Vec::new()
        };
        drives.push(DriveInfo {
            letter,
            root,
            total_bytes,
            free_bytes: total_free,
            used_bytes: total_bytes.saturating_sub(total_free),
            is_system_drive: is_sys,
            full_walk_enabled: is_sys,
            exclusions,
        });
    }
    drives
}

#[must_use]
pub fn resolve_personal_shell_folders(drives: &[DriveInfo]) -> Vec<String> {
    let mut folders = Vec::new();
    let mut seen = HashSet::new();

    let mut push_folder = |raw: &str| {
        let expanded = expand_env_vars(raw);
        let norm = normalize_windows_path(&expanded);
        if norm.len() > 3 && Path::new(&norm).exists() {
            let key = normalize_path_key(&norm);
            if seen.insert(key) {
                folders.push(norm);
            }
        }
    };

    for raw in read_user_shell_folders_raw() {
        push_folder(&raw);
    }

    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let p = profile.to_string_lossy();
        for name in [
            "Desktop",
            "Documents",
            "Downloads",
            "Music",
            "Pictures",
            "Videos",
            "OneDrive",
            "Saved Games",
        ] {
            push_folder(&format!("{p}\\{name}"));
        }
    }

    if let Some(od) = std::env::var_os("OneDrive") {
        push_folder(&od.to_string_lossy());
    }

    // Also include standard personal shell folders for any other user profiles on system drives.
    for drv in drives.iter().filter(|d| d.is_system_drive) {
        let users_dir = format!("{}Users", drv.root);
        if let Ok(rd) = std::fs::read_dir(&users_dir) {
            for ent in rd.flatten() {
                if let Ok(ft) = ent.file_type() {
                    if ft.is_dir() && !ft.is_symlink() {
                        let base = ent.path();
                        for name in [
                            "Desktop",
                            "Documents",
                            "Downloads",
                            "Music",
                            "Pictures",
                            "Videos",
                            "OneDrive",
                            "Saved Games",
                        ] {
                            let cand = base.join(name);
                            if cand.exists() {
                                push_folder(&cand.to_string_lossy());
                            }
                        }
                    }
                }
            }
        }
    }

    folders
}

#[must_use]
pub fn current_local_timestamp() -> (u64, String) {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    let display = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    );
    (unix, display)
}

#[must_use]
pub fn has_game_manifest_dir(dir: &str) -> bool {
    let p = std::path::Path::new(dir);
    if p.join(".egstore").is_dir() {
        return true;
    }
    if let Ok(rd) = std::fs::read_dir(p) {
        for ent in rd.flatten() {
            let n = ent.file_name().to_string_lossy().to_ascii_lowercase();
            if n.starts_with("goggame-") && n.ends_with(".info") {
                return true;
            }
        }
    }
    false
}
