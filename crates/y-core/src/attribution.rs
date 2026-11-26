use std::collections::{BTreeMap, HashMap};

use crate::discovery::{
    extract_guid, is_subpath_of, normalize_app_token, normalize_path_key, path_drive_letter,
};
use crate::history::{HistoryEntryRef, compute_growth};
use crate::model::{
    AttributedApp, AttributionBasis, BreakdownItem, DriveInfo, InstalledApp, LocationQualifier,
    NoTouchCategory, RegistryDiscoveryStats, ScanReport, ScannedUnit, ScannedUnitKind,
    SystemDriveBreakdown, UnattributedItem, ZoneKind,
};
use crate::zones::classify_zone;

struct AppBucket {
    app: InstalledApp,
    bases: Vec<AttributionBasis>,
    items: Vec<BreakdownItem>,
    total_bytes: u64,
}

#[must_use]
pub fn attribute_scan(
    timestamp_unix: u64,
    timestamp_display: String,
    duration_ms: u64,
    drives: Vec<DriveInfo>,
    registry_stats: RegistryDiscoveryStats,
    installed_apps: &[InstalledApp],
    guid_to_app_name: &HashMap<String, String>,
    scanned_units: &[ScannedUnit],
    previous_history: Option<&HistoryEntryRef>,
) -> ScanReport {
    let mut buckets: Vec<AppBucket> = installed_apps
        .iter()
        .cloned()
        .map(|app| AppBucket {
            app,
            bases: Vec::new(),
            items: Vec::new(),
            total_bytes: 0,
        })
        .collect();

    let mut store_buckets: HashMap<String, usize> = HashMap::new();
    let mut shared_caches: Vec<BreakdownItem> = Vec::new();
    let mut your_files: Vec<BreakdownItem> = Vec::new();
    let mut unattributed: Vec<UnattributedItem> = Vec::new();
    let mut root_sys_by_drive: HashMap<char, Vec<BreakdownItem>> = HashMap::new();
    let mut scanned_non_system_by_drive: HashMap<char, u64> = HashMap::new();

    for unit in scanned_units {
        if unit.bytes == 0 && unit.kind != ScannedUnitKind::RootSystemFile {
            continue;
        }
        let d_letter = unit.drive_letter.to_ascii_uppercase();

        match unit.kind {
            ScannedUnitKind::RootSystemFile => {
                root_sys_by_drive
                    .entry(d_letter)
                    .or_default()
                    .push(BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: None,
                        qualifier: None,
                        basis: None,
                        no_touch: Some(NoTouchCategory::SystemRuntime),
                        drive_letter: d_letter,
                    });
                continue;
            }
            ScannedUnitKind::PersonalShell => {
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
                your_files.push(BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: None,
                    qualifier: None,
                    basis: None,
                    no_touch: None,
                    drive_letter: d_letter,
                });
                continue;
            }
            ScannedUnitKind::RegistryUserHive => {
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
                let reg_idx = ensure_synthetic_app(
                    &mut buckets,
                    &mut store_buckets,
                    "synth:ntuser-registry",
                    "Windows User Registry (NTUSER.DAT)",
                    "Microsoft Corporation",
                    Some(NoTouchCategory::Registry),
                    false,
                );
                let (zone, qual) = classify_zone(&unit.path, false);
                push_item_to_bucket(
                    &mut buckets[reg_idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: qual,
                        basis: Some(AttributionBasis::Token),
                        no_touch: Some(NoTouchCategory::Registry),
                        drive_letter: d_letter,
                    },
                    AttributionBasis::Token,
                );
                continue;
            }
            ScannedUnitKind::Regular => {
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
            }
        }

        // Regular attribution pipeline.
        let path_lower = normalize_path_key(&unit.path);
        let leaf = leaf_name(&unit.path);
        let leaf_lower = leaf.to_ascii_lowercase();

        // Rule 2(a): Known shared/system GREEN caches belong to a labelled cache group, not Unattributed and not any app.
        if is_shared_green_cache(&path_lower) {
            let qual = if path_lower.contains(":\\programdata\\") {
                Some(LocationQualifier::MachineShared)
            } else {
                None
            };
            shared_caches.push(BreakdownItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                zone: Some(ZoneKind::Green),
                qualifier: qual,
                basis: None,
                no_touch: None,
                drive_letter: d_letter,
            });
            continue;
        }

        // Exclude drive root loose metadata like $Recycle.Bin, Config.Msi, System Volume Information, .GamingRoot from force-matching.
        if is_unattributable_os_noise(&path_lower, &leaf_lower) {
            let (zone, _) = classify_zone(&unit.path, false);
            unattributed.push(UnattributedItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                drive_letter: d_letter,
                zone_hint: Some(zone),
            });
            continue;
        }

        // 1. InstallLocation match (prefer longest / most specific matching install location).
        if let Some((idx, is_install_loc)) = find_by_install_location(&buckets, &path_lower) {
            let (zone, qual) = classify_zone(&unit.path, is_install_loc || unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::InstallLocation),
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::InstallLocation,
            );
            continue;
        }

        // 2. ExactName match.
        if let Some(idx) = find_by_exact_name(&buckets, &leaf) {
            let (zone, qual) = classify_zone(&unit.path, unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::ExactName),
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::ExactName,
            );
            continue;
        }

        // 3. Token match (Package Cache GUID lookup, curated rules, versioned publisher products, and <X>Data rule).
        if let Some((idx, is_install_override)) = find_by_token(
            &mut buckets,
            &mut store_buckets,
            guid_to_app_name,
            &path_lower,
            &leaf,
            &leaf_lower,
        ) {
            let (zone, qual) =
                classify_zone(&unit.path, is_install_override || unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::Token),
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::Token,
            );
            continue;
        }

        // 4. Publisher match (specific non-generic publishers only, never "Microsoft Corporation").
        if let Some(idx) = find_by_publisher(&buckets, &leaf) {
            let (zone, qual) = classify_zone(&unit.path, unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::Publisher),
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::Publisher,
            );
            continue;
        }

        // 5. Store Packages fallback (`AppData\Local\Packages\<Pkg>` or `Program Files\WindowsApps\<Pkg>`).
        if (path_lower.contains("\\appdata\\local\\packages\\")
            && !path_lower.ends_with("\\appdata\\local\\packages"))
            || (path_lower.contains("\\program files\\windowsapps\\")
                && !path_lower.ends_with("\\program files\\windowsapps"))
        {
            let is_winapps = path_lower.contains("\\program files\\windowsapps\\");
            if let Some(idx) = attribute_store_package(&mut buckets, &mut store_buckets, &leaf) {
                let (zone, qual) = classify_zone(&unit.path, is_winapps);
                let nt = buckets[idx].app.no_touch;
                push_item_to_bucket(
                    &mut buckets[idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: qual,
                        basis: Some(AttributionBasis::PackageFallback),
                        no_touch: nt,
                        drive_letter: d_letter,
                    },
                    AttributionBasis::PackageFallback,
                );
                continue;
            }
        }

        // 5b. Unmatched ProgramData\Package Cache items belong to Shared & system caches (GREEN).
        if path_lower.contains("\\programdata\\package cache\\") {
            shared_caches.push(BreakdownItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                zone: Some(ZoneKind::Green),
                qualifier: Some(LocationQualifier::MachineShared),
                basis: None,
                no_touch: None,
                drive_letter: d_letter,
            });
            continue;
        }

        // 5c. Rule 2(b) & 2(c): Top-level user-profile dot-folders and standalone Roaming\flutter stand alone under their folder name.
        if is_user_profile_dot_folder(&path_lower, &leaf_lower) {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                &format!("dot:{leaf_lower}"),
                &leaf,
                "User profile dot-folder",
                None,
                false,
            );
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Red),
                    qualifier: None,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        if path_lower.ends_with("\\appdata\\roaming\\flutter")
            || path_lower.ends_with("\\appdata\\local\\flutter")
        {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                "standalone:flutter",
                "flutter",
                "Standalone SDK / toolchain",
                None,
                false,
            );
            let qual = if path_lower.contains("\\appdata\\roaming\\") {
                Some(LocationQualifier::RoamingConfig)
            } else {
                None
            };
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Body),
                    qualifier: qual,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        if path_lower.ends_with("\\appdata\\local\\ms-playwright") {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                "standalone:playwright",
                "Playwright",
                "Standalone toolchain · regenerable dev-tool cache (downloaded browser binaries)",
                None,
                false,
            );
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Green),
                    qualifier: None,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        if path_lower.ends_with("\\appdata\\local\\gitkrakencli") {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                "standalone:gitkraken-cli",
                "GitKraken CLI",
                "Standalone CLI toolchain (gk.exe)",
                None,
                false,
            );
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        // 6. Openly shown Unattributed.
        let (zone, _) = classify_zone(&unit.path, false);
        unattributed.push(UnattributedItem {
            path: unit.path.clone(),
            bytes: unit.bytes,
            drive_letter: d_letter,
            zone_hint: Some(zone),
        });
    }

    // Build AttributedApps (keep all apps with > 0 bytes, sorted desc by total_bytes).
    let mut apps: Vec<AttributedApp> = buckets
        .into_iter()
        .filter(|b| b.total_bytes > 0)
        .map(|mut b| {
            b.items.sort_by(|a, c| c.bytes.cmp(&a.bytes));
            b.bases.sort();
            b.bases.dedup();
            let prev = previous_history.and_then(|h| h.get(&b.app.display_name).copied());
            let growth = compute_growth(previous_history.is_some(), prev, b.total_bytes);
            let install_location = b.app.primary_install_location().map(|s| s.to_string());
            AttributedApp {
                id: b.app.id,
                display_name: b.app.display_name,
                publishers: b.app.publishers,
                install_location,
                total_bytes: b.total_bytes,
                previous_bytes: prev,
                growth,
                no_touch: b.app.no_touch,
                bases: b.bases,
                items: b.items,
                dependencies: Vec::new(),
            }
        })
        .collect();

    apps.sort_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    shared_caches.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let shared_caches_total_bytes = shared_caches.iter().map(|i| i.bytes).sum();

    your_files.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let your_files_total_bytes = your_files.iter().map(|i| i.bytes).sum();

    unattributed.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let unattributed_total_bytes = unattributed.iter().map(|i| i.bytes).sum();

    // Compute per-drive Windows & system remainder and reconciliation totals.
    let mut windows_system = Vec::new();
    let mut windows_system_total_bytes = 0u64;
    for drv in &drives {
        let d = drv.letter.to_ascii_uppercase();
        let mut root_files = root_sys_by_drive.remove(&d).unwrap_or_default();
        root_files.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        let root_sum: u64 = root_files.iter().map(|i| i.bytes).sum();
        let scanned_non_sys = *scanned_non_system_by_drive.get(&d).unwrap_or(&0);
        let remainder_bytes = if drv.full_walk_enabled {
            drv.used_bytes
                .saturating_sub(scanned_non_sys.saturating_add(root_sum))
        } else {
            0
        };
        let sys_total = root_sum.saturating_add(remainder_bytes);
        windows_system_total_bytes = windows_system_total_bytes.saturating_add(sys_total);

        if drv.full_walk_enabled || sys_total > 0 {
            windows_system.push(SystemDriveBreakdown {
                drive_letter: d,
                root_files,
                remainder_bytes,
                total_bytes: sys_total,
                scanned_non_system_bytes: scanned_non_sys,
                drive_used_bytes: drv.used_bytes,
                drive_free_bytes: drv.free_bytes,
                drive_total_bytes: drv.total_bytes,
            });
        }
    }

    let _ = path_drive_letter; // referenced via model helpers

    ScanReport {
        scan_timestamp_unix: timestamp_unix,
        scan_timestamp_display: timestamp_display,
        duration_ms,
        drives,
        registry_stats,
        apps,
        shared_caches,
        shared_caches_total_bytes,
        your_files,
        your_files_total_bytes,
        windows_system,
        windows_system_total_bytes,
        unattributed,
        unattributed_total_bytes,
        reverse_dependencies: Vec::new(),
        catalog_source: String::new(),
        warnings: Vec::new(),
    }
}

