use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP, HDC, HGDIOBJ,
};
use windows_sys::Win32::UI::Shell::{ExtractIconExW, SHDefExtractIconW};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use y_core::dependencies::WindowsServiceEntry;
use y_core::discovery::{
    build_registry_catalog, contains_whole_token_sequence, normalize_app_token, normalize_path_key,
    normalize_windows_path,
};
use y_core::model::{AttributedApp, CachedAppIcon, NoTouchCategory, ScanReport, ZoneKind};

use crate::paths::{expand_env_vars, to_wide_null};
use crate::registry::{enumerate_uninstall_registry, read_steam_install_path};
use crate::services::enumerate_windows_services;

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

/// Extracts and embeds `18x18` RGBA icons once per scan from legitimate executables, service binaries, or MSIX logos.
pub fn populate_report_icons(report: &mut ScanReport) {
    let needs_reg_lookup = report
        .apps
        .iter()
        .any(|a| a.id.starts_with("reg:") && (a.display_icon.is_none() || a.install_location.is_none()));

    let reg_meta_map: HashMap<String, (Option<String>, Option<String>)> = if needs_reg_lookup {
        let raw_reg = enumerate_uninstall_registry();
        let steam_path = read_steam_install_path();
        let catalog = build_registry_catalog(&raw_reg, steam_path.as_deref());
        catalog
            .apps
            .into_iter()
            .filter_map(|app| {
                let install = app.primary_install_location().map(str::to_string);
                let dicon = app.display_icon;
                if dicon.is_some() || install.is_some() {
                    Some((app.id, (dicon, install)))
                } else {
                    None
                }
            })
            .collect()
    } else {
        HashMap::new()
    };

    let services = enumerate_windows_services();

    let mut icon_cache: HashMap<(String, i32), Option<CachedAppIcon>> = HashMap::new();
    for app in &mut report.apps {
        if app.icon_rgba.is_some() {
            continue;
        }
        let mut fallback_install_dir: Option<&str> = None;
        if app.id.starts_with("reg:") {
            if let Some((reg_icon, reg_install)) = reg_meta_map.get(&app.id) {
                if app.display_icon.is_none() {
                    if let Some(ic) = reg_icon {
                        app.display_icon = Some(ic.clone());
                    }
                }
                if app.install_location.is_none() {
                    fallback_install_dir = reg_install.as_deref();
                }
            }
        }
        app.icon_rgba = extract_app_icon_cached(app, &services, fallback_install_dir, &mut icon_cache);
    }
}

