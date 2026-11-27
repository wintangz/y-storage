use std::collections::HashMap;
use std::path::PathBuf;

use y_core::attribution::attribute_scan;
use y_core::cache::{FolderSizeCache, load_last_scan, save_last_scan_atomic};
use y_core::config::{AppConfig, ThemeMode};
use y_core::discovery::{build_registry_catalog, infer_install_root_from_exe, normalize_windows_path};
use y_core::history::{ScanHistory, compute_growth};
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, GrowthDelta, InstalledApp, LocationQualifier,
    NoTouchCategory, RawRegistryApp, ScannedUnit, ScannedUnitKind, ZoneKind,
};
use y_core::tree::{decompose_scanned_tree, deduplicate_secondary_install_trees};
use y_core::zones::classify_zone;

fn test_tmp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-tmp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn registry_normalization_merges_duplicate_install_locations_and_retains_guids() {
    let raw = vec![
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "Steam App 1811260".into(),
            display_name: "EA SPORTS™ FIFA 23".into(),
            display_version: "".into(),
            publisher: "EA Canada".into(),
            install_location: "D:/Games/Steam/steamapps/common/FIFA 23/".into(),
            uninstall_string: "".into(),
            display_icon: "".into(),
            system_component: false,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{B2A14ADC-C2DC-432C-BA7E-FA59282364AB}".into(),
            display_name: "FIFA 23".into(),
            display_version: "".into(),
            publisher: "Electronic Arts, Inc.".into(),
            install_location: "\"D:\\Games\\Steam\\steamapps\\common\\FIFA 23\"".into(),
            uninstall_string: "".into(),
            display_icon: "".into(),
            system_component: false,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "{C2622085-ABD2-49E5-8AB9-D3D6A642C091}".into(),
            display_name: "EA app".into(),
            display_version: "".into(),
            publisher: "Electronic Arts".into(),
            install_location: "".into(),
            uninstall_string: "MsiExec.exe /I{C2622085-ABD2-49E5-8AB9-D3D6A642C091}".into(),
            display_icon: "".into(),
            system_component: true,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{c8007f09-887b-4f6c-a7d2-385cb8638d10}".into(),
            display_name: "EA app".into(),
            display_version: "".into(),
            publisher: "Electronic Arts".into(),
            install_location: "".into(),
            uninstall_string:
                "\"C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EAUninstall.exe\" /uninstall"
                    .into(),
            display_icon: "".into(),
            system_component: false,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
    ];

    let cat = build_registry_catalog(&raw, Some("Z:\\Games\\Steam"));
    assert_eq!(cat.stats.raw_display_name_entries, 4);
    assert_eq!(cat.stats.raw_with_install_location, 2);
    assert_eq!(cat.stats.canonical_apps, 2);
    assert_eq!(
        cat.guid_to_app_name.get("{c2622085-abd2-49e5-8ab9-d3d6a642c091}").map(String::as_str),
        Some("EA app")
    );
    let fifa = cat
        .apps
        .iter()
        .find(|a| a.display_name.contains("FIFA 23"))
        .unwrap();
    assert_eq!(fifa.aliases, vec!["FIFA 23"]);
    assert_eq!(fifa.publishers.len(), 2);

    let ea = cat.apps.iter().find(|a| a.display_name == "EA app").unwrap();
    assert_eq!(
        ea.inferred_install_location.as_deref(),
        Some("C:\\Program Files\\Electronic Arts")
    );
    assert_eq!(
        normalize_windows_path("\"C:/Program Files/Test/\""),
        "C:\\Program Files\\Test"
    );
    assert_eq!(
        infer_install_root_from_exe("C:\\Windows\\System32\\msiexec.exe /x {1234}"),
        None
    );
}