fn push_item_to_bucket(bucket: &mut AppBucket, item: BreakdownItem, basis: AttributionBasis) {
    bucket.total_bytes = bucket.total_bytes.saturating_add(item.bytes);
    if !bucket.bases.contains(&basis) {
        bucket.bases.push(basis);
    }
    bucket.items.push(item);
}

fn leaf_name(path: &str) -> String {
    let trimmed = path.trim_end_matches('\\');
    trimmed
        .rsplit('\\')
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

fn is_shared_green_cache(path_lower: &str) -> bool {
    let basic = path_lower.ends_with("\\appdata\\local\\temp")
        || path_lower.contains("\\appdata\\local\\temp\\")
        || path_lower.ends_with("\\appdata\\locallow\\temp")
        || path_lower.contains("\\appdata\\locallow\\temp\\")
        || path_lower.ends_with("\\appdata\\local\\d3dscache")
        || path_lower.ends_with("\\appdata\\local\\nvidia\\dxcache")
        || path_lower.ends_with("\\appdata\\local\\nvidia\\glcache")
        || path_lower.ends_with("\\appdata\\local\\amd\\dxcache")
        || path_lower.ends_with("\\appdata\\local\\amd\\dxccache")
        || path_lower.ends_with("\\appdata\\local\\amd\\vkcache")
        || path_lower.ends_with("\\appdata\\local\\crashdumps")
        || path_lower.ends_with("\\appdata\\local\\squirreltemp")
        || path_lower.ends_with("\\appdata\\local\\cache")
        || path_lower.ends_with("\\programdata\\epicinstallertemp");
    if basic {
        return true;
    }

    let win_caches = [
        "\\appdata\\local\\microsoft\\windows\\webcache",
        "\\appdata\\local\\microsoft\\windows\\explorer",
        "\\appdata\\local\\microsoft\\windows\\inetcache",
        "\\appdata\\local\\microsoft\\windows\\caches",
        "\\appdata\\local\\microsoft\\windows\\actioncentercache",
        "\\appdata\\local\\microsoft\\windows\\pricache",
        "\\appdata\\local\\microsoft\\windows\\appcache",
        "\\appdata\\local\\microsoft\\windows\\iecompatcache",
        "\\appdata\\local\\microsoft\\windows\\iecompatuacache",
        "\\appdata\\local\\microsoft\\windows\\ppbcompatcache",
        "\\appdata\\local\\microsoft\\windows\\ppbcompatuacache",
        "\\appdata\\local\\microsoft\\windows\\temporary internet files",
    ];
    win_caches
        .iter()
        .any(|sfx| path_lower.ends_with(sfx) || path_lower.contains(&format!("{sfx}\\")))
}

fn is_user_profile_dot_folder(path_lower: &str, leaf_lower: &str) -> bool {
    if !leaf_lower.starts_with('.') || leaf_lower == ".[files]" || leaf_lower == "." || leaf_lower == ".." {
        return false;
    }
    let parts: Vec<&str> = path_lower.trim_end_matches('\\').split('\\').collect();
    parts.len() == 4 && parts[1] == "users"
}

fn is_unattributable_os_noise(path_lower: &str, leaf_lower: &str) -> bool {
    if leaf_lower.starts_with("$recycle.bin")
        || leaf_lower == "config.msi"
        || leaf_lower == "system volume information"
        || leaf_lower == "recovery"
        || leaf_lower == ".gamingroot"
        || leaf_lower == "usoshared"
        || leaf_lower == "usoprivate"
        || leaf_lower == "softwaredistribution"
        || leaf_lower == "whesvc"
        || leaf_lower == "windowsholographicdevices"
        || leaf_lower == "elevateddiagnostics"
        || leaf_lower == "connecteddevicesplatform"
    {
        return true;
    }
    // Generic shared Temp / CrashDumps / Package Cache root files without app owner stay unattributed.
    if path_lower.ends_with("\\appdata\\local\\temp")
        || path_lower.ends_with("\\appdata\\locallow\\temp")
        || path_lower.ends_with("\\appdata\\local\\d3dscache")
        || path_lower.ends_with("\\appdata\\local\\crashdumps")
        || path_lower.ends_with("\\appdata\\roaming\\microsoft\\windows")
        || path_lower.ends_with("\\appdata\\roaming\\microsoft\\installer")
        || path_lower.ends_with("\\appdata\\roaming\\microsoft\\crypto")
        || path_lower.ends_with("\\appdata\\local\\packages\\[files]")
        || path_lower.ends_with("\\program files\\windowsapps\\[files]")
        || path_lower.ends_with("\\[root files]")
        || (path_lower.ends_with("\\[files]")
            && !path_lower.contains("\\programdata\\nvidia\\")
            && !path_lower.contains("\\programdata\\nvidia corporation\\"))
    {
        return true;
    }
    false
}

fn find_by_install_location(buckets: &[AppBucket], path_lower: &str) -> Option<(usize, bool)> {
    let mut best: Option<(usize, usize, bool)> = None;
    for (i, b) in buckets.iter().enumerate() {
        for loc in &b.app.all_install_locations {
            let lk = normalize_path_key(loc);
            if lk.len() < 4 {
                continue;
            }
            if is_subpath_of(path_lower, &lk) {
                let is_exact = path_lower == lk;
                if best.is_none_or(|(_, best_len, _)| lk.len() > best_len) {
                    best = Some((i, lk.len(), is_exact || is_subpath_of(path_lower, &lk)));
                }
            }
        }
    }
    best.map(|(idx, _, is_inst)| (idx, is_inst))
}

fn find_by_exact_name(buckets: &[AppBucket], leaf: &str) -> Option<usize> {
    let leaf_tok = normalize_app_token(leaf);
    if leaf_tok.len() < 3 {
        return None;
    }
    for (i, b) in buckets.iter().enumerate() {
        let name_tok = normalize_app_token(&b.app.display_name);
        if name_tok == leaf_tok {
            return Some(i);
        }
        for alias in &b.app.aliases {
            if normalize_app_token(alias) == leaf_tok {
                return Some(i);
            }
        }
    }
    None
}

fn find_app_matching(buckets: &[AppBucket], predicates: &[&str]) -> Option<usize> {
    for pred in predicates {
        for (i, b) in buckets.iter().enumerate() {
            let dn = b.app.display_name.to_ascii_lowercase();
            if dn.contains(pred) {
                return Some(i);
            }
        }
    }
    None
}

fn ensure_synthetic_app(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    key: &str,
    display_name: &str,
    publisher: &str,
    no_touch: Option<NoTouchCategory>,
    is_store_fallback: bool,
) -> usize {
    if let Some(&idx) = store_buckets.get(key) {
        return idx;
    }
    let idx = buckets.len();
    buckets.push(AppBucket {
        app: InstalledApp {
            id: key.to_string(),
            display_name: display_name.to_string(),
            aliases: Vec::new(),
            publishers: if publisher.is_empty() {
                Vec::new()
            } else {
                vec![publisher.to_string()]
            },
            direct_install_location: None,
            inferred_install_location: None,
            all_install_locations: Vec::new(),
            no_touch,
            is_store_fallback,
        },
        bases: Vec::new(),
        items: Vec::new(),
        total_bytes: 0,
    });
    store_buckets.insert(key.to_string(), idx);
    idx
}

fn find_by_token(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    guid_to_app_name: &HashMap<String, String>,
    path_lower: &str,
    leaf: &str,
    leaf_lower: &str,
) -> Option<(usize, bool)> {
    // 1. Package Cache {GUID} or known VS/SDK/EA/VC++ cache patterns.
    if path_lower.contains("\\programdata\\package cache\\") {
        if let Some(g) = extract_guid(leaf_lower) {
            if let Some(app_name) = guid_to_app_name.get(&g) {
                let an_lower = app_name.to_ascii_lowercase();
                if an_lower.contains("ea app") {
                    if let Some(idx) = find_app_matching(buckets, &["ea app"]) {
                        return Some((idx, false));
                    }
                }
                if an_lower.contains("visual studio") {
                    if let Some(idx) = find_app_matching(
                        buckets,
                        &["visual studio build tools", "visual studio community", "visual studio"],
                    ) {
                        return Some((idx, false));
                    }
                }
                if an_lower.contains("windows software development kit")
                    || an_lower.contains("windows sdk")
                    || an_lower.contains("windows app certification")
                    || an_lower.contains("universal crt")
                    || an_lower.contains("windows team extension")
                {
                    if let Some(idx) = find_app_matching(buckets, &["windows software development kit"]) {
                        return Some((idx, false));
                    }
                }
                if an_lower.contains("visual c++") {
                    if let Some(idx) = find_app_matching(buckets, &["microsoft visual c++"]) {
                        return Some((idx, false));
                    }
                }
                if an_lower.contains(".net") {
                    if let Some(idx) = find_app_matching(buckets, &["microsoft .net sdk", "microsoft .net"]) {
                        return Some((idx, false));
                    }
                }
                // Direct match by token against canonical apps.
                let tok = normalize_app_token(app_name);
                if let Some(idx) = buckets.iter().position(|b| normalize_app_token(&b.app.display_name) == tok) {
                    return Some((idx, false));
                }
            }
        }
        // Version-suffixed Windows 10/11 SDK Package Cache entries (`{...}v10.1.26100.9457`).
        if leaf_lower.contains("v10.1.26100.") {
            if let Some(idx) = find_app_matching(buckets, &["windows software development kit"]) {
                return Some((idx, false));
            }
        }
        // VS 2022 Package Cache bundles (`...v17.*` or `Microsoft.VisualStudio.*`).
        if leaf_lower.contains("v17.") || leaf_lower.contains("visualstudio") {
            if let Some(idx) = find_app_matching(
                buckets,
                &["visual studio build tools", "visual studio community", "visual studio"],
            ) {
                return Some((idx, false));
            }
        }
    }

    // 2. Curated deterministic token rules -> installed apps.
    let rules: &[(&[&str], &[&str], bool)] = &[
        // Rust toolchain
        (&[".cargo", ".rustup"], &["rustup"], false),
        // VS Code
        (&[".vscode", ".vscode-shared", "vscode-sqltools"], &["visual studio code"], false),
        (&["\\appdata\\roaming\\code"], &["visual studio code"], false),
        // Visual Studio ecosystem
        (
            &[
                "\\microsoft visual studio\\shared",
                "\\microsoft\\visualstudio",
                "\\appdata\\roaming\\visual studio setup",
                "\\appdata\\local\\microsoft\\vsapplicationinsights",
                "\\appdata\\local\\microsoft\\vscommon",
                "\\appdata\\local\\microsoft\\vstelemetry",
                "\\programdata\\microsoft devdiv",
                "\\programdata\\microsoft\\visualstudio",
                "\\program files (x86)\\microsoft visual studio",
            ],
            &["visual studio build tools", "visual studio community", "microsoft visual studio"],
            false,
        ),
        // Windows SDK / Kits
        (
            &[
                "\\program files (x86)\\windows kits",
                "\\program files (x86)\\microsoft sdks",
                "\\appdata\\local\\microsoft sdks",
                "\\programdata\\windows app certification kit",
            ],
            &["windows software development kit", "windows sdk"],
            true,
        ),
        // .NET SDK & NuGet
        (
            &[
                "\\.dotnet",
                "\\program files\\dotnet",
                "\\program files (x86)\\microsoft.net",
                "\\appdata\\local\\nuget",
                "\\appdata\\roaming\\nuget",
                "\\appdata\\local\\asp.net",
                "\\programdata\\microsoft\\netframework",
            ],
            &["microsoft .net sdk", "microsoft .net runtime", "microsoft windows desktop runtime"],
            false,
        ),
        (&["\\programdata\\asus"], &["asus framework service", "armoury crate"], false),
        (&["\\program files\\ea", "\\program files\\electronic arts", "\\program files\\common files\\eainstaller", "\\programdata\\ea desktop", "\\programdata\\electronic arts", "\\programdata\\origin", "\\programdata\\eaanticheat", "\\appdata\\local\\eadesktop", "\\appdata\\local\\ealaunchhelper", "\\appdata\\local\\electronic arts", "\\appdata\\local\\origin", "\\appdata\\roaming\\ea"], &["ea app", "ea sports", "fifa 23"], false),
        (&["\\programdata\\frostbite"], &["ea sports", "fifa 23", "ea app"], false),
        // Discord
        (
            &[
                "\\appdata\\local\\discord",
                "\\appdata\\roaming\\discord",
                "\\common files\\discord",
            ],
            &["discord"],
            false,
        ),
        // Steam
        (&["\\common files\\steam", "\\appdata\\local\\steam"], &["steam"], false),
        // Epic Games
        (
            &[
                "\\program files (x86)\\epic games",
                "\\programdata\\epic",
                "\\appdata\\local\\epicgameslauncher",
                "\\appdata\\local\\epic games",
            ],
            &["epic games launcher"],
            false,
        ),
        // Google Chrome
        (
            &["\\appdata\\local\\google\\chrome", "\\program files\\google\\chrome", "\\program files (x86)\\google\\chrome"],
            &["google chrome"],
            false,
        ),
        // Microsoft Edge & WebView2
        (
            &[
                "\\appdata\\local\\microsoft\\edgewebview",
                "\\program files (x86)\\microsoft\\edgewebview",
            ],
            &["microsoft edge webview2"],
            false,
        ),
        (
            &[
                "\\appdata\\local\\microsoft\\edge",
                "\\program files (x86)\\microsoft\\edge",
                "\\program files (x86)\\microsoft\\edgecore",
                "\\program files (x86)\\microsoft\\edgeupdate",
                "\\programdata\\microsoft\\edgeupdate",
            ],
            &["microsoft edge"],
            false,
        ),
        // NVIDIA
        (
            &["\\program files\\nvidia corporation\\installer2\\display.physx"],
            &["nvidia physx"],
            true,
        ),
        (
            &["\\appdata\\local\\nvidia corporation"],
            &["nvidia app", "nvidia graphics driver"],
            false,
        ),
        (
            &[
                "\\program files\\nvidia corporation\\nv",
                "\\program files\\nvidia corporation\\display",
                "\\appdata\\roaming\\nvidia",
                "\\programdata\\nvidia corporation",
                "\\programdata\\nvidia",
            ],
            &["nvidia graphics driver", "nvidia app"],
            false,
        ),
        // AMD
        (
            &[
                "c:\\amd",
                "\\appdata\\local\\amd",
                "\\appdata\\local\\amd_common",
                "\\appdata\\local\\amdsoftwareinstaller",
                "\\appdata\\roaming\\amd",
                "\\programdata\\amd",
                "\\program files\\amd",
            ],
            &["amd software", "amd chipset"],
            false,
        ),
        // Ollama
        (&["\\.ollama", "\\appdata\\local\\ollama", "\\appdata\\roaming\\ollama", "\\appdata\\roaming\\ollama app.exe"], &["ollama"], false),
        // Unity
        (
            &[
                "\\.unity",
                "\\unity user templates",
                "\\appdata\\local\\unityhub",
                "\\appdata\\local\\unityhub-updater",
                "\\appdata\\roaming\\unityhub",
            ],
            &["unity hub", "unity"],
            false,
        ),
        (&["\\appdata\\local\\unity", "\\appdata\\roaming\\unity", "\\appdata\\locallow\\unity", "\\programdata\\unity"], &["unity"], false),
        // Office
        (
            &[
                "\\programdata\\microsoft\\clicktorun",
                "\\programdata\\microsoft\\office",
                "\\appdata\\local\\microsoft\\office",
                "\\appdata\\roaming\\microsoft\\office",
            ],
            &["microsoft office"],
            false,
        ),
        // Logitech G Hub
        (&["\\program files\\lghub", "\\program files\\logi", "\\programdata\\lghub", "\\appdata\\local\\lghub", "\\appdata\\roaming\\lghub", "\\appdata\\roaming\\g hub"], &["logitech g hub"], false),
        // Copilot CLI
        (&["\\.copilot", "\\appdata\\local\\githubcopilotcli", "\\appdata\\local\\copilot"], &["github copilot cli"], false),
        // HoYoPlay / Genshin
        (&["\\appdata\\local\\hoyoverse", "\\appdata\\locallow\\mihoyo", "\\appdata\\roaming\\cognosphere"], &["hoyoplay", "genshin impact"], false),
        // Rockstar
        (&["\\program files\\rockstar games", "\\program files (x86)\\rockstar games", "\\programdata\\rockstar games", "\\appdata\\local\\rockstar games"], &["rockstar games launcher"], false),
        // Minecraft / Xbox
        (&["c:\\xboxgames\\minecraft for windows", "c:\\xboxgames\\minecraft launcher", "c:\\xboxgames\\gamesave", "\\appdata\\roaming\\.minecraft", "\\appdata\\roaming\\minecraft bedrock", "\\appdata\\local\\minecraftinstaller"], &["xbox service"], true),
        // Wacom
        (&["\\appdata\\roaming\\wtablet", "\\appdata\\roaming\\wpersistent"], &["wacom tablet"], false),
        // WinRAR, IDM, TreeSize, Tailscale, TeamViewer, RustDesk, Spotify, Inkscape, Tiled, Node, Python
        (&["\\appdata\\roaming\\winrar"], &["winrar"], false),
        (&["\\programdata\\idm", "\\appdata\\roaming\\idm", "\\program files (x86)\\internet download manager"], &["internet download manager"], false),
        (&["\\appdata\\roaming\\jam software", "\\program files\\jam software"], &["treesize"], false),
        (&["\\programdata\\tailscale", "\\appdata\\local\\tailscale"], &["tailscale"], false),
        (&["\\appdata\\local\\teamviewer", "\\appdata\\roaming\\teamviewer"], &["teamviewer"], false),
        (&["\\appdata\\local\\rustdesk", "\\appdata\\roaming\\rustdesk"], &["rustdesk"], false),
        (&["\\appdata\\local\\spotify", "\\appdata\\roaming\\spotify"], &["spotify"], false),
        (&["\\appdata\\roaming\\inkscape"], &["inkscape"], false),
        (&["\\appdata\\local\\tiled", "\\appdata\\roaming\\tiled"], &["tiled"], false),
        (&["\\appdata\\roaming\\npm", "\\appdata\\local\\pnpm"], &["node.js"], false),
        (&["\\appdata\\local\\pip", "\\appdata\\local\\uv", "\\appdata\\roaming\\uv"], &["python"], false),
        (&["\\appdata\\local\\easyanticheat", "\\appdata\\roaming\\easyanticheat", "\\program files (x86)\\easyanticheat"], &["easyanticheat"], false),
        (&["\\appdata\\local\\battleye", "\\common files\\battleye"], &["battleye"], false),
        (&["\\appdata\\local\\artcraft"], &["artcraft"], false),
        (&["\\appdata\\roaming\\filmcraft"], &["filmcraft"], false),
        (&["\\appdata\\roaming\\photocraft"], &["photocraft"], false),
        (&["\\appdata\\local\\mister download manager", "\\appdata\\roaming\\misterdownloadmanager"], &["mister download manager"], false),
        (&["\\appdata\\local\\theisle"], &["the isle"], false),
        (
            &[
                "\\appdata\\roaming\\apple computer",
                "\\appdata\\local\\apple computer",
                "\\programdata\\apple computer",
                "\\programdata\\apple",
            ],
            &["appleinc.itunes", "itunes", "appledevices"],
            false,
        ),
    ];

    for (path_needles, app_needles, is_body) in rules {
        let matched_path = path_needles.iter().any(|needle| {
            path_matches_token(path_lower, leaf_lower, needle)
        });
        if matched_path {
            if let Some(idx) = find_app_matching(buckets, app_needles) {
                return Some((idx, *is_body));
            }
        }
    }

    // General rule: versioned per-product folder under a publisher folder (e.g. Local|Roaming\Google\AndroidStudio2024.2).
    if let Some((idx, is_body)) =
        match_versioned_publisher_product(buckets, store_buckets, path_lower, leaf)
    {
        return Some((idx, is_body));
    }

    // General rule: AppData\Roaming|Local\<X>Data attributes to app X when X is installed or a curated known app.
    if let Some((idx, is_body)) =
        match_xdata_folder(buckets, store_buckets, path_lower, leaf, leaf_lower)
    {
        return Some((idx, is_body));
    }

    // Verified known-app fallbacks (data folders of standalone or un-registered apps resolved cleanly).
    let known_app_fallbacks: &[(&[&str], &[&str], &str, &str, &str, bool)] = &[
        (
            &["\\appdata\\roaming\\tabnine", "\\appdata\\local\\tabnine"],
            &["tabnine"],
            "app:tabnine",
            "TabNine",
            "Tabnine",
            false,
        ),
        (
            &[
                "\\appdata\\local\\zalo",
                "\\appdata\\local\\zalopc",
                "\\appdata\\roaming\\zalopc",
            ],
            &["zalo"],
            "app:zalo",
            "Zalo",
            "VNG Corp.",
            false,
        ),
        (
            &[
                "\\appdata\\local\\jellyfin",
                "\\programdata\\jellyfin",
                "\\appdata\\local\\jellyfinmediaplayer",
            ],
            &["jellyfin"],
            "app:jellyfin",
            "Jellyfin",
            "Jellyfin",
            false,
        ),
        (
            &[
                "\\appdata\\roaming\\postman",
                "\\appdata\\local\\postman",
                "\\appdata\\roaming\\postmanagent",
            ],
            &["postman"],
            "app:postman",
            "Postman",
            "Postman, Inc.",
            false,
        ),
        (
            &["\\microsoft\\power bi desktop store app"],
            &["power bi desktop"],
            "app:power-bi-desktop",
            "Microsoft Power BI Desktop",
            "Microsoft Corporation",
            false,
        ),
        (
            &[
                "\\common files\\adobe",
                "\\appdata\\roaming\\adobe",
                "\\appdata\\local\\adobe",
                "\\appdata\\locallow\\adobe",
                "\\appdata\\roaming\\com.adobe.dunamis",
            ],
            &["adobe creative cloud", "adobe acrobat"],
            "app:adobe-creative-cloud",
            "Adobe Creative Cloud",
            "Adobe Inc.",
            false,
        ),
        (
            &["\\colossal order\\cities skylines ii"],
            &["cities: skylines ii", "cities skylines ii"],
            "app:cities-skylines-ii",
            "Cities: Skylines II",
            "Colossal Order",
            false,
        ),
        (
            &["\\appdata\\roaming\\riot-client-ux", "\\appdata\\local\\riot games"],
            &["riot client"],
            "app:riot-client",
            "Riot Client",
            "Riot Games, Inc.",
            false,
        ),
    ];

    for (path_needles, app_needles, synth_key, display_name, publisher, is_body) in
        known_app_fallbacks
    {
        if path_needles
            .iter()
            .any(|needle| path_matches_token(path_lower, leaf_lower, needle))
        {
            if let Some(idx) = find_app_matching(buckets, app_needles) {
                return Some((idx, *is_body));
            }
            let idx = ensure_synthetic_app(
                buckets,
                store_buckets,
                synth_key,
                display_name,
                publisher,
                None,
                false,
            );
            return Some((idx, *is_body));
        }
    }

    // EdgeWebView2 Runtime when SystemComponent=1 filtered it out of canonical registry apps.
    if path_matches_token(path_lower, leaf_lower, "\\program files (x86)\\microsoft\\edgewebview")
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\local\\microsoft\\edgewebview")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:edge-webview2",
            "Microsoft Edge WebView2 Runtime",
            "Microsoft Corporation",
            Some(NoTouchCategory::SystemRuntime),
            false,
        );
        return Some((idx, path_lower.contains("\\program files")));
    }

    // Standalone EasyAntiCheat data/install when no separate Uninstall entry exists.
    if path_matches_token(path_lower, leaf_lower, "\\appdata\\roaming\\easyanticheat")
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\local\\easyanticheat")
        || path_matches_token(path_lower, leaf_lower, "\\program files (x86)\\easyanticheat_eos")
        || path_matches_token(path_lower, leaf_lower, "\\program files (x86)\\easyanticheat")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:easyanticheat",
            "EasyAntiCheat",
            "Epic Games, Inc.",
            Some(NoTouchCategory::AntiCheat),
            false,
        );
        return Some((idx, path_lower.contains("\\program files")));
    }

    // Special handling for `C:\XboxGames\Minecraft for Windows` -> Store package fallback or dedicated Minecraft entry if Xbox Service isn't desired.
    if path_lower.starts_with("c:\\xboxgames\\") {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            &format!("xbox:{leaf_lower}"),
            &format!("Xbox Game — {leaf_lower}"),
            "Microsoft Xbox",
            None,
            true,
        );
        return Some((idx, true));
    }

    // Also attribute `Program Files (x86)\Reference Assemblies` / `MSBuild` to Visual Studio if installed.
    if path_lower.ends_with("\\program files (x86)\\msbuild")
        || path_lower.ends_with("\\program files\\msbuild")
        || path_lower.ends_with("\\reference assemblies")
    {
        if let Some(idx) = find_app_matching(
            buckets,
            &["visual studio build tools", "visual studio community", "visual studio"],
        ) {
            return Some((idx, true));
        }
    }

    // Office Click-to-Run / Office Add-in Local\Packages (`oice_16_*`) & Microsoft Shared
    if path_lower.contains("\\program files\\microsoft office")
        || path_lower.contains("\\program files (x86)\\microsoft office")
        || path_lower.contains("\\common files\\microsoft shared")
        || path_lower.contains("\\appdata\\local\\packages\\oice_16_")
    {
        if let Some(idx) = find_app_matching(buckets, &["microsoft office"]) {
            return Some((idx, !path_lower.contains("\\appdata\\local\\packages\\")));
        }
    }

    // Standalone BattlEye anti-cheat files when no separate Uninstall entry exists.
    if path_matches_token(path_lower, leaf_lower, "\\common files\\battleye")
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\local\\battleye")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:battleye",
            "BattlEye",
            "BattlEye Innovations",
            Some(NoTouchCategory::AntiCheat),
            false,
        );
        return Some((idx, path_lower.contains("\\program files")));
    }

    // NVIDIA PhysX installer folder fallback (`Display.PhysX.{...}`)
    if path_lower.contains("\\nvidia corporation\\installer2\\display.physx") {
        if let Some(idx) = find_app_matching(buckets, &["nvidia physx"]) {
            return Some((idx, true));
        }
    }

    // Windows Defender & System services in ProgramData / Program Files
    if path_lower.contains("\\program files\\windows defender")
        || path_lower.contains("\\programdata\\microsoft\\windows defender")
        || path_lower.contains("\\programdata\\microsoft\\windows security health")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:windows-defender",
            "Windows Defender",
            "Microsoft Corporation",
            Some(NoTouchCategory::Service),
            false,
        );
        return Some((idx, true));
    }

    None
}

