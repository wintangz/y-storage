use std::collections::{HashMap, HashSet};
use std::path::Path;

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP, HDC, HGDIOBJ,
};
use windows_sys::Win32::UI::Shell::{ExtractIconExW, SHDefExtractIconW};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use y_core::discovery::{
    build_registry_catalog, normalize_app_token, normalize_path_key, normalize_windows_path,
    path_drive_letter,
};
use y_core::model::{AttributedApp, CachedAppIcon, NoTouchCategory, ScanReport};

use crate::paths::{expand_env_vars, to_wide_null};
use crate::registry::{enumerate_uninstall_registry, read_steam_install_path};

pub const APP_ICON_PX: u16 = 18;
const EXTRACT_SRC_PX: u32 = 36;

struct IconGuard(HICON);
impl Drop for IconGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { DestroyIcon(self.0) };
        }
    }
}

struct BitmapGuard(HBITMAP);
impl Drop for BitmapGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { DeleteObject(self.0 as HGDIOBJ) };
        }
    }
}

struct DcGuard(HDC);
impl Drop for DcGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { DeleteDC(self.0) };
        }
    }
}

/// Extracts and embeds `18x18` RGBA icons once per scan for registry applications.
pub fn populate_report_icons(report: &mut ScanReport) {
    let needs_reg_lookup = report
        .apps
        .iter()
        .any(|a| a.id.starts_with("reg:") && a.display_icon.is_none());

    let reg_icon_map: HashMap<String, String> = if needs_reg_lookup {
        let raw_reg = enumerate_uninstall_registry();
        let steam_path = read_steam_install_path();
        let catalog = build_registry_catalog(&raw_reg, steam_path.as_deref());
        catalog
            .apps
            .into_iter()
            .filter_map(|app| app.display_icon.map(|ic| (app.id, ic)))
            .collect()
    } else {
        HashMap::new()
    };

    let excluded_drives: HashSet<char> = report
        .drives
        .iter()
        .filter(|d| !d.full_walk_enabled)
        .map(|d| d.letter.to_ascii_uppercase())
        .collect();

    let mut icon_cache: HashMap<(String, i32), Option<CachedAppIcon>> = HashMap::new();
    for app in &mut report.apps {
        if !(app.id.starts_with("reg:")
            || app.id.starts_with("steam:")
            || app.id.starts_with("container:")
            || app.id.starts_with("exe:"))
        {
            continue;
        }
        if app.icon_rgba.is_some() {
            continue;
        }
        if app.id.starts_with("reg:") && app.display_icon.is_none() {
            if let Some(ic) = reg_icon_map.get(&app.id) {
                if path_drive_letter(ic)
                    .is_none_or(|dl| !excluded_drives.contains(&dl.to_ascii_uppercase()))
                {
                    app.display_icon = Some(ic.clone());
                }
            }
        }
        app.icon_rgba = extract_app_icon_cached(app, &excluded_drives, &mut icon_cache);
    }
}

fn extract_app_icon_cached(
    app: &AttributedApp,
    excluded_drives: &HashSet<char>,
    cache: &mut HashMap<(String, i32), Option<CachedAppIcon>>,
) -> Option<CachedAppIcon> {
    let candidates = collect_icon_candidates(app);
    for (path, icon_index) in candidates {
        if path_drive_letter(&path)
            .is_some_and(|dl| excluded_drives.contains(&dl.to_ascii_uppercase()))
        {
            continue;
        }
        let key = (normalize_path_key(&path), icon_index);
        if let Some(cached) = cache.get(&key) {
            if cached.is_some() {
                return cached.clone();
            }
            continue;
        }
        let extracted = extract_icon_from_file(&path, icon_index);
        let found = extracted.is_some();
        cache.insert(key, extracted.clone());
        if found {
            return extracted;
        }
    }
    None
}

fn collect_icon_candidates(app: &AttributedApp) -> Vec<(String, i32)> {
    let mut primary: Vec<(String, i32)> = Vec::new();
    let mut secondary: Vec<(String, i32)> = Vec::new();

    if let Some(raw_icon) = app.display_icon.as_deref() {
        if let Some((path, index)) = parse_display_icon_spec(raw_icon) {
            if !is_generic_shell_binary(&path) && Path::new(&path).is_file() {
                if looks_like_uninstaller_or_setup(&path) {
                    secondary.push((path, index));
                } else {
                    primary.push((path, index));
                }
            }
        }
    }

    // Services without their own valid DisplayIcon stay neutral rather than grabbing helper tools.
    let allow_folder_scan =
        !matches!(app.no_touch, Some(NoTouchCategory::Service)) || !primary.is_empty();
    if allow_folder_scan {
        if let Some(install_root) = app.install_location.as_deref() {
            for exe in find_best_exes_in_install_dir(install_root, &app.display_name) {
                if !primary
                    .iter()
                    .any(|(p, _)| normalize_path_key(p) == normalize_path_key(&exe))
                {
                    primary.push((exe, 0));
                }
            }
        }
    }

    primary.extend(secondary);
    primary
}

