use std::collections::HashMap;
use std::path::PathBuf;

use y_core::attribution::{AttributionOptions, RootExeCandidate, attribute_scan_with_options};
use y_core::config::{AppConfig, PinKind, ZoneRuleKind, ZoneRulebook};
use y_core::dependencies::parse_epic_manifest_item;
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, InstalledApp, LocationQualifier, ManifestAppEntry,
    RegistryDiscoveryStats, ScanReport,
};
use y_core::tree::{DecompositionOptions, decompose_scanned_tree_with_options};

fn drive_fixture(letter: char, used: u64, free: u64, is_sys: bool) -> DriveInfo {
    DriveInfo {
        letter,
        root: format!("{letter}:\\"),
        total_bytes: used + free,
        free_bytes: free,
        used_bytes: used,
        is_system_drive: is_sys,
        full_walk_enabled: true,
        exclusions: if is_sys {
            vec![format!("{letter}:\\Windows")]
        } else {
            vec![]
        },
        last_scanned_unix: Some(1_795_775_000),
        last_scanned_display: Some("2026-11-27 19:50:00".into()),
    }
}

fn empty_stats(n: usize) -> RegistryDiscoveryStats {
    RegistryDiscoveryStats {
        raw_display_name_entries: n,
        raw_with_install_location: n,
        canonical_apps: n,
        canonical_with_direct_install_location: n,
        canonical_with_inferred_install_location: 0,
    }
}