fn publisher_display_for_folder(pub_lower: &str) -> Option<&'static str> {
    match pub_lower {
        "google" => Some("Google LLC"),
        "jetbrains" => Some("JetBrains s.r.o."),
        "autodesk" => Some("Autodesk"),
        "adobe" => Some("Adobe Inc."),
        _ => None,
    }
}

fn split_camel_case(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len() + 4);
    let mut prev: Option<char> = None;
    for ch in stem.chars() {
        if let Some(p) = prev {
            if p.is_ascii_lowercase() && ch.is_ascii_uppercase() {
                out.push(' ');
            }
        }
        out.push(ch);
        prev = Some(ch);
    }
    out
}

fn canonicalize_product_stem(stem: &str) -> String {
    let compact = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match compact.as_str() {
        "androidstudio" => "Android Studio".to_string(),
        "intellijidea" | "ideaic" | "ideau" => "IntelliJ IDEA".to_string(),
        "pycharm" | "pycharmce" => "PyCharm".to_string(),
        "webstorm" => "WebStorm".to_string(),
        "goland" => "GoLand".to_string(),
        "rustrover" => "RustRover".to_string(),
        "clion" => "CLion".to_string(),
        "phpstorm" => "PhpStorm".to_string(),
        "datagrip" => "DataGrip".to_string(),
        "rider" => "Rider".to_string(),
        _ => split_camel_case(stem),
    }
}

