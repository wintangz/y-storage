use std::collections::BTreeSet;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use egui::{
    Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2, ViewportId,
};
use serde::Serialize;
use windows_sys::Win32::Foundation::{FILETIME, FreeLibrary, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateBitmap, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EndPaint, GetDC, HDC, HGDIOBJ,
    InvalidateRect, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject, UpdateWindow,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, OpenClipboard,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, HICON, ICON_BIG, ICON_SMALL, ICONINFO,
    IDC_ARROW, IDC_IBEAM, IDC_SIZENS, IMAGE_ICON, KillTimer, LR_DEFAULTCOLOR, LoadCursorW,
    LoadImageW, MSG, PM_REMOVE, PeekMessageW, PostMessageW, PostQuitMessage, RegisterClassExW,
    SM_CXICON, SM_CXSMICON, SM_CXVIRTUALSCREEN, SM_CYICON, SM_CYSMICON, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, SW_SHOWMAXIMIZED, SWP_NOACTIVATE, SWP_NOZORDER,
    SendMessageW, SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    TranslateMessage, WINDOWPLACEMENT, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_KEYUP,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEWHEEL, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SETCURSOR, WM_SETICON, WM_SIZE, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use y_core::cache::{FolderSizeCache, default_last_scan_path, load_last_scan, save_last_scan_atomic};
use y_core::config::{AppConfig, ThemeMode, WindowPlacementConfig};
use y_core::history::{ScanHistory, ScanScopeKind};
use y_core::model::ScanReport;
use y_platform_windows::paths::{enumerate_fixed_drives, get_windows_directory};
use y_platform_windows::scanner::{
    ScanProgressSnapshot, execute_scan_with_history, execute_single_drive_rescan,
};

use crate::sw_painter::{PaintStats, SoftwarePainter};
use crate::theme::{CLEAR_BG_DARK_RGB, CLEAR_BG_LIGHT_RGB};
use crate::ui::{
    RulebookBrowseTarget, StorageUiState, UiAction, configure_fonts_and_theme,
    configure_fonts_and_theme_with, draw_app_ui_with_drive_busy,
};

const WM_APP_SCAN_UPDATE: u32 = 0x8000 + 10;
const WM_APP_FOLLOWUP_PAINT: u32 = 0x8000 + 11;
const WM_APP_BROWSE_FOLDER: u32 = 0x8000 + 12;
const TIMER_REPAINT_ID: usize = 9000;
const TIMER_BENCH_DONE_ID: usize = 9001;
const TIMER_SAVE_CONFIG_ID: usize = 9002;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_SETFOCUS_MSG: u32 = 0x0007;
const WM_KILLFOCUS_MSG: u32 = 0x0008;
const WM_MOUSELEAVE_MSG: u32 = 0x02A3;
const TME_LEAVE_FLAG: u32 = 0x0000_0002;

#[repr(C)]
struct WinTrackMouseEvent {
    cb_size: u32,
    dw_flags: u32,
    hwnd_track: HWND,
    dw_hover_time: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn TrackMouseEvent(lp_event_track: *mut WinTrackMouseEvent) -> i32;
    fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> i32;
    fn SetForegroundWindow(hwnd: HWND) -> i32;
}

const DEFAULT_LOGICAL_W: i32 = 1020;
const DEFAULT_LOGICAL_H: i32 = 720;
const MIN_LOGICAL_W: i32 = 760;
const MIN_LOGICAL_H: i32 = 520;
const GWLP_USERDATA: i32 = -21;

#[repr(C)]
struct MinMaxInfo {
    pt_reserved: windows_sys::Win32::Foundation::POINT,
    pt_max_size: windows_sys::Win32::Foundation::POINT,
    pt_max_position: windows_sys::Win32::Foundation::POINT,
    pt_min_track_size: windows_sys::Win32::Foundation::POINT,
    pt_max_track_size: windows_sys::Win32::Foundation::POINT,
}

pub struct AppController {
    pub config: AppConfig,
    pub config_path: std::path::PathBuf,
    pub history: ScanHistory,
    pub history_path: std::path::PathBuf,
    pub cache: FolderSizeCache,
    pub cache_path: std::path::PathBuf,
    pub last_scan_path: std::path::PathBuf,
    pub report: Option<Arc<ScanReport>>,
    pub scanning: bool,
    pub rescanning_drives: BTreeSet<char>,
    pub progress: Option<ScanProgressSnapshot>,
    pub system_windows_dir: String,
    pub hwnd: usize,
}

impl AppController {
    pub fn load_initial() -> Self {
        let drives = enumerate_fixed_drives();
        let sys_win = get_windows_directory();
        let config_path = AppConfig::default_path();
        let history_path = ScanHistory::default_path();
        let cache_path = FolderSizeCache::default_path();
        let last_scan_path = default_last_scan_path();

        let config = AppConfig::load_or_default(&config_path, &drives, &sys_win);
        let history = ScanHistory::load_or_default(&history_path);
        let cache = FolderSizeCache::load_or_default(&cache_path);
        let mut report = load_last_scan(&last_scan_path);
        if let Some(ref mut rep) = report {
            let mut updated = false;
            for d in &mut rep.drives {
                if d.full_walk_enabled && d.last_scanned_unix.is_none() {
                    d.last_scanned_unix = Some(rep.scan_timestamp_unix);
                    d.last_scanned_display = Some(rep.scan_timestamp_display.clone());
                    updated = true;
                }
            }
            if !rep.apps.is_empty() && rep.apps.iter().all(|a| a.icon_rgba.is_none()) {
                y_platform_windows::icons::populate_report_icons(rep);
                updated = true;
            }
            if !rep.apps.is_empty() && rep.apps.iter().all(|a| a.category_basis.is_empty()) {
                y_core::categories::classify_report_apps(
                    rep,
                    y_platform_windows::paths::has_game_manifest_dir,
                );
                updated = true;
            }
            if updated {
                let _ = save_last_scan_atomic(&last_scan_path, rep);
            }
        }

        Self {
            config,
            config_path,
            history,
            history_path,
            cache,
            cache_path,
            last_scan_path,
            report: report.map(Arc::new),
            scanning: false,
            rescanning_drives: BTreeSet::new(),
            progress: None,
            system_windows_dir: sys_win,
            hwnd: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FrameTimingSample {
    total_ms: f64,
    lock_clone_ms: f64,
    egui_run_ms: f64,
    tessellate_ms: f64,
    raster_blit_ms: f64,
    paint: PaintStats,
}

struct WindowHostState {
    controller: Arc<Mutex<AppController>>,
    ui_state: StorageUiState,
    egui_ctx: egui::Context,
    painter: SoftwarePainter,
    applied_titlebar_theme: Option<ThemeMode>,
    large_icon: HICON,
    small_icon: HICON,
    events: Vec<Event>,
    width: usize,
    height: usize,
    dpi: u32,
    pixels_per_point: f32,
    needs_redraw: bool,
    followup_budget: u8,
    window_focused: bool,
    last_mouse_px: Option<(i32, i32)>,
    tracking_mouse_leave: bool,
    hover_cursor_icon: egui::CursorIcon,
    repaint_timer_active: bool,
    config_save_dirty: bool,
    pending_browse_target: Option<RulebookBrowseTarget>,
    smoke_mode: bool,
    start_time: Instant,
    bench_recording: bool,
    bench_idle_wm_paints: usize,
    bench_timer_fired: bool,
    bench_samples: Vec<FrameTimingSample>,
}

impl Drop for WindowHostState {
    fn drop(&mut self) {
        unsafe {
            if !self.small_icon.is_null() {
                DestroyIcon(self.small_icon);
                self.small_icon = null_mut();
            }
            if !self.large_icon.is_null() {
                DestroyIcon(self.large_icon);
                self.large_icon = null_mut();
            }
        }
    }
}

#[inline]
pub fn dpi_to_ppp(dpi: u32) -> f32 {
    (dpi.max(96) as f32) / 96.0
}

fn get_dpi_for_system() -> u32 {
    type FnGetDpiForSystem = unsafe extern "system" fn() -> u32;
    let dll: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    if !module.is_null() {
        if let Some(proc) = unsafe { GetProcAddress(module, b"GetDpiForSystem\0".as_ptr()) } {
            let f: FnGetDpiForSystem = unsafe { std::mem::transmute_copy(&proc) };
            let dpi = unsafe { f() };
            if dpi >= 72 {
                return dpi;
            }
        }
    }
    96
}

fn get_dpi_for_window(hwnd: HWND) -> u32 {
    if hwnd.is_null() {
        return get_dpi_for_system();
    }
    type FnGetDpiForWindow = unsafe extern "system" fn(HWND) -> u32;
    let dll: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    if !module.is_null() {
        if let Some(proc) = unsafe { GetProcAddress(module, b"GetDpiForWindow\0".as_ptr()) } {
            let f: FnGetDpiForWindow = unsafe { std::mem::transmute_copy(&proc) };
            let dpi = unsafe { f(hwnd) };
            if dpi >= 72 {
                return dpi;
            }
        }
    }
    get_dpi_for_system()
}

pub fn outer_size_for_dpi(
    client_logical_w: i32,
    client_logical_h: i32,
    style: u32,
    dpi: u32,
) -> (i32, i32) {
    let ppp = dpi_to_ppp(dpi);
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: (client_logical_w as f32 * ppp).round() as i32,
        bottom: (client_logical_h as f32 * ppp).round() as i32,
    };
    type FnAdjustForDpi = unsafe extern "system" fn(*mut RECT, u32, i32, u32, u32) -> i32;
    let dll: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    let adjusted = if !module.is_null() {
        if let Some(proc) = unsafe { GetProcAddress(module, b"AdjustWindowRectExForDpi\0".as_ptr()) }
        {
            let f: FnAdjustForDpi = unsafe { std::mem::transmute_copy(&proc) };
            unsafe { f(&mut rc, style, 0, 0, dpi.max(96)) != 0 }
        } else {
            unsafe { AdjustWindowRectEx(&mut rc, style, 0, 0) != 0 }
        }
    } else {
        unsafe { AdjustWindowRectEx(&mut rc, style, 0, 0) != 0 }
    };
    if adjusted {
        (rc.right - rc.left, rc.bottom - rc.top)
    } else {
        (
            (client_logical_w as f32 * ppp).round() as i32 + 16,
            (client_logical_h as f32 * ppp).round() as i32 + 39,
        )
    }
}

fn create_hicon_from_rgba(size: usize, rgba: &[u8]) -> HICON {
    if size == 0 || rgba.len() < size * size * 4 {
        return null_mut();
    }
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: (size * size * 4) as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }],
    };
    let mut bits: *mut std::ffi::c_void = null_mut();
    let hbm_color = unsafe { CreateDIBSection(null_mut(), &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    if hbm_color.is_null() || bits.is_null() {
        if !hbm_color.is_null() {
            unsafe { DeleteObject(hbm_color as HGDIOBJ) };
        }
        return null_mut();
    }
    unsafe {
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, size * size * 4);
        for px in 0..(size * size) {
            let s = px * 4;
            dst[s] = rgba[s + 2];
            dst[s + 1] = rgba[s + 1];
            dst[s + 2] = rgba[s];
            dst[s + 3] = rgba[s + 3];
        }
    }
    let mask_bytes_len = (((size + 15) / 16) * 2) * size;
    let mask_buf = vec![0u8; mask_bytes_len];
    let hbm_mask = unsafe { CreateBitmap(size as i32, size as i32, 1, 1, mask_buf.as_ptr().cast()) };
    if hbm_mask.is_null() {
        unsafe { DeleteObject(hbm_color as HGDIOBJ) };
        return null_mut();
    }
    let ii = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };
    let icon = unsafe { CreateIconIndirect(&ii) };
    unsafe {
        DeleteObject(hbm_mask as HGDIOBJ);
        DeleteObject(hbm_color as HGDIOBJ);
    }
    icon
}

