use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use y_core::attribution::{AttributionOptions, RootExeCandidate, attribute_scan_with_options};
use y_core::categories::{CategoryCatalog, classify_app, classify_report_apps};
use y_core::config::ZoneRulebook;
use y_core::model::{
    AppCategory, AttributedApp, AttributionBasis, DriveInfo, FolderNode, GrowthDelta, InstalledApp,
    RegistryDiscoveryStats, ScanReport,
};
use y_core::pe::parse_pe_version_strings;
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
        last_scanned_unix: Some(1_795_820_000),
        last_scanned_display: Some("2026-11-28 03:15:00".into()),
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

fn build_synthetic_pe_with_version_strings(pairs: &[(&str, &str)]) -> Vec<u8> {
    fn push_utf16z(dst: &mut Vec<u8>, s: &str) {
        for u in s.encode_utf16() {
            dst.extend_from_slice(&u.to_le_bytes());
        }
        dst.extend_from_slice(&0u16.to_le_bytes());
    }
    fn align4_vec(v: &mut Vec<u8>) {
        while v.len() % 4 != 0 {
            v.push(0);
        }
    }
    fn build_block(key: &str, w_val_len: u16, w_type: u16, val_bytes: &[u8], children: &[u8]) -> Vec<u8> {
        let mut blk = vec![0u8; 6];
        blk[2..4].copy_from_slice(&w_val_len.to_le_bytes());
        blk[4..6].copy_from_slice(&w_type.to_le_bytes());
        push_utf16z(&mut blk, key);
        align4_vec(&mut blk);
        blk.extend_from_slice(val_bytes);
        align4_vec(&mut blk);
        blk.extend_from_slice(children);
        let len = blk.len() as u16;
        blk[0..2].copy_from_slice(&len.to_le_bytes());
        blk
    }

    let mut string_children = Vec::new();
    for (k, v) in pairs {
        let mut val_bytes = Vec::new();
        push_utf16z(&mut val_bytes, v);
        let words = (val_bytes.len() / 2) as u16;
        let sb = build_block(k, words, 1, &val_bytes, &[]);
        string_children.extend_from_slice(&sb);
        align4_vec(&mut string_children);
    }
    let st_blk = build_block("040904b0", 0, 1, &[], &string_children);
    let sfi_blk = build_block("StringFileInfo", 0, 1, &[], &st_blk);
    let fixed_info = vec![0u8; 52];
    let vs_root = build_block("VS_VERSION_INFO", 52, 0, &fixed_info, &sfi_blk);

    let mut buf = vec![0u8; 0x2000];
    buf[0..2].copy_from_slice(&0x5A4Du16.to_le_bytes());
    buf[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    buf[0x80..0x84].copy_from_slice(&0x0000_4550u32.to_le_bytes());
    buf[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    buf[0x94..0x96].copy_from_slice(&240u16.to_le_bytes());

    let opt = 0x80 + 24;
    buf[opt..opt + 2].copy_from_slice(&0x20Bu16.to_le_bytes());
    buf[opt + 128..opt + 132].copy_from_slice(&0x2000u32.to_le_bytes());
    buf[opt + 132..opt + 136].copy_from_slice(&0x800u32.to_le_bytes());

    let sec = opt + 240;
    buf[sec + 8..sec + 12].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 12..sec + 16].copy_from_slice(&0x2000u32.to_le_bytes());
    buf[sec + 16..sec + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 20..sec + 24].copy_from_slice(&0x400u32.to_le_bytes());

    // RT_VERSION (16) -> ID 1 -> Lang 1033 -> DataEntry
    buf[0x40E..0x410].copy_from_slice(&1u16.to_le_bytes());
    buf[0x410..0x414].copy_from_slice(&16u32.to_le_bytes());
    buf[0x414..0x418].copy_from_slice(&0x8000_0020u32.to_le_bytes());

    buf[0x42E..0x430].copy_from_slice(&1u16.to_le_bytes());
    buf[0x430..0x434].copy_from_slice(&1u32.to_le_bytes());
    buf[0x434..0x438].copy_from_slice(&0x8000_0040u32.to_le_bytes());

    buf[0x44E..0x450].copy_from_slice(&1u16.to_le_bytes());
    buf[0x450..0x454].copy_from_slice(&1033u32.to_le_bytes());
    buf[0x454..0x458].copy_from_slice(&0x0000_0060u32.to_le_bytes());

    buf[0x460..0x464].copy_from_slice(&0x2100u32.to_le_bytes());
    buf[0x464..0x468].copy_from_slice(&(vs_root.len() as u32).to_le_bytes());

    buf[0x500..0x500 + vs_root.len()].copy_from_slice(&vs_root);
    buf
}

#[test]
fn f1_registered_acme_player_reunites_acmeplayer9box_via_tier2() {
    let installed = vec![installed_app(
        "reg:acme-player",
        "Acme Player 14",
        Some("Acme Corp"),
        Some(r"D:\Apps\AcmePlayer"),
    )];
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\AcmePlayer", 1_500_000_000, vec![]),
                dir_node(r"D:\Apps\acmeplayer9box", 750_000_000, vec![]),
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
        "2026-11-28 03:15:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
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

    assert_eq!(report.apps.len(), 1, "companion folder must reunite into a single row");
    let acme = &report.apps[0];
    assert_eq!(acme.display_name, "Acme Player 14");
    assert_eq!(acme.total_bytes, 2_250_000_000);
    assert!(acme.bases.contains(&AttributionBasis::InstallLocation));
    assert!(acme.bases.contains(&AttributionBasis::CompanionReunion));
    let reunited = acme
        .items
        .iter()
        .find(|it| it.path.eq_ignore_ascii_case(r"D:\Apps\acmeplayer9box"))
        .expect("reunited item present");
    assert_eq!(reunited.basis, Some(AttributionBasis::CompanionReunion));
    let detail = reunited.basis_detail.as_deref().unwrap_or("");
    assert!(detail.contains("Tier 2"), "expected Tier 2 detail, got: {detail}");
    assert!(detail.contains("Acme Player 14"));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn f2_acmeplayer9box_without_registered_app_stays_standalone() {
    let installed: Vec<InstalledApp> = vec![];
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![dir_node(r"D:\Apps\acmeplayer9box", 750_000_000, vec![])],
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
        "2026-11-28 03:15:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
        empty_stats(0),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 1);
    assert_eq!(report.apps[0].display_name, "acmeplayer9box");
    assert_eq!(report.apps[0].bases, vec![AttributionBasis::ContainerRule]);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn f3_msteams_never_reunites_with_registered_steam() {
    let installed = vec![installed_app(
        "reg:steam",
        "Steam",
        Some("Valve Corporation"),
        Some(r"C:\Program Files (x86)\Steam"),
    )];
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![dir_node(r"D:\Apps\msteams", 250_000_000, vec![])],
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
        "2026-11-28 03:15:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
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

    assert!(
        report.apps.iter().find(|a| a.display_name == "Steam").is_none(),
        "Steam had 0 scanned units and must not absorb msteams"
    );
    let ms = report
        .apps
        .iter()
        .find(|a| a.display_name.eq_ignore_ascii_case("msteams"))
        .expect("msteams stays standalone");
    assert_eq!(ms.total_bytes, 250_000_000);
    assert_eq!(ms.bases, vec![AttributionBasis::ContainerRule]);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn f4_registered_steamvr_not_stolen_by_steam() {
    let installed = vec![
        installed_app(
            "reg:steam",
            "Steam",
            Some("Valve Corporation"),
            Some(r"D:\Games\Steam"),
        ),
        installed_app(
            "reg:steamvr",
            "SteamVR",
            Some("Valve Corporation"),
            Some(r"D:\Games\Steam\steamapps\common\SteamVR"),
        ),
    ];
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Games",
            0,
            vec![dir_node(
                r"D:\Games\Steam",
                400_000_000,
                vec![dir_node(
                    r"D:\Games\Steam\steamapps",
                    0,
                    vec![dir_node(
                        r"D:\Games\Steam\steamapps\common",
                        0,
                        vec![dir_node(r"D:\Games\Steam\steamapps\common\SteamVR", 900_000_000, vec![])],
                    )],
                )],
            )],
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
        "2026-11-28 03:15:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
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

    let steam = report.apps.iter().find(|a| a.display_name == "Steam").unwrap();
    let steamvr = report.apps.iter().find(|a| a.display_name == "SteamVR").unwrap();
    assert_eq!(steam.total_bytes, 400_000_000);
    assert_eq!(steamvr.total_bytes, 900_000_000);
    assert!(!steam.bases.contains(&AttributionBasis::CompanionReunion));

    // Verify separately with ONLY Steam registered and folder "D:\Apps\SteamVR" -> never Tier 2 matched
    let only_steam = vec![installed[0].clone()];
    let orphan_tree = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![dir_node(r"D:\Apps\SteamVR", 120_000_000, vec![])],
        )],
    );
    let orphan_units = decompose_scanned_tree_with_options(
        &orphan_tree,
        &only_steam,
        &[],
        &[],
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: None,
        },
    );
    let rep2 = attribute_scan_with_options(
        11,
        "2026-11-28 03:15:00".into(),
        5,
        vec![drive_fixture('D', orphan_tree.total_bytes, 1_000_000_000, false)],
        empty_stats(1),
        &only_steam,
        &HashMap::new(),
        &orphan_units,
        None,
        &AttributionOptions {
            zones: Some(&zones),
            ..AttributionOptions::default()
        },
    );
    let sv = rep2.apps.iter().find(|a| a.display_name == "SteamVR").unwrap();
    assert_eq!(sv.bases, vec![AttributionBasis::ContainerRule]);
    assert_drive_diff_zero(&report, 'D');
    assert_drive_diff_zero(&rep2, 'D');
}