fn extract_app_icon_cached(
    app: &AttributedApp,
    services: &[WindowsServiceEntry],
    fallback_install_dir: Option<&str>,
    cache: &mut HashMap<(String, i32), Option<CachedAppIcon>>,
) -> Option<CachedAppIcon> {
    let candidates = collect_icon_candidates(app, services, fallback_install_dir);
    for (path, icon_index) in candidates {
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

fn is_user_or_cache_folder(path: &str) -> bool {
    let lower = normalize_path_key(path);
    lower.contains("\\appdata\\")
        || lower.contains("\\programdata\\package cache")
        || lower.ends_with("\\temp")
        || lower.contains("\\windows\\winsxs")
        || lower.contains("\\windows\\installer")
}

fn match_service_binaries_for_app(
    app: &AttributedApp,
    services: &[WindowsServiceEntry],
) -> Vec<String> {
    let app_tok = normalize_app_token(&app.display_name);
    let app_compact = app_tok.replace(' ', "");
    let body_roots: Vec<String> = app
        .install_location
        .iter()
        .chain(
            app.items
                .iter()
                .filter(|it| it.zone == Some(ZoneKind::Body))
                .map(|it| &it.path),
        )
        .map(|p| normalize_path_key(p))
        .filter(|p| !p.is_empty() && !is_user_or_cache_folder(p))
        .collect();

    let mut out: Vec<String> = Vec::new();
    for svc in services {
        let img = normalize_windows_path(&svc.normalized_image_path);
        if img.is_empty() || !img.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        let img_key = normalize_path_key(&img);
        if img_key.contains("\\windows\\system32\\")
            || img_key.contains("\\windows\\syswow64\\")
            || !Path::new(&img).is_file()
        {
            continue;
        }
        let in_app_dir = body_roots
            .iter()
            .any(|r| img_key == *r || img_key.starts_with(&format!("{r}\\")));
        let svc_tok = normalize_app_token(&svc.service_name);
        let disp_tok = normalize_app_token(&svc.display_name);
        let tok_match = !app_tok.is_empty()
            && (svc_tok == app_tok
                || disp_tok == app_tok
                || (!app_compact.is_empty()
                    && app_compact.len() >= 5
                    && (svc_tok.replace(' ', "") == app_compact
                        || disp_tok.replace(' ', "") == app_compact))
                || (app_tok.len() >= 6
                    && (contains_whole_token_sequence(&svc.display_name, &app_tok)
                        || contains_whole_token_sequence(&svc.service_name, &app_tok))));
        if in_app_dir || tok_match {
            if !out.iter().any(|p| normalize_path_key(p) == img_key) {
                out.push(img);
            }
        }
    }
    out
}

fn find_appx_logo_pngs(pkg_dir: &str) -> Vec<String> {
    let root_manifest = Path::new(pkg_dir).join("AppxManifest.xml");
    let manifest_path = if root_manifest.is_file() {
        root_manifest
    } else {
        let content_manifest = Path::new(pkg_dir).join("Content").join("AppxManifest.xml");
        if content_manifest.is_file() {
            content_manifest
        } else {
            return Vec::new();
        }
    };
    let manifest_base = manifest_path.parent().unwrap_or_else(|| Path::new(pkg_dir));
    let Ok(xml) = std::fs::read_to_string(&manifest_path) else {
        return Vec::new();
    };
    let Some(start) = xml.find("<Logo>") else {
        return Vec::new();
    };
    let rest = &xml[start + 6..];
    let Some(end) = rest.find("</Logo>") else {
        return Vec::new();
    };
    let rel_logo = rest[..end].trim().trim_start_matches(['\\', '/']);
    if rel_logo.is_empty() {
        return Vec::new();
    }
    let rel_path = manifest_base.join(rel_logo);
    let mut hits: Vec<(i32, String)> = Vec::new();
    let push_if_png = |p: &Path, prio: i32, out: &mut Vec<(i32, String)>| {
        if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            let norm = normalize_windows_path(&p.to_string_lossy());
            if !out.iter().any(|(_, existing)| normalize_path_key(existing) == normalize_path_key(&norm)) {
                out.push((prio, norm));
            }
        }
    };
    push_if_png(&rel_path, 0, &mut hits);
    if let (Some(parent), Some(stem), Some(ext)) = (
        rel_path.parent(),
        rel_path.file_stem().and_then(|s| s.to_str()),
        rel_path.extension().and_then(|e| e.to_str()),
    ) {
        let stem_lower = stem.to_ascii_lowercase();
        let ext_dot = format!(".{}", ext.to_ascii_lowercase());
        if let Ok(entries) = std::fs::read_dir(parent) {
            for ent in entries.flatten() {
                let p = ent.path();
                if !p.is_file() {
                    continue;
                }
                let fname = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if fname.starts_with(&stem_lower) && fname.ends_with(&ext_dot) {
                    if fname.contains("contrast-white") || fname.contains("contrast-black") {
                        continue;
                    }
                    let prio = if fname.contains("scale-200") {
                        1
                    } else if fname.contains("scale-150") {
                        2
                    } else if fname.contains("scale-125") {
                        3
                    } else if fname.contains("scale-100") {
                        4
                    } else {
                        5
                    };
                    push_if_png(&p, prio, &mut hits);
                }
            }
        }
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    hits.into_iter().take(2).map(|(_, p)| p).collect()
}

fn collect_icon_candidates(
    app: &AttributedApp,
    services: &[WindowsServiceEntry],
    fallback_install_dir: Option<&str>,
) -> Vec<(String, i32)> {
    let mut primary: Vec<(String, i32)> = Vec::new();
    let mut secondary: Vec<(String, i32)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push_unique = |target: &mut Vec<(String, i32)>, path: String, idx: i32| {
        let k = format!("{}#{idx}", normalize_path_key(&path));
        if seen.insert(k) {
            target.push((path, idx));
        }
    };

    if let Some(raw_icon) = app.display_icon.as_deref() {
        if let Some((path, index)) = parse_display_icon_spec(raw_icon) {
            if !is_generic_shell_binary(&path) && Path::new(&path).is_file() {
                if looks_like_uninstaller_or_setup(&path) {
                    push_unique(&mut secondary, path, index);
                } else {
                    push_unique(&mut primary, path, index);
                }
            }
        }
    }

    for svc_exe in match_service_binaries_for_app(app, services) {
        push_unique(&mut primary, svc_exe, 0);
    }

    // Collect non-AppData / non-cache binary search roots (InstallLocation, registry fallback, and Body items).
    // Services still keep a hard wall against scanning user AppData folders (e.g. `.minecraft` on Xbox Service).
    let mut scan_roots: Vec<String> = Vec::new();
    if let Some(install_root) = app.install_location.as_deref().or(fallback_install_dir) {
        if !is_user_or_cache_folder(install_root) {
            scan_roots.push(install_root.to_string());
        }
    }
    if !matches!(app.no_touch, Some(NoTouchCategory::Service)) {
        for item in &app.items {
            if item.zone == Some(ZoneKind::Body) && !is_user_or_cache_folder(&item.path) {
                let k = normalize_path_key(&item.path);
                if !scan_roots.iter().any(|r| normalize_path_key(r) == k) {
                    scan_roots.push(item.path.clone());
                }
            }
        }
    }

    for root in &scan_roots {
        for exe in find_best_exes_in_install_dir(root, &app.display_name) {
            push_unique(&mut primary, exe, 0);
        }
    }

    // Store / MSIX packages often keep PNG artwork declared in AppxManifest.xml <Logo> rather than Win32 RT_ICONs.
    for root in &scan_roots {
        for png_path in find_appx_logo_pngs(root) {
            push_unique(&mut secondary, png_path, 0);
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

fn is_skipped_helper_exe_stem(stem_lower: &str) -> bool {
    let compact: String = stem_lower
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    const SKIP_TOKENS: &[&str] = &[
        "setup",
        "uninstall",
        "unins",
        "launcher",
        "changename",
        "crash",
        "report",
        "redist",
        "vcredist",
        "dxsetup",
        "update",
        "helper",
        "anticheat",
        "createdump",
    ];
    SKIP_TOKENS
        .iter()
        .any(|tok| stem_lower.contains(tok) || compact.contains(tok))
}

fn binaries_path_rank(rel_path: &Path) -> u8 {
    let mut has_binaries = false;
    let mut has_bin = false;
    let mut has_win64 = false;
    let mut has_win32 = false;
    if let Some(parent) = rel_path.parent() {
        for comp in parent.components() {
            let s = comp.as_os_str().to_string_lossy().to_ascii_lowercase();
            match s.as_str() {
                "binaries" => has_binaries = true,
                "bin" => has_bin = true,
                "win64" | "x64" => has_win64 = true,
                "win32" | "x86" => has_win32 = true,
                _ => {}
            }
        }
    }
    if has_binaries && has_win64 {
        3
    } else if has_binaries && has_win32 {
        2
    } else if has_binaries || has_bin || has_win64 {
        1
    } else {
        0
    }
}

fn identity_match_score(
    stem_lower: &str,
    app_tok: &str,
    folder_tok: &str,
    identity_words: &[String],
) -> u32 {
    let stem_tok = normalize_app_token(stem_lower);
    let core_words: Vec<&str> = stem_tok
        .split_whitespace()
        .filter(|w| {
            !matches!(
                *w,
                "win64" | "win32" | "x64" | "x86" | "shipping" | "retail" | "dx11" | "dx12"
            )
        })
        .collect();
    let core_stem = if core_words.is_empty() {
        stem_tok.clone()
    } else {
        core_words.join(" ")
    };

    let compact_core = core_stem.replace(' ', "");
    let compact_app = app_tok.replace(' ', "");
    let compact_folder = folder_tok.replace(' ', "");

    if (!app_tok.is_empty() && (core_stem == app_tok || compact_core == compact_app))
        || (!folder_tok.is_empty() && (core_stem == folder_tok || compact_core == compact_folder))
    {
        return 1000;
    }

    let matched_words = identity_words
        .iter()
        .filter(|w| stem_tok.contains(w.as_str()) || compact_core.contains(w.as_str()))
        .count() as u32;

    if compact_core.len() >= 4
        && ((!compact_app.is_empty()
            && (compact_app.contains(&compact_core) || compact_core.contains(&compact_app)))
            || (!compact_folder.is_empty()
                && (compact_folder.contains(&compact_core)
                    || compact_core.contains(&compact_folder))))
    {
        return 500 + matched_words * 50;
    }

    matched_words * 50
}

fn find_best_exes_in_install_dir(root: &str, display_name: &str) -> Vec<String> {
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        return Vec::new();
    }

    let app_tok = normalize_app_token(display_name);
    let folder_tok = root_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(normalize_app_token)
        .unwrap_or_default();
    let mut identity_words: Vec<String> = Vec::new();
    for src in [app_tok.as_str(), folder_tok.as_str()] {
        for w in src.split_whitespace() {
            if w.len() >= 2 && !identity_words.iter().any(|existing| existing == w) {
                identity_words.push(w.to_string());
            }
        }
    }

    // BFS across the app folder tree (depth < 12, capped at 2048 dirs) so deeply nested
    // principal binaries such as `<App>\Engine\Binaries\Win64\<App>-Win64-Shipping.exe` are found.
    let mut dirs_to_check: Vec<PathBuf> = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root_path.to_path_buf(), 0));
    while let Some((dir, depth)) = queue.pop_front() {
        dirs_to_check.push(dir.clone());
        if dirs_to_check.len() >= 2048 || depth >= 12 {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut children: Vec<PathBuf> = Vec::new();
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
                            | "_commonredist"
                            | "commonredist"
                            | "support"
                            | "directx"
                            | "crashdumps"
                            | "doc"
                            | "docs"
                            | "content"
                            | "movies"
                            | "paks"
                            | "assets"
                            | "node_modules"
                            | "saved"
                            | "intermediate"
                            | ".git"
                            | "__pycache__"
                    ) {
                        continue;
                    }
                    children.push(ent.path());
                }
            }
            children.sort();
            for c in children {
                queue.push_back((c, depth + 1));
            }
        }
    }

    let mut scored: Vec<(u32, u64, u8, String)> = Vec::new();
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
            if is_generic_shell_binary(&path_str) {
                continue;
            }
            let stem_lower = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if is_skipped_helper_exe_stem(&stem_lower) {
                continue;
            }

            let meta_bytes = ent.metadata().map(|m| m.len()).unwrap_or(0);
            let name_score =
                identity_match_score(&stem_lower, &app_tok, &folder_tok, &identity_words);
            let rel_path = p.strip_prefix(root_path).unwrap_or(&p);
            let path_rank = binaries_path_rank(rel_path);
            scored.push((name_score, meta_bytes, path_rank, path_str));
        }
    }

    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.3.cmp(&b.3))
    });
    scored.into_iter().take(4).map(|x| x.3).collect()
}

