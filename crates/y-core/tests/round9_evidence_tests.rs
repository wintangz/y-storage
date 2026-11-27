use std::collections::HashMap;
use std::path::PathBuf;

use y_core::attribution::{
    AppPathEvidence, AttributionOptions, EvidenceBundle, RootExeCandidate, ShortcutEvidence,
    attribute_scan_with_options,
};
use y_core::cache::{CachedExeEvidence, ExeEvidenceCache};
use y_core::config::ZoneRulebook;
use y_core::dependencies::WindowsServiceEntry;
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, InstalledApp, RegistryDiscoveryStats, ScanReport,
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
        last_scanned_unix: Some(1_795_830_000),
        last_scanned_display: Some("2026-11-28 04:45:00".into()),
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

fn installed_app(
    id: &str,
    name: &str,
    publisher: Option<&str>,
    install_loc: Option<&str>,
) -> InstalledApp {
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

fn dir_node(path: &str, direct_file_bytes: u64, children: Vec<FolderNode>) -> FolderNode {
    let name = path
        .trim_end_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or(path)
        .to_string();
    let child_sum: u64 = children.iter().map(|c| c.total_bytes).sum();
    FolderNode {
        path: path.to_string(),
        name,
        direct_file_bytes,
        total_bytes: direct_file_bytes + child_sum,
        children,
    }
}

fn assert_drive_diff_zero(report: &ScanReport, drive: char) {
    let drv = report
        .drives
        .iter()
        .find(|d| d.letter == drive)
        .expect("drive present");
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
        + drv.free_bytes;
    assert_eq!(total, drv.total_bytes, "reconciliation mismatch on {drive}:");
}

#[test]
fn e1_signer_and_product_claims_unrelated_name_folder() {
    let installed = vec![installed_app(
        "reg:quux-studio",
        "Quux Studio",
        Some("Quux Interactive Inc."),
        Some(r"D:\Apps\QuuxStudio"),
    )];
    let mut install_exes = HashMap::new();
    install_exes.insert(
        r"d:\apps\quuxstudio".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\QuuxStudio\quux.exe".into(),
            product_name: Some("Quux Studio".into()),
            company_name: Some("Quux Interactive Inc.".into()),
            signer_org: Some("Quux Interactive Inc.".into()),
        },
    );
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\z9_bin_drop".to_string(),
        RootExeCandidate {
            exe_path: r"D:\z9_bin_drop\render_worker.exe".into(),
            product_name: Some("Quux Studio".into()),
            company_name: None,
            signer_org: Some("Quux Interactive Inc.".into()),
        },
    );

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![
            dir_node(r"D:\Apps", 0, vec![dir_node(r"D:\Apps\QuuxStudio", 1_000_000_000, vec![])]),
            dir_node(r"D:\z9_bin_drop", 450_000_000, vec![]),
        ],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: Some(&install_exes),
                app_paths: &[],
                shortcuts: &[],
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 1);
    let quux = &report.apps[0];
    assert_eq!(quux.id, "reg:quux-studio");
    assert_eq!(quux.total_bytes, 1_450_000_000);
    assert!(quux.bases.contains(&AttributionBasis::Signature));
    let claimed = quux
        .items
        .iter()
        .find(|i| i.path.eq_ignore_ascii_case(r"D:\z9_bin_drop"))
        .expect("unrelated folder claimed");
    assert_eq!(claimed.basis, Some(AttributionBasis::Signature));
    let detail = claimed.basis_detail.as_deref().unwrap_or("");
    assert!(detail.contains("signature:"), "got detail: {detail}");
    assert!(detail.contains("Quux Interactive Inc."), "got detail: {detail}");
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e2_signer_only_with_different_product_does_not_claim() {
    let installed = vec![installed_app(
        "reg:quux-studio",
        "Quux Studio",
        Some("Quux Interactive Inc."),
        Some(r"D:\Apps\QuuxStudio"),
    )];
    let mut install_exes = HashMap::new();
    install_exes.insert(
        r"d:\apps\quuxstudio".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\QuuxStudio\quux.exe".into(),
            product_name: Some("Quux Studio".into()),
            company_name: Some("Quux Interactive Inc.".into()),
            signer_org: Some("Quux Interactive Inc.".into()),
        },
    );
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\z9_other_tool".to_string(),
        RootExeCandidate {
            exe_path: r"D:\z9_other_tool\other.exe".into(),
            product_name: Some("Completely Different Tool".into()),
            company_name: None,
            signer_org: Some("Quux Interactive Inc.".into()),
        },
    );

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![
            dir_node(r"D:\Apps", 0, vec![dir_node(r"D:\Apps\QuuxStudio", 1_000_000_000, vec![])]),
            dir_node(r"D:\z9_other_tool", 300_000_000, vec![]),
        ],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: Some(&install_exes),
                app_paths: &[],
                shortcuts: &[],
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 2, "signer-only must NOT claim different product");
    let quux = report.apps.iter().find(|a| a.id == "reg:quux-studio").unwrap();
    assert_eq!(quux.total_bytes, 1_000_000_000);
    assert!(!quux.bases.contains(&AttributionBasis::Signature));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e3_unsigned_exe_falls_through_unchanged() {
    let installed = vec![installed_app(
        "reg:quux-studio",
        "Quux Studio",
        Some("Quux Interactive Inc."),
        Some(r"D:\Apps\QuuxStudio"),
    )];
    let mut install_exes = HashMap::new();
    install_exes.insert(
        r"d:\apps\quuxstudio".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\QuuxStudio\quux.exe".into(),
            product_name: Some("Quux Studio".into()),
            company_name: Some("Quux Interactive Inc.".into()),
            signer_org: Some("Quux Interactive Inc.".into()),
        },
    );
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\portable_unsigned".to_string(),
        RootExeCandidate {
            exe_path: r"D:\portable_unsigned\app.exe".into(),
            product_name: Some("Unsigned Utility".into()),
            company_name: None,
            signer_org: None,
        },
    );

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![
            dir_node(r"D:\Apps", 0, vec![dir_node(r"D:\Apps\QuuxStudio", 1_000_000_000, vec![])]),
            dir_node(r"D:\portable_unsigned", 200_000_000, vec![]),
        ],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: Some(&install_exes),
                app_paths: &[],
                shortcuts: &[],
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 2);
    let standalone = report
        .apps
        .iter()
        .find(|a| a.display_name == "Unsigned Utility")
        .expect("unsigned exe promoted via normal Step 5");
    assert_eq!(standalone.bases, vec![AttributionBasis::ExecutableScan]);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e4_shortcut_target_claims_unrelated_folder() {
    let installed = vec![installed_app(
        "reg:quux-studio",
        "Quux Studio",
        Some("Quux Interactive Inc."),
        Some(r"D:\Apps\QuuxStudio"),
    )];
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\mystery_dir_a".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\mystery_dir_a\entry.exe".into(),
            product_name: None,
            company_name: None,
            signer_org: None,
        },
    );
    let shortcuts = vec![ShortcutEvidence {
        display_name: "Quux Studio".into(),
        target_path: r"D:\Apps\mystery_dir_a\entry.exe".into(),
        shortcut_path: r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Quux Studio.lnk"
            .into(),
    }];

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\QuuxStudio", 1_000_000_000, vec![]),
                dir_node(r"D:\Apps\mystery_dir_a", 320_000_000, vec![]),
            ],
        )],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: None,
                app_paths: &[],
                shortcuts: &shortcuts,
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 1);
    let quux = &report.apps[0];
    assert_eq!(quux.total_bytes, 1_320_000_000);
    assert!(quux.bases.contains(&AttributionBasis::Shortcut));
    let item = quux
        .items
        .iter()
        .find(|i| i.path.eq_ignore_ascii_case(r"D:\Apps\mystery_dir_a"))
        .expect("shortcut target folder claimed");
    assert_eq!(item.basis, Some(AttributionBasis::Shortcut));
    assert!(item.basis_detail.as_deref().unwrap_or("").contains("shortcut:"));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e5_app_paths_target_claims_unrelated_folder() {
    let mut app = installed_app(
        "reg:quux-studio",
        "Quux Studio",
        Some("Quux Interactive Inc."),
        Some(r"D:\Apps\QuuxStudio"),
    );
    app.display_icon = Some(r"D:\Apps\QuuxStudio\quuxstudio.exe".into());
    let installed = vec![app];

    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\mystery_dir_b".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\mystery_dir_b\quuxstudio.exe".into(),
            product_name: None,
            company_name: None,
            signer_org: None,
        },
    );
    let app_paths = vec![AppPathEvidence {
        exe_name: "quuxstudio.exe".into(),
        target_path: r"D:\Apps\mystery_dir_b\quuxstudio.exe".into(),
    }];

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\QuuxStudio", 1_000_000_000, vec![]),
                dir_node(r"D:\Apps\mystery_dir_b", 280_000_000, vec![]),
            ],
        )],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: None,
                app_paths: &app_paths,
                shortcuts: &[],
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 1);
    let quux = &report.apps[0];
    assert_eq!(quux.total_bytes, 1_280_000_000);
    assert!(quux.bases.contains(&AttributionBasis::AppPaths));
    let item = quux
        .items
        .iter()
        .find(|i| i.path.eq_ignore_ascii_case(r"D:\Apps\mystery_dir_b"))
        .expect("App Paths target folder claimed");
    assert_eq!(item.basis, Some(AttributionBasis::AppPaths));
    assert!(item.basis_detail.as_deref().unwrap_or("").contains("App Paths:"));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e6_equal_grade_conflict_between_two_apps_makes_no_claim() {
    let installed = vec![
        installed_app("reg:alpha-tool", "Alpha Tool", Some("Vendor A"), Some(r"D:\Apps\AlphaTool")),
        installed_app("reg:beta-tool", "Beta Tool", Some("Vendor B"), Some(r"D:\Apps\BetaTool")),
    ];
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\shared_drop".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\shared_drop\shared.exe".into(),
            product_name: None,
            company_name: None,
            signer_org: None,
        },
    );
    let shortcuts = vec![
        ShortcutEvidence {
            display_name: "Alpha Tool".into(),
            target_path: r"D:\Apps\shared_drop\shared.exe".into(),
            shortcut_path: r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Alpha.lnk".into(),
        },
        ShortcutEvidence {
            display_name: "Beta Tool".into(),
            target_path: r"D:\Apps\shared_drop\shared.exe".into(),
            shortcut_path: r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Beta.lnk".into(),
        },
    ];

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\AlphaTool", 500_000_000, vec![]),
                dir_node(r"D:\Apps\BetaTool", 500_000_000, vec![]),
                dir_node(r"D:\Apps\shared_drop", 250_000_000, vec![]),
            ],
        )],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: None,
                app_paths: &[],
                shortcuts: &shortcuts,
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 3, "equal-grade conflict must NOT claim shared_drop");
    let shared = report
        .apps
        .iter()
        .find(|a| a.id == r"container:d:\apps\shared_drop")
        .expect("shared_drop remains standalone");
    assert_eq!(shared.total_bytes, 250_000_000);
    assert_eq!(shared.bases, vec![AttributionBasis::ContainerRule]);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e7_msteams_steam_and_steamvr_regression_guards() {
    let installed = vec![
        installed_app("reg:steam", "Steam", Some("Valve Corporation"), Some(r"D:\Steam")),
        installed_app(
            "reg:steamvr",
            "SteamVR",
            Some("Valve Corporation"),
            Some(r"D:\Steam\steamapps\common\SteamVR"),
        ),
    ];
    let mut install_exes = HashMap::new();
    install_exes.insert(
        r"d:\steam".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Steam\steam.exe".into(),
            product_name: Some("Steam".into()),
            company_name: Some("Valve Corporation".into()),
            signer_org: Some("Valve".into()),
        },
    );
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\msteams".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\msteams\ms-teams.exe".into(),
            product_name: Some("Microsoft Teams".into()),
            company_name: Some("Microsoft Corporation".into()),
            signer_org: Some("Microsoft Corporation".into()),
        },
    );

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![
            dir_node(
                r"D:\Steam",
                400_000_000,
                vec![dir_node(
                    r"D:\Steam\steamapps",
                    0,
                    vec![dir_node(
                        r"D:\Steam\steamapps\common",
                        0,
                        vec![dir_node(r"D:\Steam\steamapps\common\SteamVR", 900_000_000, vec![])],
                    )],
                )],
            ),
            dir_node(r"D:\Apps", 0, vec![dir_node(r"D:\Apps\msteams", 279_428_566, vec![])]),
        ],
    );
    let zones = ZoneRulebook::default();
    let units = decompose_scanned_tree_with_options(
        &tree_d,
        &installed,
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: Some(&install_exes),
                app_paths: &[],
                shortcuts: &[],
            },
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 3);
    let steam = report.apps.iter().find(|a| a.display_name == "Steam").unwrap();
    let steamvr = report.apps.iter().find(|a| a.display_name == "SteamVR").unwrap();
    let msteams = report.apps.iter().find(|a| a.display_name == "msteams").unwrap();
    assert_eq!(steam.total_bytes, 400_000_000);
    assert_eq!(steamvr.total_bytes, 900_000_000);
    assert_eq!(msteams.total_bytes, 279_428_566);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e8_medium_alone_does_not_claim_but_medium_plus_independent_weak_does() {
    let installed = vec![installed_app(
        "reg:acme-player",
        "Acme Player 14",
        Some("Acme Corp"),
        Some(r"D:\Apps\AcmePlayer"),
    )];
    let zones = ZoneRulebook::default();

    // Part 1: 1 MEDIUM (VersionResource) alone on unrelated-leaf folder must NOT claim.
    let mut exe_map_alone = HashMap::new();
    exe_map_alone.insert(
        r"d:\apps\unrelated_box".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\unrelated_box\helper.exe".into(),
            product_name: Some("Acme Player Helper".into()),
            company_name: Some("Acme Corporation".into()),
            signer_org: None,
        },
    );
    let tree_alone = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\AcmePlayer", 1_000_000_000, vec![]),
                dir_node(r"D:\Apps\unrelated_box", 150_000_000, vec![]),
            ],
        )],
    );
    let units_alone = decompose_scanned_tree_with_options(
        &tree_alone,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map_alone),
        },
    );
    let rep_alone = attribute_scan_with_options(
        10,
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_alone.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units_alone,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map_alone),
            ..AttributionOptions::default()
        },
    );
    let unrelated = rep_alone
        .apps
        .iter()
        .find(|a| a.id == r"container:d:\apps\unrelated_box")
        .expect("1 medium alone must NOT claim unrelated_box");
    assert_eq!(unrelated.bases, vec![AttributionBasis::ContainerRule]);

    // Part 2: MEDIUM (VersionResource) + independent WEAK (Stem) DOES claim with VersionResource.
    let mut exe_map_combo = HashMap::new();
    exe_map_combo.insert(
        r"d:\apps\acmeplayer9box".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\acmeplayer9box\helper.exe".into(),
            product_name: Some("Acme Player Helper".into()),
            company_name: Some("Acme Corporation".into()),
            signer_org: None,
        },
    );
    let tree_combo = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\AcmePlayer", 1_000_000_000, vec![]),
                dir_node(r"D:\Apps\acmeplayer9box", 250_000_000, vec![]),
            ],
        )],
    );
    let units_combo = decompose_scanned_tree_with_options(
        &tree_combo,
        &installed,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: Some(&exe_map_combo),
        },
    );
    let rep_combo = attribute_scan_with_options(
        10,
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_combo.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units_combo,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map_combo),
            ..AttributionOptions::default()
        },
    );
    assert_eq!(rep_combo.apps.len(), 1);
    let acme = &rep_combo.apps[0];
    assert_eq!(acme.total_bytes, 1_250_000_000);
    assert!(acme.bases.contains(&AttributionBasis::VersionResource));
    let item = acme
        .items
        .iter()
        .find(|i| i.path.eq_ignore_ascii_case(r"D:\Apps\acmeplayer9box"))
        .unwrap();
    let detail = item.basis_detail.as_deref().unwrap_or("");
    assert!(detail.contains("version resource:"), "got: {detail}");
    assert!(detail.contains("stem match"), "got: {detail}");
    assert_drive_diff_zero(&rep_combo, 'D');
}