#[test]
fn zone_and_no_touch_classification_match_constitution() {
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Local\\Temp", false),
        (ZoneKind::Green, None)
    );
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Local\\D3DSCache", false),
        (ZoneKind::Green, None)
    );
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Local\\NVIDIA\\DXCache", false),
        (ZoneKind::Green, None)
    );
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Local\\AMD\\DxCache", false),
        (ZoneKind::Green, None)
    );
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Local\\CrashDumps", false),
        (ZoneKind::Green, None)
    );
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Local\\Programs\\AppX", false),
        (ZoneKind::Body, None)
    );
    assert_eq!(
        classify_zone("C:\\Users\\Win\\AppData\\Roaming\\discord", false),
        (ZoneKind::Red, Some(LocationQualifier::RoamingConfig))
    );
    assert_eq!(
        classify_zone("C:\\ProgramData\\EA Desktop", false),
        (ZoneKind::Red, Some(LocationQualifier::MachineShared))
    );
    assert_eq!(
        classify_zone("C:\\ProgramData\\Package Cache\\{ABC}", false),
        (ZoneKind::Green, Some(LocationQualifier::MachineShared))
    );

    let raw = vec![
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "1".into(),
            display_name: "NVIDIA Graphics Driver 621.01".into(),
            display_version: "621.01".into(),
            publisher: "NVIDIA Corporation".into(),
            install_location: "".into(),
            uninstall_string: "".into(),
            display_icon: "".into(),
            system_component: false,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "2".into(),
            display_name: "Microsoft Visual C++ v14 Redistributable (x64)".into(),
            display_version: "14.51.36247.0".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: "".into(),
            uninstall_string: "".into(),
            display_icon: "".into(),
            system_component: false,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "3".into(),
            display_name: "ROG Live Service".into(),
            display_version: "".into(),
            publisher: "ASUSTek COMPUTER INC.".into(),
            install_location: "".into(),
            uninstall_string: "".into(),
            display_icon: "".into(),
            system_component: false,
            parent_key_name: "".into(),
            release_type: "".into(),
        },
    ];
    let cat = build_registry_catalog(&raw, None);
    assert_eq!(cat.apps[0].no_touch, Some(NoTouchCategory::Driver));
    assert_eq!(cat.apps[1].no_touch, Some(NoTouchCategory::SystemRuntime));
    assert_eq!(cat.apps[2].no_touch, Some(NoTouchCategory::Service));
}

