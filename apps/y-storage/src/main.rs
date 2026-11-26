#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod list_query;
mod logo;
mod logo_png;
mod png;
mod sw_painter;
mod ui;
mod win32_host;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{Pos2, RawInput, Rect, Vec2, ViewportId};
use serde::Serialize;
use y_core::cache::{FolderSizeCache, default_last_scan_path, load_last_scan, save_last_scan_atomic};
use y_core::config::{AppConfig, AppFilterChip, AppSortMode, SortDirection, ThemeMode};
use y_core::history::ScanHistory;
use y_core::model::ScanReport;
use y_platform_windows::paths::{enumerate_fixed_drives, get_windows_directory};
use y_platform_windows::query_process_memory_bytes;
use y_platform_windows::scanner::execute_scan_with_progress;

use crate::png::save_bgra_u32_as_png;
use crate::sw_painter::SoftwarePainter;
use crate::ui::{ActiveView, StorageUiState, configure_fonts_and_theme, draw_app_ui};
use crate::win32_host::{
    AppController, dpi_to_ppp, run_gui_app, run_perf_benchmark, run_titlebar_capture,
};

#[derive(Serialize)]
struct CliScanOutput {
    working_set_bytes: u64,
    private_bytes: u64,
    report: ScanReport,
}

#[derive(Serialize)]
struct GuiSmokeReport {
    working_set_bytes: u64,
    private_bytes: u64,
    apps_count: usize,
    has_cached_report: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut scan_cli_out: Option<PathBuf> = None;
    let mut ephemeral = false;
    let mut smoke_gui_ms: Option<u64> = None;
    let mut smoke_gui_out: Option<PathBuf> = None;
    let mut bench_perf_out: Option<PathBuf> = None;
    let mut capture_titlebar_out: Option<PathBuf> = None;
    let mut screenshot_out: Option<PathBuf> = None;
    let mut screenshot_view = "main".to_string();
    let mut screenshot_theme = ThemeMode::Dark;
    let mut screenshot_dpi: u32 = 96;
    let mut screenshot_sort = AppSortMode::Size;
    let mut screenshot_sort_dir: Option<SortDirection> = None;
    let mut screenshot_filter = AppFilterChip::All;
    let mut screenshot_search = String::new();
    let mut screenshot_ledger_collapsed: Option<bool> = None;
    let mut screenshot_ledger_height: Option<f32> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scan-cli" => {
                if i + 1 < args.len() {
                    scan_cli_out = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--ephemeral" => ephemeral = true,
            "--smoke-gui-ms" => {
                if i + 2 < args.len() {
                    smoke_gui_ms = args[i + 1].parse().ok();
                    smoke_gui_out = Some(PathBuf::from(&args[i + 2]));
                    i += 2;
                }
            }
            "--render-screenshot" => {
                if i + 1 < args.len() {
                    screenshot_out = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--bench-perf" => {
                if i + 1 < args.len() {
                    bench_perf_out = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--capture-titlebar" => {
                if i + 1 < args.len() {
                    capture_titlebar_out = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--view" => {
                if i + 1 < args.len() {
                    screenshot_view = args[i + 1].clone();
                    i += 1;
                }
            }
            "--theme" => {
                if i + 1 < args.len() {
                    screenshot_theme = match args[i + 1].to_ascii_lowercase().as_str() {
                        "light" => ThemeMode::Light,
                        _ => ThemeMode::Dark,
                    };
                    i += 1;
                }
            }
            "--dpi" => {
                if i + 1 < args.len() {
                    screenshot_dpi = args[i + 1].parse().unwrap_or(96);
                    i += 1;
                }
            }
            "--sort" => {
                if i + 1 < args.len() {
                    screenshot_sort = match args[i + 1].to_ascii_lowercase().as_str() {
                        "name" => AppSortMode::Name,
                        "publisher" => AppSortMode::Publisher,
                        "category" => AppSortMode::Category,
                        "growth" => AppSortMode::Growth,
                        _ => AppSortMode::Size,
                    };
                    i += 1;
                }
            }
            "--sort-dir" => {
                if i + 1 < args.len() {
                    screenshot_sort_dir = match args[i + 1].to_ascii_lowercase().as_str() {
                        "asc" => Some(SortDirection::Asc),
                        "desc" => Some(SortDirection::Desc),
                        _ => None,
                    };
                    i += 1;
                }
            }
            "--filter" => {
                if i + 1 < args.len() {
                    screenshot_filter = match args[i + 1].to_ascii_lowercase().as_str() {
                        "grew" => AppFilterChip::Grew,
                        "no-touch" | "notouch" => AppFilterChip::NoTouch,
                        "standalone" => AppFilterChip::Standalone,
                        "has-c" | "system-drive" => AppFilterChip::HasSystemDriveFootprint,
                        _ => AppFilterChip::All,
                    };
                    i += 1;
                }
            }
            "--search" => {
                if i + 1 < args.len() {
                    screenshot_search = args[i + 1].clone();
                    i += 1;
                }
            }
            "--ledger-collapsed" => {
                screenshot_ledger_collapsed = Some(true);
            }
            "--ledger-height" => {
                if i + 1 < args.len() {
                    screenshot_ledger_height = args[i + 1].parse::<f32>().ok();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(out_path) = scan_cli_out {
        run_cli_scan(&out_path, ephemeral);
        return;
    }

    if let Some(png_path) = screenshot_out {
        let sort_dir = screenshot_sort_dir.unwrap_or_else(|| screenshot_sort.default_direction());
        if let Err(err) = render_screenshot(
            &png_path,
            &screenshot_view,
            screenshot_theme,
            screenshot_dpi,
            screenshot_sort,
            sort_dir,
            screenshot_filter,
            &screenshot_search,
            screenshot_ledger_collapsed,
            screenshot_ledger_height,
        ) {
            eprintln!("render screenshot failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(bench_path) = bench_perf_out {
        if let Err(err) = run_perf_benchmark(&bench_path) {
            eprintln!("perf benchmark failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(cap_path) = capture_titlebar_out {
        let controller = Arc::new(Mutex::new(AppController::load_initial()));
        if let Err(err) = run_titlebar_capture(controller, &cap_path) {
            eprintln!("titlebar capture failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    let controller = Arc::new(Mutex::new(AppController::load_initial()));
    if let Err(err) = run_gui_app(Arc::clone(&controller), smoke_gui_ms) {
        eprintln!("y-storage GUI error: {err}");
        std::process::exit(1);
    }

    if let Some(smoke_path) = smoke_gui_out {
        let (ws, priv_b) = query_process_memory_bytes();
        let g = controller.lock().unwrap();
        let smoke = GuiSmokeReport {
            working_set_bytes: ws,
            private_bytes: priv_b,
            apps_count: g.report.as_ref().map_or(0, |r| r.apps.len()),
            has_cached_report: g.report.is_some(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&smoke) {
            let _ = std::fs::write(&smoke_path, json);
        }
    }
}

fn run_cli_scan(out_path: &PathBuf, ephemeral: bool) {
    let drives = enumerate_fixed_drives();
    let sys_win = get_windows_directory();
    let config_path = AppConfig::default_path();
    let history_path = ScanHistory::default_path();
    let cache_path = FolderSizeCache::default_path();
    let last_scan_path = default_last_scan_path();

    let config = if ephemeral {
        AppConfig::for_detected_drives(&drives, &sys_win)
    } else {
        AppConfig::load_or_default(&config_path, &drives, &sys_win)
    };
    let mut history = if ephemeral {
        ScanHistory::default()
    } else {
        ScanHistory::load_or_default(&history_path)
    };

    let report = execute_scan_with_progress(&config, history.latest_per_app(), None);
    if !ephemeral {
        history.record_report(&report);
        let _ = history.save_atomic(&history_path);
        let mut cache = FolderSizeCache::load_or_default(&cache_path);
        cache.update_from_report(&report);
        let _ = cache.save_atomic(&cache_path);
        let _ = save_last_scan_atomic(&last_scan_path, &report);
        let _ = config.save_atomic(&config_path);
    }

    let (ws, priv_b) = query_process_memory_bytes();
    let out = CliScanOutput {
        working_set_bytes: ws,
        private_bytes: priv_b,
        report,
    };
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&out).expect("serialize scan output");
    std::fs::write(out_path, json).expect("write CLI scan JSON");
}

fn render_screenshot_frame(
    report: &y_core::model::ScanReport,
    config: &AppConfig,
    sys_win: &str,
    view: &str,
    theme: ThemeMode,
    dpi: u32,
    sort_mode: AppSortMode,
    sort_direction: SortDirection,
    filter_chip: AppFilterChip,
    search_query: &str,
    ledger_collapsed: Option<bool>,
    ledger_height_px: Option<f32>,
) -> (usize, usize, Vec<u32>) {
    let mut cfg = config.clone();
    cfg.theme_mode = theme;

    let mut ui_state = StorageUiState::default();
    ui_state.sort_mode = sort_mode;
    ui_state.sort_direction = sort_direction;
    ui_state.filter_chip = filter_chip;
    ui_state.search_query = search_query.to_string();
    if let Some(c) = ledger_collapsed {
        ui_state.ledger_collapsed = c;
    }
    if let Some(h) = ledger_height_px {
        ui_state.ledger_height_px = Some(h);
    }

    let hover_size_mode = view == "hover-size";
    match view {
        "expanded-fifa23" | "expanded" => {
            ui_state.active_view = ActiveView::Main;
            if let Some(fifa) = report
                .apps
                .iter()
                .find(|a| a.display_name.to_ascii_lowercase().contains("fifa 23"))
            {
                ui_state.expanded_apps.insert(fifa.id.clone());
            } else if let Some(first) = report.apps.first() {
                ui_state.expanded_apps.insert(first.id.clone());
            }
        }
        "expanded-epic" => {
            ui_state.active_view = ActiveView::Main;
            if ui_state.search_query.is_empty() {
                ui_state.search_query = "Epic".to_string();
            }
            for app in &report.apps {
                let lower = app.display_name.to_ascii_lowercase();
                if lower.contains("epic games launcher") || lower.contains("epic online services") {
                    ui_state.expanded_apps.insert(app.id.clone());
                }
            }
        }
        "ledger-resized" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_shared_caches = true;
            ui_state.ledger_height_px = Some(268.0);
        }
        "reverse-ea-app" => {
            ui_state.active_view = ActiveView::Main;
            if let Some(fifa) = report
                .apps
                .iter()
                .find(|a| a.display_name.to_ascii_lowercase().contains("fifa 23"))
            {
                ui_state.expanded_apps.insert(fifa.id.clone());
            }
            ui_state.open_reverse_dep_id = Some("ea-app".to_string());
        }
        "settings" => {
            ui_state.active_view = ActiveView::Settings;
        }
        "settings-typed" => {
            ui_state.active_view = ActiveView::Settings;
            ui_state
                .new_exclusion_inputs
                .insert('C', r"C:\TempBuildCache".to_string());
        }
        _ => {
            ui_state.active_view = ActiveView::Main;
        }
    }

    let logical_w = 1020.0_f32;
    let logical_h = 720.0_f32;
    let ppp = dpi_to_ppp(dpi);
    let phys_w = (logical_w * ppp).round() as usize;
    let phys_h = (logical_h * ppp).round() as usize;

    let ctx = egui::Context::default();
    configure_fonts_and_theme(&ctx, theme);
    let mut painter = SoftwarePainter::new();
    let clear_rgb = match theme {
        ThemeMode::Dark => 0x000A_0A10,
        ThemeMode::Light => 0x00EE_F4FA,
    };

    let steps = if hover_size_mode { 4 } else { 3 };
    for step in 0..steps {
        let mut events = Vec::new();
        let time_s = if hover_size_mode {
            match step {
                0 => 0.0,
                1 => 0.1,
                2 => 0.9,
                _ => 1.5,
            }
        } else {
            step as f64 * 0.05
        };
        if hover_size_mode && step >= 1 {
            if let Some(size_r) = ui_state.first_row_size_rect {
                events.push(egui::Event::PointerMoved(size_r.center()));
            }
        }
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(logical_w, logical_h),
            )),
            time: Some(time_s),
            events,
            focused: true,
            ..Default::default()
        };
        raw.viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);
        let full_out = ctx.run(raw, |c| {
            draw_app_ui(
                c,
                &mut ui_state,
                &cfg,
                Some(report),
                false,
                None,
                sys_win,
                &mut |_| {},
            );
            if hover_size_mode && step + 1 == steps {
                if let Some(size_r) = ui_state.first_row_size_rect {
                    let hp = size_r.center();
                    let p = c.layer_painter(egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("hover_cursor_indicator"),
                    ));
                    let tip_pts = vec![
                        hp,
                        hp + Vec2::new(0.0, 13.0),
                        hp + Vec2::new(3.5, 9.5),
                        hp + Vec2::new(9.5, 9.5),
                    ];
                    p.add(egui::Shape::convex_polygon(
                        tip_pts,
                        egui::Color32::WHITE,
                        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
                    ));
                }
            }
        });
        painter.apply_textures(&full_out.textures_delta);
        if step + 1 == steps {
            let clipped = ctx.tessellate(full_out.shapes, full_out.pixels_per_point);
            painter.paint_and_blit(
                std::ptr::null_mut(),
                phys_w,
                phys_h,
                full_out.pixels_per_point,
                &clipped,
                clear_rgb,
                None,
            );
        }
    }
    (phys_w, phys_h, painter.backbuffer_pixels().to_vec())
}

fn upscale_crop_nn(
    src: &[u32],
    src_w: usize,
    cx0: usize,
    cy0: usize,
    cw: usize,
    ch: usize,
    scale: usize,
) -> (usize, usize, Vec<u32>) {
    let out_w = cw * scale;
    let out_h = ch * scale;
    let mut out = vec![0u32; out_w * out_h];
    for y in 0..ch {
        for x in 0..cw {
            let px = src[(cy0 + y) * src_w + (cx0 + x)];
            for dy in 0..scale {
                let row = (y * scale + dy) * out_w + (x * scale);
                out[row..row + scale].fill(px);
            }
        }
    }
    (out_w, out_h, out)
}

#[allow(clippy::too_many_arguments)]
fn render_screenshot(
    out_path: &PathBuf,
    view: &str,
    theme: ThemeMode,
    dpi: u32,
    sort_mode: AppSortMode,
    sort_direction: SortDirection,
    filter_chip: AppFilterChip,
    search_query: &str,
    ledger_collapsed: Option<bool>,
    ledger_height_px: Option<f32>,
) -> Result<(), String> {
    let drives = enumerate_fixed_drives();
    let sys_win = get_windows_directory();
    let config = AppConfig::load_or_default(&AppConfig::default_path(), &drives, &sys_win);

    let mut report = load_last_scan(&default_last_scan_path())
        .unwrap_or_else(|| execute_scan_with_progress(&config, None, None));
    if !report.apps.is_empty() && report.apps.iter().all(|a| a.icon_rgba.is_none()) {
        y_platform_windows::icons::populate_report_icons(&mut report);
    }
    if !report.apps.is_empty() && report.apps.iter().all(|a| a.category_basis.is_empty()) {
        y_core::categories::classify_report_apps(
            &mut report,
            y_platform_windows::paths::has_game_manifest_dir,
        );
    }

    if view == "centering-zoom-200" {
        // Composite 200% zoomed optical centering proof: 100% Dark + 125% Light header buttons & badge rows.
        let (dw, _dh, d_px) = render_screenshot_frame(
            &report,
            &config,
            &sys_win,
            "expanded-fifa23",
            ThemeMode::Dark,
            96,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            Some(false),
            None,
        );
        let (lw, _lh, l_px) = render_screenshot_frame(
            &report,
            &config,
            &sys_win,
            "expanded-fifa23",
            ThemeMode::Light,
            120,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            Some(false),
            None,
        );

        // Extract top-right header buttons & expanded badge row (Dependencies proven/shared/used-by strip).
        let (d_hdr_w, d_hdr_h, d_hdr) = upscale_crop_nn(&d_px, dw, 756, 8, 256, 40, 2);
        let (d_badge_w, d_badge_h, d_badge) = upscale_crop_nn(&d_px, dw, 58, 336, 860, 128, 2);
        let l_hx0 = (756.0_f32 * 1.25).round() as usize;
        let l_hw = (256.0_f32 * 1.25).round() as usize;
        let l_hy0 = (8.0_f32 * 1.25).round() as usize;
        let l_hh = (40.0_f32 * 1.25).round() as usize;
        let (l_hdr_w, l_hdr_h, l_hdr) = upscale_crop_nn(&l_px, lw, l_hx0, l_hy0, l_hw, l_hh, 2);
        let l_bx0 = (58.0_f32 * 1.25).round() as usize;
        let l_bw = (860.0_f32 * 1.25).round() as usize;
        let l_by0 = (336.0_f32 * 1.25).round() as usize;
        let l_bh = (128.0_f32 * 1.25).round() as usize;
        let (l_badge_w, l_badge_h, l_badge) = upscale_crop_nn(&l_px, lw, l_bx0, l_by0, l_bw, l_bh, 2);

        let pad = 16usize;
        let total_w = d_badge_w.max(l_badge_w).max(d_hdr_w + l_hdr_w + pad) + pad * 2;
        let total_h = pad + d_hdr_h.max(l_hdr_h) + pad + d_badge_h + pad + l_badge_h + pad;
        let mut canvas = vec![0x0014_141F_u32; total_w * total_h];

        let mut blit = |src: &[u32], sw: usize, sh: usize, dx: usize, dy: usize| {
            for r in 0..sh {
                let dst_off = (dy + r) * total_w + dx;
                let src_off = r * sw;
                canvas[dst_off..dst_off + sw].copy_from_slice(&src[src_off..src_off + sw]);
            }
        };
        let row1_y = pad;
        blit(&d_hdr, d_hdr_w, d_hdr_h, pad, row1_y);
        blit(&l_hdr, l_hdr_w, l_hdr_h, pad + d_hdr_w + pad, row1_y);
        let row2_y = row1_y + d_hdr_h.max(l_hdr_h) + pad;
        blit(&d_badge, d_badge_w, d_badge_h, pad, row2_y);
        let row3_y = row2_y + d_badge_h + pad;
        blit(&l_badge, l_badge_w, l_badge_h, pad, row3_y);

        return save_bgra_u32_as_png(out_path, total_w, total_h, &canvas);
    }

    let ppp = dpi_to_ppp(dpi);
    let (phys_w, phys_h, fb) = render_screenshot_frame(
        &report,
        &config,
        &sys_win,
        view,
        theme,
        dpi,
        sort_mode,
        sort_direction,
        filter_chip,
        search_query,
        ledger_collapsed,
        ledger_height_px,
    );

    if view == "header-buttons-zoom" {
        let crop_x0 = (756.0 * ppp).round() as usize;
        let crop_w = phys_w.saturating_sub(crop_x0).max(1);
        let crop_h = ((52.0 * ppp).round() as usize).clamp(1, phys_h);
        let (out_w, out_h, zoomed) = upscale_crop_nn(&fb, phys_w, crop_x0, 0, crop_w, crop_h, 3);
        return save_bgra_u32_as_png(out_path, out_w, out_h, &zoomed);
    }

    save_bgra_u32_as_png(out_path, phys_w, phys_h, &fb)
}
