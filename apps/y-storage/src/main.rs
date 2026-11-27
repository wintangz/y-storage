#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod list_query;
mod logo;
mod logo_png;
mod png;
mod sw_painter;
mod theme;
mod ui;
mod unattributed_assist;
mod win32_host;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{Pos2, RawInput, Rect, Vec2, ViewportId};
use serde::{Deserialize, Serialize};
use y_core::cache::{FolderSizeCache, default_last_scan_path, load_last_scan, save_last_scan_atomic};
use y_core::config::{
    AppConfig, AppFilterChip, AppSortMode, ContainerRulebook, DriveScanTarget, SizeBandFilter,
    SortDirection, ThemeMode, ThemePresetChoice, UnattributedLocationZone, ZoneRuleKind,
    ZoneRulebook,
};
use y_core::history::{ScanHistory, ScanScopeKind};
use y_core::model::{
    AppCategory, AttributedApp, AttributionBasis, BreakdownItem, DriveInfo, GrowthDelta,
    RegistryDiscoveryStats, ScanReport, SystemDriveBreakdown, UnattributedItem, ZoneKind,
};
use y_platform_windows::paths::{enumerate_fixed_drives, get_windows_directory};
use y_platform_windows::query_process_memory_bytes;
use y_platform_windows::scanner::{execute_scan_with_history, execute_scan_with_progress};

use crate::png::save_bgra_u32_as_png;
use crate::sw_painter::SoftwarePainter;
use crate::ui::{
    ActiveView, StorageUiState, configure_fonts_and_theme_with, draw_app_ui,
};
use crate::win32_host::{
    AppController, dpi_to_ppp, run_gui_app, run_perf_benchmark, run_titlebar_capture,
};

