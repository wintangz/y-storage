use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};

struct SectionInfo {
    virtual_address: u32,
    virtual_span: u32,
    raw_ptr: u32,
    raw_size: u32,
}

fn read_u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn rva_to_offset(rva: u32, sections: &[SectionInfo]) -> Option<u64> {
    for s in sections {
        if rva >= s.virtual_address {
            let delta = rva - s.virtual_address;
            if delta < s.virtual_span && delta < s.raw_size {
                return Some(s.raw_ptr as u64 + delta as u64);
            }
        }
    }
    None
}

/// Reads the PE32 / PE32+ Import Directory Table using seek-only header I/O,
/// returning unique lowercased imported DLL names.
pub fn parse_pe_imported_dlls<R: Read + Seek>(r: &mut R) -> Vec<String> {
    let Ok(file_len) = r.seek(SeekFrom::End(0)) else {
        return Vec::new();
    };
    if file_len < 64 || r.seek(SeekFrom::Start(0)).is_err() {
        return Vec::new();
    }

    let mut dos = [0u8; 64];
    if r.read_exact(&mut dos).is_err() || read_u16_le(&dos, 0) != 0x5A4D {
        return Vec::new();
    }
    let e_lfanew = read_u32_le(&dos, 0x3C) as u64;
    if e_lfanew < 64 || e_lfanew + 24 >= file_len {
        return Vec::new();
    }
    if r.seek(SeekFrom::Start(e_lfanew)).is_err() {
        return Vec::new();
    }

    let mut coff = [0u8; 24];
    if r.read_exact(&mut coff).is_err() || read_u32_le(&coff, 0) != 0x0000_4550 {
        return Vec::new();
    }
    let num_sections = read_u16_le(&coff, 6) as usize;
    let opt_size = read_u16_le(&coff, 20) as usize;
    if opt_size < 120 || opt_size > 1024 || num_sections == 0 || num_sections > 96 {
        return Vec::new();
    }

    let opt_start = e_lfanew + 24;
    if opt_start + opt_size as u64 + (num_sections as u64 * 40) > file_len {
        return Vec::new();
    }

    let mut opt = vec![0u8; opt_size];
    if r.read_exact(&mut opt).is_err() {
        return Vec::new();
    }
    let magic = read_u16_le(&opt, 0);
    let import_dir_off = match magic {
        0x010B => 96 + 8,  // PE32 DataDirectory[1] (Import)
        0x020B => 112 + 8, // PE32+ DataDirectory[1] (Import)
        _ => return Vec::new(),
    };
    if import_dir_off + 8 > opt.len() {
        return Vec::new();
    }
    let import_rva = read_u32_le(&opt, import_dir_off);
    if import_rva == 0 {
        return Vec::new();
    }

    let mut sec_bytes = vec![0u8; num_sections * 40];
    if r.read_exact(&mut sec_bytes).is_err() {
        return Vec::new();
    }
    let mut sections = Vec::with_capacity(num_sections);
    for chunk in sec_bytes.chunks_exact(40) {
        let vsize = read_u32_le(chunk, 8);
        let va = read_u32_le(chunk, 12);
        let raw_size = read_u32_le(chunk, 16);
        let raw_ptr = read_u32_le(chunk, 20);
        sections.push(SectionInfo {
            virtual_address: va,
            virtual_span: vsize.max(raw_size),
            raw_ptr,
            raw_size,
        });
    }

    let Some(desc_off) = rva_to_offset(import_rva, &sections) else {
        return Vec::new();
    };
    if desc_off + 20 > file_len || r.seek(SeekFrom::Start(desc_off)).is_err() {
        return Vec::new();
    }

    // Read up to 512 IMAGE_IMPORT_DESCRIPTOR records in one small contiguous buffer when possible.
    let max_desc_bytes = ((file_len - desc_off) as usize).min(512 * 20);
    let mut desc_buf = vec![0u8; max_desc_bytes];
    if r.read_exact(&mut desc_buf).is_err() {
        return Vec::new();
    }

    let mut name_rvas = Vec::new();
    for entry in desc_buf.chunks_exact(20) {
        let oft = read_u32_le(entry, 0);
        let name_rva = read_u32_le(entry, 12);
        let ft = read_u32_le(entry, 16);
        if oft == 0 && name_rva == 0 && ft == 0 {
            break;
        }
        if name_rva != 0 {
            name_rvas.push(name_rva);
        }
    }

    let mut dlls = BTreeSet::new();
    let mut str_buf = [0u8; 260];
    for rva in name_rvas {
        let Some(noff) = rva_to_offset(rva, &sections) else {
            continue;
        };
        if noff >= file_len || r.seek(SeekFrom::Start(noff)).is_err() {
            continue;
        }
        let avail = ((file_len - noff) as usize).min(str_buf.len());
        if r.read_exact(&mut str_buf[..avail]).is_err() {
            continue;
        }
        let end = str_buf[..avail]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(avail);
        if let Ok(s) = std::str::from_utf8(&str_buf[..end]) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                dlls.insert(trimmed.to_ascii_lowercase());
            }
        }
    }

    dlls.into_iter().collect()
}