fn load_app_icons_for_dpi(hinstance: HINSTANCE, dpi: u32) -> (HICON, HICON) {
    let dpi = dpi.max(96);
    let lg_px = ((32 * dpi + 48) / 96).clamp(32, 128) as usize;
    let sm_px = ((16 * dpi + 48) / 96).clamp(16, 64) as usize;

    let mut large = create_hicon_from_rgba(lg_px, &crate::logo::rasterize_wt_logo_rgba(lg_px));
    let mut small = create_hicon_from_rgba(sm_px, &crate::logo::rasterize_wt_logo_rgba(sm_px));

    let res_id = 1usize as *const u16;
    if large.is_null() {
        let cx_lg = unsafe { GetSystemMetrics(SM_CXICON) }.max(32);
        let cy_lg = unsafe { GetSystemMetrics(SM_CYICON) }.max(32);
        large = unsafe { LoadImageW(hinstance, res_id, IMAGE_ICON, cx_lg, cy_lg, LR_DEFAULTCOLOR) }
            as HICON;
    }
    if small.is_null() {
        let cx_sm = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let cy_sm = unsafe { GetSystemMetrics(SM_CYSMICON) }.max(16);
        small = unsafe { LoadImageW(hinstance, res_id, IMAGE_ICON, cx_sm, cy_sm, LR_DEFAULTCOLOR) }
            as HICON;
    }
    (large, small)
}

fn load_app_icons(hinstance: HINSTANCE) -> (HICON, HICON) {
    load_app_icons_for_dpi(hinstance, get_dpi_for_system())
}

fn apply_titlebar_theme(hwnd: HWND, mode: ThemeMode) {
    let dll: Vec<u16> = "dwmapi.dll\0".encode_utf16().collect();
    let module = unsafe { LoadLibraryW(dll.as_ptr()) };
    if module.is_null() {
        return;
    }
    type DwmSetAttrFn =
        unsafe extern "system" fn(HWND, u32, *const std::ffi::c_void, u32) -> i32;
    if let Some(proc) = unsafe { GetProcAddress(module, b"DwmSetWindowAttribute\0".as_ptr()) } {
        let set_attr: DwmSetAttrFn = unsafe { std::mem::transmute_copy(&proc) };
        let dark_mode: i32 = match mode {
            ThemeMode::Dark => 1,
            ThemeMode::Light => 0,
        };
        unsafe {
            set_attr(
                hwnd,
                20,
                (&dark_mode as *const i32).cast(),
                size_of::<i32>() as u32,
            );
        }
        let caption_color: u32 = match mode {
            ThemeMode::Dark => 0x000A_0A0A,
            ThemeMode::Light => 0x00F2_F2F2,
        };
        unsafe {
            set_attr(
                hwnd,
                35,
                (&caption_color as *const u32).cast(),
                size_of::<u32>() as u32,
            );
        }
    }
    unsafe { FreeLibrary(module) };
}