fn parse_versioned_product_name(leaf: &str) -> Option<String> {
    let bytes = leaf.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii_digit() {
            continue;
        }
        let suffix = &leaf[i..];
        let valid_ver_chars = suffix
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
        let has_version_shape = suffix.len() >= 2 && (suffix.contains('.') || suffix.chars().filter(|c| c.is_ascii_digit()).count() >= 2);
        if valid_ver_chars && has_version_shape {
            let mut stem_end = i;
            let trimmed_sep = leaf[..stem_end].trim_end_matches([' ', '-', '_']);
            stem_end = trimmed_sep.len();
            if stem_end >= 2 {
                let last = leaf.as_bytes()[stem_end - 1];
                let prev = leaf.as_bytes()[stem_end - 2];
                if matches!(last, b'v' | b'V') && !prev.is_ascii_alphabetic() {
                    stem_end -= 1;
                }
            }
            let stem = leaf[..stem_end].trim_end_matches([' ', '-', '_']);
            let alpha_count = stem.chars().filter(|c| c.is_ascii_alphabetic()).count();
            if alpha_count >= 4 {
                return Some(canonicalize_product_stem(stem));
            }
            return None;
        }
    }
    None
}

fn match_versioned_publisher_product(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    path_lower: &str,
    leaf: &str,
) -> Option<(usize, bool)> {
    let parts: Vec<&str> = path_lower.trim_end_matches('\\').split('\\').collect();
    if parts.len() < 4 {
        return None;
    }
    let tier = parts[parts.len() - 3];
    let pub_dir = parts[parts.len() - 2];
    if !matches!(tier, "local" | "roaming" | "locallow") {
        return None;
    }
    if parts.len() < 5 || parts[parts.len() - 4] != "appdata" {
        return None;
    }
    let pub_display = publisher_display_for_folder(pub_dir)?;
    let product_name = parse_versioned_product_name(leaf)?;
    let product_tok = normalize_app_token(&product_name);
    if product_tok.is_empty() {
        return None;
    }
    if let Some(idx) = buckets
        .iter()
        .position(|b| normalize_app_token(&b.app.display_name) == product_tok)
    {
        return Some((idx, false));
    }
    let idx = ensure_synthetic_app(
        buckets,
        store_buckets,
        &format!("product:{product_tok}"),
        &product_name,
        pub_display,
        None,
        false,
    );
    Some((idx, false))
}