fn extract_png_via_gdiplus(path: &str) -> Option<(usize, usize, Vec<u8>)> {
    #[repr(C)]
    struct GdiplusStartupInput {
        gdiplus_version: u32,
        debug_event_callback: usize,
        suppress_background_thread: i32,
        suppress_external_codecs: i32,
    }
    #[repr(C)]
    struct GpRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }
    #[repr(C)]
    struct BitmapData {
        width: u32,
        height: u32,
        stride: i32,
        pixel_format: i32,
        scan0: *mut u8,
        reserved: usize,
    }
    #[link(name = "gdiplus")]
    unsafe extern "system" {
        fn GdiplusStartup(
            token: *mut usize,
            input: *const GdiplusStartupInput,
            output: *mut core::ffi::c_void,
        ) -> i32;
        fn GdipCreateBitmapFromFile(
            filename: *const u16,
            bitmap: *mut *mut core::ffi::c_void,
        ) -> i32;
        fn GdipGetImageWidth(image: *mut core::ffi::c_void, width: *mut u32) -> i32;
        fn GdipGetImageHeight(image: *mut core::ffi::c_void, height: *mut u32) -> i32;
        fn GdipBitmapLockBits(
            bitmap: *mut core::ffi::c_void,
            rect: *const GpRect,
            flags: u32,
            format: i32,
            locked_bitmap_data: *mut BitmapData,
        ) -> i32;
        fn GdipBitmapUnlockBits(
            bitmap: *mut core::ffi::c_void,
            locked_bitmap_data: *mut BitmapData,
        ) -> i32;
    }
    const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A;
    const IMAGE_LOCK_MODE_READ: u32 = 1;

    let mut token: usize = 0;
    let input = GdiplusStartupInput {
        gdiplus_version: 1,
        debug_event_callback: 0,
        suppress_background_thread: 0,
        suppress_external_codecs: 0,
    };
    if unsafe { GdiplusStartup(&mut token, &input, std::ptr::null_mut()) } != 0 || token == 0 {
        return None;
    }
    struct GdipTokenGuard(usize);
    impl Drop for GdipTokenGuard {
        fn drop(&mut self) {
            #[link(name = "gdiplus")]
            unsafe extern "system" {
                fn GdiplusShutdown(token: usize);
            }
            unsafe { GdiplusShutdown(self.0) };
        }
    }
    let _token_guard = GdipTokenGuard(token);

    let wide = to_wide_null(path);
    let mut gp_bmp: *mut core::ffi::c_void = std::ptr::null_mut();
    if unsafe { GdipCreateBitmapFromFile(wide.as_ptr(), &mut gp_bmp) } != 0 || gp_bmp.is_null() {
        return None;
    }
    struct GdipImgGuard(*mut core::ffi::c_void);
    impl Drop for GdipImgGuard {
        fn drop(&mut self) {
            #[link(name = "gdiplus")]
            unsafe extern "system" {
                fn GdipDisposeImage(image: *mut core::ffi::c_void) -> i32;
            }
            if !self.0.is_null() {
                unsafe {
                    let _ = GdipDisposeImage(self.0);
                }
            }
        }
    }
    let _img_guard = GdipImgGuard(gp_bmp);

    let (mut w, mut h) = (0u32, 0u32);
    if unsafe { GdipGetImageWidth(gp_bmp, &mut w) } != 0
        || unsafe { GdipGetImageHeight(gp_bmp, &mut h) } != 0
        || w == 0
        || h == 0
        || w > 1024
        || h > 1024
    {
        return None;
    }

    let rect = GpRect {
        x: 0,
        y: 0,
        width: w as i32,
        height: h as i32,
    };
    let mut bd = BitmapData {
        width: 0,
        height: 0,
        stride: 0,
        pixel_format: 0,
        scan0: std::ptr::null_mut(),
        reserved: 0,
    };
    if unsafe {
        GdipBitmapLockBits(
            gp_bmp,
            &rect,
            IMAGE_LOCK_MODE_READ,
            PIXEL_FORMAT_32BPP_ARGB,
            &mut bd,
        )
    } != 0
        || bd.scan0.is_null()
    {
        return None;
    }

    let uw = w as usize;
    let uh = h as usize;
    let mut rgba = Vec::with_capacity(uw * uh * 4);
    for y in 0..uh {
        let row_ptr = unsafe { bd.scan0.offset((y as isize) * (bd.stride as isize)) };
        let row = unsafe { std::slice::from_raw_parts(row_ptr, uw * 4) };
        for px in row.chunks_exact(4) {
            rgba.push(px[2]);
            rgba.push(px[1]);
            rgba.push(px[0]);
            rgba.push(px[3]);
        }
    }
    unsafe {
        let _ = GdipBitmapUnlockBits(gp_bmp, &mut bd);
    }
    Some((uw, uh, rgba))
}

