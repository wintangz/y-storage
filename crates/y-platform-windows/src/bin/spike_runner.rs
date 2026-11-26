use std::collections::HashMap;

use y_core::config::AppConfig;
use y_core::model::{AttributionBasis, format_bytes};
use y_platform_windows::paths::{enumerate_fixed_drives, get_windows_directory};
use y_platform_windows::scanner::execute_scan_with_progress;

#[allow(dead_code)]
#[derive(Clone)]
struct FolderRankEntry {
    path: String,
    bytes: u64,
    drive: char,
    attributed_to: Option<String>,
    basis: Option<AttributionBasis>,
}

fn main() {
    let drives = enumerate_fixed_drives();
    let win_dir = get_windows_directory();
    let cfg = AppConfig::for_detected_drives(&drives, &win_dir);

    let report = execute_scan_with_progress(&cfg, None, None);

    println!("=== FEASIBILITY SPIKE REPORT ===");
    println!(
        "(1) Registry enumeration: raw_display_name={}, raw_with_install_location={}, canonical_apps={}, canonical_direct_install_location={}, canonical_inferred_install_location={}",
        report.registry_stats.raw_display_name_entries,
        report.registry_stats.raw_with_install_location,
        report.registry_stats.canonical_apps,
        report.registry_stats.canonical_with_direct_install_location,
        report.registry_stats.canonical_with_inferred_install_location
    );
    println!(
        "(2) Default-scope walk duration: {:.3} s ({} ms)",
        report.duration_ms as f64 / 1000.0,
        report.duration_ms
    );

    for d in &report.windows_system {
        let apps_d = report.total_apps_bytes_on_drive(d.drive_letter);
        let yf_d = report.total_your_files_on_drive(d.drive_letter);
        let un_d = report.total_unattributed_on_drive(d.drive_letter);
        let sum_all = apps_d + yf_d + un_d + d.total_bytes + d.drive_free_bytes;
        println!(
            "Drive {}:\\ reconciliation: apps={} ({}) + your_files={} ({}) + unattributed={} ({}) + win_sys={} ({}) + free={} ({}) = {} vs total={} (diff={})",
            d.drive_letter,
            apps_d,
            format_bytes(apps_d),
            yf_d,
            format_bytes(yf_d),
            un_d,
            format_bytes(un_d),
            d.total_bytes,
            format_bytes(d.total_bytes),
            d.drive_free_bytes,
            format_bytes(d.drive_free_bytes),
            sum_all,
            d.drive_total_bytes,
            sum_all as i128 - d.drive_total_bytes as i128
        );
    }

    // Collect non-system, non-personal-shell folders across the default-scope walk.
    let mut folder_map: HashMap<String, FolderRankEntry> = HashMap::new();
    for app in &report.apps {
        for item in &app.items {
            folder_map
                .entry(item.path.clone())
                .or_insert_with(|| FolderRankEntry {
                    path: item.path.clone(),
                    bytes: item.bytes,
                    drive: item.drive_letter,
                    attributed_to: Some(app.display_name.clone()),
                    basis: item.basis,
                });
        }
    }
    for un in &report.unattributed {
        folder_map
            .entry(un.path.clone())
            .or_insert_with(|| FolderRankEntry {
                path: un.path.clone(),
                bytes: un.bytes,
                drive: un.drive_letter,
                attributed_to: None,
                basis: None,
            });
    }

    let mut all_folders: Vec<FolderRankEntry> = folder_map.into_values().collect();
    all_folders.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    let top50_all: Vec<&FolderRankEntry> = all_folders.iter().take(50).collect();
    let attr_all = top50_all
        .iter()
        .filter(|e| e.attributed_to.is_some())
        .count();
    let unattr_all: Vec<&&FolderRankEntry> = top50_all
        .iter()
        .filter(|e| e.attributed_to.is_none())
        .collect();

    println!(
        "(3a) Top 50 folders (default scope C:\\ + secondary InstallLocations): {}/{} attributed ({:.1}%), {} unattributed",
        attr_all,
        top50_all.len(),
        (attr_all as f64 / top50_all.len().max(1) as f64) * 100.0,
        unattr_all.len()
    );
    println!("Top 5 Unattributed (default scope):");
    for (i, u) in unattr_all.iter().take(5).enumerate() {
        println!(
            "  #{}: {} | {} bytes ({})",
            i + 1,
            u.path,
            u.bytes,
            format_bytes(u.bytes)
        );
    }

    let c_only: Vec<&FolderRankEntry> = all_folders
        .iter()
        .filter(|e| e.drive.eq_ignore_ascii_case(&'C'))
        .take(50)
        .collect();
    let attr_c = c_only.iter().filter(|e| e.attributed_to.is_some()).count();
    let unattr_c: Vec<&&FolderRankEntry> =
        c_only.iter().filter(|e| e.attributed_to.is_none()).collect();
    println!(
        "(3b) Top 50 folders (C:\\ walk only): {}/{} attributed ({:.1}%), {} unattributed",
        attr_c,
        c_only.len(),
        (attr_c as f64 / c_only.len().max(1) as f64) * 100.0,
        unattr_c.len()
    );
    println!("Top 5 Unattributed (C:\\ walk only):");
    for (i, u) in unattr_c.iter().take(5).enumerate() {
        println!(
            "  #{}: {} | {} bytes ({})",
            i + 1,
            u.path,
            u.bytes,
            format_bytes(u.bytes)
        );
    }

    println!("--- TOP 15 APPS FOUND ---");
    for (i, app) in report.apps.iter().take(15).enumerate() {
        println!(
            "  {:2}. {:40} | {:>10} | items={}",
            i + 1,
            app.display_name,
            format_bytes(app.total_bytes),
            app.items.len()
        );
    }
    println!("--- TOP 15 UNATTRIBUTED FOLDERS ---");
    for (i, u) in report.unattributed.iter().take(15).enumerate() {
        println!(
            "  {:2}. {:60} | {:>10}",
            i + 1,
            u.path,
            format_bytes(u.bytes)
        );
    }

    println!("--- TARGET APP BREAKDOWNS (EA / FIFA / VS / DISCORD / NVIDIA) ---");
    for app in &report.apps {
        let dn = app.display_name.to_ascii_lowercase();
        if dn.contains("ea ") || dn.contains("fifa") || dn.contains("visual studio") || dn.contains("discord") || dn.contains("nvidia graphics") {
            println!("APP: {} = {} bytes ({})", app.display_name, app.total_bytes, format_bytes(app.total_bytes));
            for it in &app.items {
                println!("    {:>10} | {:?} | {:?} | {}", format_bytes(it.bytes), it.zone, it.basis, it.path);
            }
        }
    }
}