fn lookup_known_xdata_app(stem_tok: &str) -> Option<(&'static str, &'static str)> {
    match stem_tok {
        "zalo" => Some(("Zalo", "VNG Corp.")),
        "tabnine" => Some(("TabNine", "Tabnine")),
        "jellyfin" => Some(("Jellyfin", "Jellyfin")),
        "postman" => Some(("Postman", "Postman, Inc.")),
        "riot client" => Some(("Riot Client", "Riot Games, Inc.")),
        _ => None,
    }
}

fn match_xdata_folder(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    path_lower: &str,
    leaf: &str,
    leaf_lower: &str,
) -> Option<(usize, bool)> {
    let parts: Vec<&str> = path_lower.trim_end_matches('\\').split('\\').collect();
    if parts.len() < 3 {
        return None;
    }
    if parts[parts.len() - 3] != "appdata"
        || !matches!(parts[parts.len() - 2], "roaming" | "local")
    {
        return None;
    }
    if !leaf_lower.ends_with("data")
        || leaf.len() < 7
        || matches!(leaf_lower, "appdata" | "application data" | "programdata")
    {
        return None;
    }
    let x_stem = leaf[..leaf.len() - 4].trim_end_matches(['-', '_', ' ']);
    if x_stem.len() < 3 {
        return None;
    }
    if let Some(idx) = find_by_exact_name(buckets, x_stem) {
        return Some((idx, false));
    }
    let stem_tok = normalize_app_token(x_stem);
    if let Some((display_name, publisher)) = lookup_known_xdata_app(&stem_tok) {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            &format!("app:{stem_tok}"),
            display_name,
            publisher,
            None,
            false,
        );
        return Some((idx, false));
    }
    None
}