fn extract_icon_from_file(path: &str, icon_index: i32) -> Option<CachedAppIcon> {
    if path
        .rsplit('.')
        .next()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        let (src_w, src_h, src_rgba) = extract_png_via_gdiplus(path)?;
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
        return Some(CachedAppIcon::from_rgba(APP_ICON_PX, APP_ICON_PX, &out_rgba));
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use y_core::discovery::path_drive_letter;
    use y_core::model::{
        AppCategory, AttributionBasis, BreakdownItem, DriveInfo, GrowthDelta,
        RegistryDiscoveryStats,
    };

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
        let mut a: u32 = 1;
        let mut b: u32 = 0;
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn png_chunk(tag: &[u8; 4], payload: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(payload);
        let mut crc_input = Vec::with_capacity(4 + payload.len());
        crc_input.extend_from_slice(tag);
        crc_input.extend_from_slice(payload);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    }

    fn build_uncompressed_rgba_png_ico(size: u32) -> Vec<u8> {
        let mut raw_scanlines = Vec::with_capacity((size as usize) * (size as usize * 4 + 1));
        for y in 0..size {
            raw_scanlines.push(0); // filter 0
            for x in 0..size {
                let inside = (4..size - 4).contains(&x) && (4..size - 4).contains(&y);
                if inside {
                    raw_scanlines.extend_from_slice(&[240, 240, 245, 255]);
                } else {
                    raw_scanlines.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        let len = raw_scanlines.len() as u16;
        let nlen = !len;
        let mut zlib = Vec::with_capacity(raw_scanlines.len() + 11);
        zlib.extend_from_slice(&[0x78, 0x01, 0x01]); // zlib header + final stored block
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&nlen.to_le_bytes());
        zlib.extend_from_slice(&raw_scanlines);
        zlib.extend_from_slice(&adler32(&raw_scanlines).to_be_bytes());

        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&size.to_be_bytes());
        ihdr.extend_from_slice(&size.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
        png_chunk(b"IHDR", &ihdr, &mut png);
        png_chunk(b"IDAT", &zlib, &mut png);
        png_chunk(b"IEND", &[], &mut png);

        // Wrap PNG payload inside a standard single-entry Windows .ico (width/height 0 == 256 convention / PNG entry).
        let mut ico = Vec::with_capacity(22 + png.len());
        ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // ICON
        ico.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        ico.push(0); // width (0 denotes 256 / PNG)
        ico.push(0); // height
        ico.push(0); // color count
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&22u32.to_le_bytes()); // offset
        ico.extend_from_slice(&png);
        ico
    }

    #[test]
    fn c1_icon_populates_when_display_icon_sits_on_excluded_drive() {
        let dir = std::env::temp_dir().join(format!(
            "y_storage_c1_icon_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let ico_path = dir.join("app.ico");
        std::fs::write(&ico_path, build_uncompressed_rgba_png_ico(32)).unwrap();
        let ico_str = normalize_windows_path(&ico_path.to_string_lossy());
        let icon_drive = path_drive_letter(&ico_str).unwrap_or('C').to_ascii_uppercase();
        let other_drive = if icon_drive == 'Z' { 'D' } else { 'Z' };

        let mut report = ScanReport {
            scan_timestamp_unix: 100,
            scan_timestamp_display: "2026-11-28 02:30:00".into(),
            duration_ms: 1,
            drives: vec![
                DriveInfo {
                    letter: icon_drive,
                    root: format!("{icon_drive}:\\"),
                    total_bytes: 1_000,
                    free_bytes: 500,
                    used_bytes: 500,
                    is_system_drive: icon_drive == 'C',
                    full_walk_enabled: false,
                    exclusions: vec![],
                    last_scanned_unix: None,
                    last_scanned_display: None,
                },
                DriveInfo {
                    letter: other_drive,
                    root: format!("{other_drive}:\\"),
                    total_bytes: 1_000,
                    free_bytes: 500,
                    used_bytes: 500,
                    is_system_drive: false,
                    full_walk_enabled: true,
                    exclusions: vec![],
                    last_scanned_unix: Some(100),
                    last_scanned_display: Some("2026-11-28 02:30:00".into()),
                },
            ],
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: 1,
                raw_with_install_location: 0,
                canonical_apps: 1,
                canonical_with_direct_install_location: 0,
                canonical_with_inferred_install_location: 0,
            },
            apps: vec![AttributedApp {
                id: "synth:sample-cross-drive-tool".into(),
                display_name: "Sample Cross-Drive Tool".into(),
                publishers: vec!["Example Corp".into()],
                install_location: None,
                total_bytes: 120_000_000,
                previous_bytes: None,
                growth: GrowthDelta::NoBaseline,
                per_drive_growth: BTreeMap::new(),
                no_touch: None,
                bases: vec![AttributionBasis::Token],
                items: vec![BreakdownItem {
                    path: format!("{other_drive}:\\Users\\Test\\AppData\\Roaming\\SampleTool"),
                    bytes: 120_000_000,
                    zone: Some(ZoneKind::Red),
                    qualifier: None,
                    basis: Some(AttributionBasis::Token),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: other_drive,
                }],
                carved_out_apps: vec![],
                dependencies: vec![],
                display_icon: Some(ico_str),
                icon_rgba: None,
                category: AppCategory::Development,
                category_basis: "test".into(),
            }],
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![],
            your_files_total_bytes: 0,
            windows_system: vec![],
            windows_system_total_bytes: 0,
            unattributed: vec![],
            unattributed_total_bytes: 0,
            reverse_dependencies: vec![],
            installed_registry_names: vec![],
            catalog_source: "test".into(),
            warnings: vec![],
            applied_containers: None,
            applied_zones: None,
            evidence_curated_conflicts: vec![],
        };

        populate_report_icons(&mut report);
        let _ = std::fs::remove_dir_all(&dir);

        let populated = report.apps[0]
            .icon_rgba
            .as_ref()
            .expect("icon_rgba must populate even when display_icon lives on an unchecked drive");
        assert_eq!(populated.width, APP_ICON_PX);
        assert_eq!(populated.height, APP_ICON_PX);
        let decoded = populated.decode_rgba().expect("valid decoded rgba");
        assert_eq!(decoded.len(), (APP_ICON_PX as usize) * (APP_ICON_PX as usize) * 4);
        assert!(decoded.chunks_exact(4).any(|px| px[3] >= 24));
        assert!(report.apps[0].install_location.is_none());
    }

    fn write_dummy_file(path: &Path, size_bytes: usize) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, vec![0u8; size_bytes]).unwrap();
    }

    #[test]
    fn folder_fallback_ranks_nested_principal_exe_over_top_level_helpers() {
        let root = std::env::temp_dir().join(format!(
            "y_storage_icon_rank_principal_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let app_dir = root.join("SparkingZERO");
        std::fs::create_dir_all(&app_dir).unwrap();

        write_dummy_file(&app_dir.join("SparkingZERO_Launcher.exe"), 786 * 1024);
        write_dummy_file(&app_dir.join("Change Name.exe"), 35 * 1024);
        write_dummy_file(&app_dir.join("CrashReportClient.exe"), 500 * 1024);
        write_dummy_file(&app_dir.join("vcredist_x64.exe"), 1024 * 1024);
        write_dummy_file(&app_dir.join("misc_tool.exe"), 50 * 1024);
        let principal = app_dir
            .join("SparkingZERO")
            .join("Engine")
            .join("Binaries")
            .join("Win64")
            .join("SparkingZERO-Win64-Shipping.exe");
        write_dummy_file(&principal, 2 * 1024 * 1024);

        let ranked = find_best_exes_in_install_dir(
            &app_dir.to_string_lossy(),
            "DRAGON BALL: Sparking! ZERO",
        );
        let _ = std::fs::remove_dir_all(&root);

        assert!(!ranked.is_empty(), "expected at least one exe candidate");
        assert_eq!(
            ranked[0],
            normalize_windows_path(&principal.to_string_lossy()),
            "nested principal exe should win over top-level launcher and helpers; ranked={ranked:?}"
        );
        for candidate in &ranked {
            let lower = candidate.to_ascii_lowercase();
            assert!(
                !lower.contains("launcher")
                    && !lower.contains("change name")
                    && !lower.contains("crashreport")
                    && !lower.contains("vcredist"),
                "helper binary should have been hard-skipped: {candidate}"
            );
        }
    }

    #[test]
    fn folder_fallback_prefers_larger_size_and_binaries_win64_on_name_tie() {
        let root = std::env::temp_dir().join(format!(
            "y_storage_icon_rank_tiebreak_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        let size_tie_dir = root.join("AlphaGame");
        let small_top = size_tie_dir.join("AlphaGame.exe");
        let large_nested = size_tie_dir.join("Game").join("AlphaGame.exe");
        write_dummy_file(&small_top, 32 * 1024);
        write_dummy_file(&large_nested, 1024 * 1024);

        let size_ranked = find_best_exes_in_install_dir(
            &size_tie_dir.to_string_lossy(),
            "AlphaGame",
        );
        assert_eq!(
            size_ranked.first().map(String::as_str),
            Some(normalize_windows_path(&large_nested.to_string_lossy()).as_str()),
            "larger exe should beat smaller top-level exe on equal name match"
        );

        let path_tie_dir = root.join("BetaGame");
        let top_equal = path_tie_dir.join("BetaGame.exe");
        let win64_equal = path_tie_dir
            .join("Beta")
            .join("Binaries")
            .join("Win64")
            .join("BetaGame.exe");
        write_dummy_file(&top_equal, 512 * 1024);
        write_dummy_file(&win64_equal, 512 * 1024);

        let path_ranked = find_best_exes_in_install_dir(
            &path_tie_dir.to_string_lossy(),
            "BetaGame",
        );
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(
            path_ranked.first().map(String::as_str),
            Some(normalize_windows_path(&win64_equal.to_string_lossy()).as_str()),
            "Binaries\\Win64 exe should beat top-level exe on equal name match and size"
        );
    }
}
