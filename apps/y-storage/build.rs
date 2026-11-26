use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_exe() -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let cand = dir.join("rc.exe");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    let kits_bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(entries) = fs::read_dir(kits_bin) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.join("x64").join("rc.exe").is_file())
            .collect();
        versions.sort();
        if let Some(latest) = versions.pop() {
            return Some(latest.join("x64").join("rc.exe"));
        }
    }
    None
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8
}

fn palette_grad(t: f32) -> [u8; 3] {
    let blue = [84u8, 185, 242];
    let lav = [216u8, 166, 255];
    let pink = [247u8, 168, 216];
    let t = t.clamp(0.0, 1.0);
    if t <= 0.5 {
        let u = t * 2.0;
        [
            lerp_u8(blue[0], lav[0], u),
            lerp_u8(blue[1], lav[1], u),
            lerp_u8(blue[2], lav[2], u),
        ]
    } else {
        let u = (t - 0.5) * 2.0;
        [
            lerp_u8(lav[0], pink[0], u),
            lerp_u8(lav[1], pink[1], u),
            lerp_u8(lav[2], pink[2], u),
        ]
    }
}

fn sdf_round_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let dx = (px - cx).abs() - (hx - r);
    let dy = (py - cy).abs() - (hy - r);
    let ox = dx.max(0.0);
    let oy = dy.max(0.0);
    (ox * ox + oy * oy).sqrt() + dx.min(0.0).max(dy.min(0.0)) - r
}

fn blend(dst: &mut [u8; 4], src: [u8; 4]) {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-5 {
        *dst = [0, 0, 0, 0];
        return;
    }
    for i in 0..3 {
        let c = (src[i] as f32 * sa + dst[i] as f32 * da * (1.0 - sa)) / out_a;
        dst[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn rasterize_icon_rgba(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let s = size as f32;
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / s * 256.0;
            let v = (y as f32 + 0.5) / s * 256.0;
            let mut px = [0u8, 0, 0, 0];

            // Outer rounded square (#0A0A10) + pastel rim (#54B9F2 -> #D8A6FF -> #F7A8D8)
            let d_bg = sdf_round_box(u, v, 128.0, 128.0, 108.0, 108.0, 48.0);
            if d_bg < 1.5 {
                let cov = (1.5 - d_bg).clamp(0.0, 1.0);
                let rim = palette_grad((u + v) / 512.0);
                if d_bg > -10.5 {
                    blend(&mut px, [rim[0], rim[1], rim[2], (cov * 255.0) as u8]);
                } else {
                    blend(&mut px, [10, 10, 16, (cov * 255.0) as u8]);
                }
            }

            // Three stacked storage bars
            let bars = [
                (80.0_f32, [84u8, 185, 242], 94.0_f32, [84u8, 185, 242]),
                (128.0_f32, [216u8, 166, 255], 122.0_f32, [247u8, 168, 216]),
                (176.0_f32, [247u8, 168, 216], 66.0_f32, [40u8, 214, 108]),
            ];
            for (cy, stroke_col, fill_w, fill_col) in bars {
                let d_tray = sdf_round_box(u, v, 128.0, cy, 76.0, 16.0, 8.0);
                if d_tray < 1.2 {
                    let cov = (1.2 - d_tray).clamp(0.0, 1.0);
                    if d_tray > -4.5 {
                        blend(
                            &mut px,
                            [stroke_col[0], stroke_col[1], stroke_col[2], (cov * 255.0) as u8],
                        );
                    } else {
                        blend(&mut px, [20, 20, 31, (cov * 255.0) as u8]);
                    }
                }
                let d_fill = sdf_round_box(u, v, 64.0 + fill_w * 0.5, cy, fill_w * 0.5, 5.0, 4.0);
                if d_fill < 1.0 {
                    let cov = (1.0 - d_fill).clamp(0.0, 1.0);
                    blend(
                        &mut px,
                        [fill_col[0], fill_col[1], fill_col[2], (cov * 255.0) as u8],
                    );
                }
            }

            let off = (y * size + x) * 4;
            out[off..off + 4].copy_from_slice(&px);
        }
    }
    out
}

fn encode_ico_bmp_entry(size: usize, rgba: &[u8]) -> Vec<u8> {
    // 40-byte BITMAPINFOHEADER + bottom-up BGRA pixels + AND mask (all 0)
    let mut data = Vec::new();
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(size as i32).to_le_bytes());
    data.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // color + AND mask height
    data.extend_from_slice(&1u16.to_le_bytes()); // planes
    data.extend_from_slice(&32u16.to_le_bytes()); // bpp
    data.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    data.extend_from_slice(&((size * size * 4) as u32).to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..size).rev() {
        for x in 0..size {
            let off = (y * size + x) * 4;
            data.push(rgba[off + 2]);
            data.push(rgba[off + 1]);
            data.push(rgba[off]);
            data.push(rgba[off + 3]);
        }
    }
    let mask_row_bytes = ((size + 31) / 32) * 4;
    data.resize(data.len() + mask_row_bytes * size, 0u8);
    data
}

fn build_multi_size_ico() -> Vec<u8> {
    let sizes = [16usize, 20, 24, 32, 40, 48, 64, 256];
    let mut entries = Vec::with_capacity(sizes.len());
    for s in sizes {
        let rgba = rasterize_icon_rgba(s);
        entries.push((s, encode_ico_bmp_entry(s, &rgba)));
    }

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
    ico.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    let mut offset = 6 + entries.len() * 16;
    for (s, payload) in &entries {
        let dim = if *s >= 256 { 0u8 } else { *s as u8 };
        ico.push(dim);
        ico.push(dim);
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += payload.len();
    }
    for (_, payload) in entries {
        ico.extend_from_slice(&payload);
    }
    ico
}

fn main() {
    println!("cargo:rerun-if-changed=y-storage.manifest");
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=../../icon.svg");

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let assets_dir = manifest_dir.join("assets");
    let _ = fs::create_dir_all(&assets_dir);
    let ico_path = assets_dir.join("y-storage.ico");
    if !ico_path.is_file() {
        fs::write(&ico_path, build_multi_size_ico()).expect("write y-storage.ico");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let manifest = manifest_dir.join("y-storage.manifest");

        println!("cargo:rustc-link-arg-bin=y-storage=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=y-storage=/MANIFESTINPUT:{}",
            manifest.display()
        );

        let rc_exe = find_rc_exe().expect("Windows SDK rc.exe not found to compile app.rc");
        let res_path = out_dir.join("app.res");
        let status = Command::new(&rc_exe)
            .arg("/nologo")
            .arg("/fo")
            .arg(&res_path)
            .arg(manifest_dir.join("app.rc"))
            .status()
            .expect("failed to execute rc.exe");
        assert!(status.success(), "rc.exe failed with status {status}");
        println!("cargo:rustc-link-arg-bin=y-storage={}", res_path.display());
    }
}