fn parse_display_icon_spec(raw: &str) -> Option<(String, i32)> {
    let expanded = expand_env_vars(raw);
    let mut s = expanded.trim();
    if s.is_empty() {
        return None;
    }

    let mut icon_index = 0i32;
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end_q) = rest.find('"') {
            let path_part = &rest[..end_q];
            let tail = rest[end_q + 1..].trim();
            if let Some(idx_str) = tail.strip_prefix(',') {
                if let Ok(parsed) = idx_str.trim().parse::<i32>() {
                    icon_index = parsed;
                }
            }
            let norm = normalize_windows_path(path_part);
            return if norm.is_empty() { None } else { Some((norm, icon_index)) };
        }
    }

    if let Some(comma) = s.rfind(',') {
        let after = s[comma + 1..].trim();
        if !after.is_empty() && after.parse::<i32>().is_ok() {
            icon_index = after.parse::<i32>().unwrap_or(0);
            s = &s[..comma];
        }
    }

    let norm = normalize_windows_path(s);
    if norm.is_empty() {
        None
    } else {
        Some((norm, icon_index))
    }
}

fn is_generic_shell_binary(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let leaf = lower.rsplit('\\').next().unwrap_or(&lower);
    matches!(
        leaf,
        "msiexec.exe"
            | "rundll32.exe"
            | "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "control.exe"
            | "explorer.exe"
            | "wscript.exe"
            | "cscript.exe"
    )
}

fn looks_like_uninstaller_or_setup(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let leaf = lower.rsplit('\\').next().unwrap_or(&lower);
    leaf.starts_with("unins")
        || leaf.contains("uninstall")
        || leaf.starts_with("setup")
        || leaf.starts_with("install")
}

fn find_best_exes_in_install_dir(root: &str, display_name: &str) -> Vec<String> {
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        return Vec::new();
    }

    let app_tok = normalize_app_token(display_name);
    let app_words: Vec<&str> = app_tok
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .collect();

    let mut dirs_to_check = vec![root_path.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root_path) {
        let mut subdirs = Vec::new();
        for ent in entries.flatten() {
            let Ok(ft) = ent.file_type() else { continue };
            if ft.is_dir() && !ft.is_symlink() {
                let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
                if matches!(
                    name.as_str(),
                    "locales"
                        | "resources"
                        | "logs"
                        | "cache"
                        | "shaders"
                        | "__installer"
                        | "redist"
                        | "support"
                        | "directx"
                        | "crashdumps"
                        | "doc"
                        | "docs"
                ) {
                    continue;
                }
                subdirs.push(ent.path());
                if subdirs.len() >= 18 {
                    break;
                }
            }
        }
        // Check depth-2 common subdirs (`Binaries\Win64`, `app-x\...`) up to a strict cap.
        for sub in &subdirs {
            if let Ok(sub_entries) = std::fs::read_dir(sub) {
                for ent in sub_entries.flatten() {
                    let Ok(ft) = ent.file_type() else { continue };
                    if ft.is_dir() && !ft.is_symlink() {
                        let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
                        if matches!(name.as_str(), "win64" | "x64" | "bin" | "binaries" | "game") {
                            dirs_to_check.push(ent.path());
                        }
                    }
                }
            }
        }
        dirs_to_check.extend(subdirs);
    }

    let mut scored: Vec<(i32, u64, String)> = Vec::new();
    for dir in dirs_to_check {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for ent in entries.flatten() {
            let Ok(ft) = ent.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let p = ent.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            if ext != "exe" && ext != "ico" {
                continue;
            }
            let path_str = normalize_windows_path(&p.to_string_lossy());
            let stem_lower = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if stem_lower.starts_with("unins")
                || stem_lower.contains("uninstall")
                || stem_lower.contains("crashhandler")
                || stem_lower.contains("createdump")
                || stem_lower.starts_with("vc_redist")
                || stem_lower.starts_with("vcredist")
                || stem_lower.starts_with("dxsetup")
                || stem_lower.contains("anticheat")
            {
                continue;
            }

            let meta_bytes = ent.metadata().map(|m| m.len()).unwrap_or(0);
            let stem_tok = normalize_app_token(&stem_lower);
            let mut score = 0i32;
            if ext == "exe" {
                score += 10;
            } else {
                score += 12; // Explicit app .ico in install root is usually crisp.
            }
            if !app_tok.is_empty() && stem_tok == app_tok {
                score += 120;
            } else {
                let compact_app = app_tok.replace(' ', "");
                let compact_stem = stem_tok.replace(' ', "");
                if !compact_app.is_empty()
                    && (compact_stem == compact_app || compact_app.contains(&compact_stem))
                    && compact_stem.len() >= 3
                {
                    score += 80;
                }
                for w in &app_words {
                    if stem_tok.contains(w) {
                        score += 25;
                    }
                }
            }
            if stem_lower.contains("update")
                || stem_lower.contains("setup")
                || stem_lower.contains("helper")
                || stem_lower.contains("service")
                || stem_lower.contains("reporter")
            {
                score -= 35;
            }
            if dir == root_path {
                score += 15;
            }
            if meta_bytes >= 120_000 {
                score += 10;
            }
            scored.push((score, meta_bytes, path_str));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)).then_with(|| a.2.cmp(&b.2)));
    scored.into_iter().take(4).map(|x| x.2).collect()
}

