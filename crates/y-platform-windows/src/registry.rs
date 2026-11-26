use std::collections::HashSet;

use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_DWORD, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
};
use y_core::model::RawRegistryApp;

use crate::paths::{expand_env_vars, from_wide_slice, to_wide_null};

struct RegKeyGuard(HKEY);

impl Drop for RegKeyGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { RegCloseKey(self.0) };
        }
    }
}

fn open_key(root: HKEY, subkey: &str, flags: u32) -> Option<RegKeyGuard> {
    let w = to_wide_null(subkey);
    let mut handle: HKEY = std::ptr::null_mut();
    let res = unsafe { RegOpenKeyExW(root, w.as_ptr(), 0, flags, &mut handle) };
    if res == ERROR_SUCCESS && !handle.is_null() {
        Some(RegKeyGuard(handle))
    } else {
        None
    }
}

fn query_string(hkey: HKEY, value_name: &str) -> Option<String> {
    let wname = to_wide_null(value_name);
    let mut reg_type: u32 = 0;
    let mut byte_len: u32 = 0;
    let res = unsafe {
        RegQueryValueExW(
            hkey,
            wname.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            std::ptr::null_mut(),
            &mut byte_len,
        )
    };
    if res != ERROR_SUCCESS || byte_len == 0 {
        return None;
    }
    if reg_type != REG_SZ && reg_type != REG_EXPAND_SZ {
        return None;
    }
    let word_len = (byte_len as usize).div_ceil(2);
    let mut buf = vec![0u16; word_len];
    let mut actual_len = (word_len * 2) as u32;
    let res2 = unsafe {
        RegQueryValueExW(
            hkey,
            wname.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            buf.as_mut_ptr().cast::<u8>(),
            &mut actual_len,
        )
    };
    if res2 != ERROR_SUCCESS {
        return None;
    }
    let s = from_wide_slice(&buf);
    if reg_type == REG_EXPAND_SZ {
        Some(expand_env_vars(&s))
    } else {
        Some(s)
    }
}

fn query_dword(hkey: HKEY, value_name: &str) -> Option<u32> {
    let wname = to_wide_null(value_name);
    let mut reg_type: u32 = 0;
    let mut val: u32 = 0;
    let mut byte_len: u32 = std::mem::size_of::<u32>() as u32;
    let res = unsafe {
        RegQueryValueExW(
            hkey,
            wname.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            (&mut val as *mut u32).cast::<u8>(),
            &mut byte_len,
        )
    };
    if res == ERROR_SUCCESS && reg_type == REG_DWORD {
        Some(val)
    } else {
        None
    }
}

fn enum_uninstall_view(
    root: HKEY,
    subkey_path: &str,
    sam: u32,
    view_label: &str,
    seen_dedup: &mut HashSet<String>,
    out: &mut Vec<RawRegistryApp>,
) {
    let Some(parent) = open_key(root, subkey_path, KEY_READ | sam) else {
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
        let child_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let dedup_key = format!(
            "{}:{}:{}",
            if root == HKEY_LOCAL_MACHINE {
                "HKLM"
            } else {
                "HKCU"
            },
            if (sam & KEY_WOW64_32KEY) != 0 || subkey_path.contains("WOW6432Node") {
                "32"
            } else {
                "64"
            },
            child_name.to_ascii_lowercase()
        );
        if !seen_dedup.insert(dedup_key) {
            continue;
        }

        let Some(child) = open_key(parent.0, &child_name, KEY_READ | sam) else {
            continue;
        };

        let display_name = query_string(child.0, "DisplayName").unwrap_or_default();
        if display_name.trim().is_empty() {
            continue;
        }
        let publisher = query_string(child.0, "Publisher").unwrap_or_default();
        let install_location = query_string(child.0, "InstallLocation").unwrap_or_default();
        let uninstall_string = query_string(child.0, "UninstallString")
            .or_else(|| query_string(child.0, "QuietUninstallString"))
            .unwrap_or_default();
        let display_icon = query_string(child.0, "DisplayIcon").unwrap_or_default();
        let system_component = query_dword(child.0, "SystemComponent").unwrap_or(0) == 1;
        let parent_key_name = query_string(child.0, "ParentKeyName").unwrap_or_default();
        let release_type = query_string(child.0, "ReleaseType").unwrap_or_default();

        out.push(RawRegistryApp {
            hive_view: view_label.to_string(),
            subkey_name: child_name,
            display_name,
            publisher,
            install_location,
            uninstall_string,
            display_icon,
            system_component,
            parent_key_name,
            release_type,
        });
    }
}

#[must_use]
pub fn enumerate_uninstall_registry() -> Vec<RawRegistryApp> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const UNINSTALL_WOW32: &str =
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

    enum_uninstall_view(
        HKEY_LOCAL_MACHINE,
        UNINSTALL_PATH,
        KEY_WOW64_64KEY,
        "HKLM-64",
        &mut seen,
        &mut out,
    );
    enum_uninstall_view(
        HKEY_LOCAL_MACHINE,
        UNINSTALL_PATH,
        KEY_WOW64_32KEY,
        "HKLM-32",
        &mut seen,
        &mut out,
    );
    enum_uninstall_view(
        HKEY_CURRENT_USER,
        UNINSTALL_PATH,
        KEY_WOW64_64KEY,
        "HKCU-64",
        &mut seen,
        &mut out,
    );
    enum_uninstall_view(
        HKEY_CURRENT_USER,
        UNINSTALL_PATH,
        KEY_WOW64_32KEY,
        "HKCU-32",
        &mut seen,
        &mut out,
    );
    enum_uninstall_view(
        HKEY_CURRENT_USER,
        UNINSTALL_WOW32,
        KEY_WOW64_64KEY,
        "HKCU-WOW32",
        &mut seen,
        &mut out,
    );

    out
}

#[must_use]
pub fn read_steam_install_path() -> Option<String> {
    let key = open_key(HKEY_CURRENT_USER, r"Software\Valve\Steam", KEY_READ)?;
    query_string(key.0, "SteamPath").filter(|s| !s.trim().is_empty())
}

#[must_use]
pub fn read_user_shell_folders_raw() -> Vec<String> {
    let Some(key) = open_key(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders",
        KEY_READ,
    ) else {
        return Vec::new();
    };

    let names = [
        "Personal",                               // Documents
        "{374DE290-123F-4565-9164-39C4925E467B}", // Downloads
        "{7D83EE9B-2244-4E70-B1F5-5393042AF1E4}", // Downloads (localized fallback)
        "My Pictures",
        "My Music",
        "My Video",
        "Desktop",
        "{4C5C32FF-BB9D-43B0-B5B4-2D72E54EAAA4}", // Saved Games
    ];
    let mut out = Vec::new();
    for name in names {
        if let Some(val) = query_string(key.0, name) {
            if !val.trim().is_empty() {
                out.push(val);
            }
        }
    }
    out
}