fn trigger_background_scan(controller: Arc<Mutex<AppController>>) {
    let (config, history, hwnd_usize) = {
        let mut guard = controller.lock().unwrap();
        if guard.scanning {
            return;
        }
        guard.scanning = true;
        guard.progress = Some(ScanProgressSnapshot {
            phase: "Starting scan".into(),
            current_path: String::new(),
            dirs_walked: 0,
            files_counted: 0,
            bytes_counted: 0,
        });
        (guard.config.clone(), guard.history.clone(), guard.hwnd)
    };

    if hwnd_usize != 0 {
        unsafe { PostMessageW(hwnd_usize as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
    }

    std::thread::spawn(move || {
        let ctrl_prog = Arc::clone(&controller);
        let report = execute_scan_with_history(
            &config,
            &history,
            Some(&move |snap| {
                if let Ok(mut g) = ctrl_prog.try_lock() {
                    g.progress = Some(snap);
                    if g.hwnd != 0 {
                        unsafe { PostMessageW(g.hwnd as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
                    }
                }
            }),
        );

        let refreshed_drives: Vec<char> = report
            .drives
            .iter()
            .filter(|d| d.full_walk_enabled)
            .map(|d| d.letter.to_ascii_uppercase())
            .collect();
        let mut guard = controller.lock().unwrap();
        guard
            .history
            .record_report_with_scope(&report, ScanScopeKind::Full, &refreshed_drives);
        let _ = guard.history.save_atomic(&guard.history_path);
        guard.cache.update_from_report(&report);
        let _ = guard.cache.save_atomic(&guard.cache_path);
        let _ = save_last_scan_atomic(&guard.last_scan_path, &report);
        let _ = guard.config.save_atomic(&guard.config_path);
        guard.report = Some(Arc::new(report));
        guard.scanning = false;
        guard.progress = None;
        let hwnd_done = guard.hwnd;
        drop(guard);

        if hwnd_done != 0 {
            unsafe { PostMessageW(hwnd_done as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
        }
    });
}

fn trigger_single_drive_rescan(controller: Arc<Mutex<AppController>>, drive_letter: char) {
    let dl = drive_letter.to_ascii_uppercase();
    let (base_opt, config, history, hwnd_usize) = {
        let mut guard = controller.lock().unwrap();
        if guard.scanning || guard.rescanning_drives.contains(&dl) {
            return;
        }
        if !guard.config.is_drive_enabled(dl) {
            return;
        }
        guard.rescanning_drives.insert(dl);
        guard.progress = Some(ScanProgressSnapshot {
            phase: format!("Rescanning {dl}:\\"),
            current_path: String::new(),
            dirs_walked: 0,
            files_counted: 0,
            bytes_counted: 0,
        });
        (
            guard.report.as_ref().map(|r| (**r).clone()),
            guard.config.clone(),
            guard.history.clone(),
            guard.hwnd,
        )
    };

    if hwnd_usize != 0 {
        unsafe { PostMessageW(hwnd_usize as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
    }

    std::thread::spawn(move || {
        let ctrl_prog = Arc::clone(&controller);
        let progress_cb = move |snap| {
            if let Ok(mut g) = ctrl_prog.try_lock() {
                g.progress = Some(snap);
                if g.hwnd != 0 {
                    unsafe { PostMessageW(g.hwnd as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
                }
            }
        };
        let (report, scope_kind, refreshed) = match base_opt {
            Some(ref base) => (
                execute_single_drive_rescan(base, dl, &config, &history, Some(&progress_cb)),
                ScanScopeKind::Partial,
                vec![dl],
            ),
            None => {
                let rep = execute_scan_with_history(&config, &history, Some(&progress_cb));
                let list: Vec<char> = rep
                    .drives
                    .iter()
                    .filter(|d| d.full_walk_enabled)
                    .map(|d| d.letter.to_ascii_uppercase())
                    .collect();
                (rep, ScanScopeKind::Full, list)
            }
        };

        let mut guard = controller.lock().unwrap();
        guard
            .history
            .record_report_with_scope(&report, scope_kind, &refreshed);
        let _ = guard.history.save_atomic(&guard.history_path);
        guard.cache.update_from_report(&report);
        let _ = guard.cache.save_atomic(&guard.cache_path);
        let _ = save_last_scan_atomic(&guard.last_scan_path, &report);
        let _ = guard.config.save_atomic(&guard.config_path);
        guard.report = Some(Arc::new(report));
        guard.rescanning_drives.remove(&dl);
        if guard.rescanning_drives.is_empty() && !guard.scanning {
            guard.progress = None;
        }
        let hwnd_done = guard.hwnd;
        drop(guard);

        if hwnd_done != 0 {
            unsafe { PostMessageW(hwnd_done as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
        }
    });
}

pub fn run_gui_app(
    controller: Arc<Mutex<AppController>>,
    smoke_ms: Option<u64>,
) -> Result<(), String> {
    let hinstance = unsafe { GetModuleHandleW(null()) } as HINSTANCE;
    let class_name: Vec<u16> = "YStorageMainWindow\0".encode_utf16().collect();
    let title: Vec<u16> = "y-storage — True App Storage Footprint\0"
        .encode_utf16()
        .collect();
    let (large_icon, small_icon) = load_app_icons(hinstance);

    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: large_icon,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: small_icon,
    };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return Err("RegisterClassExW failed".into());
    }

    let init_dpi = get_dpi_for_system();
    let (default_w, default_h) =
        outer_size_for_dpi(DEFAULT_LOGICAL_W, DEFAULT_LOGICAL_H, WS_OVERLAPPEDWINDOW, init_dpi);
    let (min_w, min_h) =
        outer_size_for_dpi(MIN_LOGICAL_W, MIN_LOGICAL_H, WS_OVERLAPPEDWINDOW, init_dpi);
    let (init_theme, saved_win, init_config_snap) = {
        let g = controller.lock().unwrap();
        (g.config.theme_mode, g.config.window, g.config.clone())
    };
    let (win_x, win_y, init_w, init_h, start_max) = if smoke_ms.is_none() {
        resolve_initial_window_bounds(saved_win, default_w, default_h, min_w, min_h)
    } else {
        (100, 60, default_w, default_h, false)
    };

    let egui_ctx = egui::Context::default();
    configure_fonts_and_theme_with(
        &egui_ctx,
        init_theme,
        init_config_snap.theme_preset,
        init_config_snap.theme_custom_rgb,
    );

    let host_state = Box::new(WindowHostState {
        controller: Arc::clone(&controller),
        ui_state: StorageUiState::from_config(&init_config_snap),
        egui_ctx,
        painter: SoftwarePainter::new(),
        applied_titlebar_theme: None,
        large_icon,
        small_icon,
        events: Vec::new(),
        width: (DEFAULT_LOGICAL_W as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        height: (DEFAULT_LOGICAL_H as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        dpi: init_dpi,
        pixels_per_point: dpi_to_ppp(init_dpi),
        needs_redraw: true,
        followup_budget: 2,
        window_focused: true,
        last_mouse_px: None,
        tracking_mouse_leave: false,
        hover_cursor_icon: egui::CursorIcon::Default,
        repaint_timer_active: false,
        config_save_dirty: false,
        pending_browse_target: None,
        smoke_mode: smoke_ms.is_some(),
        start_time: Instant::now(),
        bench_recording: false,
        bench_idle_wm_paints: 0,
        bench_timer_fired: false,
        bench_samples: Vec::new(),
    });
    let state_ptr = Box::into_raw(host_state);

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            win_x,
            win_y,
            init_w,
            init_h,
            null_mut(),
            null_mut(),
            hinstance,
            state_ptr.cast(),
        )
    };
    if hwnd.is_null() {
        unsafe { drop(Box::from_raw(state_ptr)) };
        return Err("CreateWindowExW failed".into());
    }

    unsafe {
        if !large_icon.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, large_icon as isize);
        }
        if !small_icon.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small_icon as isize);
        }
    }

    controller.lock().unwrap().hwnd = hwnd as usize;
    apply_titlebar_theme(hwnd, init_theme);

    let actual_dpi = get_dpi_for_window(hwnd);
    let (cw, ch) = {
        let mut crc: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetClientRect(hwnd, &mut crc) };
        (
            (crc.right - crc.left).max(0) as usize,
            (crc.bottom - crc.top).max(0) as usize,
        )
    };
    unsafe {
        let st = &mut *state_ptr;
        st.dpi = actual_dpi;
        st.pixels_per_point = dpi_to_ppp(actual_dpi);
        st.width = cw;
        st.height = ch;
        st.applied_titlebar_theme = Some(init_theme);
        ShowWindow(hwnd, if start_max { SW_SHOWMAXIMIZED } else { SW_SHOW });
        InvalidateRect(hwnd, null(), 0);
        UpdateWindow(hwnd);
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    if let Some(ms) = smoke_ms {
        let deadline = Instant::now() + std::time::Duration::from_millis(ms);
        while Instant::now() < deadline {
            while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
                if msg.message == 0x0012 {
                    break;
                }
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        unsafe { DestroyWindow(hwnd) };
    } else {
        while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    // `WM_DESTROY` drops `WindowHostState` (including its `small_icon`/`large_icon` handles).
    Ok(())
}

fn cursor_res_for_egui(icon: egui::CursorIcon) -> *const u16 {
    match icon {
        egui::CursorIcon::Text => IDC_IBEAM,
        egui::CursorIcon::ResizeVertical => IDC_SIZENS,
        _ => IDC_ARROW,
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == 0x0081 {
        let cs = lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
        let ptr = unsafe { (*cs).lpCreateParams } as isize;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr) };
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowHostState;
    if state_ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    match msg {
        WM_ERASEBKGND => 1,
        0x0024 /* WM_GETMINMAXINFO */ => {
            let st = unsafe { &*state_ptr };
            if lparam != 0 {
                let mmi = unsafe { &mut *(lparam as *mut MinMaxInfo) };
                let (min_w, min_h) =
                    outer_size_for_dpi(MIN_LOGICAL_W, MIN_LOGICAL_H, WS_OVERLAPPEDWINDOW, st.dpi);
                mmi.pt_min_track_size.x = min_w;
                mmi.pt_min_track_size.y = min_h;
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_SIZE => {
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let st = unsafe { &mut *state_ptr };
            let nw = (rc.right - rc.left).max(0) as usize;
            let nh = (rc.bottom - rc.top).max(0) as usize;
            if nw != st.width || nh != st.height {
                st.width = nw;
                st.height = nh;
                st.needs_redraw = true;
                unsafe { InvalidateRect(hwnd, null(), 0) };
            }
            0
        }
        WM_DPICHANGED => {
            let new_dpi = ((wparam & 0xFFFF) as u32).max(96);
            {
                let hinst = unsafe { GetModuleHandleW(null()) } as HINSTANCE;
                let (new_lg, new_sm) = load_app_icons_for_dpi(hinst, new_dpi);
                let st = unsafe { &mut *state_ptr };
                st.dpi = new_dpi;
                st.pixels_per_point = dpi_to_ppp(new_dpi);
                st.needs_redraw = true;
                st.followup_budget = 2;
                if !new_lg.is_null() {
                    let old = std::mem::replace(&mut st.large_icon, new_lg);
                    unsafe {
                        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, new_lg as isize);
                        if !old.is_null() {
                            DestroyIcon(old);
                        }
                    }
                }
                if !new_sm.is_null() {
                    let old = std::mem::replace(&mut st.small_icon, new_sm);
                    unsafe {
                        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, new_sm as isize);
                        if !old.is_null() {
                            DestroyIcon(old);
                        }
                    }
                }
            }
            if lparam != 0 {
                let rect = unsafe { *(lparam as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            let mut crc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut crc) };
            let st = unsafe { &mut *state_ptr };
            st.width = (crc.right - crc.left).max(0) as usize;
            st.height = (crc.bottom - crc.top).max(0) as usize;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_APP_SCAN_UPDATE | WM_APP_FOLLOWUP_PAINT => {
            let st = unsafe { &mut *state_ptr };
            st.needs_redraw = true;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_APP_BROWSE_FOLDER => {
            let target = {
                let st = unsafe { &mut *state_ptr };
                st.pending_browse_target.take()
            };
            if let Some(target) = target {
                let title = match target {
                    RulebookBrowseTarget::ExplicitPath => "Select folder for explicit zone rule",
                    RulebookBrowseTarget::ManualPin => "Select folder to pin",
                };
                if let Some(picked) = pick_folder_dialog(hwnd, title) {
                    let st = unsafe { &mut *state_ptr };
                    match target {
                        RulebookBrowseTarget::ExplicitPath => {
                            st.ui_state.new_container_path_input = picked;
                        }
                        RulebookBrowseTarget::ManualPin => {
                            st.ui_state.new_pin_path_input = picked;
                        }
                    }
                    st.needs_redraw = true;
                    unsafe { InvalidateRect(hwnd, null(), 0) };
                }
            }
            0
        }
        WM_TIMER => {
            let tid = wparam as usize;
            if tid == TIMER_REPAINT_ID {
                unsafe { KillTimer(hwnd, TIMER_REPAINT_ID) };
                let st = unsafe { &mut *state_ptr };
                st.repaint_timer_active = false;
                st.needs_redraw = true;
                unsafe { InvalidateRect(hwnd, null(), 0) };
                return 0;
            }
            if tid == TIMER_BENCH_DONE_ID {
                unsafe { KillTimer(hwnd, TIMER_BENCH_DONE_ID) };
                let st = unsafe { &mut *state_ptr };
                st.bench_timer_fired = true;
                return 0;
            }
            if tid == TIMER_SAVE_CONFIG_ID {
                unsafe { KillTimer(hwnd, TIMER_SAVE_CONFIG_ID) };
                let st = unsafe { &mut *state_ptr };
                if st.config_save_dirty && !st.smoke_mode {
                    let g = st.controller.lock().unwrap();
                    let _ = g.config.save_atomic(&g.config_path);
                }
                st.config_save_dirty = false;
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_PAINT => {
            let st = unsafe { &mut *state_ptr };
            let t_paint_start = Instant::now();
            st.bench_idle_wm_paints += 1;
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

            if st.width > 0 && st.height > 0 {
                if st.painter.has_ready_backbuffer(st.width, st.height)
                    && !st.needs_redraw
                    && st.events.is_empty()
                {
                    st.painter.present_rect(hdc, Some(ps.rcPaint));
                    unsafe { EndPaint(hwnd, &ps) };
                    return 0;
                }

                let ppp = st.pixels_per_point.max(0.5);
                let logical_w = (st.width as f32 / ppp).max(1.0);
                let logical_h = (st.height as f32 / ppp).max(1.0);
                let events = std::mem::take(&mut st.events);
                let elapsed = st.start_time.elapsed().as_secs_f64();

                let mut raw = RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(logical_w, logical_h),
                    )),
                    time: Some(elapsed),
                    modifiers: current_modifiers(),
                    events,
                    focused: st.window_focused,
                    ..Default::default()
                };
                raw.viewports
                    .entry(ViewportId::ROOT)
                    .or_default()
                    .native_pixels_per_point = Some(ppp);

                let t_lock_start = Instant::now();
                let (config_snap, report_snap, scanning, busy_drives, progress_snap, sys_win) = {
                    let g = st.controller.lock().unwrap();
                    (
                        g.config.clone(),
                        g.report.clone(),
                        g.scanning,
                        g.rescanning_drives.clone(),
                        g.progress.clone(),
                        g.system_windows_dir.clone(),
                    )
                };
                let lock_clone_ms = t_lock_start.elapsed().as_secs_f64() * 1000.0;

                if st.applied_titlebar_theme != Some(config_snap.theme_mode) {
                    apply_titlebar_theme(hwnd, config_snap.theme_mode);
                    st.applied_titlebar_theme = Some(config_snap.theme_mode);
                }

                let t_egui_start = Instant::now();
                let mut actions: Vec<UiAction> = Vec::new();
                let ui_state = &mut st.ui_state;
                let full_out = st.egui_ctx.run(raw, |ctx| {
                    draw_app_ui_with_drive_busy(
                        ctx,
                        ui_state,
                        &config_snap,
                        report_snap.as_deref(),
                        scanning,
                        &busy_drives,
                        progress_snap.as_ref(),
                        &sys_win,
                        &mut |act| actions.push(act),
                    );
                });
                let egui_run_ms = t_egui_start.elapsed().as_secs_f64() * 1000.0;
                let next_cursor = full_out.platform_output.cursor_icon;
                if next_cursor != st.hover_cursor_icon {
                    st.hover_cursor_icon = next_cursor;
                    if st.tracking_mouse_leave {
                        unsafe { SetCursor(LoadCursorW(null_mut(), cursor_res_for_egui(next_cursor))) };
                    }
                }

                let clear_bg = match config_snap.theme_mode {
                    ThemeMode::Dark => CLEAR_BG_DARK_RGB,
                    ThemeMode::Light => CLEAR_BG_LIGHT_RGB,
                };
                let t_tess_start = Instant::now();
                let clipped = st
                    .egui_ctx
                    .tessellate(full_out.shapes, full_out.pixels_per_point);
                let tessellate_ms = t_tess_start.elapsed().as_secs_f64() * 1000.0;

                let t_raster_start = Instant::now();
                st.painter.apply_textures(&full_out.textures_delta);
                let mut paint_stats = PaintStats::default();
                st.painter.paint_and_blit_profiled(
                    hdc,
                    st.width,
                    st.height,
                    full_out.pixels_per_point,
                    &clipped,
                    clear_bg,
                    Some(ps.rcPaint),
                    st.bench_recording.then_some(&mut paint_stats),
                );
                let raster_blit_ms = t_raster_start.elapsed().as_secs_f64() * 1000.0;
                st.needs_redraw = false;

                if st.bench_recording {
                    st.bench_samples.push(FrameTimingSample {
                        total_ms: t_paint_start.elapsed().as_secs_f64() * 1000.0,
                        lock_clone_ms,
                        egui_run_ms,
                        tessellate_ms,
                        raster_blit_ms,
                        paint: paint_stats,
                    });
                }

                let min_repaint_delay = full_out
                    .viewport_output
                    .values()
                    .map(|v| v.repaint_delay)
                    .min()
                    .unwrap_or(std::time::Duration::MAX);
                if min_repaint_delay.is_zero() && st.followup_budget > 0 {
                    st.followup_budget -= 1;
                    unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                } else {
                    st.followup_budget = 0;
                    if !st.bench_recording
                        && min_repaint_delay < std::time::Duration::from_secs(3600)
                    {
                        let delay_ms = (min_repaint_delay.as_millis() as u32).clamp(16, 2000);
                        unsafe { SetTimer(hwnd, TIMER_REPAINT_ID, delay_ms, None) };
                        st.repaint_timer_active = true;
                    } else if st.repaint_timer_active {
                        unsafe { KillTimer(hwnd, TIMER_REPAINT_ID) };
                        st.repaint_timer_active = false;
                    }
                }

                for act in actions {
                    match act {
                        UiAction::StartRescan => {
                            trigger_background_scan(Arc::clone(&st.controller));
                        }
                        UiAction::StartDriveRescan(drive_letter) => {
                            trigger_single_drive_rescan(Arc::clone(&st.controller), drive_letter);
                        }
                        UiAction::ToggleTheme(next_mode) => {
                            let mut g = st.controller.lock().unwrap();
                            g.config.theme_mode = next_mode;
                            if !st.smoke_mode {
                                let _ = g.config.save_atomic(&g.config_path);
                            }
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::SetTheme {
                            mode,
                            preset,
                            custom_rgb,
                        } => {
                            let mut g = st.controller.lock().unwrap();
                            g.config.theme_mode = mode;
                            g.config.theme_preset = preset;
                            g.config.theme_custom_rgb = custom_rgb;
                            if !st.smoke_mode {
                                let _ = g.config.save_atomic(&g.config_path);
                            }
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::UpdateListView {
                            sort_mode,
                            sort_direction,
                            filter_chip,
                            search_query,
                            facets,
                        } => {
                            let mut g = st.controller.lock().unwrap();
                            g.config.sort_mode = sort_mode;
                            g.config.sort_direction = sort_direction;
                            g.config.filter_chip = filter_chip;
                            g.config.search_query = search_query;
                            g.config.facets = facets;
                            drop(g);
                            if !st.smoke_mode {
                                st.config_save_dirty = true;
                                unsafe { SetTimer(hwnd, TIMER_SAVE_CONFIG_ID, 350, None) };
                            }
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::UpdateLedgerLayout {
                            collapsed,
                            height_px,
                        } => {
                            let mut g = st.controller.lock().unwrap();
                            g.config.ledger_collapsed = collapsed;
                            g.config.ledger_height_px = height_px;
                            drop(g);
                            if !st.smoke_mode {
                                st.config_save_dirty = true;
                                unsafe { SetTimer(hwnd, TIMER_SAVE_CONFIG_ID, 350, None) };
                            }
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::SaveConfig(mut new_cfg) => {
                            let mut g = st.controller.lock().unwrap();
                            new_cfg.theme_mode = g.config.theme_mode;
                            new_cfg.theme_preset = g.config.theme_preset;
                            new_cfg.theme_custom_rgb = g.config.theme_custom_rgb;
                            new_cfg.window = g.config.window;
                            new_cfg.sort_mode = g.config.sort_mode;
                            new_cfg.sort_direction = g.config.sort_direction;
                            new_cfg.filter_chip = g.config.filter_chip;
                            new_cfg.search_query = g.config.search_query.clone();
                            new_cfg.facets = g.config.facets.clone();
                            new_cfg.ledger_collapsed = g.config.ledger_collapsed;
                            new_cfg.ledger_height_px = g.config.ledger_height_px;
                            g.config = new_cfg;
                            let _ = g.config.save_atomic(&g.config_path);
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::ResetAllSettings => {
                            let mut g = st.controller.lock().unwrap();
                            let win_cfg = g.config.window;
                            let sys_win = g.system_windows_dir.clone();
                            let drives = enumerate_fixed_drives();
                            let mut reset_cfg = AppConfig::for_detected_drives(&drives, &sys_win);
                            reset_cfg.window = win_cfg;
                            g.config = reset_cfg;
                            if !st.smoke_mode {
                                let _ = g.config.save_atomic(&g.config_path);
                            }
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::LocateInExplorer(path) => {
                            if !st.smoke_mode {
                                launch_explorer_select(&path);
                            }
                        }
                        UiAction::OpenWebSearch(url) => {
                            if !st.smoke_mode {
                                open_external_search_url(hwnd, &url);
                            }
                        }
                        UiAction::BrowseRulebookFolder(target) => {
                            if !st.smoke_mode {
                                st.pending_browse_target = Some(target);
                                unsafe { PostMessageW(hwnd, WM_APP_BROWSE_FOLDER, 0, 0) };
                            }
                        }
                    }
                }
            }
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_SETFOCUS_MSG => {
            let st = unsafe { &mut *state_ptr };
            st.window_focused = true;
            st.needs_redraw = true;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_KILLFOCUS_MSG => {
            let st = unsafe { &mut *state_ptr };
            st.window_focused = false;
            if st.repaint_timer_active {
                unsafe { KillTimer(hwnd, TIMER_REPAINT_ID) };
                st.repaint_timer_active = false;
            }
            st.needs_redraw = true;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_SETCURSOR => {
            let hit_code = (lparam & 0xFFFF) as u16;
            if hit_code == 1 {
                let st = unsafe { &*state_ptr };
                unsafe { SetCursor(LoadCursorW(null_mut(), cursor_res_for_egui(st.hover_cursor_icon))) };
                return 1;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_MOUSELEAVE_MSG => {
            let st = unsafe { &mut *state_ptr };
            st.tracking_mouse_leave = false;
            st.hover_cursor_icon = egui::CursorIcon::Default;
            st.last_mouse_px = None;
            st.events.push(Event::PointerGone);
            st.needs_redraw = true;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = lp_xy(lparam);
            let ix = x.round() as i32;
            let iy = y.round() as i32;
            let st = unsafe { &mut *state_ptr };
            if st.last_mouse_px == Some((ix, iy)) {
                return 0;
            }
            st.last_mouse_px = Some((ix, iy));
            if !st.tracking_mouse_leave {
                let mut tme = WinTrackMouseEvent {
                    cb_size: size_of::<WinTrackMouseEvent>() as u32,
                    dw_flags: TME_LEAVE_FLAG,
                    hwnd_track: hwnd,
                    dw_hover_time: 0,
                };
                if unsafe { TrackMouseEvent(&mut tme) } != 0 {
                    st.tracking_mouse_leave = true;
                }
            }
            st.events.push(Event::PointerMoved(Pos2::new(
                x / st.pixels_per_point.max(0.5),
                y / st.pixels_per_point.max(0.5),
            )));
            st.needs_redraw = true;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => {
            let (x, y) = lp_xy(lparam);
            let button = if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP {
                PointerButton::Primary
            } else {
                PointerButton::Secondary
            };
            let pressed = msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN;
            unsafe {
                if pressed {
                    SetCapture(hwnd);
                } else {
                    ReleaseCapture();
                }
                let st = &mut *state_ptr;
                let ppp = st.pixels_per_point.max(0.5);
                st.followup_budget = 2;
                st.needs_redraw = true;
                st.events.push(Event::PointerButton {
                    pos: Pos2::new(x / ppp, y / ppp),
                    button,
                    pressed,
                    modifiers: current_modifiers(),
                });
                InvalidateRect(hwnd, null(), 0);
                UpdateWindow(hwnd);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta_raw = ((wparam >> 16) as i16) as f32 / 120.0;
            let st = unsafe { &mut *state_ptr };
            st.needs_redraw = true;
            st.events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: Vec2::new(0.0, delta_raw * 2.5),
                modifiers: current_modifiers(),
            });
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_KEYDOWN | WM_KEYUP => {
            let vk = wparam as u32;
            let mods = current_modifiers();
            let pressed = msg == WM_KEYDOWN;
            if pressed && mods.ctrl {
                match vk {
                    0x43 /* C */ => {
                        let st = unsafe { &mut *state_ptr };
                        st.events.push(Event::Copy);
                        st.needs_redraw = true;
                        unsafe { InvalidateRect(hwnd, null(), 0) };
                        return 0;
                    }
                    0x58 /* X */ => {
                        let st = unsafe { &mut *state_ptr };
                        st.events.push(Event::Cut);
                        st.needs_redraw = true;
                        unsafe { InvalidateRect(hwnd, null(), 0) };
                        return 0;
                    }
                    0x56 /* V */ => {
                        if let Some(txt) = read_clipboard_unicode_text(hwnd) {
                            let st = unsafe { &mut *state_ptr };
                            st.events.push(Event::Paste(txt));
                            st.needs_redraw = true;
                            unsafe { InvalidateRect(hwnd, null(), 0) };
                        }
                        return 0;
                    }
                    _ => {}
                }
            }
            if let Some(key) = vk_to_egui_key(vk) {
                let st = unsafe { &mut *state_ptr };
                st.followup_budget = 2;
                st.needs_redraw = true;
                st.events.push(Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: mods,
                });
                unsafe { InvalidateRect(hwnd, null(), 0) };
            }
            0
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam as u32) {
                if !ch.is_control() {
                    let st = unsafe { &mut *state_ptr };
                    st.needs_redraw = true;
                    st.events.push(Event::Text(ch.to_string()));
                    unsafe { InvalidateRect(hwnd, null(), 0) };
                }
            }
            0
        }
        0x0232 /* WM_EXITSIZEMOVE */ => {
            let st = unsafe { &mut *state_ptr };
            if !st.smoke_mode {
                save_window_placement(hwnd, &st.controller);
            }
            0
        }
        WM_CLOSE => {
            let st = unsafe { &mut *state_ptr };
            if st.config_save_dirty && !st.smoke_mode {
                unsafe { KillTimer(hwnd, TIMER_SAVE_CONFIG_ID) };
                let g = st.controller.lock().unwrap();
                let _ = g.config.save_atomic(&g.config_path);
                st.config_save_dirty = false;
            }
            if !st.smoke_mode {
                save_window_placement(hwnd, &st.controller);
            }
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            let boxed = unsafe { Box::from_raw(state_ptr) };
            boxed.controller.lock().unwrap().hwnd = 0;
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn resolve_initial_window_bounds(
    saved: Option<WindowPlacementConfig>,
    default_w: i32,
    default_h: i32,
    min_w: i32,
    min_h: i32,
) -> (i32, i32, i32, i32, bool) {
    let Some(wp) = saved else {
        return (100, 60, default_w, default_h, false);
    };
    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(800);
    let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(600);
    let w = wp.width.max(min_w).min(vw);
    let h = wp.height.max(min_h).min(vh);
    let visible_x = wp.x < vx + vw - 80 && wp.x + w > vx + 80;
    let visible_y = wp.y < vy + vh - 60 && wp.y + h > vy + 40;
    if visible_x && visible_y {
        (wp.x, wp.y, w, h, wp.maximized)
    } else {
        (100, 60, w, h, wp.maximized)
    }
}

fn save_window_placement(hwnd: HWND, controller: &Arc<Mutex<AppController>>) {
    let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
    wp.length = size_of::<WINDOWPLACEMENT>() as u32;
    if unsafe { GetWindowPlacement(hwnd, &mut wp) } == 0 {
        return;
    }
    let rc = wp.rcNormalPosition;
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    if w < 400 || h < 300 {
        return;
    }
    let placement = WindowPlacementConfig {
        x: rc.left,
        y: rc.top,
        width: w,
        height: h,
        maximized: wp.showCmd == SW_SHOWMAXIMIZED as u32,
    };
    let mut g = controller.lock().unwrap();
    if g.config.window != Some(placement) {
        g.config.window = Some(placement);
        let _ = g.config.save_atomic(&g.config_path);
    }
}

fn process_cpu_time_ms() -> f64 {
    let mut create_ft: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit_ft: FILETIME = unsafe { std::mem::zeroed() };
    let mut kernel_ft: FILETIME = unsafe { std::mem::zeroed() };
    let mut user_ft: FILETIME = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut create_ft,
            &mut exit_ft,
            &mut kernel_ft,
            &mut user_ft,
        )
    };
    if ok == 0 {
        return 0.0;
    }
    let to_u64 = |ft: FILETIME| ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
    let ticks_100ns = to_u64(kernel_ft) + to_u64(user_ft);
    ticks_100ns as f64 / 10_000.0
}

#[derive(Serialize)]
struct BenchIdleMetrics {
    elapsed_ms: f64,
    cpu_delta_ms: f64,
    single_core_cpu_pct: f64,
    system_cpu_pct: f64,
    wm_paint_count: usize,
}

#[derive(Serialize)]
struct BenchPhaseMetrics {
    elapsed_ms: f64,
    frames: usize,
    avg_total_ms: f64,
    p50_total_ms: f64,
    p95_total_ms: f64,
    max_total_ms: f64,
    avg_lock_clone_ms: f64,
    avg_egui_run_ms: f64,
    avg_tessellate_ms: f64,
    avg_raster_blit_ms: f64,
    avg_triangles: f64,
    avg_solid_quads: f64,
    avg_solid_tris: f64,
    avg_grad_tris: f64,
    avg_tex_tris: f64,
    avg_clear_ms: f64,
    avg_quad_ms: f64,
    avg_solid_tri_ms: f64,
    avg_grad_tri_ms: f64,
    avg_tex_tri_ms: f64,
    avg_bitblt_ms: f64,
}

#[derive(Serialize)]
struct PerfBenchmarkReport {
    apps_count: usize,
    dpi: u32,
    pixels_per_point: f32,
    client_width_px: usize,
    client_height_px: usize,
    idle_5s: BenchIdleMetrics,
    scroll_collapsed_5s: BenchPhaseMetrics,
    scroll_expanded_5s: BenchPhaseMetrics,
    resize_drag_5s: BenchPhaseMetrics,
}

fn summarize_samples(elapsed_ms: f64, samples: &[FrameTimingSample]) -> BenchPhaseMetrics {
    if samples.is_empty() {
        return BenchPhaseMetrics {
            elapsed_ms,
            frames: 0,
            avg_total_ms: 0.0,
            p50_total_ms: 0.0,
            p95_total_ms: 0.0,
            max_total_ms: 0.0,
            avg_lock_clone_ms: 0.0,
            avg_egui_run_ms: 0.0,
            avg_tessellate_ms: 0.0,
            avg_raster_blit_ms: 0.0,
            avg_triangles: 0.0,
            avg_solid_quads: 0.0,
            avg_solid_tris: 0.0,
            avg_grad_tris: 0.0,
            avg_tex_tris: 0.0,
            avg_clear_ms: 0.0,
            avg_quad_ms: 0.0,
            avg_solid_tri_ms: 0.0,
            avg_grad_tri_ms: 0.0,
            avg_tex_tri_ms: 0.0,
            avg_bitblt_ms: 0.0,
        };
    }
    let n = samples.len() as f64;
    let mut totals: Vec<f64> = samples.iter().map(|s| s.total_ms).collect();
    totals.sort_by(|a, b| a.total_cmp(b));
    let p50 = totals[(totals.len() - 1) / 2];
    let p95 = totals[((totals.len() - 1) * 95) / 100];
    let max_v = *totals.last().unwrap_or(&0.0);
    BenchPhaseMetrics {
        elapsed_ms,
        frames: samples.len(),
        avg_total_ms: samples.iter().map(|s| s.total_ms).sum::<f64>() / n,
        p50_total_ms: p50,
        p95_total_ms: p95,
        max_total_ms: max_v,
        avg_lock_clone_ms: samples.iter().map(|s| s.lock_clone_ms).sum::<f64>() / n,
        avg_egui_run_ms: samples.iter().map(|s| s.egui_run_ms).sum::<f64>() / n,
        avg_tessellate_ms: samples.iter().map(|s| s.tessellate_ms).sum::<f64>() / n,
        avg_raster_blit_ms: samples.iter().map(|s| s.raster_blit_ms).sum::<f64>() / n,
        avg_triangles: samples.iter().map(|s| s.paint.triangles_total as f64).sum::<f64>() / n,
        avg_solid_quads: samples.iter().map(|s| s.paint.solid_quads as f64).sum::<f64>() / n,
        avg_solid_tris: samples
            .iter()
            .map(|s| (s.paint.solid_tri_opaque + s.paint.solid_tri_alpha) as f64)
            .sum::<f64>()
            / n,
        avg_grad_tris: samples.iter().map(|s| s.paint.grad_feather_tri as f64).sum::<f64>() / n,
        avg_tex_tris: samples.iter().map(|s| s.paint.textured_tri as f64).sum::<f64>() / n,
        avg_clear_ms: samples.iter().map(|s| s.paint.clear_ms).sum::<f64>() / n,
        avg_quad_ms: samples.iter().map(|s| s.paint.quad_ms).sum::<f64>() / n,
        avg_solid_tri_ms: samples.iter().map(|s| s.paint.solid_tri_ms).sum::<f64>() / n,
        avg_grad_tri_ms: samples.iter().map(|s| s.paint.grad_tri_ms).sum::<f64>() / n,
        avg_tex_tri_ms: samples.iter().map(|s| s.paint.tex_tri_ms).sum::<f64>() / n,
        avg_bitblt_ms: samples.iter().map(|s| s.paint.bitblt_ms).sum::<f64>() / n,
    }
}

fn drain_messages_and_paint(hwnd: HWND) {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    unsafe { UpdateWindow(hwnd) };
}

pub fn run_perf_benchmark(out_path: &std::path::Path) -> Result<(), String> {
    let controller = Arc::new(Mutex::new(AppController::load_initial()));
    let apps_count = {
        let mut g = controller.lock().unwrap();
        let count = g.report.as_ref().map_or(0, |r| r.apps.len());
        if count == 0 {
            return Err("no cached scan report found in AppData for benchmark".into());
        }
        g.config.sort_mode = y_core::config::AppSortMode::Size;
        g.config.sort_direction = y_core::config::SortDirection::Desc;
        g.config.filter_chip = y_core::config::AppFilterChip::All;
        g.config.search_query.clear();
        count
    };

    let hinstance = unsafe { GetModuleHandleW(null()) } as HINSTANCE;
    let class_name: Vec<u16> = "YStorageBenchWindow\0".encode_utf16().collect();
    let title: Vec<u16> = "y-storage Perf Benchmark\0".encode_utf16().collect();
    let (large_icon, small_icon) = load_app_icons(hinstance);
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: large_icon,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: small_icon,
    };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return Err("RegisterClassExW failed in bench".into());
    }

    let init_dpi = get_dpi_for_system();
    let (default_w, default_h) =
        outer_size_for_dpi(DEFAULT_LOGICAL_W, DEFAULT_LOGICAL_H, WS_OVERLAPPEDWINDOW, init_dpi);
    let (init_theme, init_config_snap) = {
        let g = controller.lock().unwrap();
        (g.config.theme_mode, g.config.clone())
    };
    let egui_ctx = egui::Context::default();
    configure_fonts_and_theme_with(
        &egui_ctx,
        init_theme,
        init_config_snap.theme_preset,
        init_config_snap.theme_custom_rgb,
    );

    let host_state = Box::new(WindowHostState {
        controller: Arc::clone(&controller),
        ui_state: StorageUiState::from_config(&init_config_snap),
        egui_ctx,
        painter: SoftwarePainter::new(),
        applied_titlebar_theme: None,
        large_icon,
        small_icon,
        events: Vec::new(),
        width: (DEFAULT_LOGICAL_W as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        height: (DEFAULT_LOGICAL_H as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        dpi: init_dpi,
        pixels_per_point: dpi_to_ppp(init_dpi),
        needs_redraw: true,
        followup_budget: 2,
        window_focused: true,
        last_mouse_px: None,
        tracking_mouse_leave: false,
        hover_cursor_icon: egui::CursorIcon::Default,
        repaint_timer_active: false,
        config_save_dirty: false,
        pending_browse_target: None,
        smoke_mode: true,
        start_time: Instant::now(),
        bench_recording: false,
        bench_idle_wm_paints: 0,
        bench_timer_fired: false,
        bench_samples: Vec::new(),
    });
    let state_ptr = Box::into_raw(host_state);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            100,
            60,
            default_w,
            default_h,
            null_mut(),
            null_mut(),
            hinstance,
            state_ptr.cast(),
        )
    };
    if hwnd.is_null() {
        unsafe { drop(Box::from_raw(state_ptr)) };
        return Err("CreateWindowExW failed in bench".into());
    }
    controller.lock().unwrap().hwnd = hwnd as usize;
    apply_titlebar_theme(hwnd, init_theme);

    let actual_dpi = get_dpi_for_window(hwnd);
    let ppp = dpi_to_ppp(actual_dpi);
    let (cw, ch) = {
        let mut crc: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetClientRect(hwnd, &mut crc) };
        (
            (crc.right - crc.left).max(0) as usize,
            (crc.bottom - crc.top).max(0) as usize,
        )
    };
    unsafe {
        let st = &mut *state_ptr;
        st.dpi = actual_dpi;
        st.pixels_per_point = ppp;
        st.width = cw;
        st.height = ch;
        st.applied_titlebar_theme = Some(init_theme);
        ShowWindow(hwnd, SW_SHOW);
        InvalidateRect(hwnd, null(), 0);
        UpdateWindow(hwnd);
    }

    // Settle startup frames (~500ms)
    let settle_end = Instant::now() + std::time::Duration::from_millis(500);
    while Instant::now() < settle_end {
        drain_messages_and_paint(hwnd);
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    // 1. Idle 5s untouched via real blocking GetMessageW loop + 5s Win32 timer
    let idle_5s = unsafe {
        let st = &mut *state_ptr;
        st.bench_recording = false;
        st.bench_idle_wm_paints = 0;
        st.bench_timer_fired = false;
        let cpu0 = process_cpu_time_ms();
        let t0 = Instant::now();
        SetTimer(hwnd, TIMER_BENCH_DONE_ID, 5000, None);
        let mut msg: MSG = std::mem::zeroed();
        while !st.bench_timer_fired && GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let cpu_delta_ms = (process_cpu_time_ms() - cpu0).max(0.0);
        let logical_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f64;
        let single_core_pct = (cpu_delta_ms / elapsed_ms.max(1.0)) * 100.0;
        BenchIdleMetrics {
            elapsed_ms,
            cpu_delta_ms,
            single_core_cpu_pct: single_core_pct,
            system_cpu_pct: single_core_pct / logical_cpus,
            wm_paint_count: st.bench_idle_wm_paints,
        }
    };

    // Place pointer inside the main app scroll region
    let ptr_x = (510.0 * ppp).round() as isize;
    let ptr_y = (420.0 * ppp).round() as isize;
    let lp_ptr = ((ptr_y & 0xFFFF) << 16) | (ptr_x & 0xFFFF);
    unsafe {
        SendMessageW(hwnd, WM_MOUSEMOVE, 0, lp_ptr);
        drain_messages_and_paint(hwnd);
    }

    // 2. Scroll collapsed 5s
    let scroll_collapsed_5s = {
        let st = unsafe { &mut *state_ptr };
        st.ui_state.expanded_apps.clear();
        st.bench_samples.clear();
        st.bench_recording = true;
        let t0 = Instant::now();
        let dur = std::time::Duration::from_secs(5);
        let mut tick = 0usize;
        while t0.elapsed() < dur {
            let dir_down = (tick / 60) % 2 == 0;
            let wheel_delta: i16 = if dir_down { -120 } else { 120 };
            let wp = ((wheel_delta as u16) as usize) << 16;
            unsafe { SendMessageW(hwnd, WM_MOUSEWHEEL, wp, 0) };
            drain_messages_and_paint(hwnd);
            tick += 1;
        }
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        st.bench_recording = false;
        summarize_samples(elapsed_ms, &st.bench_samples)
    };

    // Reset to top & expand FIFA 23 (or top app) for phase 3
    let expand_id = {
        let g = controller.lock().unwrap();
        g.report
            .as_ref()
            .and_then(|r| {
                r.apps
                    .iter()
                    .find(|a| a.display_name.to_ascii_lowercase().contains("fifa 23"))
                    .or_else(|| r.apps.first())
                    .map(|a| a.id.clone())
            })
    };
    {
        let st = unsafe { &mut *state_ptr };
        st.bench_recording = false;
        for _ in 0..40 {
            let wp = ((1200i16 as u16) as usize) << 16;
            unsafe { SendMessageW(hwnd, WM_MOUSEWHEEL, wp, 0) };
            drain_messages_and_paint(hwnd);
        }
        if let Some(id) = expand_id {
            st.ui_state.expanded_apps.insert(id);
        }
        st.needs_redraw = true;
        unsafe {
            InvalidateRect(hwnd, null(), 0);
            drain_messages_and_paint(hwnd);
        }
    }

    // 3. Scroll expanded 5s
    let scroll_expanded_5s = {
        let st = unsafe { &mut *state_ptr };
        st.bench_samples.clear();
        st.bench_recording = true;
        let t0 = Instant::now();
        let dur = std::time::Duration::from_secs(5);
        let mut tick = 0usize;
        while t0.elapsed() < dur {
            let dir_down = (tick / 60) % 2 == 0;
            let wheel_delta: i16 = if dir_down { -120 } else { 120 };
            let wp = ((wheel_delta as u16) as usize) << 16;
            unsafe { SendMessageW(hwnd, WM_MOUSEWHEEL, wp, 0) };
            drain_messages_and_paint(hwnd);
            tick += 1;
        }
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        st.bench_recording = false;
        summarize_samples(elapsed_ms, &st.bench_samples)
    };

    // Reset collapsed + default size before phase 4
    {
        let st = unsafe { &mut *state_ptr };
        st.ui_state.expanded_apps.clear();
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                100,
                60,
                default_w,
                default_h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            st.needs_redraw = true;
            InvalidateRect(hwnd, null(), 0);
            drain_messages_and_paint(hwnd);
        }
    }

    // 4. Resize drag 5s
    let resize_drag_5s = {
        let st = unsafe { &mut *state_ptr };
        st.bench_samples.clear();
        st.bench_recording = true;
        let t0 = Instant::now();
        let dur = std::time::Duration::from_secs(5);
        let mut step_idx = 0isize;
        while t0.elapsed() < dur {
            let phase = (step_idx % 160) as i32;
            let tri = if phase < 80 { phase } else { 160 - phase };
            let cur_w = default_w + tri * 4;
            let cur_h = default_h + tri * 3;
            unsafe {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    100,
                    60,
                    cur_w,
                    cur_h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            drain_messages_and_paint(hwnd);
            step_idx += 1;
        }
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        st.bench_recording = false;
        summarize_samples(elapsed_ms, &st.bench_samples)
    };

    unsafe {
        DestroyWindow(hwnd);
    }

    let rep = PerfBenchmarkReport {
        apps_count,
        dpi: actual_dpi,
        pixels_per_point: ppp,
        client_width_px: cw,
        client_height_px: ch,
        idle_5s,
        scroll_collapsed_5s,
        scroll_expanded_5s,
        resize_drag_5s,
    };
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&rep).map_err(|e| e.to_string())?;
    std::fs::write(out_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn get_extended_frame_bounds(hwnd: HWND) -> Option<RECT> {
    type DwmGetAttrFn =
        unsafe extern "system" fn(HWND, u32, *mut std::ffi::c_void, u32) -> i32;
    let dll: Vec<u16> = "dwmapi.dll\0".encode_utf16().collect();
    let module = unsafe { LoadLibraryW(dll.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    let ok = if let Some(proc) = unsafe { GetProcAddress(module, b"DwmGetWindowAttribute\0".as_ptr()) } {
        let f: DwmGetAttrFn = unsafe { std::mem::transmute_copy(&proc) };
        unsafe { f(hwnd, 9, (&mut rc as *mut RECT).cast(), size_of::<RECT>() as u32) == 0 }
    } else {
        false
    };
    unsafe { FreeLibrary(module) };
    if ok { Some(rc) } else { None }
}

fn capture_window_crop_bgra(hwnd: HWND, crop_h_px: usize) -> Result<(usize, usize, Vec<u32>), String> {
    let mut wr: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut wr) } == 0 {
        return Err("GetWindowRect failed".into());
    }
    let ext = get_extended_frame_bounds(hwnd).unwrap_or(wr);
    let win_w = (wr.right - wr.left).max(1) as usize;
    let win_h = (wr.bottom - wr.top).max(1) as usize;
    let off_x = (ext.left - wr.left).max(0) as usize;
    let off_y = (ext.top - wr.top).max(0) as usize;
    let ext_w = ((ext.right - ext.left).max(1) as usize).min(win_w.saturating_sub(off_x));
    let out_h = crop_h_px.clamp(1, (ext.bottom - ext.top).max(1) as usize);

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: win_w as i32,
            biHeight: -(win_h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: (win_w * win_h * 4) as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }],
    };
    let mut bits: *mut std::ffi::c_void = null_mut();
    let mem_dc = unsafe { CreateCompatibleDC(null_mut()) };
    if mem_dc.is_null() {
        return Err("CreateCompatibleDC failed".into());
    }
    let hbm = unsafe { CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    if hbm.is_null() || bits.is_null() {
        unsafe {
            if !hbm.is_null() {
                DeleteObject(hbm as HGDIOBJ);
            }
            DeleteDC(mem_dc);
        }
        return Err("CreateDIBSection failed during titlebar capture".into());
    }
    let old_bm = unsafe { SelectObject(mem_dc, hbm as HGDIOBJ) };
    let pw_ok = unsafe { PrintWindow(hwnd, mem_dc, 2) };
    let mut pixels = vec![0u32; ext_w * out_h];
    let copy_cropped = |buf_ptr: *const u32, dst: &mut [u32]| {
        let src = unsafe { std::slice::from_raw_parts(buf_ptr, win_w * win_h) };
        for r in 0..out_h {
            let sy = off_y + r;
            if sy < win_h {
                let s_off = sy * win_w + off_x;
                let d_off = r * ext_w;
                dst[d_off..d_off + ext_w].copy_from_slice(&src[s_off..s_off + ext_w]);
            }
        }
    };
    if pw_ok != 0 {
        copy_cropped(bits as *const u32, &mut pixels);
    }
    // Verify title-bar row actually has non-zero RGB content; fallback to desktop DC BitBlt if PrintWindow left black.
    let top_row_has_pixels = pixels
        .iter()
        .take(ext_w * out_h.min(28))
        .any(|&p| (p & 0x00FF_FFFF) != 0);
    if pw_ok == 0 || !top_row_has_pixels {
        let screen_dc = unsafe { GetDC(null_mut()) };
        if !screen_dc.is_null() {
            unsafe {
                BitBlt(
                    mem_dc,
                    0,
                    0,
                    ext_w as i32,
                    out_h as i32,
                    screen_dc,
                    ext.left,
                    ext.top,
                    SRCCOPY,
                );
                ReleaseDC(null_mut(), screen_dc);
            }
            let src = unsafe { std::slice::from_raw_parts(bits as *const u32, win_w * win_h) };
            for r in 0..out_h {
                let s_off = r * win_w;
                let d_off = r * ext_w;
                pixels[d_off..d_off + ext_w].copy_from_slice(&src[s_off..s_off + ext_w]);
            }
        }
    }
    unsafe {
        SelectObject(mem_dc, old_bm);
        DeleteObject(hbm as HGDIOBJ);
        DeleteDC(mem_dc);
    }
    Ok((ext_w, out_h, pixels))
}

/// Opens a live native window briefly to capture the real Windows non-client title bar (`16px` WT icon) + client header lockup (`24px` WT icon) in Dark and Light modes.
pub fn run_titlebar_capture(
    controller: Arc<Mutex<AppController>>,
    out_path: &std::path::Path,
) -> Result<(), String> {
    let orig_theme = controller.lock().unwrap().config.theme_mode;
    let hinstance = unsafe { GetModuleHandleW(null()) } as HINSTANCE;
    let class_name: Vec<u16> = "YStorageTitlebarCaptureWindow\0".encode_utf16().collect();
    let title: Vec<u16> = "y-storage — True App Storage Footprint\0"
        .encode_utf16()
        .collect();
    let (large_icon, small_icon) = load_app_icons(hinstance);

    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: large_icon,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: small_icon,
    };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return Err("RegisterClassExW failed in titlebar capture".into());
    }

    let init_dpi = get_dpi_for_system();
    let (default_w, default_h) =
        outer_size_for_dpi(DEFAULT_LOGICAL_W, DEFAULT_LOGICAL_H, WS_OVERLAPPEDWINDOW, init_dpi);
    let init_config_snap = controller.lock().unwrap().config.clone();
    let egui_ctx = egui::Context::default();
    configure_fonts_and_theme(&egui_ctx, ThemeMode::Dark);

    let host_state = Box::new(WindowHostState {
        controller: Arc::clone(&controller),
        ui_state: StorageUiState::from_config(&init_config_snap),
        egui_ctx,
        painter: SoftwarePainter::new(),
        applied_titlebar_theme: None,
        large_icon,
        small_icon,
        events: Vec::new(),
        width: (DEFAULT_LOGICAL_W as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        height: (DEFAULT_LOGICAL_H as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        dpi: init_dpi,
        pixels_per_point: dpi_to_ppp(init_dpi),
        needs_redraw: true,
        followup_budget: 2,
        window_focused: true,
        last_mouse_px: None,
        tracking_mouse_leave: false,
        hover_cursor_icon: egui::CursorIcon::Default,
        repaint_timer_active: false,
        config_save_dirty: false,
        pending_browse_target: None,
        smoke_mode: true,
        start_time: Instant::now(),
        bench_recording: false,
        bench_idle_wm_paints: 0,
        bench_timer_fired: false,
        bench_samples: Vec::new(),
    });
    let state_ptr = Box::into_raw(host_state);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            80,
            80,
            default_w,
            default_h,
            null_mut(),
            null_mut(),
            hinstance,
            state_ptr.cast(),
        )
    };
    if hwnd.is_null() {
        unsafe { drop(Box::from_raw(state_ptr)) };
        return Err("CreateWindowExW failed in titlebar capture".into());
    }
    let actual_dpi = get_dpi_for_window(hwnd);
    let ppp = dpi_to_ppp(actual_dpi);
    let crop_h = (112.0_f32 * ppp).round() as usize;

    let mut strips: Vec<(usize, usize, Vec<u32>)> = Vec::with_capacity(2);
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        controller.lock().unwrap().config.theme_mode = mode;
        unsafe {
            let st = &mut *state_ptr;
            st.dpi = actual_dpi;
            st.pixels_per_point = ppp;
            st.applied_titlebar_theme = Some(mode);
            st.needs_redraw = true;
            configure_fonts_and_theme(&st.egui_ctx, mode);
            apply_titlebar_theme(hwnd, mode);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            InvalidateRect(hwnd, null(), 0);
        }
        let end = Instant::now() + std::time::Duration::from_millis(260);
        while Instant::now() < end {
            drain_messages_and_paint(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        strips.push(capture_window_crop_bgra(hwnd, crop_h)?);
    }

    controller.lock().unwrap().config.theme_mode = orig_theme;
    unsafe {
        DestroyWindow(hwnd);
    }

    let out_w = strips[0].0.max(strips[1].0);
    let gap = 4usize;
    let out_h = strips[0].1 + gap + strips[1].1;
    let mut canvas = vec![0x0026_263A_u32; out_w * out_h];
    for (idx, (sw, sh, px)) in strips.into_iter().enumerate() {
        let dy = if idx == 0 { 0 } else { out_h - sh };
        for r in 0..sh {
            let s_off = r * sw;
            let d_off = (dy + r) * out_w;
            canvas[d_off..d_off + sw].copy_from_slice(&px[s_off..s_off + sw]);
        }
    }
    crate::png::save_bgra_u32_as_png(out_path, out_w, out_h, &canvas)
}

fn lp_xy(lparam: LPARAM) -> (f32, f32) {
    let x = (lparam & 0xFFFF) as i16 as f32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as f32;
    (x, y)
}

fn current_modifiers() -> Modifiers {
    let shift = unsafe { GetKeyState(VK_SHIFT as i32) } < 0;
    let ctrl = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
    let alt = unsafe { GetKeyState(VK_MENU as i32) } < 0;
    Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: false,
        command: ctrl,
    }
}

fn read_clipboard_unicode_text(hwnd: HWND) -> Option<String> {
    const CF_UNICODETEXT: u32 = 13;
    if unsafe { OpenClipboard(hwnd) } == 0 {
        return None;
    }
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    let out = if !handle.is_null() {
        let ptr = unsafe { GlobalLock(handle) } as *const u16;
        if !ptr.is_null() {
            let mut len = 0usize;
            while unsafe { *ptr.add(len) } != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(ptr, len) });
            unsafe { GlobalUnlock(handle) };
            Some(s)
        } else {
            None
        }
    } else {
        None
    };
    unsafe { CloseClipboard() };
    out
}

fn launch_explorer_select(path: &str) {
    use std::os::windows::process::CommandExt;

    let norm = crate::unattributed_assist::normalize_unattributed_target_path(path);
    if norm.is_empty() || !std::path::Path::new(&norm).exists() {
        return;
    }
    let raw_arg = crate::unattributed_assist::build_explorer_select_raw_arg(&norm);
    let _ = std::process::Command::new("explorer.exe")
        .raw_arg(raw_arg)
        .spawn();
}

fn pick_folder_dialog(hwnd: HWND, title: &str) -> Option<String> {
    use core::ffi::c_void;
    use windows_sys::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    };
    use windows_sys::Win32::UI::Shell::{
        FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PICKFOLDERS, FileOpenDialog, SIGDN_FILESYSPATH,
    };
    use windows_sys::core::{GUID, HRESULT};

    const IID_IFILE_OPEN_DIALOG: GUID = GUID::from_u128(0xd57c7288_d4ad_4768_be02_9d969532d960);

    #[inline]
    unsafe fn vtbl_slot(obj: *mut c_void, idx: usize) -> *const c_void {
        let vtbl = unsafe { *(obj as *const *const *const c_void) };
        unsafe { *vtbl.add(idx) }
    }

    #[inline]
    unsafe fn com_release(obj: *mut c_void) {
        if !obj.is_null() {
            let release: unsafe extern "system" fn(*mut c_void) -> u32 =
                unsafe { std::mem::transmute(vtbl_slot(obj, 2)) };
            unsafe { release(obj) };
        }
    }

    unsafe {
        let co_hr = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        // S_OK (0) or S_FALSE (1) means this call owns a CoUninitialize() pair.
        let need_uninit = co_hr == 0 || co_hr == 1;
        if co_hr < 0 && co_hr != -2147417850 {
            return None;
        }

        let mut dialog: *mut c_void = null_mut();
        let hr = CoCreateInstance(
            &FileOpenDialog,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IFILE_OPEN_DIALOG,
            &mut dialog,
        );
        if hr < 0 || dialog.is_null() {
            if need_uninit {
                CoUninitialize();
            }
            return None;
        }

        let get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT =
            std::mem::transmute(vtbl_slot(dialog, 10));
        let set_options: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT =
            std::mem::transmute(vtbl_slot(dialog, 9));
        let set_title: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT =
            std::mem::transmute(vtbl_slot(dialog, 17));
        let show: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT =
            std::mem::transmute(vtbl_slot(dialog, 3));
        let get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(vtbl_slot(dialog, 20));

        let mut opts: u32 = 0;
        if get_options(dialog, &mut opts) >= 0 {
            let _ = set_options(
                dialog,
                opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR,
            );
        }
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = set_title(dialog, title_w.as_ptr());

        let mut picked: Option<String> = None;
        if show(dialog, hwnd) >= 0 {
            let mut item: *mut c_void = null_mut();
            if get_result(dialog, &mut item) >= 0 && !item.is_null() {
                let get_display_name: unsafe extern "system" fn(
                    *mut c_void,
                    i32,
                    *mut *mut u16,
                ) -> HRESULT = std::mem::transmute(vtbl_slot(item, 5));
                let mut psz: *mut u16 = null_mut();
                if get_display_name(item, SIGDN_FILESYSPATH, &mut psz) >= 0 && !psz.is_null() {
                    let mut len = 0usize;
                    while *psz.add(len) != 0 {
                        len += 1;
                    }
                    picked = Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                        psz, len,
                    )));
                    CoTaskMemFree(psz as *const c_void);
                }
                com_release(item);
            }
        }
        com_release(dialog);
        if need_uninit {
            CoUninitialize();
        }
        picked
    }
}

fn open_external_search_url(hwnd: HWND, url: &str) {
    if !url.starts_with("https://www.google.com/search?q=") {
        return;
    }
    let op_w: Vec<u16> = "open\0".encode_utf16().collect();
    let url_w: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            hwnd,
            op_w.as_ptr(),
            url_w.as_ptr(),
            null(),
            null(),
            SW_SHOW,
        );
    }
}

fn vk_to_egui_key(vk: u32) -> Option<Key> {
    match vk {
        0x08 => Some(Key::Backspace),
        0x09 => Some(Key::Tab),
        0x0D => Some(Key::Enter),
        0x1B => Some(Key::Escape),
        0x25 => Some(Key::ArrowLeft),
        0x26 => Some(Key::ArrowUp),
        0x27 => Some(Key::ArrowRight),
        0x28 => Some(Key::ArrowDown),
        0x2E => Some(Key::Delete),
        0x24 => Some(Key::Home),
        0x23 => Some(Key::End),
        0x41 => Some(Key::A),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_scaling_and_outer_window_size_scale_monotonically() {
        assert!((dpi_to_ppp(96) - 1.0).abs() < 1e-4);
        assert!((dpi_to_ppp(120) - 1.25).abs() < 1e-4);
        assert!((dpi_to_ppp(144) - 1.5).abs() < 1e-4);
        assert!((dpi_to_ppp(192) - 2.0).abs() < 1e-4);

        let (w100, h100) =
            outer_size_for_dpi(DEFAULT_LOGICAL_W, DEFAULT_LOGICAL_H, WS_OVERLAPPEDWINDOW, 96);
        let (w150, h150) =
            outer_size_for_dpi(DEFAULT_LOGICAL_W, DEFAULT_LOGICAL_H, WS_OVERLAPPEDWINDOW, 144);
        assert!(w150 > w100 + 400);
        assert!(h150 > h100 + 300);
    }
}