#[test]
fn e9_evidence_beats_curated_table_on_conflict_and_records_conflict_in_report() {
    let installed = vec![
        installed_app("reg:discord", "Discord", Some("Discord Inc."), Some(r"D:\Apps\DiscordApp")),
        installed_app("reg:my-chat", "My Custom Chat", Some("Custom Chat Corp"), Some(r"D:\Apps\MyCustomChat")),
    ];
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"c:\users\win\appdata\local\discord".to_string(),
        RootExeCandidate {
            exe_path: r"C:\Users\Win\AppData\Local\Discord\chat.exe".into(),
            product_name: None,
            company_name: None,
            signer_org: None,
        },
    );
    let shortcuts = vec![ShortcutEvidence {
        display_name: "My Custom Chat".into(),
        target_path: r"C:\Users\Win\AppData\Local\Discord\chat.exe".into(),
        shortcut_path: r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\My Custom Chat.lnk"
            .into(),
    }];

    let tree_c = dir_node(
        r"C:\",
        0,
        vec![dir_node(
            r"C:\Users",
            0,
            vec![dir_node(
                r"C:\Users\Win",
                0,
                vec![dir_node(
                    r"C:\Users\Win\AppData",
                    0,
                    vec![dir_node(
                        r"C:\Users\Win\AppData\Local",
                        0,
                        vec![dir_node(r"C:\Users\Win\AppData\Local\Discord", 180_000_000, vec![])],
                    )],
                )],
            )],
        )],
    );
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\DiscordApp", 400_000_000, vec![]),
                dir_node(r"D:\Apps\MyCustomChat", 600_000_000, vec![]),
            ],
        )],
    );
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![
            drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true),
            drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false),
        ],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            root_exe_candidates: Some(&exe_map),
            evidence: EvidenceBundle {
                install_root_exes: None,
                app_paths: &[],
                shortcuts: &shortcuts,
            },
            ..AttributionOptions::default()
        },
    );

    let chat = report.apps.iter().find(|a| a.id == "reg:my-chat").unwrap();
    assert_eq!(chat.total_bytes, 780_000_000);
    assert!(chat.bases.contains(&AttributionBasis::Shortcut));
    assert_eq!(report.evidence_curated_conflicts.len(), 1);
    let c = &report.evidence_curated_conflicts[0];
    assert_eq!(c.evidence_app_id, "reg:my-chat");
    assert_eq!(c.curated_app_id, "reg:discord");
    assert_eq!(c.evidence_basis, AttributionBasis::Shortcut);
    assert_drive_diff_zero(&report, 'C');
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e10_service_image_path_strong_and_medium_rules() {
    let mut app = installed_app(
        "reg:quux-studio",
        "Quux Studio",
        Some("Quux Interactive Inc."),
        Some(r"D:\Apps\QuuxStudio"),
    );
    app.display_icon = Some(r"D:\Apps\svc_bin\quux_svc.exe".into());
    let installed = vec![app];

    let services = vec![
        WindowsServiceEntry {
            service_name: "QuuxWorker".into(),
            display_name: "Quux Background Worker".into(),
            service_type: 16,
            raw_image_path: r"D:\Apps\svc_bin\quux_svc.exe".into(),
            normalized_image_path: r"D:\Apps\svc_bin\quux_svc.exe".into(),
            depend_on_service: vec![],
        },
        WindowsServiceEntry {
            service_name: "QuuxMon".into(),
            display_name: "Quux Studio Monitor".into(),
            service_type: 16,
            raw_image_path: r"D:\Apps\unrelated_svc\mon.exe".into(),
            normalized_image_path: r"D:\Apps\unrelated_svc\mon.exe".into(),
            depend_on_service: vec![],
        },
        WindowsServiceEntry {
            service_name: "QuuxRt".into(),
            display_name: "Quux Studio Service".into(),
            service_type: 16,
            raw_image_path: r"D:\Apps\quuxstudio9runtime\rt.exe".into(),
            normalized_image_path: r"D:\Apps\quuxstudio9runtime\rt.exe".into(),
            depend_on_service: vec![],
        },
    ];

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\QuuxStudio", 1_000_000_000, vec![]),
                dir_node(r"D:\Apps\svc_bin", 100_000_000, vec![]),
                dir_node(r"D:\Apps\unrelated_svc", 200_000_000, vec![]),
                dir_node(r"D:\Apps\quuxstudio9runtime", 300_000_000, vec![]),
            ],
        )],
    );
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
        "2026-11-28 04:45:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            services: &services,
            ..AttributionOptions::default()
        },
    );

    let quux = report.apps.iter().find(|a| a.id == "reg:quux-studio").unwrap();
    assert!(quux.items.iter().any(|i| {
        i.path.eq_ignore_ascii_case(r"D:\Apps\svc_bin")
            && i.basis == Some(AttributionBasis::ServiceRegistration)
    }));
    assert!(quux.items.iter().any(|i| {
        i.path.eq_ignore_ascii_case(r"D:\Apps\quuxstudio9runtime")
            && i.basis == Some(AttributionBasis::ServiceRegistration)
    }));
    assert!(!quux.items.iter().any(|i| i.path.eq_ignore_ascii_case(r"D:\Apps\unrelated_svc")));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn e11_exe_evidence_cache_hits_and_invalidates_on_size_or_mtime_change() {
    let tmp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-tmp/round9-evidence-cache");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let cache_path = tmp_dir.join("evidence_cache.json");

    let mut cache = ExeEvidenceCache::default();
    cache.upsert(
        r"d:\apps\quuxstudio\quux.exe".into(),
        CachedExeEvidence {
            size: 123_456,
            mtime_unix_secs: 1_700_000_000,
            mtime_subsec_nanos: 42,
            signer_org: Some("Quux Interactive Inc.".into()),
            company_name: Some("Quux Interactive Inc.".into()),
            product_name: Some("Quux Studio".into()),
            file_description: Some("Quux Studio".into()),
        },
    );
    cache.save_atomic(&cache_path).unwrap();

    let loaded = ExeEvidenceCache::load_or_default(&cache_path);
    let hit = loaded
        .get_fresh(r"d:\apps\quuxstudio\quux.exe", 123_456, 1_700_000_000, 42)
        .expect("exact key+size+mtime hits");
    assert_eq!(hit.signer_org.as_deref(), Some("Quux Interactive Inc."));
    assert!(loaded.get_fresh(r"d:\apps\quuxstudio\quux.exe", 123_457, 1_700_000_000, 42).is_none());
    assert!(loaded.get_fresh(r"d:\apps\quuxstudio\quux.exe", 123_456, 1_700_000_001, 42).is_none());
    assert!(loaded.get_fresh(r"d:\apps\quuxstudio\quux.exe", 123_456, 1_700_000_000, 99).is_none());

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
