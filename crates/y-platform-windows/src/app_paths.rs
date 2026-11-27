use std::collections::HashSet;

use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
};
use y_core::discovery::{normalize_path_key, normalize_windows_path};

use crate::paths::{expand_env_vars, from_wide_slice, to_wide_null};

const APP_PATHS_SUBKEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPathEntry {
    pub exe_name: String,
    pub target_path: String,
}

struct RegGuard(HKEY);

impl Drop for RegGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
}

fn open_key(root: HKEY, subkey: &str, flags: u32) -> Option<RegGuard> {
    let w = to_wide_null(subkey);
    let mut handle: HKEY = std::ptr::null_mut();
    let res = unsafe { RegOpenKeyExW(root, w.as_ptr(), 0, flags, &mut handle) };
    if res == ERROR_SUCCESS && !handle.is_null() {
        Some(RegGuard(handle))
    } else {
        None
    }
}

fn query_default_string(hkey: HKEY) -> Option<String> {
    let empty_name = [0u16];
    let mut reg_type: u32 = 0;
    let mut byte_len: u32 = 0;
    let res = unsafe {
        RegQueryValueExW(
            hkey,
            empty_name.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            std::ptr::null_mut(),
            &mut byte_len,
        )
    };
    if res != ERROR_SUCCESS || byte_len == 0 || (reg_type != REG_SZ && reg_type != REG_EXPAND_SZ) {
        return None;
    }
    let words = (byte_len as usize).div_ceil(2);
    let mut buf = vec![0u16; words];
    let mut actual = (words * 2) as u32;
    let res2 = unsafe {
        RegQueryValueExW(
            hkey,
            empty_name.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            buf.as_mut_ptr().cast::<u8>(),
            &mut actual,
        )
    };
    if res2 != ERROR_SUCCESS {
        return None;
    }
    let s = from_wide_slice(&buf);
    Some(expand_env_vars(&s))
}

fn clean_app_path_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let unquoted = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest).trim()
    } else {
        trimmed
    };
    if unquoted.is_empty() {
        return None;
    }
    let expanded = expand_env_vars(unquoted);
    let norm = normalize_windows_path(expanded.trim());
    if norm.is_empty() {
        None
    } else {
        Some(norm)
    }
}

fn collect_app_paths_from_root(
    root: HKEY,
    flags: u32,
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<AppPathEntry>,
) {
    let Some(parent) = open_key(root, APP_PATHS_SUBKEY, flags) else {
        return;
    };
    let mut index: u32 = 0;
    let mut name_buf = [0u16; 512];

    loop {
        let mut name_len = name_buf.len() as u32;
        let res = unsafe {
            RegEnumKeyExW(
                parent.0,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        index += 1;
        if res == ERROR_NO_MORE_ITEMS {
            break;
        }
        if res != ERROR_SUCCESS {
            continue;
        }
        let exe_name = String::from_utf16_lossy(&name_buf[..name_len as usize])
            .trim()
            .to_string();
        if exe_name.is_empty() {
            continue;
        }
        let Some(child) = open_key(
            root,
            &format!("{APP_PATHS_SUBKEY}\\{exe_name}"),
            flags,
        ) else {
            continue;
        };
        let Some(raw_default) = query_default_string(child.0) else {
            continue;
        };
        let Some(target_path) = clean_app_path_value(&raw_default) else {
            continue;
        };
        let key = (exe_name.to_ascii_lowercase(), normalize_path_key(&target_path));
        if seen.insert(key) {
            out.push(AppPathEntry {
                exe_name,
                target_path,
            });
        }
    }
}

/// Enumerates Windows `App Paths` registrations from `HKLM` (64-bit & 32-bit views)
/// and `HKCU`, mapping executable names to normalized target paths.
#[must_use]
pub fn enumerate_app_paths() -> Vec<AppPathEntry> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    collect_app_paths_from_root(
        HKEY_LOCAL_MACHINE,
        KEY_READ | KEY_WOW64_64KEY,
        &mut seen,
        &mut out,
    );
    collect_app_paths_from_root(
        HKEY_LOCAL_MACHINE,
        KEY_READ | KEY_WOW64_32KEY,
        &mut seen,
        &mut out,
    );
    collect_app_paths_from_root(HKEY_CURRENT_USER, KEY_READ, &mut seen, &mut out);

    out.sort_by(|a, b| {
        a.exe_name
            .to_ascii_lowercase()
            .cmp(&b.exe_name.to_ascii_lowercase())
            .then_with(|| {
                normalize_path_key(&a.target_path).cmp(&normalize_path_key(&b.target_path))
            })
    });
    out
}