#[test]
fn personal_shell_folders_and_drive_reconciliation_conserve_every_byte() {
    let apps = vec![InstalledApp {
        id: "reg:discord".into(),
        display_name: "Discord".into(),
        display_version: String::new(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: vec!["Discord Inc.".into()],
        direct_install_location: Some("C:\\Users\\Win\\AppData\\Local\\Discord".into()),
        inferred_install_location: None,
        all_install_locations: vec!["C:\\Users\\Win\\AppData\\Local\\Discord".into()],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }];
    let shell_folders = vec!["C:\\Users\\Win\\Documents".to_string()];

    let root_tree = FolderNode {
        path: "C:\\".into(),
        name: "C:\\".into(),
        direct_file_bytes: 100,
        total_bytes: 10_100,
        children: vec![FolderNode {
            path: "C:\\Users".into(),
            name: "Users".into(),
            direct_file_bytes: 0,
            total_bytes: 10_000,
            children: vec![FolderNode {
                path: "C:\\Users\\Win".into(),
                name: "Win".into(),
                direct_file_bytes: 500, // e.g. NTUSER.DAT
                total_bytes: 10_000,
                children: vec![
                    FolderNode {
                        path: "C:\\Users\\Win\\Documents".into(),
                        name: "Documents".into(),
                        direct_file_bytes: 3_000,
                        total_bytes: 3_000,
                        children: vec![FolderNode {
                            // Must NEVER split Documents\Discord out of Your files!
                            path: "C:\\Users\\Win\\Documents\\Discord".into(),
                            name: "Discord".into(),
                            direct_file_bytes: 1_500,
                            total_bytes: 1_500,
                            children: vec![],
                        }],
                    },
                    FolderNode {
                        path: "C:\\Users\\Win\\AppData".into(),
                        name: "AppData".into(),
                        direct_file_bytes: 0,
                        total_bytes: 6_500,
                        children: vec![
                            FolderNode {
                                path: "C:\\Users\\Win\\AppData\\Local".into(),
                                name: "Local".into(),
                                direct_file_bytes: 0,
                                total_bytes: 4_500,
                                children: vec![
                                    FolderNode {
                                        path: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
                                        name: "Discord".into(),
                                        direct_file_bytes: 4_000,
                                        total_bytes: 4_000,
                                        children: vec![],
                                    },
                                    FolderNode {
                                        path: "C:\\Users\\Win\\AppData\\Local\\RandomLeftover"
                                            .into(),
                                        name: "RandomLeftover".into(),
                                        direct_file_bytes: 500,
                                        total_bytes: 500,
                                        children: vec![],
                                    },
                                ],
                            },
                            FolderNode {
                                path: "C:\\Users\\Win\\AppData\\Roaming".into(),
                                name: "Roaming".into(),
                                direct_file_bytes: 0,
                                total_bytes: 2_000,
                                children: vec![FolderNode {
                                    path: "C:\\Users\\Win\\AppData\\Roaming\\discord".into(),
                                    name: "discord".into(),
                                    direct_file_bytes: 2_000,
                                    total_bytes: 2_000,
                                    children: vec![],
                                }],
                            },
                        ],
                    },
                ],
            }],
        }],
    };

    let units = decompose_scanned_tree(
        &root_tree,
        &apps,
        &shell_folders,
        &[("C:\\pagefile.sys".into(), 5_000)],
    );

    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 50_000,
        free_bytes: 20_000,
        used_bytes: 30_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
        last_scanned_unix: None,
        last_scanned_display: None,
    };
    let stats = y_core::model::RegistryDiscoveryStats {
        raw_display_name_entries: 1,
        raw_with_install_location: 1,
        canonical_apps: 1,
        canonical_with_direct_install_location: 1,
        canonical_with_inferred_install_location: 0,
    };

    let report = attribute_scan(
        1000,
        "2026-11-26 09:00:00".into(),
        10,
        vec![drive],
        stats,
        &apps,
        &HashMap::new(),
        &units,
        None,
    );

    let discord = report
        .apps
        .iter()
        .find(|a| a.display_name == "Discord")
        .unwrap();
    assert_eq!(discord.total_bytes, 6_000);
    assert!(discord.bases.contains(&AttributionBasis::InstallLocation));
    assert!(discord.bases.contains(&AttributionBasis::ExactName));

    // Your files stays atomic, never zoned, never attributed to Discord
    assert_eq!(report.your_files_total_bytes, 3_000);
    assert!(report.your_files.iter().all(|i| i.zone.is_none()));

    // Exact byte reconciliation on C:\
    let apps_c = report.total_apps_bytes_on_drive('C');
    let yf_c = report.total_your_files_on_drive('C');
    let un_c = report.total_unattributed_on_drive('C');
    let sys_c = report.windows_system[0].total_bytes;
    assert_eq!(apps_c + yf_c + un_c + sys_c + 20_000, 50_000);
    assert_eq!(report.windows_system[0].remainder_bytes, 14_900);
}

#[test]
fn nested_secondary_install_locations_subtract_without_double_counting() {
    let units = deduplicate_secondary_install_trees(&[
        ("Z:\\Games\\Steam".into(), 10_000),
        ("Z:\\Games\\Steam\\steamapps\\common\\Slay the Spire 2".into(), 3_000),
        ("Z:\\Games\\Steam\\steamapps\\common\\Slay the Spire 2".into(), 3_000),
    ]);
    let sum: u64 = units.iter().map(|u| u.bytes).sum();
    assert_eq!(sum, 10_000);
    let steam = units.iter().find(|u| u.path == "Z:\\Games\\Steam").unwrap();
    assert_eq!(steam.bytes, 7_000);
    let sts2 = units
        .iter()
        .find(|u| u.path.ends_with("Slay the Spire 2"))
        .unwrap();
    assert_eq!(sts2.bytes, 3_000);
}

