use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use y_core::discovery::{normalize_path_key, normalize_windows_path};

use crate::paths::expand_env_vars;

const LNK_HEADER_SIZE: usize = 0x4C;
const MAX_LNK_BYTES: u64 = 256 * 1024;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
const LINK_CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

const FLAG_HAS_LINK_TARGET_ID_LIST: u32 = 0x01;
const FLAG_HAS_LINK_INFO: u32 = 0x02;
const FLAG_HAS_NAME: u32 = 0x04;
const FLAG_HAS_RELATIVE_PATH: u32 = 0x08;
const FLAG_HAS_WORKING_DIR: u32 = 0x10;
const FLAG_HAS_ARGUMENTS: u32 = 0x20;
const FLAG_HAS_ICON_LOCATION: u32 = 0x40;
const FLAG_IS_UNICODE: u32 = 0x80;

const LINK_INFO_VOLUME_AND_LOCAL: u32 = 0x01;
const ENV_VAR_BLOCK_SIG: u32 = 0xA000_0001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartMenuShortcut {
    pub display_name: String,
    pub target_path: String,
    pub shortcut_path: String,
}

fn read_u16_le(buf: &[u8], off: usize) -> Option<u16> {
    let bytes = buf.get(off..off + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(buf: &[u8], off: usize) -> Option<u32> {
    let bytes = buf.get(off..off + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_cstr_lossy(buf: &[u8], off: usize) -> Option<String> {
    if off >= buf.len() {
        return None;
    }
    let slice = &buf[off..];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    let s = String::from_utf8_lossy(&slice[..end]).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn read_utf16le_z(buf: &[u8], off: usize) -> Option<String> {
    if off + 2 > buf.len() {
        return None;
    }
    let mut units = Vec::new();
    let mut i = off;
    while i + 1 < buf.len() {
        let u = u16::from_le_bytes([buf[i], buf[i + 1]]);
        if u == 0 {
            break;
        }
        units.push(u);
        i += 2;
    }
    let s = String::from_utf16_lossy(&units).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn join_base_and_suffix(base: Option<String>, suffix: Option<String>) -> Option<String> {
    match (base, suffix) {
        (Some(b), Some(s)) if !s.is_empty() => {
            if b.ends_with('\\') || s.starts_with('\\') {
                Some(format!("{b}{s}"))
            } else {
                Some(format!("{b}\\{s}"))
            }
        }
        (Some(b), _) => Some(b),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    }
}

fn read_string_data_field(
    buf: &[u8],
    pos: &mut usize,
    is_unicode: bool,
) -> Option<String> {
    let count = read_u16_le(buf, *pos)? as usize;
    *pos += 2;
    let byte_len = if is_unicode {
        count.checked_mul(2)?
    } else {
        count
    };
    let slice = buf.get(*pos..*pos + byte_len)?;
    *pos += byte_len;
    let s = if is_unicode {
        let units: Vec<u16> = slice
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(slice).into_owned()
    };
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn normalize_exe_candidate(raw: &str, lnk_dir: Option<&Path>) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = expand_env_vars(trimmed);
    let candidate = expanded.trim().trim_matches('"').trim();
    if !candidate.to_ascii_lowercase().ends_with(".exe") {
        return None;
    }
    let p = Path::new(candidate);
    let resolved = if p.is_relative() {
        lnk_dir.map(|d| d.join(p)).unwrap_or_else(|| p.to_path_buf())
    } else {
        p.to_path_buf()
    };
    let norm = normalize_windows_path(&resolved.to_string_lossy());
    if norm.to_ascii_lowercase().ends_with(".exe") {
        Some(norm)
    } else {
        None
    }
}

fn parse_lnk_bytes(buf: &[u8], lnk_dir: Option<&Path>) -> Option<String> {
    if buf.len() < LNK_HEADER_SIZE {
        return None;
    }
    if read_u32_le(buf, 0)? != LNK_HEADER_SIZE as u32 {
        return None;
    }
    if buf.get(4..20)? != LINK_CLSID {
        return None;
    }
    let flags = read_u32_le(buf, 20)?;
    let is_unicode = (flags & FLAG_IS_UNICODE) != 0;
    let mut pos = LNK_HEADER_SIZE;

    if (flags & FLAG_HAS_LINK_TARGET_ID_LIST) != 0 {
        let id_list_size = read_u16_le(buf, pos)? as usize;
        pos = pos.checked_add(2)?.checked_add(id_list_size)?;
        if pos > buf.len() {
            return None;
        }
    }

    let mut link_info_candidate: Option<String> = None;
    if (flags & FLAG_HAS_LINK_INFO) != 0 {
        let link_info_size = read_u32_le(buf, pos)? as usize;
        if link_info_size >= 28 && pos.checked_add(link_info_size)? <= buf.len() {
            let li = &buf[pos..pos + link_info_size];
            let li_hdr_size = read_u32_le(li, 4).unwrap_or(0);
            let li_flags = read_u32_le(li, 8).unwrap_or(0);
            if (li_flags & LINK_INFO_VOLUME_AND_LOCAL) != 0 {
                let unicode_path = if li_hdr_size >= 0x24 {
                    let base_off_u = read_u32_le(li, 0x1C).unwrap_or(0) as usize;
                    let suffix_off_u = read_u32_le(li, 0x20).unwrap_or(0) as usize;
                    let base = if base_off_u > 0 {
                        read_utf16le_z(li, base_off_u)
                    } else {
                        None
                    };
                    let suffix = if suffix_off_u > 0 {
                        read_utf16le_z(li, suffix_off_u)
                    } else {
                        None
                    };
                    join_base_and_suffix(base, suffix)
                } else {
                    None
                };
                link_info_candidate = unicode_path.or_else(|| {
                    let base_off_a = read_u32_le(li, 0x10).unwrap_or(0) as usize;
                    let suffix_off_a = read_u32_le(li, 0x18).unwrap_or(0) as usize;
                    let base = if base_off_a > 0 {
                        read_cstr_lossy(li, base_off_a)
                    } else {
                        None
                    };
                    let suffix = if suffix_off_a > 0 {
                        read_cstr_lossy(li, suffix_off_a)
                    } else {
                        None
                    };
                    join_base_and_suffix(base, suffix)
                });
            }
            pos += link_info_size;
        } else {
            return None;
        }
    }

    if (flags & FLAG_HAS_NAME) != 0 {
        let _ = read_string_data_field(buf, &mut pos, is_unicode);
    }
    let relative_path_candidate = if (flags & FLAG_HAS_RELATIVE_PATH) != 0 {
        read_string_data_field(buf, &mut pos, is_unicode)
    } else {
        None
    };
    if (flags & FLAG_HAS_WORKING_DIR) != 0 {
        let _ = read_string_data_field(buf, &mut pos, is_unicode);
    }
    if (flags & FLAG_HAS_ARGUMENTS) != 0 {
        let _ = read_string_data_field(buf, &mut pos, is_unicode);
    }
    if (flags & FLAG_HAS_ICON_LOCATION) != 0 {
        let _ = read_string_data_field(buf, &mut pos, is_unicode);
    }

    let mut env_var_candidate: Option<String> = None;
    while pos + 4 <= buf.len() {
        let Some(block_size) = read_u32_le(buf, pos).map(|v| v as usize) else {
            break;
        };
        if block_size < 4 || pos + block_size > buf.len() {
            break;
        }
        if block_size >= 8 {
            if let Some(sig) = read_u32_le(buf, pos + 4) {
                if sig == ENV_VAR_BLOCK_SIG && env_var_candidate.is_none() {
                    let blk = &buf[pos..pos + block_size];
                    let uni = if blk.len() >= 268 + 2 {
                        read_utf16le_z(blk, 268)
                    } else {
                        None
                    };
                    let ansi = if blk.len() > 8 {
                        read_cstr_lossy(blk, 8)
                    } else {
                        None
                    };
                    env_var_candidate = uni.or(ansi);
                }
            }
        }
        pos += block_size;
    }

    for cand in [
        link_info_candidate,
        env_var_candidate,
        relative_path_candidate,
    ]
    .into_iter()
    .flatten()
    {
        if let Some(exe) = normalize_exe_candidate(&cand, lnk_dir) {
            return Some(exe);
        }
    }
    None
}

/// Resolves the target `.exe` path of a Windows `.lnk` file purely in Rust using `[MS-SHLLINK]`.
///
/// Reads at most 256 KB from disk and expands embedded environment variables offline.
#[must_use]
pub fn resolve_lnk_target_exe(lnk_path: impl AsRef<Path>) -> Option<String> {
    let path_ref = lnk_path.as_ref();
    let Ok(meta) = fs::metadata(path_ref) else {
        return None;
    };
    if !meta.is_file() || meta.len() < LNK_HEADER_SIZE as u64 || meta.len() > MAX_LNK_BYTES {
        return None;
    }
    let mut file = File::open(path_ref).ok()?;
    let mut buf = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut buf).ok()?;
    parse_lnk_bytes(&buf, path_ref.parent())
}

fn collect_lnk_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        let path = ent.path();
        if let Ok(meta) = ent.metadata() {
            if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                continue;
            }
        }
        if ft.is_dir() {
            collect_lnk_files(&path, depth + 1, out);
        } else if ft.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("lnk") {
                    out.push(path);
                }
            }
        }
    }
}