/// Returns `true` if `file_name` looks like a plausible primary application executable
/// (rejecting uninstallers, setup stubs, redistributables, archivers, crash reporters, and updaters).
#[must_use]
pub fn is_plausible_main_exe_name(file_name: &str) -> bool {
    let lower = file_name.trim().to_ascii_lowercase();
    let Some(stem) = lower.strip_suffix(".exe") else {
        return false;
    };
    if stem.is_empty() || stem.starts_with('.') || stem.starts_with('_') {
        return false;
    }
    if stem.starts_with("unins")
        || stem.contains("uninstall")
        || stem.starts_with("setup")
        || stem.starts_with("install")
        || stem.starts_with("vc_redist")
        || stem.starts_with("vcredist")
        || stem.starts_with("dxsetup")
        || stem == "7z"
        || stem == "7za"
        || stem.starts_with("crash")
        || stem.contains("crashhandler")
        || stem.contains("crashreport")
        || stem == "createdump"
        || stem == "update"
        || stem.starts_with("updater")
        || stem.starts_with("autoupdate")
        || stem.contains("create shortcut")
        || stem.contains("__selfdelete__")
        || stem.contains("__relocated__")
    {
        return false;
    }
    true
}

fn align4(off: usize) -> usize {
    (off + 3) & !3
}

fn read_utf16le_z(buf: &[u8], start: usize, limit: usize) -> Option<(String, usize)> {
    let mut i = start;
    let mut units = Vec::new();
    while i + 1 < limit {
        let u = u16::from_le_bytes([buf[i], buf[i + 1]]);
        i += 2;
        if u == 0 {
            return Some((String::from_utf16_lossy(&units), i));
        }
        units.push(u);
    }
    Some((String::from_utf16_lossy(&units), limit))
}