#[test]
fn scan_history_config_and_cache_roundtrip_atomically() {
    let tmp = test_tmp_dir("history-config-cache");
    let cfg_path = tmp.join("config.toml");
    let hist_path = tmp.join("scan_history.json");
    let cache_path = tmp.join("folder_cache.json");
    let last_path = tmp.join("last_scan.json");

    let drives = vec![
        DriveInfo {
            letter: 'C',
            root: "C:\\".into(),
            total_bytes: 100,
            free_bytes: 50,
            used_bytes: 50,
            is_system_drive: true,
            full_walk_enabled: true,
            exclusions: vec!["C:\\Windows".into()],
            last_scanned_unix: None,
            last_scanned_display: None,
        },
        DriveInfo {
            letter: 'D',
            root: "D:\\".into(),
            total_bytes: 200,
            free_bytes: 100,
            used_bytes: 100,
            is_system_drive: false,
            full_walk_enabled: false,
            exclusions: vec![],
            last_scanned_unix: None,
            last_scanned_display: None,
        },
    ];

    let mut cfg = AppConfig::for_detected_drives(&drives, "C:\\Windows");
    assert!(cfg.is_drive_enabled('C'));
    assert!(!cfg.is_drive_enabled('D'));
    cfg.theme_mode = ThemeMode::Light;
    cfg.save_atomic(&cfg_path).unwrap();
    let loaded_cfg = AppConfig::load_or_default(&cfg_path, &drives, "C:\\Windows");
    assert_eq!(loaded_cfg.theme_mode, ThemeMode::Light);
    assert!(loaded_cfg.is_drive_enabled('C'));

    assert_eq!(compute_growth(false, None, 100), GrowthDelta::NoBaseline);
    assert_eq!(compute_growth(true, None, 500), GrowthDelta::New(500));
    assert_eq!(
        compute_growth(true, Some(1_000), 1_500),
        GrowthDelta::Changed(500)
    );
    assert_eq!(
        compute_growth(true, Some(1_000), 750),
        GrowthDelta::Changed(-250)
    );

    let report = attribute_scan(
        10,
        "2026-11-26 09:00:00".into(),
        5,
        drives,
        y_core::model::RegistryDiscoveryStats {
            raw_display_name_entries: 0,
            raw_with_install_location: 0,
            canonical_apps: 0,
            canonical_with_direct_install_location: 0,
            canonical_with_inferred_install_location: 0,
        },
        &[],
        &HashMap::new(),
        &[ScannedUnit {
            path: "C:\\TempStuff".into(),
            drive_letter: 'C',
            bytes: 123,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        }],
        None,
    );
    let mut hist = ScanHistory::default();
    hist.record_report(&report);
    hist.save_atomic(&hist_path).unwrap();
    let loaded_hist = ScanHistory::load_or_default(&hist_path);
    assert_eq!(loaded_hist.snapshots.len(), 1);

    let mut cache = FolderSizeCache::default();
    cache.update_from_report(&report);
    cache.save_atomic(&cache_path).unwrap();
    let loaded_cache = FolderSizeCache::load_or_default(&cache_path);
    assert_eq!(loaded_cache.folder_bytes.get("C:\\TempStuff"), Some(&123));

    save_last_scan_atomic(&last_path, &report).unwrap();
    assert!(load_last_scan(&last_path).is_some());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn cached_app_icon_base64_and_json_roundtrip() {
    use y_core::model::CachedAppIcon;
    let mut pixels = Vec::with_capacity(18 * 18 * 4);
    for idx in 0..(18 * 18) {
        pixels.push((idx % 255) as u8);
        pixels.push(((idx * 3) % 255) as u8);
        pixels.push(((idx * 7) % 255) as u8);
        pixels.push(if idx % 2 == 0 { 255 } else { 128 });
    }
    let icon = CachedAppIcon::from_rgba(18, 18, &pixels);
    let json = serde_json::to_string(&icon).unwrap();
    let parsed: CachedAppIcon = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.width, 18);
    assert_eq!(parsed.height, 18);
    assert_eq!(parsed.decode_rgba().as_deref(), Some(pixels.as_slice()));
}
