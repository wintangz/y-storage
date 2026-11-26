#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod png;
mod sw_painter;
mod ui;
mod win32_host;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{Pos2, RawInput, Rect, Vec2, ViewportId};
use serde::Serialize;
use y_core::cache::{FolderSizeCache, default_last_scan_path, load_last_scan, save_last_scan_atomic};
use y_core::config::{AppConfig, ThemeMode};
use y_core::history::ScanHistory;
use y_core::model::ScanReport;
use y_platform_windows::paths::{enumerate_fixed_drives, get_windows_directory};
use y_platform_windows::query_process_memory_bytes;
use y_platform_windows::scanner::execute_scan_with_progress;

use crate::png::save_bgra_u32_as_png;
use crate::sw_painter::SoftwarePainter;
use crate::ui::{ActiveView, StorageUiState, configure_fonts_and_theme, draw_app_ui};
use crate::win32_host::{AppController, dpi_to_ppp, run_gui_app};

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
    let mut screenshot_out: Option<PathBuf> = None;
    let mut screenshot_view = "main".to_string();
    let mut screenshot_theme = ThemeMode::Dark;
    let mut screenshot_dpi: u32 = 96;

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
            _ => {}
        }
        i += 1;
    }

    if let Some(out_path) = scan_cli_out {
        run_cli_scan(&out_path, ephemeral);
        return;
    }

    if let Some(png_path) = screenshot_out {
        if let Err(err) = render_screenshot(&png_path, &screenshot_view, screenshot_theme, screenshot_dpi) {
            eprintln!("render screenshot failed: {err}");
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

fn render_screenshot(
    out_path: &PathBuf,
    view: &str,
    theme: ThemeMode,
    dpi: u32,
) -> Result<(), String> {
    let drives = enumerate_fixed_drives();
    let sys_win = get_windows_directory();
    let mut config = AppConfig::load_or_default(&AppConfig::default_path(), &drives, &sys_win);
    config.theme_mode = theme;

    let report = load_last_scan(&default_last_scan_path())
        .unwrap_or_else(|| execute_scan_with_progress(&config, None, None));

    let mut ui_state = StorageUiState::default();
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

    // Run 3 layout passes so ScrollAreas, Areas, and panel heights settle crisply.
    for step in 0..3 {
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(logical_w, logical_h),
            )),
            time: Some(step as f64 * 0.05),
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
                &config,
                Some(&report),
                false,
                None,
                &sys_win,
                &mut |_| {},
            );
        });
        painter.apply_textures(&full_out.textures_delta);
        if step == 2 {
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

    save_bgra_u32_as_png(out_path, phys_w, phys_h, painter.backbuffer_pixels())
}
