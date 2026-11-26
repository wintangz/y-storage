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