fn path_matches_token(path_lower: &str, leaf_lower: &str, needle: &str) -> bool {
    if needle.starts_with('.') {
        return leaf_lower == needle;
    }
    if !needle.contains('\\') {
        return leaf_lower == needle;
    }
    if path_lower == needle || path_lower.ends_with(needle) {
        return true;
    }
    if needle.starts_with('\\') {
        let with_slash = format!("{needle}\\");
        path_lower.contains(&with_slash)
    } else {
        let with_slash = format!("{needle}\\");
        path_lower.starts_with(&with_slash)
    }
}

fn find_by_publisher(buckets: &[AppBucket], leaf: &str) -> Option<usize> {
    let leaf_tok = normalize_app_token(leaf);
    if leaf_tok.len() < 4 {
        return None;
    }
    let mut matches: Vec<usize> = Vec::new();
    for (i, b) in buckets.iter().enumerate() {
        for pub_name in &b.app.publishers {
            let p_tok = normalize_app_token(pub_name);
            // Never attribute solely on generic "Microsoft" / "Microsoft Corporation".
            if p_tok.is_empty()
                || p_tok == "microsoft"
                || p_tok == "microsoft corporation"
                || p_tok.len() < 4
            {
                continue;
            }
            if p_tok == leaf_tok
                || p_tok.starts_with(&format!("{leaf_tok} "))
                || (leaf_tok.len() >= 6 && p_tok.contains(&leaf_tok))
            {
                matches.push(i);
                break;
            }
        }
    }
    // Only attribute when unambiguous (1 owning app, OR all matching apps belong to same publisher).
    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn attribute_store_package(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    pkg_folder: &str,
) -> Option<usize> {
    let lower = pkg_folder.to_ascii_lowercase();
    // Skip non-package synthetic sub-items
    if lower == "activesync" || lower == "packages" || lower == "windowsapps" || lower.starts_with("sandbox.") || lower.starts_with("oice_") {
        return None;
    }

    // Store package names are either `<FamilyStem>_<PubId>` (AppData\Local\Packages)
    // or `<FamilyStem>_<Version>_<Arch>_<ResourceId>_<PubId>` (Program Files\WindowsApps).
    let parts: Vec<&str> = pkg_folder.split('_').collect();
    let stem = parts.first().copied().unwrap_or(pkg_folder);
    let pub_id = if parts.len() >= 2 { parts.last().copied().unwrap_or("") } else { "" };
    let friendly = stem
        .strip_prefix("Microsoft.")
        .or_else(|| stem.strip_prefix("MicrosoftWindows."))
        .or_else(|| stem.strip_prefix("MicrosoftCorporationII."))
        .or_else(|| stem.strip_prefix("AppleInc."))
        .or_else(|| stem.strip_prefix("NVIDIACorp."))
        .or_else(|| stem.strip_prefix("RealtekSemiconductorCorp."))
        .or_else(|| stem.strip_prefix("B9ECED6F."))
        .or_else(|| stem.strip_prefix("40459File-New-Project."))
        .or_else(|| stem.strip_prefix("JAMSoftware."))
        .unwrap_or(stem);

    // Match against existing registry apps first if unambiguously named (e.g. TreeSize, ArmouryCrate, Edge).
    let friendly_tok = normalize_app_token(friendly);
    if friendly_tok.len() >= 5 {
        if let Some(idx) = buckets.iter().position(|b| {
            let dn = normalize_app_token(&b.app.display_name);
            dn == friendly_tok
        }) {
            return Some(idx);
        }
    }

    let is_runtime_or_system = lower.starts_with("microsoft.windows.")
        || lower.starts_with("microsoftwindows.")
        || lower.starts_with("microsoft.windowsappruntime")
        || lower.starts_with("microsoftcorporationii.winappruntime")
        || lower.starts_with("microsoft.net.native")
        || lower.starts_with("microsoft.directxruntime")
        || lower.starts_with("microsoft.vclibs")
        || lower.starts_with("microsoft.ui.xaml")
        || lower.starts_with("microsoft.aad.brokerplugin")
        || lower.starts_with("microsoft.accountscontrol")
        || lower.starts_with("microsoft.creddialoghost")
        || lower.starts_with("microsoft.bioenrollment")
        || lower.starts_with("microsoft.lockapp")
        || lower.starts_with("windows.")
        || lower.starts_with("1527c705-")
        || lower.starts_with("c5e2524a-")
        || lower.starts_with("e2a4f912-")
        || lower.starts_with("f46d4000-");

    let no_touch = if lower.starts_with("microsoft.net.native")
        || lower.starts_with("microsoft.windowsappruntime")
        || lower.starts_with("microsoftcorporationii.winappruntime")
        || lower.starts_with("microsoft.directxruntime")
        || lower.starts_with("microsoft.vclibs")
        || lower.starts_with("microsoft.ui.xaml")
    {
        Some(NoTouchCategory::SystemRuntime)
    } else if is_runtime_or_system {
        Some(NoTouchCategory::Service)
    } else {
        None
    };

    let display = format!("Store: {stem}");
    let key = if pub_id.is_empty() {
        format!("store:{}", stem.to_ascii_lowercase())
    } else {
        format!("store:{}_{}", stem.to_ascii_lowercase(), pub_id.to_ascii_lowercase())
    };
    let idx = ensure_synthetic_app(buckets, store_buckets, &key, &display, "Microsoft Store", no_touch, true);
    Some(idx)
}

pub fn build_store_package_map(items: &[(&str, &str)]) -> BTreeMap<String, String> {
    items
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}
