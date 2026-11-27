use std::collections::HashMap;
use std::path::PathBuf;

use y_core::attribution::attribute_scan;
use y_core::dependencies::{CatalogSourceKind, DependencyCatalog, analyze_report_dependencies, resolve_catalog_targets};
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, InstalledApp, RawRegistryApp, RegistryDiscoveryStats,
    ScannedUnit, ScannedUnitKind, ZoneKind,
};
use y_core::tree::decompose_scanned_tree;

fn test_tmp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-tmp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn versioned_product_under_publisher_rule_sums_versions_and_skips_non_versions() {
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 5_000_000_000,
        free_bytes: 2_000_000_000,
        used_bytes: 3_000_000_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let stats = RegistryDiscoveryStats {
        raw_display_name_entries: 1,
        raw_with_install_location: 1,
        canonical_apps: 1,
        canonical_with_direct_install_location: 1,
        canonical_with_inferred_install_location: 0,
    };
    let apps = vec![InstalledApp {
        id: "reg:chrome".into(),
        display_name: "Google Chrome".into(),
        display_version: String::new(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: vec!["Google LLC".into()],
        direct_install_location: Some("C:\\Program Files\\Google\\Chrome\\Application".into()),
        inferred_install_location: None,
        all_install_locations: vec!["C:\\Program Files\\Google\\Chrome\\Application".into()],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }];
    let units = vec![
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Google\\AndroidStudio2024.2".into(),
            drive_letter: 'C',
            bytes: 820_748_873,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Google\\AndroidStudio2023.2".into(),
            drive_letter: 'C',
            bytes: 582_578_704,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\Google\\AndroidStudio2024.2".into(),
            drive_letter: 'C',
            bytes: 27_865_918,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Google\\Chrome".into(),
            drive_letter: 'C',
            bytes: 100_000_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\JetBrains\\PyCharm2024.2".into(),
            drive_letter: 'C',
            bytes: 40_000_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\JetBrains\\PyCharm2023.3".into(),
            drive_letter: 'C',
            bytes: 10_000_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\Google\\consentOptions".into(),
            drive_letter: 'C',
            bytes: 1_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];

    let report = attribute_scan(
        1,
        "2026-11-26 17:45:00".into(),
        5,
        vec![drive],
        stats,
        &apps,
        &HashMap::new(),
        &units,
        None,
    );

    let android = report
        .apps
        .iter()
        .find(|a| a.display_name == "Android Studio")
        .expect("Android Studio entry");
    assert_eq!(android.total_bytes, 820_748_873 + 582_578_704 + 27_865_918);
    assert_eq!(android.items.len(), 3);

    let pycharm = report
        .apps
        .iter()
        .find(|a| a.display_name == "PyCharm")
        .expect("PyCharm entry");
    assert_eq!(pycharm.total_bytes, 50_000_000);

    let chrome = report
        .apps
        .iter()
        .find(|a| a.display_name == "Google Chrome")
        .unwrap();
    assert_eq!(chrome.total_bytes, 100_000_000);

    // Non-versioned consentOptions stays unattributed rather than force-matched
    assert_eq!(report.unattributed_total_bytes, 1_000);
    assert_eq!(
        report.unattributed[0].path,
        "C:\\Users\\Win\\AppData\\Roaming\\Google\\consentOptions"
    );
}

#[test]
fn xdata_rule_matches_installed_and_known_apps_without_force_matching_unknowns() {
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 5_000_000,
        free_bytes: 2_000_000,
        used_bytes: 3_000_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let stats = RegistryDiscoveryStats {
        raw_display_name_entries: 1,
        raw_with_install_location: 0,
        canonical_apps: 1,
        canonical_with_direct_install_location: 0,
        canonical_with_inferred_install_location: 0,
    };
    let apps = vec![InstalledApp {
        id: "reg:signal".into(),
        display_name: "Signal".into(),
        display_version: String::new(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: vec!["Signal Foundation".into()],
        direct_install_location: None,
        inferred_install_location: None,
        all_install_locations: vec![],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }];
    let units = vec![
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\ZaloData".into(),
            drive_letter: 'C',
            bytes: 457_156_942,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\SignalData".into(),
            drive_letter: 'C',
            bytes: 12_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\MysteryUnknownData".into(),
            drive_letter: 'C',
            bytes: 9_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];

    let report = attribute_scan(
        1,
        "2026-11-26 17:45:00".into(),
        5,
        vec![drive],
        stats,
        &apps,
        &HashMap::new(),
        &units,
        None,
    );

    let zalo = report.apps.iter().find(|a| a.display_name == "Zalo").unwrap();
    assert_eq!(zalo.total_bytes, 457_156_942);

    let signal = report
        .apps
        .iter()
        .find(|a| a.display_name == "Signal")
        .unwrap();
    assert_eq!(signal.total_bytes, 12_000);

    assert_eq!(report.unattributed_total_bytes, 9_000);
    assert_eq!(
        report.unattributed[0].path,
        "C:\\Users\\Win\\AppData\\Roaming\\MysteryUnknownData"
    );
}

#[test]
fn standalone_playwright_tabnine_and_split_containers_preserve_reconciliation() {
    let root_tree = FolderNode {
        path: "C:\\".into(),
        name: "C:\\".into(),
        direct_file_bytes: 0,
        total_bytes: 1_400_000,
        children: vec![FolderNode {
            path: "C:\\Users".into(),
            name: "Users".into(),
            direct_file_bytes: 0,
            total_bytes: 1_400_000,
            children: vec![FolderNode {
                path: "C:\\Users\\Win".into(),
                name: "Win".into(),
                direct_file_bytes: 0,
                total_bytes: 1_400_000,
                children: vec![
                    FolderNode {
                        path: "C:\\Users\\Win\\Microsoft".into(),
                        name: "Microsoft".into(),
                        direct_file_bytes: 0,
                        total_bytes: 200_000,
                        children: vec![FolderNode {
                            path: "C:\\Users\\Win\\Microsoft\\Power BI Desktop Store App".into(),
                            name: "Power BI Desktop Store App".into(),
                            direct_file_bytes: 200_000,
                            total_bytes: 200_000,
                            children: vec![],
                        }],
                    },
                    FolderNode {
                        path: "C:\\Users\\Win\\AppData".into(),
                        name: "AppData".into(),
                        direct_file_bytes: 0,
                        total_bytes: 1_200_000,
                        children: vec![
                            FolderNode {
                                path: "C:\\Users\\Win\\AppData\\Local".into(),
                                name: "Local".into(),
                                direct_file_bytes: 0,
                                total_bytes: 700_000,
                                children: vec![
                                    FolderNode {
                                        path: "C:\\Users\\Win\\AppData\\Local\\ms-playwright".into(),
                                        name: "ms-playwright".into(),
                                        direct_file_bytes: 400_000,
                                        total_bytes: 400_000,
                                        children: vec![],
                                    },
                                    FolderNode {
                                        path: "C:\\Users\\Win\\AppData\\Local\\Microsoft".into(),
                                        name: "Microsoft".into(),
                                        direct_file_bytes: 0,
                                        total_bytes: 300_000,
                                        children: vec![FolderNode {
                                            path: "C:\\Users\\Win\\AppData\\Local\\Microsoft\\Windows".into(),
                                            name: "Windows".into(),
                                            direct_file_bytes: 50_000,
                                            total_bytes: 300_000,
                                            children: vec![
                                                FolderNode {
                                                    path: "C:\\Users\\Win\\AppData\\Local\\Microsoft\\Windows\\WebCache".into(),
                                                    name: "WebCache".into(),
                                                    direct_file_bytes: 220_000,
                                                    total_bytes: 220_000,
                                                    children: vec![],
                                                },
                                                FolderNode {
                                                    path: "C:\\Users\\Win\\AppData\\Local\\Microsoft\\Windows\\Fonts".into(),
                                                    name: "Fonts".into(),
                                                    direct_file_bytes: 30_000,
                                                    total_bytes: 30_000,
                                                    children: vec![],
                                                },
                                            ],
                                        }],
                                    },
                                ],
                            },
                            FolderNode {
                                path: "C:\\Users\\Win\\AppData\\Roaming".into(),
                                name: "Roaming".into(),
                                direct_file_bytes: 0,
                                total_bytes: 500_000,
                                children: vec![FolderNode {
                                    path: "C:\\Users\\Win\\AppData\\Roaming\\TabNine".into(),
                                    name: "TabNine".into(),
                                    direct_file_bytes: 500_000,
                                    total_bytes: 500_000,
                                    children: vec![],
                                }],
                            },
                        ],
                    },
                ],
            }],
        }],
    };

    let units = decompose_scanned_tree(&root_tree, &[], &[], &[]);
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 10_000_000,
        free_bytes: 5_000_000,
        used_bytes: 5_000_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let stats = RegistryDiscoveryStats {
        raw_display_name_entries: 0,
        raw_with_install_location: 0,
        canonical_apps: 0,
        canonical_with_direct_install_location: 0,
        canonical_with_inferred_install_location: 0,
    };
    let report = attribute_scan(
        1,
        "2026-11-26 17:45:00".into(),
        5,
        vec![drive],
        stats,
        &[],
        &HashMap::new(),
        &units,
        None,
    );

    let pw = report
        .apps
        .iter()
        .find(|a| a.display_name == "Playwright")
        .expect("Playwright standalone");
    assert_eq!(pw.total_bytes, 400_000);
    assert!(pw.bases.contains(&AttributionBasis::StandaloneFolder));
    assert_eq!(pw.items[0].zone, Some(ZoneKind::Green));
    assert!(pw.publishers[0].contains("regenerable"));

    let tn = report
        .apps
        .iter()
        .find(|a| a.display_name == "TabNine")
        .expect("TabNine entry");
    assert_eq!(tn.total_bytes, 500_000);

    let pbi = report
        .apps
        .iter()
        .find(|a| a.display_name == "Microsoft Power BI Desktop")
        .expect("Power BI Desktop entry");
    assert_eq!(pbi.total_bytes, 200_000);

    assert_eq!(report.shared_caches_total_bytes, 220_000);
    assert_eq!(report.unattributed_total_bytes, 30_000);

    let apps_c = report.total_apps_bytes_on_drive('C');
    let caches_c = report.total_shared_caches_on_drive('C');
    let yf_c = report.total_your_files_on_drive('C');
    let un_c = report.total_unattributed_on_drive('C');
    let sys_c = report.windows_system[0].total_bytes;
    assert_eq!(apps_c + caches_c + yf_c + un_c + sys_c + 5_000_000, 10_000_000);
    let _ = RawRegistryApp {
        hive_view: String::new(),
        subkey_name: String::new(),
        display_name: String::new(),
        display_version: String::new(),
        publisher: String::new(),
        install_location: String::new(),
        uninstall_string: String::new(),
        display_icon: String::new(),
        system_component: false,
        parent_key_name: String::new(),
        release_type: String::new(),
    };
}

#[test]
fn catalog_embed_override_and_corrupt_fallback_work_as_specified() {
    let dir_no_sidecar = test_tmp_dir("round3-cat-none");
    let res_none = DependencyCatalog::load_for_exe_dir(Some(&dir_no_sidecar));
    assert_eq!(res_none.source_kind, CatalogSourceKind::Embedded);
    assert!(res_none.warning.is_none());
    assert!(!res_none.catalog.targets.is_empty());

    // Valid modified override sidecar placed directly next to exe
    let dir_override = test_tmp_dir("round3-cat-override");
    let base_toml = y_core::dependencies::EMBEDDED_DEPENDENCY_CATALOG;
    let modified = base_toml.replace(
        "FIFA 23 uses Denuvo Anti-Tamper DRM",
        "FIFA 23 uses Denuvo Anti-Tamper DRM [TEST-OVERRIDE]",
    );
    assert_ne!(modified, base_toml);
    std::fs::write(dir_override.join("dependency_catalog.toml"), &modified).unwrap();
    let res_override = DependencyCatalog::load_for_exe_dir(Some(&dir_override));
    assert_eq!(res_override.source_kind, CatalogSourceKind::ExternalOverride);
    assert!(res_override.warning.is_none());

    // Verify override reason propagates through analyze_report_dependencies
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 100_000,
        free_bytes: 50_000,
        used_bytes: 50_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let mut report = attribute_scan(
        1,
        "2026-11-26 17:45:00".into(),
        5,
        vec![drive],
        RegistryDiscoveryStats {
            raw_display_name_entries: 1,
            raw_with_install_location: 1,
            canonical_apps: 1,
            canonical_with_direct_install_location: 1,
            canonical_with_inferred_install_location: 0,
        },
        &[InstalledApp {
            id: "reg:fifa".into(),
            display_name: "EA SPORTS™ FIFA 23".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["EA Canada".into()],
            direct_install_location: Some("D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into()),
            inferred_install_location: None,
            all_install_locations: vec!["D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        }],
        &HashMap::new(),
        &[ScannedUnit {
            path: "D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into(),
            drive_letter: 'D',
            bytes: 1_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        }],
        None,
    );
    let statuses = resolve_catalog_targets(&res_override.catalog, &report.apps, &[], |_| false);
    analyze_report_dependencies(&mut report, &res_override.catalog, &[], &HashMap::new(), &statuses);
    let denuvo = report.apps[0]
        .dependencies
        .iter()
        .find(|d| d.target_id == "denuvo")
        .unwrap();
    assert!(denuvo.basis.contains("[TEST-OVERRIDE]"));

    // Corrupt syntax override -> falls back cleanly to embedded catalog with warning naming the file
    let dir_corrupt = test_tmp_dir("round3-cat-corrupt");
    std::fs::write(
        dir_corrupt.join("dependency_catalog.toml"),
        "<<<CORRUPT_TOML [[[",
    )
    .unwrap();
    let res_corrupt = DependencyCatalog::load_for_exe_dir(Some(&dir_corrupt));
    assert_eq!(
        res_corrupt.source_kind,
        CatalogSourceKind::EmbeddedFallbackAfterError
    );
    let warn = res_corrupt.warning.expect("warning must be logged");
    assert!(warn.contains("dependency_catalog.toml"));
    assert_eq!(res_corrupt.catalog.targets.len(), res_none.catalog.targets.len());

    // Valid TOML syntax with unknown schema field -> also rejected & falls back with warning
    let dir_bad_schema = test_tmp_dir("round3-cat-badschema");
    std::fs::write(
        dir_bad_schema.join("dependency_catalog.toml"),
        "unknown_top_key = 123\n",
    )
    .unwrap();
    let res_bad_schema = DependencyCatalog::load_for_exe_dir(Some(&dir_bad_schema));
    assert_eq!(
        res_bad_schema.source_kind,
        CatalogSourceKind::EmbeddedFallbackAfterError
    );
    assert!(res_bad_schema.warning.unwrap().contains("dependency_catalog.toml"));

    let _ = std::fs::remove_dir_all(&dir_no_sidecar);
    let _ = std::fs::remove_dir_all(&dir_override);
    let _ = std::fs::remove_dir_all(&dir_corrupt);
    let _ = std::fs::remove_dir_all(&dir_bad_schema);
}

#[test]
fn msteams_and_steam_collision_attributes_steam_folders_to_steam_and_cleans_msteams() {
    use y_core::attribution::match_installed_app_index;
    use y_core::categories::classify_report_apps;
    use y_core::dependencies::{WindowsServiceEntry, resolve_catalog_targets_with_registry};
    use y_core::discovery::build_registry_catalog;
    use y_core::model::AppCategory;

    // Part 1: protocol launcher redirect (`steam://uninstall/...`) + Steam icon cache never shadows `Steam`.
    let raw = vec![
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "Steam App 376210".into(),
            display_name: "The Isle".into(),
            display_version: String::new(),
            publisher: "Afterthought LLC".into(),
            install_location: String::new(),
            uninstall_string: "\"Z:\\Games\\Steam\\steam.exe\" steam://uninstall/376210".into(),
            display_icon: "Z:\\Games\\Steam\\steam\\games\\dcc409f581316cdb0deb73f8bcc5202d32edaee8.ico".into(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "Steam".into(),
            display_name: "Steam".into(),
            display_version: "2.10.91.91".into(),
            publisher: "Valve Corporation".into(),
            install_location: String::new(),
            uninstall_string: "Z:\\Games\\Steam\\uninstall.exe".into(),
            display_icon: "Z:\\Games\\Steam\\uninstall.exe".into(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
    ];
    let reg_cat = build_registry_catalog(&raw, Some("Z:\\Games\\Steam"));
    assert_eq!(reg_cat.apps.len(), 2);
    let steam_reg = reg_cat
        .apps
        .iter()
        .find(|a| a.display_name == "Steam")
        .expect("Steam must remain its own canonical app row");
    assert_eq!(
        steam_reg.inferred_install_location.as_deref(),
        Some("Z:\\Games\\Steam")
    );
    let the_isle_reg = reg_cat
        .apps
        .iter()
        .find(|a| a.display_name == "The Isle")
        .unwrap();
    assert_eq!(the_isle_reg.inferred_install_location, None);

    // Part 2: `"Store: MSTeams"` placed BEFORE `"Steam"` must NEVER match `"steam"` in token, category, or dep lookup.
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 2_000_000_000,
        free_bytes: 500_000_000,
        used_bytes: 1_500_000_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let apps = vec![
        InstalledApp {
            id: "store:msteams_8wekyb3d8bbwe".into(),
            display_name: "Store: MSTeams".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["Microsoft Store".into()],
            direct_install_location: None,
            inferred_install_location: None,
            all_install_locations: vec![],
            no_touch: None,
            is_store_fallback: true,
            display_icon: None,
        },
        steam_reg.clone(),
    ];
    assert_eq!(match_installed_app_index(&apps, &["steam"]), Some(1));

    let units = vec![
        ScannedUnit {
            path: "C:\\Program Files\\WindowsApps\\MSTeams_25017.203.3370.1174_x64__8wekyb3d8bbwe".into(),
            drive_letter: 'C',
            bytes: 279_412_182,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Packages\\MSTeams_8wekyb3d8bbwe".into(),
            drive_letter: 'C',
            bytes: 16_384,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Steam\\htmlcache".into(),
            drive_letter: 'C',
            bytes: 777_226_881,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Steam\\cefdata".into(),
            drive_letter: 'C',
            bytes: 19_136_578,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Program Files (x86)\\Common Files\\Steam\\drivers".into(),
            drive_letter: 'C',
            bytes: 8_966_361,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];

    let mut report = attribute_scan(
        1,
        "2026-11-27 16:45:00".into(),
        5,
        vec![drive],
        reg_cat.stats,
        &apps,
        &HashMap::new(),
        &units,
        None,
    );
    classify_report_apps(&mut report, |_| false);
    let catalog = DependencyCatalog::load_from_disk_or_embedded();
    let services = vec![WindowsServiceEntry {
        service_name: "Steam Client Service".into(),
        display_name: "Steam Client Service".into(),
        service_type: 16,
        raw_image_path: "C:\\Program Files (x86)\\Common Files\\Steam\\SteamService.exe".into(),
        normalized_image_path: "C:\\Program Files (x86)\\Common Files\\Steam\\SteamService.exe".into(),
        depend_on_service: vec![],
    }];
    let statuses = resolve_catalog_targets_with_registry(
        &catalog,
        &report.apps,
        &report.installed_registry_names,
        &services,
        |_| false,
    );
    analyze_report_dependencies(&mut report, &catalog, &services, &HashMap::new(), &statuses);

    let msteams = report
        .apps
        .iter()
        .find(|a| a.display_name == "Store: MSTeams")
        .expect("Store: MSTeams present");
    assert_eq!(msteams.total_bytes, 279_412_182 + 16_384);
    assert_eq!(msteams.items.len(), 2);
    assert!(msteams.items.iter().all(|it| it.path.contains("MSTeams")));
    assert_eq!(msteams.category, AppCategory::Communication);
    assert!(!msteams.dependencies.iter().any(|d| d.target_id == "steam"));

    let steam = report
        .apps
        .iter()
        .find(|a| a.display_name == "Steam")
        .expect("Steam present");
    assert_eq!(steam.total_bytes, 777_226_881 + 19_136_578 + 8_966_361);
    assert_eq!(steam.items.len(), 3);
    assert_eq!(steam.category, AppCategory::PlatformsAndLaunchers);
}

#[test]
fn generic_substring_and_ambiguous_app_name_collisions_never_force_match() {
    use y_core::attribution::match_installed_app_index;

    let make_app = |id: &str, name: &str, pub_name: &str| InstalledApp {
        id: id.into(),
        display_name: name.into(),
        display_version: String::new(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: vec![pub_name.into()],
        direct_install_location: None,
        inferred_install_location: None,
        all_install_locations: vec![],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    };

    // 1. Mid-word substring `X = "Nova"` inside `Y = "SuperNova"` (Y ordered first): never matches Y, picks X.
    let apps_mid = vec![
        make_app("reg:supernova", "SuperNova", "SuperNova Labs"),
        make_app("reg:nova", "Nova", "Nova Corp"),
    ];
    assert_eq!(match_installed_app_index(&apps_mid, &["nova"]), Some(1));
    assert_eq!(
        match_installed_app_index(&[make_app("reg:supernova", "SuperNova", "SuperNova Labs")], &["nova"]),
        None
    );

    // 2. Whole-word prefix `X = "Python 3.15.1 (64-bit)"` vs `Y = "Python Launcher"` (Y first): exact full token beats partial.
    let apps_exact_pref = vec![
        make_app("reg:python-launcher", "Python Launcher", "Python Software Foundation"),
        make_app("reg:python", "Python 3.15.1 (64-bit)", "Python Software Foundation"),
    ];
    assert_eq!(match_installed_app_index(&apps_exact_pref, &["python"]), Some(1));

    // 3. Ambiguous multi-app partial match ("Widget Pro" + "Widget Studio", no exact "Widget"): refuses to force-match and lands in Unattributed.
    let apps_ambig = vec![
        make_app("reg:widget-pro", "Widget Pro", "Widget Inc"),
        make_app("reg:widget-studio", "Widget Studio", "Widget Inc"),
    ];
    assert_eq!(match_installed_app_index(&apps_ambig, &["widget"]), None);

    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 1_000_000,
        free_bytes: 800_000,
        used_bytes: 200_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let units = vec![
        // Real MSTeams fallback vs absent Steam (using Generic X/Y via XData):
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Packages\\SuperWidget_8wekyb3d8bbwe".into(),
            drive_letter: 'C',
            bytes: 100_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Steam\\htmlcache".into(),
            drive_letter: 'C',
            bytes: 50_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];
    let apps_fallback = vec![
        make_app("store:supernova", "Store: SuperSteamView", "Microsoft Store"),
    ];
    let rep = attribute_scan(
        1,
        "2026-11-27 16:45:00".into(),
        5,
        vec![drive],
        RegistryDiscoveryStats {
            raw_display_name_entries: 1,
            raw_with_install_location: 0,
            canonical_apps: 1,
            canonical_with_direct_install_location: 0,
            canonical_with_inferred_install_location: 0,
        },
        &apps_fallback,
        &HashMap::new(),
        &units,
        None,
    );
    assert!(rep.unattributed.iter().any(|u| u.path.ends_with("\\AppData\\Local\\Steam\\htmlcache")));
    assert_eq!(
        rep.total_apps_bytes_on_drive('C') + rep.total_unattributed_on_drive('C') + rep.windows_system[0].total_bytes + 800_000,
        1_000_000
    );
}

#[test]
fn vc_package_cache_lands_on_its_own_versions_row() {
    use y_core::discovery::build_registry_catalog;

    let raw = vec![
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{33d1fd90-4274-48a1-9bc1-97e33d9c2d6f}".into(),
            display_name: "Microsoft Visual C++ 2012 Redistributable (x86) - 11.0.61030".into(),
            display_version: "11.0.61030.0".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: "\"C:\\ProgramData\\Package Cache\\{33d1fd90-4274-48a1-9bc1-97e33d9c2d6f}\\vcredist_x86.exe\" /u".into(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{042d26ef-3dbe-4c25-95d3-4c1b11b235a7}".into(),
            display_name: "Microsoft Visual C++ 2013 Redistributable (x64) - 12.0.40664".into(),
            display_version: "12.0.40664.0".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: "\"C:\\ProgramData\\Package Cache\\{042d26ef-3dbe-4c25-95d3-4c1b11b235a7}\\vcredist_x64.exe\" /u".into(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{0e3bb569-69d6-4c34-bff9-c2f81db5e5f0}".into(),
            display_name: "Microsoft Visual C++ v14 Redistributable (x64) - 14.51.36247".into(),
            display_version: "14.51.36247.0".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: "\"C:\\ProgramData\\Package Cache\\{0e3bb569-69d6-4c34-bff9-c2f81db5e5f0}\\VC_redist.x64.exe\" /u".into(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{B175520C-86A2-35A7-8619-86DC379688B9}".into(),
            display_name: "Microsoft Visual C++ 2012 x86 Additional Runtime - 11.0.61030".into(),
            display_version: "11.0.61030".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: true,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{53CF6934-A98D-3D84-9146-FC4EDF3D5641}".into(),
            display_name: "Microsoft Visual C++ 2013 x64 Minimum Runtime - 12.0.40664".into(),
            display_version: "12.0.40664".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: true,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{6FA797CF-6B76-4B6D-87EE-768F92008720}".into(),
            display_name: "Microsoft Visual C++ 2022 X64 Additional Runtime - 14.51.36247".into(),
            display_version: "14.51.36247".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: true,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
    ];

    let cat = build_registry_catalog(&raw, None);
    assert_eq!(cat.apps.len(), 3, "Each VC++ Redistributable version/arch must stay its own canonical app row");

    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 100_000_000,
        free_bytes: 50_000_000,
        used_bytes: 50_000_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let units = vec![
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{33d1fd90-4274-48a1-9bc1-97e33d9c2d6f}".into(),
            drive_letter: 'C',
            bytes: 456_496,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{B175520C-86A2-35A7-8619-86DC379688B9}v11.0.61030".into(),
            drive_letter: 'C',
            bytes: 5_305_368,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{042d26ef-3dbe-4c25-95d3-4c1b11b235a7}".into(),
            drive_letter: 'C',
            bytes: 466_814,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{53CF6934-A98D-3D84-9146-FC4EDF3D5641}v12.0.40664".into(),
            drive_letter: 'C',
            bytes: 1_177_788,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{0e3bb569-69d6-4c34-bff9-c2f81db5e5f0}".into(),
            drive_letter: 'C',
            bytes: 631_084,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{6FA797CF-6B76-4B6D-87EE-768F92008720}v14.51.36247".into(),
            drive_letter: 'C',
            bytes: 5_861_692,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];

    let report = attribute_scan(
        10,
        "2026-11-27 16:00:00".into(),
        5,
        vec![drive],
        cat.stats,
        &cat.apps,
        &cat.guid_to_app_name,
        &units,
        None,
    );

    let r2012 = report.apps.iter().find(|a| a.display_name.contains("2012")).unwrap();
    let r2013 = report.apps.iter().find(|a| a.display_name.contains("2013")).unwrap();
    let rv14 = report.apps.iter().find(|a| a.display_name.contains("v14")).unwrap();
    assert_eq!(r2012.total_bytes, 456_496 + 5_305_368);
    assert_eq!(r2013.total_bytes, 466_814 + 1_177_788);
    assert_eq!(rv14.total_bytes, 631_084 + 5_861_692);
    assert!(r2012.items.iter().all(|it| it.path.contains("33d1fd90") || it.path.contains("11.0.61030")));
    assert!(r2013.items.iter().all(|it| it.path.contains("042d26ef") || it.path.contains("12.0.40664")));
    assert!(rv14.items.iter().all(|it| it.path.contains("0e3bb569") || it.path.contains("14.51.36247")));
}

#[test]
fn split_family_row_sums_reconcile_with_former_merged_total() {
    use y_core::discovery::build_registry_catalog;

    let raw = vec![
        // Windows Desktop Runtime family (formerly merged)
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{29A18B06-B190-4463-9FFC-BC0D70D80B7C}".into(),
            display_name: "Microsoft Windows Desktop Runtime 10.0.14 (x64)".into(),
            display_version: "10.0.14.50000".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{4d0027d4-962f-4f65-a01b-f8764a5f4220}".into(),
            display_name: "Microsoft Windows Desktop Runtime - 8.0.33 (x64)".into(),
            display_version: "8.0.33.36622".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{64BFC674-D664-480B-AFF2-D47C5531D6D2}".into(),
            display_name: "Microsoft Windows Desktop Runtime - 10.0.14 (x64)".into(),
            display_version: "80.56.61678".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: true,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-32".into(),
            subkey_name: "{711FC301-B296-4A6D-8A4F-EB1F70A2FCA3}".into(),
            display_name: "Microsoft Windows Desktop Runtime - 8.0.33 (x64)".into(),
            display_version: "64.132.61678".into(),
            publisher: "Microsoft Corporation".into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: true,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        // Single unaffected app with HKCU reflection duplicate
        RawRegistryApp {
            hive_view: "HKCU-64".into(),
            subkey_name: "Discord".into(),
            display_name: "Discord".into(),
            display_version: "1.0.9269".into(),
            publisher: "Discord Inc.".into(),
            install_location: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKCU-32".into(),
            subkey_name: "Discord".into(),
            display_name: "Discord".into(),
            display_version: "1.0.9269".into(),
            publisher: "Discord Inc.".into(),
            install_location: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
    ];

    let cat = build_registry_catalog(&raw, None);
    assert_eq!(cat.apps.len(), 3);

    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 200_000_000,
        free_bytes: 100_000_000,
        used_bytes: 100_000_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let units = vec![
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{29A18B06-B190-4463-9FFC-BC0D70D80B7C}".into(),
            drive_letter: 'C',
            bytes: 992_466,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{64BFC674-D664-480B-AFF2-D47C5531D6D2}v80.56.61678".into(),
            drive_letter: 'C',
            bytes: 30_068_736,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{4d0027d4-962f-4f65-a01b-f8764a5f4220}".into(),
            drive_letter: 'C',
            bytes: 623_288,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\ProgramData\\Package Cache\\{711FC301-B296-4A6D-8A4F-EB1F70A2FCA3}v64.132.61678".into(),
            drive_letter: 'C',
            bytes: 30_310_400,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
            drive_letter: 'C',
            bytes: 10_000_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
    ];

    let report = attribute_scan(
        10,
        "2026-11-27 16:00:00".into(),
        5,
        vec![drive],
        cat.stats,
        &cat.apps,
        &cat.guid_to_app_name,
        &units,
        None,
    );

    let former_merged_desktop_total = 992_466 + 30_068_736 + 623_288 + 30_310_400;
    let desktop_sum: u64 = report
        .apps
        .iter()
        .filter(|a| a.display_name.contains("Windows Desktop Runtime"))
        .map(|a| a.total_bytes)
        .sum();
    assert_eq!(desktop_sum, former_merged_desktop_total);
    assert_eq!(report.apps.iter().find(|a| a.display_name == "Discord").unwrap().total_bytes, 10_000_000);
    let sys_c = report.windows_system[0].total_bytes;
    assert_eq!(
        report.total_apps_bytes_on_drive('C') + sys_c + 100_000_000,
        200_000_000
    );
}
