use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use egui::{
    Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2, ViewportId,
};
use windows_sys::Win32::Foundation::{FreeLibrary, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT, UpdateWindow,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, OpenClipboard,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow,
    DispatchMessageW, GetClientRect, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HICON,
    IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR, LoadCursorW, LoadImageW, MSG, PM_REMOVE, PeekMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXICON, SM_CXSMICON, SM_CYICON,
    SM_CYSMICON, SW_SHOW, SWP_NOZORDER, SWP_NOACTIVATE, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TranslateMessage, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEWHEEL, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SIZE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use y_core::cache::{FolderSizeCache, default_last_scan_path, load_last_scan, save_last_scan_atomic};
use y_core::config::{AppConfig, ThemeMode};
use y_core::history::ScanHistory;
use y_core::model::ScanReport;
use y_platform_windows::paths::{enumerate_fixed_drives, get_windows_directory};
use y_platform_windows::scanner::{ScanProgressSnapshot, execute_scan_with_progress};

use crate::sw_painter::SoftwarePainter;
use crate::ui::{StorageUiState, UiAction, configure_fonts_and_theme, draw_app_ui};

const WM_APP_SCAN_UPDATE: u32 = 0x8000 + 10;
const WM_APP_FOLLOWUP_PAINT: u32 = 0x8000 + 11;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_ERASEBKGND: u32 = 0x0014;

const DEFAULT_LOGICAL_W: i32 = 1020;
const DEFAULT_LOGICAL_H: i32 = 720;
const MIN_LOGICAL_W: i32 = 760;
const MIN_LOGICAL_H: i32 = 520;
const CLEAR_BG_DARK_RGB: u32 = 0x000A_0A10;
const CLEAR_BG_LIGHT_RGB: u32 = 0x00EE_F4FA;
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
    pub report: Option<ScanReport>,
    pub scanning: bool,
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
        let report = load_last_scan(&last_scan_path);

        Self {
            config,
            config_path,
            history,
            history_path,
            cache,
            cache_path,
            last_scan_path,
            report,
            scanning: false,
            progress: None,
            system_windows_dir: sys_win,
            hwnd: 0,
        }
    }
}