/// Enumerates Start Menu `.lnk` shortcuts across machine-wide (`%ProgramData%`) and
/// per-user (`%APPDATA%`) Start Menu folders whose resolved target is an `.exe`.
#[must_use]
pub fn enumerate_start_menu_shortcuts() -> Vec<StartMenuShortcut> {
    let mut roots = Vec::new();
    let prog_data = std::env::var("ProgramData")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| r"C:\ProgramData".to_string());
    roots.push(
        PathBuf::from(prog_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs"),
    );

    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.trim().is_empty() {
            roots.push(
                PathBuf::from(appdata)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
    }

    let mut lnk_files = Vec::new();
    for root in &roots {
        if root.is_dir() {
            collect_lnk_files(root, 0, &mut lnk_files);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for lnk in lnk_files {
        let Some(display_name) = lnk
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(target_path) = resolve_lnk_target_exe(&lnk) else {
            continue;
        };
        let key = (
            display_name.to_ascii_lowercase(),
            normalize_path_key(&target_path),
        );
        if seen.insert(key) {
            out.push(StartMenuShortcut {
                display_name,
                target_path,
                shortcut_path: normalize_windows_path(&lnk.to_string_lossy()),
            });
        }
    }

    out.sort_by(|a, b| {
        a.display_name
            .to_ascii_lowercase()
            .cmp(&b.display_name.to_ascii_lowercase())
            .then_with(|| {
                normalize_path_key(&a.target_path).cmp(&normalize_path_key(&b.target_path))
            })
    });
    out
}
