use std::fs;
use std::path::Path;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(payload);
    let mut crc_buf = Vec::with_capacity(4 + payload.len());
    crc_buf.extend_from_slice(tag);
    crc_buf.extend_from_slice(payload);
    out.extend_from_slice(&crc32(&crc_buf).to_be_bytes());
}

pub fn save_bgra_u32_as_png(
    path: &Path,
    width: usize,
    height: usize,
    bgra_pixels: &[u32],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let row_bytes = 1 + width * 4;
    let mut raw = Vec::with_capacity(row_bytes * height);
    for y in 0..height {
        raw.push(0); // filter type 0 (None)
        let row = &bgra_pixels[y * width..(y + 1) * width];
        for &px in row {
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let b = (px & 0xFF) as u8;
            raw.extend_from_slice(&[r, g, b, 255]);
        }
    }

    // Wrap raw scanlines in valid zlib stored blocks (no external deflate crate required).
    let mut zlib = Vec::with_capacity(raw.len() + 64);
    zlib.push(0x78);
    zlib.push(0x01);
    let mut pos = 0usize;
    while pos < raw.len() {
        let remain = raw.len() - pos;
        let chunk_len = remain.min(65535);
        let is_final = pos + chunk_len == raw.len();
        zlib.push(if is_final { 1 } else { 0 });
        let len_u16 = chunk_len as u16;
        zlib.extend_from_slice(&len_u16.to_le_bytes());
        zlib.extend_from_slice(&(!len_u16).to_le_bytes());
        zlib.extend_from_slice(&raw[pos..pos + chunk_len]);
        pos += chunk_len;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 128);
    png.extend_from_slice(b"\x89PNG\r\n\x1A\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib);
    write_chunk(&mut png, b"IEND", &[]);

    fs::write(path, png).map_err(|e| e.to_string())
}