#[test]
fn f5_ambiguous_shared_stem_foo_player_and_pro_refuses_reunion() {
    let installed = vec![
        installed_app(
            "reg:foo-player",
            "Foo Player",
            Some("Foo Software"),
            Some(r"D:\Apps\FooPlayer"),
        ),
        installed_app(
            "reg:foo-player-pro",
            "Foo Player Pro",
            Some("Foo Software"),
            Some(r"D:\Apps\FooPlayerPro"),
        ),
    ];
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\FooPlayer", 300_000_000, vec![]),
                dir_node(r"D:\Apps\FooPlayerPro", 400_000_000, vec![]),
                dir_node(r"D:\Apps\foobox", 150_000_000, vec![]),
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
        "2026-11-28 03:15:00".into(),
        5,
        vec![drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false)],
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

    assert_eq!(report.apps.len(), 3, "ambiguous shared stem must NOT reunite foobox");
    let foobox = report
        .apps
        .iter()
        .find(|a| a.display_name == "foobox")
        .expect("foobox remains standalone");
    assert_eq!(foobox.total_bytes, 150_000_000);
    assert_eq!(foobox.bases, vec![AttributionBasis::ContainerRule]);
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn f6_unknown_super_emulator_deluxe_categorises_to_games_via_keyword_rule() {
    let catalog = CategoryCatalog::parse_and_validate(y_core::categories::EMBEDDED_APP_CATEGORIES)
        .expect("embedded category catalog parses");
    let app = AttributedApp {
        id: "container:d:\\apps\\superemulatordeluxe".into(),
        display_name: "Super Emulator Deluxe".into(),
        publishers: vec![],
        install_location: Some(r"D:\Apps\SuperEmulatorDeluxe".into()),
        total_bytes: 500_000_000,
        previous_bytes: None,
        growth: GrowthDelta::NoBaseline,
        per_drive_growth: BTreeMap::new(),
        no_touch: None,
        bases: vec![AttributionBasis::ContainerRule],
        items: vec![],
        carved_out_apps: vec![],
        dependencies: vec![],
        display_icon: None,
        icon_rgba: None,
        category: AppCategory::Uncategorized,
        category_basis: String::new(),
    };

    let (cat, reason) = classify_app(&app, &catalog, |_| false);
    assert_eq!(cat, AppCategory::Games);
    assert!(
        reason.starts_with("rule:"),
        "expected generic keyword rule classification, got: {reason}"
    );
}