fn extract_icon_from_file(path: &str, icon_index: i32) -> Option<CachedAppIcon> {
    let wide = to_wide_null(path);

    let mut hicon: HICON = std::ptr::null_mut();
    let hr = unsafe {
        SHDefExtractIconW(
            wide.as_ptr(),
            icon_index,
            0,
            &mut hicon,
            std::ptr::null_mut(),
            EXTRACT_SRC_PX & 0xFFFF,
        )
    };
    if hr != 0 || hicon.is_null() {
        hicon = std::ptr::null_mut();
        let mut small: HICON = std::ptr::null_mut();
        let count = unsafe { ExtractIconExW(wide.as_ptr(), icon_index, &mut hicon, &mut small, 1) };
        if !small.is_null() {
            if hicon.is_null() {
                hicon = small;
            } else {
                let _drop_small = IconGuard(small);
            }
        }
        if count == 0 || hicon.is_null() {
            return None;
        }
    }

    let _guard = IconGuard(hicon);
    let (src_w, src_h, src_rgba) = hicon_to_rgba(hicon)?;
    if !src_rgba.chunks_exact(4).any(|px| px[3] >= 24) {
        return None;
    }

    let target = APP_ICON_PX as usize;
    let out_rgba = if src_w == target && src_h == target {
        src_rgba
    } else {
        downsample_rgba_box(&src_rgba, src_w, src_h, target, target)
    };
    if !out_rgba.chunks_exact(4).any(|px| px[3] >= 24) {
        return None;
    }
    Some(CachedAppIcon::from_rgba(APP_ICON_PX, APP_ICON_PX, &out_rgba))
}