/// Parses `VS_VERSION_INFO` -> `StringFileInfo` -> `ProductName` from a PE32/PE32+ `.rsrc` section
/// using bounded seek-only I/O (`<= 64 KB`), returning `None` if absent or placeholder.
pub fn parse_pe_version_product_name<R: Read + Seek>(r: &mut R) -> Option<String> {
    let file_len = r.seek(SeekFrom::End(0)).ok()?;
    if file_len < 64 || r.seek(SeekFrom::Start(0)).is_err() {
        return None;
    }
    let mut dos = [0u8; 64];
    r.read_exact(&mut dos).ok()?;
    if read_u16_le(&dos, 0) != 0x5A4D {
        return None;
    }
    let e_lfanew = read_u32_le(&dos, 0x3C) as u64;
    if e_lfanew < 64 || e_lfanew + 24 >= file_len {
        return None;
    }
    r.seek(SeekFrom::Start(e_lfanew)).ok()?;
    let mut coff = [0u8; 24];
    r.read_exact(&mut coff).ok()?;
    if read_u32_le(&coff, 0) != 0x0000_4550 {
        return None;
    }
    let num_sections = read_u16_le(&coff, 6) as usize;
    let opt_size = read_u16_le(&coff, 20) as usize;
    if !(128..=1024).contains(&opt_size) || num_sections == 0 || num_sections > 96 {
        return None;
    }
    let opt_start = e_lfanew + 24;
    if opt_start + opt_size as u64 + (num_sections as u64 * 40) > file_len {
        return None;
    }
    let mut opt = vec![0u8; opt_size];
    r.read_exact(&mut opt).ok()?;
    let magic = read_u16_le(&opt, 0);
    let rsrc_dir_off = match magic {
        0x010B => 96 + 16,  // PE32 DataDirectory[2] (Resource)
        0x020B => 112 + 16, // PE32+ DataDirectory[2] (Resource)
        _ => return None,
    };
    if rsrc_dir_off + 8 > opt.len() {
        return None;
    }
    let rsrc_rva = read_u32_le(&opt, rsrc_dir_off);
    let rsrc_size = read_u32_le(&opt, rsrc_dir_off + 4);
    if rsrc_rva == 0 || rsrc_size < 16 {
        return None;
    }

    let mut sec_bytes = vec![0u8; num_sections * 40];
    r.read_exact(&mut sec_bytes).ok()?;
    let mut sections = Vec::with_capacity(num_sections);
    for chunk in sec_bytes.chunks_exact(40) {
        let vsize = read_u32_le(chunk, 8);
        let va = read_u32_le(chunk, 12);
        let raw_size = read_u32_le(chunk, 16);
        let raw_ptr = read_u32_le(chunk, 20);
        sections.push(SectionInfo {
            virtual_address: va,
            virtual_span: vsize.max(raw_size),
            raw_ptr,
            raw_size,
        });
    }

    let root_off = rva_to_offset(rsrc_rva, &sections)?;
    let find_entry = |r: &mut R, dir_off: u64, want_id: Option<u32>| -> Option<(u32, bool)> {
        if dir_off + 16 > file_len || r.seek(SeekFrom::Start(dir_off)).is_err() {
            return None;
        }
        let mut hdr = [0u8; 16];
        r.read_exact(&mut hdr).ok()?;
        let named = read_u16_le(&hdr, 12) as usize;
        let ids = read_u16_le(&hdr, 14) as usize;
        let total = (named + ids).min(128);
        let mut entries = vec![0u8; total * 8];
        if r.read_exact(&mut entries).is_err() {
            return None;
        }
        for (idx, chunk) in entries.chunks_exact(8).enumerate() {
            let name_or_id = read_u32_le(chunk, 0);
            let off_val = read_u32_le(chunk, 4);
            let is_subdir = (off_val & 0x8000_0000) != 0;
            let rel_off = off_val & 0x7FFF_FFFF;
            if let Some(target_id) = want_id {
                if idx >= named && (name_or_id & 0x7FFF_FFFF) == target_id {
                    return Some((rel_off, is_subdir));
                }
            } else {
                return Some((rel_off, is_subdir));
            }
        }
        None
    };

    // Level 1: RT_VERSION (16) -> Level 2: resource ID -> Level 3: language ID -> IMAGE_RESOURCE_DATA_ENTRY
    let (lvl2_rel, is_dir2) = find_entry(r, root_off, Some(16))?;
    if !is_dir2 {
        return None;
    }
    let (lvl3_rel, is_dir3) = find_entry(r, root_off + lvl2_rel as u64, None)?;
    if !is_dir3 {
        return None;
    }
    let (data_rel, is_dir4) = find_entry(r, root_off + lvl3_rel as u64, None)?;
    if is_dir4 {
        return None;
    }
    let data_entry_off = root_off + data_rel as u64;
    if data_entry_off + 16 > file_len || r.seek(SeekFrom::Start(data_entry_off)).is_err() {
        return None;
    }
    let mut data_entry = [0u8; 16];
    r.read_exact(&mut data_entry).ok()?;
    let data_rva = read_u32_le(&data_entry, 0);
    let data_sz = read_u32_le(&data_entry, 4) as usize;
    if data_rva == 0 || data_sz < 40 {
        return None;
    }
    let data_off = rva_to_offset(data_rva, &sections)?;
    let read_len = data_sz.min(65536).min(file_len.saturating_sub(data_off) as usize);
    if read_len < 40 || r.seek(SeekFrom::Start(data_off)).is_err() {
        return None;
    }
    let mut ver_buf = vec![0u8; read_len];
    r.read_exact(&mut ver_buf).ok()?;
    parse_vs_version_product_name_bytes(&ver_buf)
}

