use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_SZ,
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
};
use y_core::dependencies::WindowsServiceEntry;
use y_core::discovery::normalize_windows_path;

use crate::paths::{expand_env_vars, from_wide_slice, get_windows_directory, to_wide_null};

struct RegGuard(HKEY);

impl Drop for RegGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { RegCloseKey(self.0) };
        }
    }
}

fn open_key(root: HKEY, subkey: &str) -> Option<RegGuard> {
    let w = to_wide_null(subkey);
    let mut handle: HKEY = std::ptr::null_mut();
    let res = unsafe { RegOpenKeyExW(root, w.as_ptr(), 0, KEY_READ, &mut handle) };
    if res == ERROR_SUCCESS && !handle.is_null() {
        Some(RegGuard(handle))
    } else {
        None
    }
}

fn query_string(hkey: HKEY, name: &str) -> Option<String> {
    let wname = to_wide_null(name);
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
    if res != ERROR_SUCCESS || byte_len == 0 || (reg_type != REG_SZ && reg_type != REG_EXPAND_SZ) {
        return None;
    }
    let words = (byte_len as usize).div_ceil(2);
    let mut buf = vec![0u16; words];
    let mut actual = (words * 2) as u32;
    let res2 = unsafe {
        RegQueryValueExW(
            hkey,
            wname.as_ptr(),
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
    Some(if reg_type == REG_EXPAND_SZ {
        expand_env_vars(&s)
    } else {
        s
    })
}

fn query_multi_sz(hkey: HKEY, name: &str) -> Vec<String> {
    let wname = to_wide_null(name);
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
    if res != ERROR_SUCCESS || byte_len == 0 || reg_type != REG_MULTI_SZ {
        return Vec::new();
    }
    let words = (byte_len as usize).div_ceil(2);
    let mut buf = vec![0u16; words];
    let mut actual = (words * 2) as u32;
    let res2 = unsafe {
        RegQueryValueExW(
            hkey,
            wname.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            buf.as_mut_ptr().cast::<u8>(),
            &mut actual,
        )
    };
    if res2 != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut out = Vec::new();
    for slice in buf.split(|&c| c == 0) {
        if !slice.is_empty() {
            let s = String::from_utf16_lossy(slice).trim().to_string();
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    out
}

fn query_dword(hkey: HKEY, name: &str) -> Option<u32> {
    let wname = to_wide_null(name);
    let mut reg_type: u32 = 0;
    let mut val: u32 = 0;
    let mut byte_len = std::mem::size_of::<u32>() as u32;
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

fn normalize_service_image_path(raw: &str, win_dir: &str) -> String {
    let expanded = expand_env_vars(raw).trim().to_string();
    if expanded.is_empty() {
        return String::new();
    }
    let mut s = if let Some(rest) = expanded.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest).to_string()
    } else {
        let lower = expanded.to_ascii_lowercase();
        if let Some(i) = lower.find(".exe") {
            expanded[..i + 4].to_string()
        } else if let Some(i) = lower.find(".sys") {
            expanded[..i + 4].to_string()
        } else {
            expanded.split_whitespace().next().unwrap_or("").to_string()
        }
    };
    if let Some(rest) = s.strip_prefix("\\??\\") {
        s = rest.to_string();
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("\\systemroot\\") {
        s = format!("{}\\{}", win_dir.trim_end_matches('\\'), &s[12..]);
    } else if lower.starts_with("system32\\") {
        s = format!("{}\\{}", win_dir.trim_end_matches('\\'), s);
    }
    normalize_windows_path(&s)
}

#[must_use]
pub fn enumerate_windows_services() -> Vec<WindowsServiceEntry> {
    let Some(parent) = open_key(HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Services") else {
        return Vec::new();
    };
    let win_dir = get_windows_directory();
    let mut out = Vec::new();
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
        let svc_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let Some(child) = open_key(
            HKEY_LOCAL_MACHINE,
            &format!("SYSTEM\\CurrentControlSet\\Services\\{svc_name}"),
        ) else {
            continue;
        };
        let raw_image = query_string(child.0, "ImagePath").unwrap_or_default();
        let norm_image = normalize_service_image_path(&raw_image, &win_dir);
        let display_name = query_string(child.0, "DisplayName").unwrap_or_default();
        let service_type = query_dword(child.0, "Type").unwrap_or(0);
        let depend_on = query_multi_sz(child.0, "DependOnService");

        out.push(WindowsServiceEntry {
            service_name: svc_name,
            display_name,
            service_type,
            raw_image_path: raw_image,
            normalized_image_path: norm_image,
            depend_on_service: depend_on,
        });
    }
    out
}