#[derive(Serialize, Deserialize)]
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
    let mut scan_all_drives = false;
    let mut smoke_gui_ms: Option<u64> = None;
    let mut smoke_gui_out: Option<PathBuf> = None;
    let mut bench_perf_out: Option<PathBuf> = None;
    let mut capture_titlebar_out: Option<PathBuf> = None;
    let mut screenshot_out: Option<PathBuf> = None;
    let mut screenshot_report_json: Option<PathBuf> = None;
    let mut screenshot_view = "main".to_string();
    let mut screenshot_theme = ThemeMode::Dark;
    let mut screenshot_preset: Option<ThemePresetChoice> = None;
    let mut screenshot_custom_rgb: Option<[u8; 3]> = None;
    let mut screenshot_dpi: u32 = 96;
    let mut screenshot_sort = AppSortMode::Size;
    let mut screenshot_sort_dir: Option<SortDirection> = None;
    let mut screenshot_filter = AppFilterChip::All;
    let mut screenshot_search = String::new();
    let mut screenshot_ledger_collapsed: Option<bool> = None;
    let mut screenshot_ledger_height: Option<f32> = None;
    let mut screenshot_facet_publisher: Option<String> = None;
    let mut screenshot_facet_drive: Option<char> = None;
    let mut screenshot_facet_size = SizeBandFilter::All;
    let mut screenshot_open_facet: Option<String> = None;
    let mut screenshot_unattrib_search = String::new();
    let mut screenshot_unattrib_drive: Option<char> = None;
    let mut screenshot_unattrib_size = SizeBandFilter::All;
    let mut screenshot_unattrib_zone = UnattributedLocationZone::All;
    let mut screenshot_exclude_drives: Vec<char> = Vec::new();
    let mut screenshot_all_drives_enabled = false;
    let mut screenshot_sim_drives: Option<usize> = None;

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
            "--all-drives" => scan_all_drives = true,
            "--all-drives-enabled" => screenshot_all_drives_enabled = true,
            "--report-json" => {
                if i + 1 < args.len() {
                    screenshot_report_json = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--exclude-drive" => {
                if i + 1 < args.len() {
                    if let Some(ch) = args[i + 1].trim().chars().next() {
                        screenshot_exclude_drives.push(ch.to_ascii_uppercase());
                    }
                    i += 1;
                }
            }
            "--unattrib-drive" => {
                if i + 1 < args.len() {
                    screenshot_unattrib_drive = args[i + 1]
                        .trim()
                        .chars()
                        .next()
                        .map(|c| c.to_ascii_uppercase());
                    i += 1;
                }
            }
            "--unattrib-size" => {
                if i + 1 < args.len() {
                    screenshot_unattrib_size = match args[i + 1].to_ascii_lowercase().as_str() {
                        "ge-10gb" | ">=10gb" => SizeBandFilter::Ge10Gb,
                        "1-10gb" | "1gb-10gb" => SizeBandFilter::Between1And10Gb,
                        "ge-1gb" | ">=1gb" => SizeBandFilter::Ge1Gb,
                        "100mb-1gb" => SizeBandFilter::Between100MbAnd1Gb,
                        "lt-100mb" | "<100mb" => SizeBandFilter::Lt100Mb,
                        _ => SizeBandFilter::All,
                    };
                    i += 1;
                }
            }
            "--unattrib-zone" => {
                if i + 1 < args.len() {
                    screenshot_unattrib_zone = match args[i + 1].to_ascii_lowercase().as_str() {
                        "appdata-local" | "local" => UnattributedLocationZone::AppDataLocal,
                        "appdata-roaming" | "roaming" => UnattributedLocationZone::AppDataRoaming,
                        "appdata-locallow" | "locallow" => UnattributedLocationZone::AppDataLocalLow,
                        "programdata" => UnattributedLocationZone::ProgramData,
                        "program-files" => UnattributedLocationZone::ProgramFiles,
                        "user-profile" | "user-root" => UnattributedLocationZone::UserProfileRoot,
                        "other" | "other-drive" => UnattributedLocationZone::OtherDriveLocation,
                        _ => UnattributedLocationZone::All,
                    };
                    i += 1;
                }
            }
            "--sim-drives" => {
                if i + 1 < args.len() {
                    screenshot_sim_drives = args[i + 1].parse::<usize>().ok();
                    i += 1;
                }
            }
            "--smoke-gui-ms" => {
                if i + 1 < args.len() {
                    smoke_gui_ms = args[i + 1].parse().ok();
                    i += 1;
                    if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                        smoke_gui_out = Some(PathBuf::from(&args[i + 1]));
                        i += 1;
                    }
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
            "--theme-preset" => {
                if i + 1 < args.len() {
                    screenshot_preset = Some(match args[i + 1].to_ascii_lowercase().as_str() {
                        "pink" => ThemePresetChoice::Pink,
                        "purple" => ThemePresetChoice::Purple,
                        "green" => ThemePresetChoice::Green,
                        "orange" => ThemePresetChoice::Orange,
                        "yellow" => ThemePresetChoice::Yellow,
                        "red" => ThemePresetChoice::Red,
                        "custom" => ThemePresetChoice::Custom,
                        _ => ThemePresetChoice::Blue,
                    });
                    i += 1;
                }
            }
            "--theme-custom" => {
                if i + 1 < args.len() {
                    if let Some(rgb) = crate::theme::parse_hex_rgb(&args[i + 1]) {
                        screenshot_preset = Some(ThemePresetChoice::Custom);
                        screenshot_custom_rgb = Some(rgb);
                    }
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
            "--facet-publisher" => {
                if i + 1 < args.len() {
                    screenshot_facet_publisher = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--facet-drive" => {
                if i + 1 < args.len() {
                    screenshot_facet_drive = args[i + 1]
                        .trim()
                        .chars()
                        .next()
                        .map(|c| c.to_ascii_uppercase());
                    i += 1;
                }
            }
            "--open-facet" => {
                if i + 1 < args.len() {
                    screenshot_open_facet = Some(args[i + 1].to_ascii_lowercase());
                    i += 1;
                }
            }
            "--facet-size" => {
                if i + 1 < args.len() {
                    screenshot_facet_size = match args[i + 1].to_ascii_lowercase().as_str() {
                        "ge-10gb" | ">=10gb" => SizeBandFilter::Ge10Gb,
                        "1-10gb" | "1gb-10gb" => SizeBandFilter::Between1And10Gb,
                        "ge-1gb" | ">=1gb" => SizeBandFilter::Ge1Gb,
                        "100mb-1gb" => SizeBandFilter::Between100MbAnd1Gb,
                        "lt-100mb" | "<100mb" => SizeBandFilter::Lt100Mb,
                        _ => SizeBandFilter::All,
                    };
                    i += 1;
                }
            }
            "--unattrib-search" => {
                if i + 1 < args.len() {
                    screenshot_unattrib_search = args[i + 1].clone();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(out_path) = scan_cli_out {
        run_cli_scan(&out_path, ephemeral, scan_all_drives);
        return;
    }

    if let Some(png_path) = screenshot_out {
        let sort_dir = screenshot_sort_dir.unwrap_or_else(|| screenshot_sort.default_direction());
        if let Err(err) = render_screenshot(
            &png_path,
            screenshot_report_json.as_ref(),
            &screenshot_view,
            screenshot_theme,
            screenshot_preset,
            screenshot_custom_rgb,
            screenshot_dpi,
            screenshot_sort,
            sort_dir,
            screenshot_filter,
            &screenshot_search,
            screenshot_ledger_collapsed,
            screenshot_ledger_height,
            screenshot_facet_publisher.as_deref(),
            screenshot_facet_drive,
            screenshot_facet_size,
            screenshot_open_facet.as_deref(),
            &screenshot_unattrib_search,
            screenshot_unattrib_drive,
            screenshot_unattrib_size,
            screenshot_unattrib_zone,
            &screenshot_exclude_drives,
            screenshot_all_drives_enabled,
            screenshot_sim_drives,
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

fn run_cli_scan(out_path: &PathBuf, ephemeral: bool, all_drives: bool) {
    let drives = enumerate_fixed_drives();
    let sys_win = get_windows_directory();
    let config_path = AppConfig::default_path();
    let history_path = ScanHistory::default_path();
    let cache_path = FolderSizeCache::default_path();
    let last_scan_path = default_last_scan_path();

    let mut config = if ephemeral {
        if config_path.exists() {
            AppConfig::load_or_default(&config_path, &drives, &sys_win)
        } else {
            AppConfig::for_detected_drives(&drives, &sys_win)
        }
    } else {
        AppConfig::load_or_default(&config_path, &drives, &sys_win)
    };
    if all_drives {
        for t in &mut config.targets {
            t.enabled = true;
        }
    }
    let mut history = if history_path.exists() {
        ScanHistory::load_or_default(&history_path)
    } else {
        ScanHistory::default()
    };

    let report = execute_scan_with_history(&config, &history, None);
    if !ephemeral {
        let refreshed: Vec<char> = config
            .targets
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.letter)
            .collect();
        history.record_report_with_scope(&report, ScanScopeKind::Full, &refreshed);
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

fn build_synthetic_screenshot_report(drive_count: usize) -> (ScanReport, AppConfig) {
    let letters = ['C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L'];
    let count = drive_count.clamp(1, letters.len());

    let sample_specs = [
        ("Microsoft Edge", "Microsoft Corporation", AppCategory::Browsers, 8_280_000_000_u64, 'C'),
        ("Visual Studio Code", "Microsoft Corporation", AppCategory::Development, 4_120_000_000, 'C'),
        ("Discord", "Discord Inc.", AppCategory::Communication, 2_310_000_000, 'C'),
        ("RustRover", "JetBrains s.r.o.", AppCategory::Development, 3_640_000_000, 'C'),
        ("ShareX", "", AppCategory::Utilities, 412_000_000, 'C'),
        ("qBittorrent", "The qBittorrent project", AppCategory::Utilities, 198_000_000, 'C'),
    ];
    let mut apps: Vec<AttributedApp> = sample_specs
        .iter()
        .enumerate()
        .map(|(i, (name, pub_name, cat, sz, dl))| AttributedApp {
            id: format!("sim:app:{i}"),
            display_name: (*name).to_string(),
            publishers: if pub_name.is_empty() {
                vec![]
            } else {
                vec![(*pub_name).to_string()]
            },
            install_location: Some(format!(r"{dl}:\Program Files\{name}")),
            total_bytes: *sz,
            previous_bytes: Some(*sz),
            growth: GrowthDelta::Changed(0),
            per_drive_growth: Default::default(),
            no_touch: None,
            bases: vec![AttributionBasis::InstallLocation],
            items: vec![BreakdownItem {
                path: format!(r"{dl}:\Program Files\{name}"),
                bytes: *sz,
                zone: Some(ZoneKind::Body),
                qualifier: None,
                basis: Some(AttributionBasis::InstallLocation),
                basis_detail: None,
                no_touch: None,
                drive_letter: *dl,
            }],
            dependencies: vec![],
            carved_out_apps: vec![],
            display_icon: None,
            icon_rgba: None,
            category: *cat,
            category_basis: "synthetic".to_string(),
        })
        .collect();
    if count >= 2 {
        apps.push(AttributedApp {
            id: "sim:folder-only:d1".to_string(),
            display_name: "Starbound".to_string(),
            publishers: vec![],
            install_location: Some(r"D:\Games\Starbound".to_string()),
            total_bytes: 5_550_000_000,
            previous_bytes: None,
            growth: GrowthDelta::NoBaseline,
            per_drive_growth: Default::default(),
            no_touch: None,
            bases: vec![AttributionBasis::ContainerRule],
            items: vec![BreakdownItem {
                path: r"D:\Games\Starbound".to_string(),
                bytes: 5_550_000_000,
                zone: Some(ZoneKind::Body),
                qualifier: None,
                basis: Some(AttributionBasis::ContainerRule),
                basis_detail: Some("layout rule: Games".to_string()),
                no_touch: None,
                drive_letter: 'D',
            }],
            dependencies: vec![],
            carved_out_apps: vec![],
            display_icon: None,
            icon_rgba: None,
            category: AppCategory::Games,
            category_basis: "layout rule: Games".to_string(),
        });
    }
    apps.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));

    let unattributed = vec![UnattributedItem {
        path: r"C:\Users\Win\AppData\Local\Firaxis Games".to_string(),
        bytes: 184_000_000,
        drive_letter: 'C',
        zone_hint: Some(ZoneKind::Red),
        is_loose_files: false,
    }];
    let unattrib_total: u64 = unattributed.iter().map(|u| u.bytes).sum();

    let mut drives = Vec::with_capacity(count);
    let mut targets = Vec::with_capacity(count);
    let mut windows_system = Vec::new();
    let mut win_total = 0_u64;

    for idx in 0..count {
        let dl = letters[idx];
        let is_sys = idx == 0;
        let total_bytes = 250_000_000_000_u64 + (idx as u64) * 180_000_000_000;
        let used_bytes = (total_bytes as f64 * (0.42 + ((idx * 3) % 5) as f64 * 0.11)) as u64;
        let free_bytes = total_bytes - used_bytes;
        let included = !((count >= 4 && idx == 1) || (count >= 8 && idx == 7));
        let excl = if is_sys {
            vec![r"C:\WINDOWS".to_string()]
        } else if idx == 1 {
            vec![format!(r"{dl}:\Backups")]
        } else {
            Vec::new()
        };
        drives.push(DriveInfo {
            letter: dl,
            root: format!(r"{dl}:\"),
            total_bytes,
            free_bytes,
            used_bytes,
            is_system_drive: is_sys,
            full_walk_enabled: included,
            exclusions: excl.clone(),
            last_scanned_unix: included.then_some(1_795_771_200 - (idx as u64) * 120),
            last_scanned_display: included.then_some(format!("2026-11-27 18:{:02}:14", 42 - idx * 3)),
        });
        targets.push(DriveScanTarget {
            letter: dl,
            enabled: included,
            exclusions: excl,
        });
        if included {
            let apps_on_drv: u64 = apps
                .iter()
                .flat_map(|a| a.items.iter())
                .filter(|it| it.drive_letter == dl)
                .map(|it| it.bytes)
                .sum();
            let unattrib_on_drv: u64 = unattributed
                .iter()
                .filter(|u| u.drive_letter == dl)
                .map(|u| u.bytes)
                .sum();
            let scanned_drv = apps_on_drv + unattrib_on_drv;
            let rem = used_bytes.saturating_sub(scanned_drv);
            win_total += rem;
            windows_system.push(SystemDriveBreakdown {
                drive_letter: dl,
                root_files: vec![],
                os_folders: vec![],
                hardlink_overlap_bytes: 0,
                remainder_bytes: rem,
                total_bytes: rem,
                scanned_non_system_bytes: scanned_drv,
                drive_used_bytes: used_bytes,
                drive_free_bytes: free_bytes,
                drive_total_bytes: total_bytes,
            });
        }
    }

    let containers = ContainerRulebook::default();
    let mut cfg = AppConfig::default();
    cfg.targets = targets;
    cfg.containers = containers.clone();

    let report = ScanReport {
        scan_timestamp_unix: 1_795_771_200,
        scan_timestamp_display: "2026-11-27 18:42:14".to_string(),
        duration_ms: 4820,
        drives,
        registry_stats: RegistryDiscoveryStats {
            raw_display_name_entries: apps.len(),
            raw_with_install_location: apps.len(),
            canonical_apps: apps.len(),
            canonical_with_direct_install_location: apps.len(),
            canonical_with_inferred_install_location: 0,
        },
        apps,
        shared_caches: vec![],
        shared_caches_total_bytes: 0,
        your_files: vec![],
        your_files_total_bytes: 0,
        windows_system,
        windows_system_total_bytes: win_total,
        unattributed,
        unattributed_total_bytes: unattrib_total,
        reverse_dependencies: vec![],
        installed_registry_names: vec![],
        applied_containers: Some(containers),
        applied_zones: Some(ZoneRulebook::default()),
        catalog_source: "synthetic-layout".to_string(),
        warnings: vec![],
        evidence_curated_conflicts: vec![],
    };
    (report, cfg)
}

fn build_group_node_fixture_screenshot_report() -> (ScanReport, AppConfig) {
    use std::collections::HashMap;
    use y_core::attribution::{AttributionOptions, RootExeCandidate, attribute_scan_with_options};
    use y_core::config::DriveScanTarget;
    use y_core::model::{FolderNode, InstalledApp};
    use y_core::tree::{DecompositionOptions, decompose_scanned_tree_with_options};

    let zones = ZoneRulebook::default();
    let apps_catalog = vec![
        InstalledApp {
            id: "reg:hoyoplay".into(),
            display_name: "HoYoPlay".into(),
            display_version: "1.19".into(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["COGNOSPHERE PTE. LTD.".into()],
            direct_install_location: Some(r"C:\Program Files\HoYoPlay".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"C:\Program Files\HoYoPlay".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "reg:genshin-impact".into(),
            display_name: "Genshin Impact".into(),
            display_version: "5.4".into(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["COGNOSPHERE PTE. LTD.".into()],
            direct_install_location: Some(r"C:\Program Files\HoYoPlay\Genshin Impact Game".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"C:\Program Files\HoYoPlay\Genshin Impact Game".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
    ];

    let tree_c = FolderNode {
        path: r"C:\".into(),
        name: r"C:\".into(),
        direct_file_bytes: 0,
        total_bytes: 97_200_000_000,
        children: vec![FolderNode {
            path: r"C:\Program Files".into(),
            name: "Program Files".into(),
            direct_file_bytes: 0,
            total_bytes: 97_200_000_000,
            children: vec![FolderNode {
                path: r"C:\Program Files\HoYoPlay".into(),
                name: "HoYoPlay".into(),
                direct_file_bytes: 1_200_000_000,
                total_bytes: 97_200_000_000,
                children: vec![
                    FolderNode {
                        path: r"C:\Program Files\HoYoPlay\LauncherData".into(),
                        name: "LauncherData".into(),
                        direct_file_bytes: 500_000_000,
                        total_bytes: 500_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"C:\Program Files\HoYoPlay\Genshin Impact Game".into(),
                        name: "Genshin Impact Game".into(),
                        direct_file_bytes: 95_500_000_000,
                        total_bytes: 95_500_000_000,
                        children: vec![],
                    },
                ],
            }],
        }],
    };
    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 14_400_000_000,
        children: vec![FolderNode {
            path: r"D:\Apps".into(),
            name: "Apps".into(),
            direct_file_bytes: 0,
            total_bytes: 14_400_000_000,
            children: vec![FolderNode {
                path: r"D:\Apps\Adobe".into(),
                name: "Adobe".into(),
                direct_file_bytes: 0,
                total_bytes: 14_400_000_000,
                children: vec![
                    FolderNode {
                        path: r"D:\Apps\Adobe\Photoshop 2025".into(),
                        name: "Photoshop 2025".into(),
                        direct_file_bytes: 4_800_000_000,
                        total_bytes: 4_800_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Apps\Adobe\Illustrator 2025".into(),
                        name: "Illustrator 2025".into(),
                        direct_file_bytes: 3_400_000_000,
                        total_bytes: 3_400_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Apps\Adobe\Premiere Pro 2025".into(),
                        name: "Premiere Pro 2025".into(),
                        direct_file_bytes: 6_200_000_000,
                        total_bytes: 6_200_000_000,
                        children: vec![],
                    },
                ],
            }],
        }],
    };

    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\adobe\photoshop 2025".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\Adobe\Photoshop 2025\Photoshop.exe".into(),
            product_name: Some("Adobe Photoshop 2025".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"d:\apps\adobe\illustrator 2025".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\Adobe\Illustrator 2025\Illustrator.exe".into(),
            product_name: Some("Adobe Illustrator 2025".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"d:\apps\adobe\premiere pro 2025".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\Adobe\Premiere Pro 2025\Premiere.exe".into(),
            product_name: Some("Adobe Premiere Pro 2025".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let decomp_opts = DecompositionOptions {
        zones: Some(&zones),
        manifest_entries: &[],
        exe_candidates: Some(&exe_map),
    };
    let mut units = decompose_scanned_tree_with_options(&tree_c, &apps_catalog, &[], &[], &decomp_opts);
    units.extend(decompose_scanned_tree_with_options(
        &tree_d,
        &apps_catalog,
        &[],
        &[],
        &decomp_opts,
    ));

    let drives = vec![
        DriveInfo {
            letter: 'C',
            root: r"C:\".into(),
            total_bytes: 250_000_000_000,
            free_bytes: 110_000_000_000,
            used_bytes: 140_000_000_000,
            is_system_drive: true,
            full_walk_enabled: true,
            exclusions: vec![r"C:\WINDOWS".into()],
            last_scanned_unix: Some(1_795_775_000),
            last_scanned_display: Some("2026-11-27 19:55:00".into()),
        },
        DriveInfo {
            letter: 'D',
            root: r"D:\".into(),
            total_bytes: 500_000_000_000,
            free_bytes: 400_000_000_000,
            used_bytes: 100_000_000_000,
            is_system_drive: false,
            full_walk_enabled: true,
            exclusions: vec![],
            last_scanned_unix: Some(1_795_775_000),
            last_scanned_display: Some("2026-11-27 19:55:00".into()),
        },
    ];
    let mut report = attribute_scan_with_options(
        1_795_775_000,
        "2026-11-27 19:55:00".into(),
        120,
        drives,
        RegistryDiscoveryStats {
            raw_display_name_entries: 2,
            raw_with_install_location: 2,
            canonical_apps: 2,
            canonical_with_direct_install_location: 2,
            canonical_with_inferred_install_location: 0,
        },
        &apps_catalog,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            containers: None,
            zones: Some(&zones),
            manifest_entries: &[],
            steam_library_roots: &[],
            root_exe_candidates: Some(&exe_map),
            services: &[],
            evidence: y_core::attribution::EvidenceBundle::default(),
        },
    );
    y_core::categories::classify_report_apps(&mut report, |_| false);

    let mut cfg = AppConfig::default();
    cfg.zones = zones;
    cfg.targets = vec![
        DriveScanTarget {
            letter: 'C',
            enabled: true,
            exclusions: vec![r"C:\WINDOWS".into()],
        },
        DriveScanTarget {
            letter: 'D',
            enabled: true,
            exclusions: vec![],
        },
    ];
    (report, cfg)
}

#[allow(clippy::too_many_arguments)]
fn render_screenshot_frame(
    report: &y_core::model::ScanReport,
    config: &AppConfig,
    sys_win: &str,
    view: &str,
    theme: ThemeMode,
    preset_override: Option<ThemePresetChoice>,
    custom_rgb_override: Option<[u8; 3]>,
    dpi: u32,
    sort_mode: AppSortMode,
    sort_direction: SortDirection,
    filter_chip: AppFilterChip,
    search_query: &str,
    ledger_collapsed: Option<bool>,
    ledger_height_px: Option<f32>,
    facet_publisher: Option<&str>,
    facet_drive: Option<char>,
    facet_size: SizeBandFilter,
    open_facet: Option<&str>,
    unattrib_search: &str,
    unattrib_drive: Option<char>,
    unattrib_size: SizeBandFilter,
    unattrib_zone: UnattributedLocationZone,
) -> (usize, usize, Vec<u32>, Option<Rect>) {
    let mut cfg = config.clone();
    cfg.theme_mode = theme;
    if let Some(p) = preset_override {
        cfg.theme_preset = p;
    }
    if let Some(rgb) = custom_rgb_override {
        cfg.theme_custom_rgb = rgb;
    }

    let mut ui_state = StorageUiState::from_config(&cfg);
    ui_state.sort_mode = sort_mode;
    ui_state.sort_direction = sort_direction;
    ui_state.filter_chip = filter_chip;
    ui_state.search_query = search_query.to_string();
    ui_state.facets.publisher = facet_publisher.map(str::to_string);
    ui_state.facets.drive = facet_drive;
    ui_state.facets.size_band = facet_size;
    ui_state.open_facet_popup = match open_facet {
        Some("publisher") => Some("publisher"),
        Some("category") => Some("category"),
        Some("drive") => Some("drive"),
        Some("size") => Some("size"),
        Some("deps") => Some("deps"),
        Some("zone") => Some("zone"),
        Some("growth") => Some("growth"),
        _ => None,
    };
    ui_state.unattrib_search_query = unattrib_search.to_string();
    ui_state.unattrib_facets.drive = unattrib_drive;
    ui_state.unattrib_facets.size_band = unattrib_size;
    ui_state.unattrib_facets.location_zone = unattrib_zone;
    if let Some(c) = ledger_collapsed {
        ui_state.ledger_collapsed = c;
    }
    if let Some(h) = ledger_height_px {
        ui_state.ledger_height_px = Some(h);
    }

    let hover_size_mode = view == "hover-size";
    let hover_web_search_tooltip = view == "unattributed-hover-query";
    let hover_web_search_btn = view == "unattributed-hover-btn";
    let hover_web_search_mode = hover_web_search_tooltip || hover_web_search_btn;
    let hover_unattrib_row = view == "unattributed-row-hover";
    let hover_mode = hover_size_mode || hover_web_search_mode || hover_unattrib_row;
    match view {
        "unattributed-expanded" | "unattributed-hover-query" | "unattributed-hover-btn" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_unattributed = true;
            ui_state.ledger_height_px = Some(300.0);
        }
        "unattributed-row-hover" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_unattributed = true;
            ui_state.ledger_height_px = Some(340.0);
        }
        "ledger-max-height" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_unattributed = true;
            ui_state.ledger_height_px = Some(2000.0);
        }
        "ledger-unattrib-bottom" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_unattributed = true;
            ui_state.ledger_height_px = Some(460.0);
            ui_state.scroll_ledger_bottom = true;
        }
        "unattributed-filter" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_unattributed = true;
            ui_state.ledger_height_px = Some(340.0);
            if ui_state.unattrib_search_query.is_empty() {
                ui_state.unattrib_search_query = "microsoft".to_string();
            }
        }
        "expanded-edge" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            if ui_state.search_query.is_empty() {
                ui_state.search_query = "Microsoft Edge".to_string();
            }
            if let Some(edge) = report
                .apps
                .iter()
                .find(|a| a.display_name.eq_ignore_ascii_case("Microsoft Edge"))
            {
                ui_state.expanded_apps.insert(edge.id.clone());
            }
        }
        "accent-showcase" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            ui_state.filter_chip = AppFilterChip::HasSystemDriveFootprint;
            if ui_state.open_facet_popup.is_none() {
                ui_state.open_facet_popup = Some("category");
            }
        }
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
        "vcpp-split" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            if ui_state.search_query.is_empty() {
                ui_state.search_query = "Visual C++".to_string();
            }
        }
        "vcpp-drawer" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            if ui_state.search_query.is_empty() {
                ui_state.search_query = "Visual C++".to_string();
            }
            if let Some(vcpp) = report.apps.iter().find(|a| {
                let lower = a.display_name.to_ascii_lowercase();
                lower.contains("visual c++") && lower.contains("v14")
            }).or_else(|| {
                report.apps.iter().find(|a| a.display_name.to_ascii_lowercase().contains("visual c++"))
            }) {
                ui_state.expanded_apps.insert(vcpp.id.clone());
            }
        }
        "msteams-drawer" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            if ui_state.search_query.is_empty() {
                ui_state.search_query = "MSTeams".to_string();
            }
            if let Some(teams) = report
                .apps
                .iter()
                .find(|a| a.display_name.eq_ignore_ascii_case("Store: MSTeams"))
            {
                ui_state.expanded_apps.insert(teams.id.clone());
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
        "settings-full" => {
            ui_state.active_view = ActiveView::Settings;
            ui_state.settings_focus_containers = false;
        }
        "settings-containers" | "settings-zones" => {
            ui_state.active_view = ActiveView::Settings;
            ui_state.settings_focus_containers = true;
        }
        "your-files-media" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_your_files = true;
            ui_state.ledger_height_px = Some(280.0);
        }
        "group-node-fixture" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            if let Some(hl) = report
                .apps
                .iter()
                .find(|a| a.display_name.eq_ignore_ascii_case("HoYoPlay"))
            {
                ui_state.expanded_apps.insert(hl.id.clone());
            }
        }
        "settings-typed" => {
            ui_state.active_view = ActiveView::Settings;
            ui_state
                .new_exclusion_inputs
                .insert('C', r"C:\TempBuildCache".to_string());
        }
        "pods-scope" | "drive-excluded" | "pods-sim" => {
            ui_state.active_view = ActiveView::Main;
            if ledger_collapsed.is_none() {
                ui_state.ledger_collapsed = true;
            }
        }
        "pod-scope-overlay" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            ui_state.open_pod_scope_drive = Some('C');
            ui_state
                .new_exclusion_inputs
                .insert('C', r"C:\BuildScratch".to_string());
        }
        "unattributed-facets" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = false;
            ui_state.expand_unattributed = true;
            ui_state.ledger_height_px = Some(430.0);
            if ui_state.open_unattrib_facet_popup.is_none() {
                ui_state.open_unattrib_facet_popup = Some("unattrib_zone");
            }
        }
        "corner-aa-zoom-4x" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            ui_state.filter_chip = AppFilterChip::HasSystemDriveFootprint;
        }
        "folder-only-apps" => {
            ui_state.active_view = ActiveView::Main;
            ui_state.ledger_collapsed = true;
            ui_state.facets.drive = Some('D');
            for a in &report.apps {
                if a.bases.iter().any(|b| {
                    matches!(
                        b,
                        AttributionBasis::ContainerRule
                            | AttributionBasis::SteamManifest
                            | AttributionBasis::ExecutableScan
                    )
                }) {
                    if ui_state.expanded_apps.is_empty() {
                        ui_state.expanded_apps.insert(a.id.clone());
                    }
                }
            }
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
    configure_fonts_and_theme_with(&ctx, cfg.theme_mode, cfg.theme_preset, cfg.theme_custom_rgb);
    let mut painter = SoftwarePainter::new();
    let clear_rgb = match theme {
        ThemeMode::Dark => crate::theme::CLEAR_BG_DARK_RGB,
        ThemeMode::Light => crate::theme::CLEAR_BG_LIGHT_RGB,
    };

    let steps = if hover_size_mode || hover_web_search_tooltip {
        4
    } else if view == "settings-containers" || view == "settings-zones" {
        8
    } else if hover_web_search_btn || hover_unattrib_row {
        3
    } else {
        3
    };
    let target_pos_for_state = |st: &StorageUiState| -> Option<Pos2> {
        if hover_size_mode {
            st.first_row_size_rect.map(|r| r.center())
        } else if hover_web_search_mode {
            st.first_unattrib_web_search_rect.map(|r| r.center())
        } else if hover_unattrib_row {
            st.unattrib_mid_row_rect
                .map(|r| Pos2::new((r.min.x + 180.0).min(r.center().x), r.center().y))
        } else {
            None
        }
    };
    for step in 0..steps {
        let mut events = Vec::new();
        let time_s = if hover_size_mode || hover_web_search_tooltip {
            match step {
                0 => 0.0,
                1 => 0.1,
                2 => 0.9,
                _ => 1.5,
            }
        } else {
            step as f64 * 0.05
        };
        if hover_mode && step >= 1 {
            if let Some(hp) = target_pos_for_state(&ui_state) {
                events.push(egui::Event::PointerMoved(hp));
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
            if hover_mode && step + 1 == steps {
                if let Some(hp) = target_pos_for_state(&ui_state) {
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
    (
        phys_w,
        phys_h,
        painter.backbuffer_pixels().to_vec(),
        ui_state.first_drive_pod_rect,
    )
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
    report_json: Option<&PathBuf>,
    view: &str,
    theme: ThemeMode,
    preset_override: Option<ThemePresetChoice>,
    custom_rgb_override: Option<[u8; 3]>,
    dpi: u32,
    sort_mode: AppSortMode,
    sort_direction: SortDirection,
    filter_chip: AppFilterChip,
    search_query: &str,
    ledger_collapsed: Option<bool>,
    ledger_height_px: Option<f32>,
    facet_publisher: Option<&str>,
    facet_drive: Option<char>,
    facet_size: SizeBandFilter,
    open_facet: Option<&str>,
    unattrib_search: &str,
    unattrib_drive: Option<char>,
    unattrib_size: SizeBandFilter,
    unattrib_zone: UnattributedLocationZone,
    exclude_drives: &[char],
    all_drives_enabled: bool,
    sim_drives: Option<usize>,
) -> Result<(), String> {
    let drives = enumerate_fixed_drives();
    let sys_win = get_windows_directory();
    let (mut report, mut config) = if view == "group-node-fixture" {
        build_group_node_fixture_screenshot_report()
    } else if let Some(n) = sim_drives {
        build_synthetic_screenshot_report(n)
    } else {
        let cfg = AppConfig::load_or_default(&AppConfig::default_path(), &drives, &sys_win);
        let rep = if let Some(rp) = report_json {
            let txt = std::fs::read_to_string(rp)
                .map_err(|e| format!("read --report-json {}: {e}", rp.display()))?;
            if let Ok(wrapped) = serde_json::from_str::<CliScanOutput>(&txt) {
                wrapped.report
            } else {
                serde_json::from_str::<ScanReport>(&txt)
                    .map_err(|e| format!("parse --report-json {}: {e}", rp.display()))?
            }
        } else {
            load_last_scan(&default_last_scan_path())
                .unwrap_or_else(|| execute_scan_with_progress(&cfg, None, None))
        };
        (rep, cfg)
    };
    if view == "settings-zones" || view == "settings-full" {
        let _ = config
            .zones
            .add_explicit_zone(r"D:\Media", ZoneRuleKind::PersonalFiles);
        let _ = config
            .zones
            .set_pin(r"D:\Games\OrphanMod", y_core::config::PinKind::Ignore);
    }
    if all_drives_enabled {
        for t in &mut config.targets {
            t.enabled = true;
        }
    }
    for dl in exclude_drives {
        if let Some(t) = config
            .targets
            .iter_mut()
            .find(|t| t.letter.to_ascii_uppercase() == *dl)
        {
            t.enabled = false;
        }
    }
    if !report.apps.is_empty() && report.apps.iter().all(|a| a.icon_rgba.is_none()) && sim_drives.is_none() {
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
        let (dw, _dh, d_px, _) = render_screenshot_frame(
            &report,
            &config,
            &sys_win,
            "expanded-fifa23",
            ThemeMode::Dark,
            preset_override,
            custom_rgb_override,
            96,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            Some(false),
            None,
            None,
            None,
            SizeBandFilter::All,
            None,
            "",
            None,
            SizeBandFilter::All,
            UnattributedLocationZone::All,
        );
        let (lw, _lh, l_px, _) = render_screenshot_frame(
            &report,
            &config,
            &sys_win,
            "expanded-fifa23",
            ThemeMode::Light,
            preset_override,
            custom_rgb_override,
            120,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            Some(false),
            None,
            None,
            None,
            SizeBandFilter::All,
            None,
            "",
            None,
            SizeBandFilter::All,
            UnattributedLocationZone::All,
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
    let (phys_w, phys_h, fb, first_pod_rect) = render_screenshot_frame(
        &report,
        &config,
        &sys_win,
        view,
        theme,
        preset_override,
        custom_rgb_override,
        dpi,
        sort_mode,
        sort_direction,
        filter_chip,
        search_query,
        ledger_collapsed,
        ledger_height_px,
        facet_publisher,
        facet_drive,
        facet_size,
        open_facet,
        unattrib_search,
        unattrib_drive,
        unattrib_size,
        unattrib_zone,
    );

    if view == "drive-pod-crop-200" {
        if let Some(pod_r) = first_pod_rect {
            let pad = 6.0_f32;
            let cx0 = ((pod_r.min.x - pad).max(0.0) * ppp).floor() as usize;
            let cy0 = ((pod_r.min.y - pad).max(0.0) * ppp).floor() as usize;
            let cx1 = (((pod_r.max.x + pad) * ppp).ceil() as usize).min(phys_w);
            let cy1 = (((pod_r.max.y + pad) * ppp).ceil() as usize).min(phys_h);
            let cw = cx1.saturating_sub(cx0).max(1);
            let ch = cy1.saturating_sub(cy0).max(1);
            let (out_w, out_h, zoomed) = upscale_crop_nn(&fb, phys_w, cx0, cy0, cw, ch, 2);
            return save_bgra_u32_as_png(out_path, out_w, out_h, &zoomed);
        }
    }

    if view == "header-buttons-zoom" {
        let crop_x0 = (756.0 * ppp).round() as usize;
        let crop_w = phys_w.saturating_sub(crop_x0).max(1);
        let crop_h = ((52.0 * ppp).round() as usize).clamp(1, phys_h);
        let (out_w, out_h, zoomed) = upscale_crop_nn(&fb, phys_w, crop_x0, 0, crop_w, crop_h, 3);
        return save_bgra_u32_as_png(out_path, out_w, out_h, &zoomed);
    }

    if view == "corner-aa-zoom-4x" {
        let (cx0, cy0, cx1, cy1) = if let Some(pod_r) = first_pod_rect {
            let lx0 = (pod_r.max.x - 116.0).max(0.0);
            let ly0 = (pod_r.max.y - 62.0).max(0.0);
            let lx1 = (pod_r.max.x + 8.0).min(1020.0);
            let ly1 = (pod_r.max.y + 8.0).min(720.0);
            (
                (lx0 * ppp).floor() as usize,
                (ly0 * ppp).floor() as usize,
                ((lx1 * ppp).ceil() as usize).min(phys_w),
                ((ly1 * ppp).ceil() as usize).min(phys_h),
            )
        } else {
            (0, 0, (124.0 * ppp).round() as usize, (72.0 * ppp).round() as usize)
        };
        let cw = cx1.saturating_sub(cx0).max(1);
        let ch = cy1.saturating_sub(cy0).max(1);
        let (out_w, out_h, zoomed) = upscale_crop_nn(&fb, phys_w, cx0, cy0, cw, ch, 4);
        return save_bgra_u32_as_png(out_path, out_w, out_h, &zoomed);
    }

    save_bgra_u32_as_png(out_path, phys_w, phys_h, &fb)
}