struct WindowHostState {
    controller: Arc<Mutex<AppController>>,
    ui_state: StorageUiState,
    egui_ctx: egui::Context,
    painter: SoftwarePainter,
    applied_titlebar_theme: Option<ThemeMode>,
    events: Vec<Event>,
    width: usize,
    height: usize,
    dpi: u32,
    pixels_per_point: f32,
    needs_redraw: bool,
    followup_budget: u8,
    start_time: Instant,
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

fn load_app_icons(hinstance: HINSTANCE) -> (HICON, HICON) {
    let cx_lg = unsafe { GetSystemMetrics(SM_CXICON) }.max(32);
    let cy_lg = unsafe { GetSystemMetrics(SM_CYICON) }.max(32);
    let cx_sm = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let cy_sm = unsafe { GetSystemMetrics(SM_CYSMICON) }.max(16);
    let res_id = 1usize as *const u16;
    let large = unsafe { LoadImageW(hinstance, res_id, IMAGE_ICON, cx_lg, cy_lg, LR_DEFAULTCOLOR) }
        as HICON;
    let small = unsafe { LoadImageW(hinstance, res_id, IMAGE_ICON, cx_sm, cy_sm, LR_DEFAULTCOLOR) }
        as HICON;
    (large, small)
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
            ThemeMode::Dark => 0x0010_0A0A,
            ThemeMode::Light => 0x00FA_F4EE,
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
    let (config, prev_hist, hwnd_usize) = {
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
        (
            guard.config.clone(),
            guard.history.latest_per_app().cloned(),
            guard.hwnd,
        )
    };

    if hwnd_usize != 0 {
        unsafe { PostMessageW(hwnd_usize as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
    }

    std::thread::spawn(move || {
        let ctrl_prog = Arc::clone(&controller);
        let report = execute_scan_with_progress(
            &config,
            prev_hist.as_ref(),
            Some(&move |snap| {
                if let Ok(mut g) = ctrl_prog.try_lock() {
                    g.progress = Some(snap);
                    if g.hwnd != 0 {
                        unsafe { PostMessageW(g.hwnd as HWND, WM_APP_SCAN_UPDATE, 0, 0) };
                    }
                }
            }),
        );

        let mut guard = controller.lock().unwrap();
        guard.history.record_report(&report);
        let _ = guard.history.save_atomic(&guard.history_path);
        guard.cache.update_from_report(&report);
        let _ = guard.cache.save_atomic(&guard.cache_path);
        let _ = save_last_scan_atomic(&guard.last_scan_path, &report);
        let _ = guard.config.save_atomic(&guard.config_path);
        guard.report = Some(report);
        guard.scanning = false;
        guard.progress = None;
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
    let (init_w, init_h) =
        outer_size_for_dpi(DEFAULT_LOGICAL_W, DEFAULT_LOGICAL_H, WS_OVERLAPPEDWINDOW, init_dpi);
    let init_theme = controller.lock().unwrap().config.theme_mode;

    let egui_ctx = egui::Context::default();
    configure_fonts_and_theme(&egui_ctx, init_theme);

    let host_state = Box::new(WindowHostState {
        controller: Arc::clone(&controller),
        ui_state: StorageUiState::default(),
        egui_ctx,
        painter: SoftwarePainter::new(),
        applied_titlebar_theme: None,
        events: Vec::new(),
        width: (DEFAULT_LOGICAL_W as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        height: (DEFAULT_LOGICAL_H as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        dpi: init_dpi,
        pixels_per_point: dpi_to_ppp(init_dpi),
        needs_redraw: true,
        followup_budget: 2,
        start_time: Instant::now(),
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
        ShowWindow(hwnd, SW_SHOW);
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

    unsafe {
        if !small_icon.is_null() {
            DestroyIcon(small_icon);
        }
        if !large_icon.is_null() {
            DestroyIcon(large_icon);
        }
    }
    Ok(())
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
                let st = unsafe { &mut *state_ptr };
                st.dpi = new_dpi;
                st.pixels_per_point = dpi_to_ppp(new_dpi);
                st.needs_redraw = true;
                st.followup_budget = 2;
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
        WM_PAINT => {
            let st = unsafe { &mut *state_ptr };
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
                    focused: true,
                    ..Default::default()
                };
                raw.viewports
                    .entry(ViewportId::ROOT)
                    .or_default()
                    .native_pixels_per_point = Some(ppp);

                let (config_snap, report_snap, scanning, progress_snap, sys_win) = {
                    let g = st.controller.lock().unwrap();
                    (
                        g.config.clone(),
                        g.report.clone(),
                        g.scanning,
                        g.progress.clone(),
                        g.system_windows_dir.clone(),
                    )
                };

                if st.applied_titlebar_theme != Some(config_snap.theme_mode) {
                    apply_titlebar_theme(hwnd, config_snap.theme_mode);
                    st.applied_titlebar_theme = Some(config_snap.theme_mode);
                }

                let mut actions: Vec<UiAction> = Vec::new();
                let ui_state = &mut st.ui_state;
                let full_out = st.egui_ctx.run(raw, |ctx| {
                    draw_app_ui(
                        ctx,
                        ui_state,
                        &config_snap,
                        report_snap.as_ref(),
                        scanning,
                        progress_snap.as_ref(),
                        &sys_win,
                        &mut |act| actions.push(act),
                    );
                });

                let clear_bg = match config_snap.theme_mode {
                    ThemeMode::Dark => CLEAR_BG_DARK_RGB,
                    ThemeMode::Light => CLEAR_BG_LIGHT_RGB,
                };
                st.painter.apply_textures(&full_out.textures_delta);
                let clipped = st
                    .egui_ctx
                    .tessellate(full_out.shapes, full_out.pixels_per_point);
                st.painter.paint_and_blit(
                    hdc,
                    st.width,
                    st.height,
                    full_out.pixels_per_point,
                    &clipped,
                    clear_bg,
                    Some(ps.rcPaint),
                );
                st.needs_redraw = false;

                let wants_zero_delay = full_out
                    .viewport_output
                    .values()
                    .any(|v| v.repaint_delay.is_zero());
                if wants_zero_delay && st.followup_budget > 0 {
                    st.followup_budget -= 1;
                    unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                } else {
                    st.followup_budget = 0;
                }

                for act in actions {
                    match act {
                        UiAction::StartRescan => {
                            trigger_background_scan(Arc::clone(&st.controller));
                        }
                        UiAction::ToggleTheme(next_mode) => {
                            let mut g = st.controller.lock().unwrap();
                            g.config.theme_mode = next_mode;
                            let _ = g.config.save_atomic(&g.config_path);
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                        UiAction::SaveConfig(new_cfg) => {
                            let mut g = st.controller.lock().unwrap();
                            g.config = new_cfg;
                            let _ = g.config.save_atomic(&g.config_path);
                            st.needs_redraw = true;
                            unsafe { PostMessageW(hwnd, WM_APP_FOLLOWUP_PAINT, 0, 0) };
                        }
                    }
                }
            }
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = lp_xy(lparam);
            let st = unsafe { &mut *state_ptr };
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
        WM_CLOSE => {
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