fn installed_app(id: &str, name: &str, publisher: Option<&str>, install_loc: Option<&str>) -> InstalledApp {
    InstalledApp {
        id: id.to_string(),
        display_name: name.to_string(),
        display_version: String::new(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: publisher.into_iter().map(str::to_string).collect(),
        direct_install_location: install_loc.map(str::to_string),
        inferred_install_location: None,
        all_install_locations: install_loc.into_iter().map(str::to_string).collect(),
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }
}

fn assert_drive_diff_zero(report: &ScanReport, drive: char) {
    let sys = report
        .windows_system
        .iter()
        .find(|w| w.drive_letter == drive)
        .expect("system breakdown present");
    let total = report.total_apps_bytes_on_drive(drive)
        + report.total_shared_caches_on_drive(drive)
        + report.total_your_files_on_drive(drive)
        + report.total_unattributed_on_drive(drive)
        + sys.total_bytes
        - sys.hardlink_overlap_bytes
        + sys.drive_free_bytes;
    assert_eq!(total, sys.drive_total_bytes, "reconciliation diff must be 0 on {drive}:\\");
}

fn assert_zero_bracketed_paths(units: &[y_core::model::ScannedUnit], report: &ScanReport) {
    for u in units {
        assert!(
            !u.path.contains("[files]") && !u.path.contains("[root files]"),
            "synthetic path found in ScannedUnit: {}",
            u.path
        );
    }
    for app in &report.apps {
        for it in &app.items {
            assert!(
                !it.path.contains("[files]") && !it.path.contains("[root files]"),
                "synthetic path found in app {}: {}",
                app.display_name,
                it.path
            );
        }
    }
    for u in &report.unattributed {
        assert!(
            !u.path.contains("[files]") && !u.path.contains("[root files]"),
            "synthetic path found in unattributed: {}",
            u.path
        );
    }
    for c in &report.shared_caches {
        assert!(
            !c.path.contains("[files]") && !c.path.contains("[root files]"),
            "synthetic path found in shared_caches: {}",
            c.path
        );
    }
}

#[test]
fn nested_carve_out_count_1_splits_launcher_and_records_carved_child() {
    let installed = vec![
        installed_app(
            "reg:hoyoplay",
            "HoYoPlay",
            Some("COGNOSPHERE PTE. LTD."),
            Some(r"C:\Program Files\HoYoPlay"),
        ),
        installed_app(
            "reg:genshin",
            "Genshin Impact",
            Some("COGNOSPHERE PTE. LTD."),
            Some(r"C:\Program Files\HoYoPlay\Genshin Impact Game"),
        ),
    ];

    let tree_c = FolderNode {
        path: r"C:\".into(),
        name: r"C:\".into(),
        direct_file_bytes: 0,
        total_bytes: 93_000_000_000,
        children: vec![FolderNode {
            path: r"C:\Program Files".into(),
            name: "Program Files".into(),
            direct_file_bytes: 0,
            total_bytes: 93_000_000_000,
            children: vec![FolderNode {
                path: r"C:\Program Files\HoYoPlay".into(),
                name: "HoYoPlay".into(),
                direct_file_bytes: 1_000_000_000,
                total_bytes: 93_000_000_000,
                children: vec![
                    FolderNode {
                        path: r"C:\Program Files\HoYoPlay\LauncherData".into(),
                        name: "LauncherData".into(),
                        direct_file_bytes: 2_000_000_000,
                        total_bytes: 2_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"C:\Program Files\HoYoPlay\Genshin Impact Game".into(),
                        name: "Genshin Impact Game".into(),
                        direct_file_bytes: 90_000_000_000,
                        total_bytes: 90_000_000_000,
                        children: vec![],
                    },
                ],
            }],
        }],
    };

    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_c,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: None,
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('C', 100_000_000_000, 100_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            ..AttributionOptions::default()
        },
    );

    assert_zero_bracketed_paths(&units, &report);
    let hoyo = report.apps.iter().find(|a| a.display_name == "HoYoPlay").unwrap();
    let genshin = report.apps.iter().find(|a| a.display_name == "Genshin Impact").unwrap();
    assert_eq!(hoyo.total_bytes, 3_000_000_000, "launcher must exclude carved-out game");
    assert_eq!(genshin.total_bytes, 90_000_000_000, "single nested child must receive full 90 GB");
    assert_eq!(hoyo.carved_out_apps, vec!["Genshin Impact"]);
    assert!(hoyo.items.iter().any(|it| {
        it.path.eq_ignore_ascii_case(r"C:\Program Files\HoYoPlay")
            && it.bytes == 1_000_000_000
            && it.qualifier == Some(LocationQualifier::InstallLooseFiles)
            && it.basis_label() == "install location"
    }));
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn nested_carve_out_count_2_and_remainder_keep_real_paths_and_zero_files_suffix() {
    let installed = vec![
        installed_app("reg:launcher", "MyLauncher", Some("Studio"), Some(r"D:\Launcher")),
        installed_app("reg:g1", "GameOne", Some("Studio"), Some(r"D:\Launcher\Game1")),
        installed_app("reg:g2", "GameTwo", Some("Studio"), Some(r"D:\Launcher\Game2")),
    ];

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 12_000,
        total_bytes: 93_250_012_000,
        children: vec![
            FolderNode {
                path: r"D:\Launcher".into(),
                name: "Launcher".into(),
                direct_file_bytes: 1_000_000_000,
                total_bytes: 93_000_000_000,
                children: vec![
                    FolderNode {
                        path: r"D:\Launcher\bin".into(),
                        name: "bin".into(),
                        direct_file_bytes: 2_000_000_000,
                        total_bytes: 2_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Launcher\Game1".into(),
                        name: "Game1".into(),
                        direct_file_bytes: 40_000_000_000,
                        total_bytes: 40_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Launcher\Game2".into(),
                        name: "Game2".into(),
                        direct_file_bytes: 50_000_000_000,
                        total_bytes: 50_000_000_000,
                        children: vec![],
                    },
                ],
            },
            FolderNode {
                path: r"D:\Softwares".into(),
                name: "Softwares".into(),
                direct_file_bytes: 50_000_000,
                total_bytes: 250_000_000,
                children: vec![FolderNode {
                    path: r"D:\Softwares\ToolA".into(),
                    name: "ToolA".into(),
                    direct_file_bytes: 200_000_000,
                    total_bytes: 200_000_000,
                    children: vec![],
                }],
            },
        ],
    };

    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: None,
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 94_000_000_000, 100_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            ..AttributionOptions::default()
        },
    );

    assert_zero_bracketed_paths(&units, &report);

    let launcher = report.apps.iter().find(|a| a.display_name == "MyLauncher").unwrap();
    let g1 = report.apps.iter().find(|a| a.display_name == "GameOne").unwrap();
    let g2 = report.apps.iter().find(|a| a.display_name == "GameTwo").unwrap();
    assert_eq!(launcher.total_bytes, 3_000_000_000);
    assert_eq!(g1.total_bytes, 40_000_000_000);
    assert_eq!(g2.total_bytes, 50_000_000_000);
    assert_eq!(launcher.carved_out_apps, vec!["GameOne", "GameTwo"]);

    let loose_in_launcher = launcher
        .items
        .iter()
        .find(|i| i.path == r"D:\Launcher")
        .expect("real path D:\\Launcher for loose files");
    assert_eq!(loose_in_launcher.bytes, 1_000_000_000);
    assert_eq!(
        loose_in_launcher.qualifier,
        Some(LocationQualifier::InstallLooseFiles)
    );
    assert_eq!(
        loose_in_launcher.qualifier.unwrap().label(),
        "loose files in the install folder"
    );

    let unattr_softwares = report
        .unattributed
        .iter()
        .find(|u| u.path == r"D:\Softwares" && u.is_loose_files)
        .expect("D:\\Softwares loose remainder keeps real path");
    assert_eq!(unattr_softwares.bytes, 50_000_000);

    let unattr_root = report
        .unattributed
        .iter()
        .find(|u| u.path == r"D:\" && u.is_loose_files)
        .expect("D:\\ root loose remainder keeps real path");
    assert_eq!(unattr_root.bytes, 12_000);

    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn two_segment_publisher_product_matches_google_drive_and_rejects_ties() {
    // 1. Google Drive listed BEFORE Google Chrome -> Google\Drive matches Drive, Google\Chrome matches Chrome.
    let installed = vec![
        installed_app(
            "reg:google-drive",
            "Google Drive",
            Some("Google LLC"),
            Some(r"C:\Program Files\Google\Drive File Stream"),
        ),
        installed_app(
            "reg:google-chrome",
            "Google Chrome",
            Some("Google LLC"),
            Some(r"C:\Program Files\Google\Chrome\Application"),
        ),
        installed_app(
            "reg:acme-sync-1",
            "Acme Sync",
            Some("Acme Corp"),
            Some(r"C:\Apps\Acme1"),
        ),
        installed_app(
            "reg:acme-sync-2",
            "Acme Sync",
            Some("Acme Beta"),
            Some(r"D:\Apps\Acme2"),
        ),
    ];

    let tree_c = FolderNode {
        path: r"C:\".into(),
        name: r"C:\".into(),
        direct_file_bytes: 0,
        total_bytes: 1_150_000_000,
        children: vec![FolderNode {
            path: r"C:\Users".into(),
            name: "Users".into(),
            direct_file_bytes: 0,
            total_bytes: 1_150_000_000,
            children: vec![FolderNode {
                path: r"C:\Users\Win".into(),
                name: "Win".into(),
                direct_file_bytes: 0,
                total_bytes: 1_150_000_000,
                children: vec![FolderNode {
                    path: r"C:\Users\Win\AppData".into(),
                    name: "AppData".into(),
                    direct_file_bytes: 0,
                    total_bytes: 1_150_000_000,
                    children: vec![FolderNode {
                        path: r"C:\Users\Win\AppData\Local".into(),
                        name: "Local".into(),
                        direct_file_bytes: 0,
                        total_bytes: 1_150_000_000,
                        children: vec![
                            FolderNode {
                                path: r"C:\Users\Win\AppData\Local\Google".into(),
                                name: "Google".into(),
                                direct_file_bytes: 0,
                                total_bytes: 1_000_000_000,
                                children: vec![
                                    FolderNode {
                                        path: r"C:\Users\Win\AppData\Local\Google\Drive".into(),
                                        name: "Drive".into(),
                                        direct_file_bytes: 300_000_000,
                                        total_bytes: 300_000_000,
                                        children: vec![],
                                    },
                                    FolderNode {
                                        path: r"C:\Users\Win\AppData\Local\Google\Chrome".into(),
                                        name: "Chrome".into(),
                                        direct_file_bytes: 700_000_000,
                                        total_bytes: 700_000_000,
                                        children: vec![],
                                    },
                                ],
                            },
                            FolderNode {
                                path: r"C:\Users\Win\AppData\Local\Acme".into(),
                                name: "Acme".into(),
                                direct_file_bytes: 0,
                                total_bytes: 150_000_000,
                                children: vec![FolderNode {
                                    path: r"C:\Users\Win\AppData\Local\Acme\Sync".into(),
                                    name: "Sync".into(),
                                    direct_file_bytes: 150_000_000,
                                    total_bytes: 150_000_000,
                                    children: vec![],
                                }],
                            },
                        ],
                    }],
                }],
            }],
        }],
    };

    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_c,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            ..DecompositionOptions::default()
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('C', 2_000_000_000, 1_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            ..AttributionOptions::default()
        },
    );

    let gdrive = report.apps.iter().find(|a| a.display_name == "Google Drive").unwrap();
    assert_eq!(gdrive.total_bytes, 300_000_000);
    assert_eq!(gdrive.bases, vec![AttributionBasis::PublisherProduct]);
    assert_eq!(gdrive.items[0].basis_label(), "publisher+folder match");

    let chrome = report.apps.iter().find(|a| a.display_name == "Google Chrome").unwrap();
    assert_eq!(chrome.total_bytes, 700_000_000);
    assert_eq!(chrome.bases, vec![AttributionBasis::PublisherProduct]);

    assert_eq!(report.unattributed_total_bytes, 150_000_000);
    assert_eq!(
        report.unattributed[0].path,
        r"C:\Users\Win\AppData\Local\Acme\Sync"
    );
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn group_node_promotes_three_children_with_group_publisher_and_no_parent_app() {
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

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 12_000_000_000,
        children: vec![FolderNode {
            path: r"D:\Apps".into(),
            name: "Apps".into(),
            direct_file_bytes: 0,
            total_bytes: 12_000_000_000,
            children: vec![FolderNode {
                path: r"D:\Apps\Adobe".into(),
                name: "Adobe".into(),
                direct_file_bytes: 0,
                total_bytes: 12_000_000_000,
                children: vec![
                    FolderNode {
                        path: r"D:\Apps\Adobe\Photoshop 2025".into(),
                        name: "Photoshop 2025".into(),
                        direct_file_bytes: 4_000_000_000,
                        total_bytes: 4_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Apps\Adobe\Illustrator 2025".into(),
                        name: "Illustrator 2025".into(),
                        direct_file_bytes: 3_000_000_000,
                        total_bytes: 3_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Apps\Adobe\Premiere Pro 2025".into(),
                        name: "Premiere Pro 2025".into(),
                        direct_file_bytes: 5_000_000_000,
                        total_bytes: 5_000_000_000,
                        children: vec![],
                    },
                ],
            }],
        }],
    };

    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &[],
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map),
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 12_000_000_000, 10_000_000_000, false)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            ..AttributionOptions::default()
        },
    );

    assert!(
        !report.apps.iter().any(|a| a.display_name.eq_ignore_ascii_case("Adobe")),
        "group folder Adobe must NOT become an app row"
    );
    assert_eq!(report.apps.len(), 3);
    for (name, bytes) in [
        ("Adobe Photoshop 2025", 4_000_000_000_u64),
        ("Adobe Illustrator 2025", 3_000_000_000_u64),
        ("Adobe Premiere Pro 2025", 5_000_000_000_u64),
    ] {
        let app = report.apps.iter().find(|a| a.display_name == name).unwrap();
        assert_eq!(app.total_bytes, bytes);
        assert_eq!(app.publishers, vec!["Adobe"]);
    }
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn single_child_wrapper_promotes_outer_folder_not_child() {
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\portabletool\v2.1".to_string(),
        RootExeCandidate {
            exe_path: r"D:\PortableTool\v2.1\PortableTool.exe".into(),
            product_name: Some("PortableTool".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 55_010_000,
        children: vec![FolderNode {
            path: r"D:\PortableTool".into(),
            name: "PortableTool".into(),
            direct_file_bytes: 10_000,
            total_bytes: 55_010_000,
            children: vec![
                FolderNode {
                    path: r"D:\PortableTool\v2.1".into(),
                    name: "v2.1".into(),
                    direct_file_bytes: 50_000_000,
                    total_bytes: 50_000_000,
                    children: vec![],
                },
                FolderNode {
                    path: r"D:\PortableTool\presets".into(),
                    name: "presets".into(),
                    direct_file_bytes: 5_000_000,
                    total_bytes: 5_000_000,
                    children: vec![],
                },
            ],
        }],
    };

    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &[],
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map),
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 55_010_000, 100_000_000, false)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 1);
    assert_eq!(report.apps[0].display_name, "PortableTool");
    assert_eq!(report.apps[0].total_bytes, 55_010_000);
    assert_eq!(report.apps[0].install_location.as_deref(), Some(r"D:\PortableTool"));
    assert!(!report.apps.iter().any(|a| a.display_name == "v2.1"));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn manifest_container_includes_manifest_members_and_excludes_non_member_siblings() {
    let manifest_entries = vec![
        ManifestAppEntry {
            id: "steam:app:1145360".into(),
            display_name: "Hades".into(),
            publisher: None,
            install_dir: r"D:\SteamLibrary\steamapps\common\Hades".into(),
            container_dir: r"D:\SteamLibrary\steamapps\common".into(),
            basis: AttributionBasis::SteamManifest,
            display_icon: None,
        },
        ManifestAppEntry {
            id: "steam:app:367520".into(),
            display_name: "Hollow Knight".into(),
            publisher: None,
            install_dir: r"D:\SteamLibrary\steamapps\common\Hollow Knight".into(),
            container_dir: r"D:\SteamLibrary\steamapps\common".into(),
            basis: AttributionBasis::SteamManifest,
            display_icon: None,
        },
    ];
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\steamlibrary\steamapps\common\orphanmod".to_string(),
        RootExeCandidate {
            exe_path: r"D:\SteamLibrary\steamapps\common\OrphanMod\Mod.exe".into(),
            product_name: Some("OrphanMod".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 26_800_000_000,
        children: vec![FolderNode {
            path: r"D:\SteamLibrary".into(),
            name: "SteamLibrary".into(),
            direct_file_bytes: 0,
            total_bytes: 26_800_000_000,
            children: vec![FolderNode {
                path: r"D:\SteamLibrary\steamapps".into(),
                name: "steamapps".into(),
                direct_file_bytes: 0,
                total_bytes: 26_800_000_000,
                children: vec![
                    FolderNode {
                        path: r"D:\SteamLibrary\steamapps\common".into(),
                        name: "common".into(),
                        direct_file_bytes: 0,
                        total_bytes: 23_300_000_000,
                        children: vec![
                            FolderNode {
                                path: r"D:\SteamLibrary\steamapps\common\Hades".into(),
                                name: "Hades".into(),
                                direct_file_bytes: 15_000_000_000,
                                total_bytes: 15_000_000_000,
                                children: vec![],
                            },
                            FolderNode {
                                path: r"D:\SteamLibrary\steamapps\common\Hollow Knight".into(),
                                name: "Hollow Knight".into(),
                                direct_file_bytes: 8_000_000_000,
                                total_bytes: 8_000_000_000,
                                children: vec![],
                            },
                            FolderNode {
                                path: r"D:\SteamLibrary\steamapps\common\OrphanMod".into(),
                                name: "OrphanMod".into(),
                                direct_file_bytes: 300_000_000,
                                total_bytes: 300_000_000,
                                children: vec![],
                            },
                        ],
                    },
                    FolderNode {
                        path: r"D:\SteamLibrary\steamapps\workshop".into(),
                        name: "workshop".into(),
                        direct_file_bytes: 2_000_000_000,
                        total_bytes: 2_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\SteamLibrary\steamapps\downloading".into(),
                        name: "downloading".into(),
                        direct_file_bytes: 1_000_000_000,
                        total_bytes: 1_000_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\SteamLibrary\steamapps\shadercache".into(),
                        name: "shadercache".into(),
                        direct_file_bytes: 500_000_000,
                        total_bytes: 500_000_000,
                        children: vec![],
                    },
                ],
            }],
        }],
    };

    let zones = ZoneRulebook::default();
    let libs = vec![r"D:\SteamLibrary".to_string()];
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &[],
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &manifest_entries,
            exe_candidates: Some(&exe_map),
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 26_800_000_000, 100_000_000_000, false)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            manifest_entries: &manifest_entries,
            steam_library_roots: &libs,
            root_exe_candidates: Some(&exe_map),
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 2);
    assert_eq!(report.apps.iter().find(|a| a.display_name == "Hades").unwrap().total_bytes, 15_000_000_000);
    assert_eq!(report.apps.iter().find(|a| a.display_name == "Hollow Knight").unwrap().total_bytes, 8_000_000_000);
    assert_eq!(report.shared_caches_total_bytes, 500_000_000);
    assert_eq!(report.unattributed_total_bytes, 3_300_000_000);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn personal_zone_routes_media_folder_to_your_files_without_app_hunting() {
    let mut zones = ZoneRulebook::default();
    assert!(zones.add_explicit_zone(r"D:\Media", ZoneRuleKind::PersonalFiles));

    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\media\jellyfin".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Media\Jellyfin\jellyfin.exe".into(),
            product_name: Some("Jellyfin".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 152_471_339_008,
        children: vec![FolderNode {
            path: r"D:\Media".into(),
            name: "Media".into(),
            direct_file_bytes: 100_000_000_000,
            total_bytes: 152_471_339_008,
            children: vec![
                FolderNode {
                    path: r"D:\Media\Movies".into(),
                    name: "Movies".into(),
                    direct_file_bytes: 52_000_000_000,
                    total_bytes: 52_000_000_000,
                    children: vec![],
                },
                FolderNode {
                    path: r"D:\Media\Jellyfin".into(),
                    name: "Jellyfin".into(),
                    direct_file_bytes: 471_339_008,
                    total_bytes: 471_339_008,
                    children: vec![],
                },
            ],
        }],
    };

    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &[],
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map),
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 152_471_339_008, 100_000_000_000, false)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            ..AttributionOptions::default()
        },
    );

    assert!(report.apps.is_empty());
    assert!(report.unattributed.is_empty());
    assert_eq!(report.your_files_total_bytes, 152_471_339_008);
    assert_eq!(report.your_files[0].path, r"D:\Media");
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn registered_app_inside_personal_zone_keeps_its_folder() {
    let mut zones = ZoneRulebook::default();
    assert!(zones.add_explicit_zone(r"D:\Media", ZoneRuleKind::PersonalFiles));

    let installed = vec![installed_app(
        "reg:vlc",
        "VLC media player",
        Some("VideoLAN"),
        Some(r"D:\Media\Tools\VLC"),
    )];

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 100_000_000_000,
        children: vec![FolderNode {
            path: r"D:\Media".into(),
            name: "Media".into(),
            direct_file_bytes: 60_000_000_000,
            total_bytes: 100_000_000_000,
            children: vec![
                FolderNode {
                    path: r"D:\Media\Movies".into(),
                    name: "Movies".into(),
                    direct_file_bytes: 35_000_000_000,
                    total_bytes: 35_000_000_000,
                    children: vec![],
                },
                FolderNode {
                    path: r"D:\Media\Tools".into(),
                    name: "Tools".into(),
                    direct_file_bytes: 0,
                    total_bytes: 5_000_000_000,
                    children: vec![FolderNode {
                        path: r"D:\Media\Tools\VLC".into(),
                        name: "VLC".into(),
                        direct_file_bytes: 5_000_000_000,
                        total_bytes: 5_000_000_000,
                        children: vec![],
                    }],
                },
            ],
        }],
    };

    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: None,
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 100_000_000_000, 100_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            ..AttributionOptions::default()
        },
    );

    let vlc = report.apps.iter().find(|a| a.display_name == "VLC media player").unwrap();
    assert_eq!(vlc.total_bytes, 5_000_000_000);
    assert_eq!(report.your_files_total_bytes, 95_000_000_000);
    assert_eq!(report.your_files[0].path, r"D:\Media");
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn manual_pins_override_wrong_inferences_and_persist_in_config() {
    let mut cfg = AppConfig::default();
    assert!(cfg.zones.set_pin(r"D:\Apps\NotAnApp", PinKind::Ignore));
    assert!(cfg.zones.set_pin(r"D:\Apps\SuiteAsOne", PinKind::App));
    assert!(cfg.zones.set_pin(r"D:\WrapPinGroup", PinKind::Group));
    assert!(cfg.zones.set_pin(r"D:\Games\HomeClip", PinKind::Personal));

    let tmp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-tmp/round5-pins");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let cfg_path = tmp_dir.join("config.toml");
    cfg.save_atomic(&cfg_path).unwrap();
    let loaded = AppConfig::load_or_default(&cfg_path, &[drive_fixture('D', 1_000, 1_000, false)], r"C:\Windows");
    assert_eq!(loaded.zones.pins, cfg.zones.pins);

    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\suiteasone\sub1".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\SuiteAsOne\Sub1\a.exe".into(),
            product_name: Some("SubOne".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"d:\apps\suiteasone\sub2".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\SuiteAsOne\Sub2\b.exe".into(),
            product_name: Some("SubTwo".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"d:\wrappinGroup\onlychild".to_ascii_lowercase(),
        RootExeCandidate {
            exe_path: r"D:\WrapPinGroup\OnlyChild\tool.exe".into(),
            product_name: Some("Split Child App".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 1_000_000,
        children: vec![
            FolderNode {
                path: r"D:\Apps".into(),
                name: "Apps".into(),
                direct_file_bytes: 0,
                total_bytes: 500_000,
                children: vec![
                    FolderNode {
                        path: r"D:\Apps\NotAnApp".into(),
                        name: "NotAnApp".into(),
                        direct_file_bytes: 100_000,
                        total_bytes: 100_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Apps\SuiteAsOne".into(),
                        name: "SuiteAsOne".into(),
                        direct_file_bytes: 400_000,
                        total_bytes: 400_000,
                        children: vec![
                            FolderNode {
                                path: r"D:\Apps\SuiteAsOne\Sub1".into(),
                                name: "Sub1".into(),
                                direct_file_bytes: 200_000,
                                total_bytes: 200_000,
                                children: vec![],
                            },
                            FolderNode {
                                path: r"D:\Apps\SuiteAsOne\Sub2".into(),
                                name: "Sub2".into(),
                                direct_file_bytes: 200_000,
                                total_bytes: 200_000,
                                children: vec![],
                            },
                        ],
                    },
                ],
            },
            FolderNode {
                path: r"D:\Games".into(),
                name: "Games".into(),
                direct_file_bytes: 0,
                total_bytes: 200_000,
                children: vec![FolderNode {
                    path: r"D:\Games\HomeClip".into(),
                    name: "HomeClip".into(),
                    direct_file_bytes: 200_000,
                    total_bytes: 200_000,
                    children: vec![],
                }],
            },
            FolderNode {
                path: r"D:\WrapPinGroup".into(),
                name: "WrapPinGroup".into(),
                direct_file_bytes: 0,
                total_bytes: 300_000,
                children: vec![FolderNode {
                    path: r"D:\WrapPinGroup\OnlyChild".into(),
                    name: "OnlyChild".into(),
                    direct_file_bytes: 300_000,
                    total_bytes: 300_000,
                    children: vec![],
                }],
            },
        ],
    };

    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &[],
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&loaded.zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map),
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 1_000_000, 1_000_000, false)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&loaded.zones),
            root_exe_candidates: Some(&exe_map),
            ..AttributionOptions::default()
        },
    );

    assert!(!report.apps.iter().any(|a| a.display_name == "NotAnApp"));
    assert_eq!(report.unattributed_total_bytes, 100_000);

    let suite = report.apps.iter().find(|a| a.display_name == "SuiteAsOne").unwrap();
    assert_eq!(suite.total_bytes, 400_000);
    assert_eq!(suite.bases, vec![AttributionBasis::ManualPin]);

    let wrapped_child = report.apps.iter().find(|a| a.display_name == "Split Child App").unwrap();
    assert_eq!(wrapped_child.total_bytes, 300_000);
    assert_eq!(wrapped_child.publishers, vec!["WrapPinGroup"]);

    assert_eq!(report.your_files_total_bytes, 200_000);
    assert_eq!(report.your_files[0].path, r"D:\Games\HomeClip");
    assert_drive_diff_zero(&report, 'D');

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn containment_is_weakest_claim_and_epic_manifest_excludes_dlc() {
    let dlc_json = r#"{
        "DisplayName": "Civilization VI : Aztec DLC",
        "AppName": "KingletAztec",
        "MainGameAppName": "Kinglet",
        "InstallLocation": "D:\\Games\\Epic\\SidMeiersCivilizationVI",
        "bIsIncompleteInstall": false
    }"#;
    assert!(parse_epic_manifest_item(dlc_json).is_none());

    let base_json = r#"{
        "DisplayName": "Sid Meier's Civilization VI",
        "AppName": "Kinglet",
        "MainGameAppName": "",
        "InstallLocation": "D:\\Games\\Epic\\SidMeiersCivilizationVI",
        "LaunchExecutable": "Base/Binaries/Win64EOS/Civ6_Launcher.exe",
        "bIsIncompleteInstall": false
    }"#;
    let base = parse_epic_manifest_item(base_json).unwrap();
    assert_eq!(base.display_name, "Sid Meier's Civilization VI");
    assert_eq!(base.install_location, r"D:\Games\Epic\SidMeiersCivilizationVI");

    let mut zones = ZoneRulebook::default();
    assert!(zones.set_pin(r"D:\Launcher\PinnedModTool", PinKind::App));
    let installed = vec![installed_app(
        "reg:launcher",
        "MyLauncher",
        Some("Studio"),
        Some(r"D:\Launcher"),
    )];
    let manifest_entries = vec![ManifestAppEntry {
        id: "steam:app:999".into(),
        display_name: "ManifestGame".into(),
        publisher: None,
        install_dir: r"D:\Launcher\ManifestGame".into(),
        container_dir: r"D:\Launcher".into(),
        basis: AttributionBasis::SteamManifest,
        display_icon: None,
    }];
    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 28_000_000_000,
        children: vec![FolderNode {
            path: r"D:\Launcher".into(),
            name: "Launcher".into(),
            direct_file_bytes: 1_000_000_000,
            total_bytes: 28_000_000_000,
            children: vec![
                FolderNode {
                    path: r"D:\Launcher\Engine".into(),
                    name: "Engine".into(),
                    direct_file_bytes: 2_000_000_000,
                    total_bytes: 2_000_000_000,
                    children: vec![],
                },
                FolderNode {
                    path: r"D:\Launcher\PinnedModTool".into(),
                    name: "PinnedModTool".into(),
                    direct_file_bytes: 5_000_000_000,
                    total_bytes: 5_000_000_000,
                    children: vec![],
                },
                FolderNode {
                    path: r"D:\Launcher\ManifestGame".into(),
                    name: "ManifestGame".into(),
                    direct_file_bytes: 20_000_000_000,
                    total_bytes: 20_000_000_000,
                    children: vec![],
                },
            ],
        }],
    };
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &manifest_entries,
            exe_candidates: None,
        },
    );
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![drive_fixture('D', 28_000_000_000, 100_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            manifest_entries: &manifest_entries,
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.iter().find(|a| a.display_name == "ManifestGame").unwrap().total_bytes, 20_000_000_000);
    assert_eq!(report.apps.iter().find(|a| a.display_name == "PinnedModTool").unwrap().total_bytes, 5_000_000_000);
    assert_eq!(report.apps.iter().find(|a| a.display_name == "MyLauncher").unwrap().total_bytes, 3_000_000_000);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn app_container_children_and_installed_roots_do_not_fracture_or_misname() {
    assert!(!y_core::pe::is_plausible_main_exe_name("__Start game + create shortcut.exe"));

    let installed = vec![
        installed_app(
            "reg:ahk",
            "AutoHotkey 2.0.19",
            Some("AutoHotkey Foundation LLC"),
            Some(r"C:\Program Files\AutoHotkey"),
        ),
        installed_app(
            "reg:gameinput",
            "Microsoft GameInput",
            Some("Microsoft Corporation"),
            None,
        ),
    ];

    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"c:\program files\autohotkey\v2".to_string(),
        RootExeCandidate {
            exe_path: r"C:\Program Files\AutoHotkey\v2\AutoHotkey64.exe".into(),
            product_name: Some("AutoHotkey".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"c:\program files\autohotkey\ux".to_string(),
        RootExeCandidate {
            exe_path: r"C:\Program Files\AutoHotkey\UX\AutoHotkeyUX.exe".into(),
            product_name: Some("AutoHotkey".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"c:\program files\microsoft gameinput\x86".to_string(),
        RootExeCandidate {
            exe_path: r"C:\Program Files\Microsoft GameInput\x86\gameinputhost.exe".into(),
            product_name: Some("Microsoft GameInput".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"c:\program files\microsoft gameinput\x64".to_string(),
        RootExeCandidate {
            exe_path: r"C:\Program Files\Microsoft GameInput\x64\gameinputhost.exe".into(),
            product_name: Some("Microsoft® Windows® GameInput".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"d:\games\lossless.scaling.v3.2.2_linkneverdie.com".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Games\Lossless.Scaling.v3.2.2_LinkNeverDie.Com\Launcher.exe".into(),
            product_name: Some("LinkNeverDie.Com Launcher".into()),
            company_name: None,
            signer_org: None,
        },
    );
    exe_map.insert(
        r"d:\games\othergame".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Games\OtherGame\Game.exe".into(),
            product_name: Some("Other Game".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let tree_c = FolderNode {
        path: r"C:\".into(),
        name: r"C:\".into(),
        direct_file_bytes: 0,
        total_bytes: 20_000_000,
        children: vec![FolderNode {
            path: r"C:\Program Files".into(),
            name: "Program Files".into(),
            direct_file_bytes: 0,
            total_bytes: 20_000_000,
            children: vec![
                FolderNode {
                    path: r"C:\Program Files\AutoHotkey".into(),
                    name: "AutoHotkey".into(),
                    direct_file_bytes: 20_000,
                    total_bytes: 16_000_000,
                    children: vec![
                        FolderNode {
                            path: r"C:\Program Files\AutoHotkey\v2".into(),
                            name: "v2".into(),
                            direct_file_bytes: 10_000_000,
                            total_bytes: 10_000_000,
                            children: vec![],
                        },
                        FolderNode {
                            path: r"C:\Program Files\AutoHotkey\UX".into(),
                            name: "UX".into(),
                            direct_file_bytes: 5_980_000,
                            total_bytes: 5_980_000,
                            children: vec![],
                        },
                    ],
                },
                FolderNode {
                    path: r"C:\Program Files\Microsoft GameInput".into(),
                    name: "Microsoft GameInput".into(),
                    direct_file_bytes: 0,
                    total_bytes: 4_000_000,
                    children: vec![
                        FolderNode {
                            path: r"C:\Program Files\Microsoft GameInput\x86".into(),
                            name: "x86".into(),
                            direct_file_bytes: 1_500_000,
                            total_bytes: 1_500_000,
                            children: vec![],
                        },
                        FolderNode {
                            path: r"C:\Program Files\Microsoft GameInput\x64".into(),
                            name: "x64".into(),
                            direct_file_bytes: 2_500_000,
                            total_bytes: 2_500_000,
                            children: vec![],
                        },
                    ],
                },
            ],
        }],
    };

    let tree_d = FolderNode {
        path: r"D:\".into(),
        name: r"D:\".into(),
        direct_file_bytes: 0,
        total_bytes: 2_800_000_000,
        children: vec![
            FolderNode {
                path: r"D:\Apps".into(),
                name: "Apps".into(),
                direct_file_bytes: 0,
                total_bytes: 2_500_000_000,
                children: vec![FolderNode {
                    path: r"D:\Apps\Rust".into(),
                    name: "Rust".into(),
                    direct_file_bytes: 0,
                    total_bytes: 2_500_000_000,
                    children: vec![FolderNode {
                        path: r"D:\Apps\Rust\cargo".into(),
                        name: "cargo".into(),
                        direct_file_bytes: 2_500_000_000,
                        total_bytes: 2_500_000_000,
                        children: vec![],
                    }],
                }],
            },
            FolderNode {
                path: r"D:\Games".into(),
                name: "Games".into(),
                direct_file_bytes: 0,
                total_bytes: 300_000_000,
                children: vec![
                    FolderNode {
                        path: r"D:\Games\Lossless.Scaling.v3.2.2_LinkNeverDie.Com".into(),
                        name: "Lossless.Scaling.v3.2.2_LinkNeverDie.Com".into(),
                        direct_file_bytes: 200_000_000,
                        total_bytes: 200_000_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"D:\Games\OtherGame".into(),
                        name: "OtherGame".into(),
                        direct_file_bytes: 100_000_000,
                        total_bytes: 100_000_000,
                        children: vec![],
                    },
                ],
            },
        ],
    };

    let zones = ZoneRulebook::default();
    let mut units = decompose_scanned_tree_with_options(
        &tree_c,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map),
        },
    );
    units.extend(decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map),
        },
    ));
    let report = attribute_scan_with_options(
        10,
        "2026-11-27 19:50:00".into(),
        5,
        vec![
            drive_fixture('C', 20_000_000, 100_000_000, true),
            drive_fixture('D', 2_800_000_000, 100_000_000, false),
        ],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            ..AttributionOptions::default()
        },
    );

    let ahk = report.apps.iter().find(|a| a.display_name == "AutoHotkey 2.0.19").unwrap();
    assert_eq!(ahk.total_bytes, 16_000_000);
    assert!(!report.apps.iter().any(|a| a.display_name == "v2" || a.display_name == "UX"));

    let gi = report.apps.iter().find(|a| a.display_name == "Microsoft GameInput").unwrap();
    assert_eq!(gi.total_bytes, 4_000_000);
    assert!(!report.apps.iter().any(|a| a.display_name == "x86"));

    let rust = report.apps.iter().find(|a| a.display_name == "Rust").unwrap();
    assert_eq!(rust.total_bytes, 2_500_000_000);
    assert_eq!(rust.bases, vec![AttributionBasis::ContainerRule]);
    assert!(rust.publishers.is_empty());

    let ls = report.apps.iter().find(|a| a.display_name == "Lossless Scaling v3.2.2").unwrap();
    assert_eq!(ls.total_bytes, 200_000_000);
    assert_eq!(ls.bases, vec![AttributionBasis::ContainerRule]);
    assert!(!report.apps.iter().any(|a| a.display_name.contains("LinkNeverDie")));

    assert_drive_diff_zero(&report, 'C');
    assert_drive_diff_zero(&report, 'D');
}