#[test]
fn f7_tier1_company_name_reunites_zxvm_with_quux_player_name_independently() {
    let pe_bytes = build_synthetic_pe_with_version_strings(&[
        ("CompanyName", "Quux Interactive Ltd."),
        ("ProductName", "ZX Virtualization Hypervisor"),
    ]);
    let ver = parse_pe_version_strings(&mut Cursor::new(pe_bytes));
    assert_eq!(ver.company_name.as_deref(), Some("Quux Interactive Ltd."));
    assert_eq!(ver.product_name.as_deref(), Some("ZX Virtualization Hypervisor"));

    let installed = vec![installed_app(
        "reg:quux-player",
        "Quux Player",
        Some("Quux Interactive, Ltd"),
        Some(r"D:\Apps\QuuxPlayer"),
    )];
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\zxvm".to_string(),
        RootExeCandidate {
            exe_path: r"D:\zxvm\zxvm_host.exe".into(),
            product_name: ver.product_name,
            company_name: ver.company_name,
        },
    );

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![
            dir_node(r"D:\Apps", 0, vec![dir_node(r"D:\Apps\QuuxPlayer", 1_000_000_000, vec![])]),
            dir_node(r"D:\zxvm", 550_000_000, vec![]),
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
    let mut report = attribute_scan_with_options(
        10,
        "2026-11-28 03:15:00".into(),
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
            ..AttributionOptions::default()
        },
    );
    classify_report_apps(&mut report, |_| false);

    assert_eq!(report.apps.len(), 1, "Tier 1 must reunite zxvm with Quux Player despite zero shared name stem");
    let quux = &report.apps[0];
    assert_eq!(quux.display_name, "Quux Player");
    assert_eq!(quux.total_bytes, 1_550_000_000);
    assert!(quux.bases.contains(&AttributionBasis::CompanionReunion));
    let item = quux
        .items
        .iter()
        .find(|it| it.path.eq_ignore_ascii_case(r"D:\zxvm"))
        .expect("zxvm attached to Quux Player");
    let detail = item.basis_detail.as_deref().unwrap_or("");
    assert!(detail.contains("Tier 1"), "expected Tier 1 detail, got: {detail}");
    assert!(detail.contains("Quux Player"));
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn tier1_contradiction_vetoes_tier2_stem_match() {
    let installed = vec![installed_app(
        "reg:acme-player",
        "Acme Player 14",
        Some("Acme Corp"),
        Some(r"D:\Apps\AcmePlayer"),
    )];
    let mut exe_map = HashMap::new();
    exe_map.insert(
        r"d:\apps\acmeplayer9box".to_string(),
        RootExeCandidate {
            exe_path: r"D:\Apps\acmeplayer9box\host.exe".into(),
            product_name: Some("Unrelated Tool".into()),
            company_name: Some("Rival Vendor Inc.".into()),
        },
    );

    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Apps",
            0,
            vec![
                dir_node(r"D:\Apps\AcmePlayer", 1_000_000_000, vec![]),
                dir_node(r"D:\Apps\acmeplayer9box", 200_000_000, vec![]),
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
        "2026-11-28 03:15:00".into(),
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
            ..AttributionOptions::default()
        },
    );

    assert_eq!(report.apps.len(), 2, "contradicting PE metadata must veto Tier 2 reunion");
    assert_drive_diff_zero(&report, 'D');
}