/// Parses `ProductName` from raw `VS_VERSION_INFO` bytes.
#[must_use]
pub fn parse_vs_version_product_name_bytes(buf: &[u8]) -> Option<String> {
    if buf.len() < 16 {
        return None;
    }
    let root_len = (read_u16_le(buf, 0) as usize).min(buf.len());
    let root_val_len = read_u16_le(buf, 2) as usize;
    let root_type = read_u16_le(buf, 4);
    let (root_key, key_end) = read_utf16le_z(buf, 6, root_len)?;
    if root_key != "VS_VERSION_INFO" {
        return None;
    }
    let val_bytes = if root_type == 1 {
        root_val_len.saturating_mul(2)
    } else {
        root_val_len
    };
    let mut pos = align4(align4(key_end).saturating_add(val_bytes));

    while pos + 6 <= root_len {
        let blk_len = read_u16_le(buf, pos) as usize;
        if blk_len < 6 {
            break;
        }
        let blk_end = (pos + blk_len).min(root_len);
        let blk_val_len = read_u16_le(buf, pos + 2) as usize;
        let blk_type = read_u16_le(buf, pos + 4);
        let (blk_key, blk_key_end) = read_utf16le_z(buf, pos + 6, blk_end)?;
        let blk_val_bytes = if blk_type == 1 {
            blk_val_len.saturating_mul(2)
        } else {
            blk_val_len
        };
        let mut st_pos = align4(align4(blk_key_end).saturating_add(blk_val_bytes));

        if blk_key == "StringFileInfo" {
            while st_pos + 6 <= blk_end {
                let st_len = read_u16_le(buf, st_pos) as usize;
                if st_len < 6 {
                    break;
                }
                let st_end = (st_pos + st_len).min(blk_end);
                let (_st_key, st_key_end) = read_utf16le_z(buf, st_pos + 6, st_end)?;
                let mut str_pos = align4(st_key_end);

                while str_pos + 6 <= st_end {
                    let s_len = read_u16_le(buf, str_pos) as usize;
                    if s_len < 6 {
                        break;
                    }
                    let s_end = (str_pos + s_len).min(st_end);
                    let s_val_words = read_u16_le(buf, str_pos + 2) as usize;
                    let (s_key, s_key_end) = read_utf16le_z(buf, str_pos + 6, s_end)?;
                    let val_start = align4(s_key_end);
                    if s_key.eq_ignore_ascii_case("ProductName") && s_val_words > 0 && val_start < s_end {
                        let (val, _) = read_utf16le_z(buf, val_start, s_end)?;
                        let clean = val.trim();
                        if !clean.is_empty()
                            && !clean.starts_with("TODO:")
                            && !clean.eq_ignore_ascii_case("Microsoft® Windows® Operating System")
                        {
                            return Some(clean.to_string());
                        }
                    }
                    str_pos = align4(s_end);
                }
                st_pos = align4(st_end);
            }
        }
        pos = align4(blk_end);
    }
    None
}
