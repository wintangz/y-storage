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
        aliases: vec![],
        publishers: vec!["Google LLC".into()],
        direct_install_location: Some("C:\\Program Files\\Google\\Chrome\\Application".into()),
        inferred_install_location: None,
        all_install_locations: vec!["C:\\Program Files\\Google\\Chrome\\Application".into()],
        no_touch: None,
        is_store_fallback: false,
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
        aliases: vec![],
        publishers: vec!["Signal Foundation".into()],
        direct_install_location: None,
        inferred_install_location: None,
        all_install_locations: vec![],
        no_touch: None,
        is_store_fallback: false,
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
            aliases: vec![],
            publishers: vec!["EA Canada".into()],
            direct_install_location: Some("D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into()),
            inferred_install_location: None,
            all_install_locations: vec!["D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into()],
            no_touch: None,
            is_store_fallback: false,
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