fn hicon_to_rgba(hicon: HICON) -> Option<(usize, usize, Vec<u8>)> {
    let mut info: ICONINFO = unsafe { std::mem::zeroed() };
    if unsafe { GetIconInfo(hicon, &mut info) } == 0 {
        return None;
    }
    let _color_guard = BitmapGuard(info.hbmColor);
    let _mask_guard = BitmapGuard(info.hbmMask);

    let hdc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if hdc.is_null() {
        return None;
    }
    let _dc_guard = DcGuard(hdc);

    if !info.hbmColor.is_null() {
        let mut bm: BITMAP = unsafe { std::mem::zeroed() };
        if unsafe {
            GetObjectW(
                info.hbmColor as HGDIOBJ,
                std::mem::size_of::<BITMAP>() as i32,
                (&mut bm as *mut BITMAP).cast(),
            )
        } == 0
        {
            return None;
        }
        let w = bm.bmWidth.max(0) as usize;
        let h = bm.bmHeight.max(0) as usize;
        if w == 0 || h == 0 || w > 512 || h > 512 {
            return None;
        }

        let mut bgra = vec![0u8; w * h * 4];
        let mut bmi = top_down_bmi32(w as i32, h as i32);
        let lines = unsafe {
            GetDIBits(
                hdc,
                info.hbmColor,
                0,
                h as u32,
                bgra.as_mut_ptr().cast(),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };
        if lines == 0 {
            return None;
        }

        let has_alpha = bgra.chunks_exact(4).any(|px| px[3] != 0);
        if !has_alpha && !info.hbmMask.is_null() {
            let mut mask_bm: BITMAP = unsafe { std::mem::zeroed() };
            if unsafe {
                GetObjectW(
                    info.hbmMask as HGDIOBJ,
                    std::mem::size_of::<BITMAP>() as i32,
                    (&mut mask_bm as *mut BITMAP).cast(),
                )
            } != 0
            {
                let mw = mask_bm.bmWidth.max(0) as usize;
                let mh = mask_bm.bmHeight.max(0) as usize;
                if mw == w && mh >= h && mh <= 1024 {
                    let mut mask_bgra = vec![0u8; mw * mh * 4];
                    let mut mbmi = top_down_bmi32(mw as i32, mh as i32);
                    let mlines = unsafe {
                        GetDIBits(
                            hdc,
                            info.hbmMask,
                            0,
                            mh as u32,
                            mask_bgra.as_mut_ptr().cast(),
                            &mut mbmi,
                            DIB_RGB_COLORS,
                        )
                    };
                    if mlines > 0 {
                        for idx in 0..(w * h) {
                            let opaque = mask_bgra[idx * 4] == 0;
                            bgra[idx * 4 + 3] = if opaque { 255 } else { 0 };
                        }
                    }
                }
            }
        }

        let mut rgba = Vec::with_capacity(w * h * 4);
        for px in bgra.chunks_exact(4) {
            rgba.push(px[2]);
            rgba.push(px[1]);
            rgba.push(px[0]);
            rgba.push(px[3]);
        }
        return Some((w, h, rgba));
    }

    if !info.hbmMask.is_null() {
        let mut bm: BITMAP = unsafe { std::mem::zeroed() };
        if unsafe {
            GetObjectW(
                info.hbmMask as HGDIOBJ,
                std::mem::size_of::<BITMAP>() as i32,
                (&mut bm as *mut BITMAP).cast(),
            )
        } == 0
        {
            return None;
        }
        let w = bm.bmWidth.max(0) as usize;
        let full_h = bm.bmHeight.max(0) as usize;
        let h = full_h / 2;
        if w == 0 || h == 0 || w > 512 || full_h > 1024 {
            return None;
        }
        let mut buf = vec![0u8; w * full_h * 4];
        let mut bmi = top_down_bmi32(w as i32, full_h as i32);
        if unsafe {
            GetDIBits(
                hdc,
                info.hbmMask,
                0,
                full_h as u32,
                buf.as_mut_ptr().cast(),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        } == 0
        {
            return None;
        }
        let mut rgba = Vec::with_capacity(w * h * 4);
        for idx in 0..(w * h) {
            let and_px = buf[idx * 4];
            let xor_px = buf[(w * h + idx) * 4];
            if and_px == 0 {
                let v = if xor_px != 0 { 255 } else { 0 };
                rgba.extend_from_slice(&[v, v, v, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
        return Some((w, h, rgba));
    }

    None
}

fn top_down_bmi32(w: i32, h: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }],
    }
}

fn downsample_rgba_box(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    let mut dst = Vec::with_capacity(dst_w * dst_h * 4);
    let sx_scale = src_w as f32 / dst_w as f32;
    let sy_scale = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        let sy0_f = dy as f32 * sy_scale;
        let sy1_f = (dy + 1) as f32 * sy_scale;
        let y_start = (sy0_f.floor() as usize).min(src_h - 1);
        let y_end = ((sy1_f.ceil() as usize).max(y_start + 1)).min(src_h);

        for dx in 0..dst_w {
            let sx0_f = dx as f32 * sx_scale;
            let sx1_f = (dx + 1) as f32 * sx_scale;
            let x_start = (sx0_f.floor() as usize).min(src_w - 1);
            let x_end = ((sx1_f.ceil() as usize).max(x_start + 1)).min(src_w);

            let mut acc_r = 0.0_f32;
            let mut acc_g = 0.0_f32;
            let mut acc_b = 0.0_f32;
            let mut acc_a = 0.0_f32;
            let mut acc_w = 0.0_f32;

            for sy in y_start..y_end {
                let wy = (sy1_f.min((sy + 1) as f32) - sy0_f.max(sy as f32)).max(0.0);
                for sx in x_start..x_end {
                    let wx = (sx1_f.min((sx + 1) as f32) - sx0_f.max(sx as f32)).max(0.0);
                    let w = wx * wy;
                    if w <= 0.0 {
                        continue;
                    }
                    let off = (sy * src_w + sx) * 4;
                    let a = src[off + 3] as f32 / 255.0;
                    acc_r += (src[off] as f32) * a * w;
                    acc_g += (src[off + 1] as f32) * a * w;
                    acc_b += (src[off + 2] as f32) * a * w;
                    acc_a += a * w;
                    acc_w += w;
                }
            }

            if acc_w <= 0.0 || acc_a <= 1e-4 {
                dst.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let avg_a = (acc_a / acc_w).clamp(0.0, 1.0);
                let out_r = (acc_r / acc_a).round().clamp(0.0, 255.0) as u8;
                let out_g = (acc_g / acc_a).round().clamp(0.0, 255.0) as u8;
                let out_b = (acc_b / acc_a).round().clamp(0.0, 255.0) as u8;
                let out_a = (avg_a * 255.0).round().clamp(0.0, 255.0) as u8;
                dst.extend_from_slice(&[out_r, out_g, out_b, out_a]);
            }
        }
    }

    dst
}
