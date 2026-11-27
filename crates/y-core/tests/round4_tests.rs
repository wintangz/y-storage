use std::collections::HashMap;
use std::io::Cursor;

use y_core::attribution::{
    AttributionOptions, RootExeCandidate, attribute_scan_with_options, merge_drive_rescan,
};
use y_core::config::{AppConfig, ContainerRulebook, DEFAULT_CONTAINER_NAMES, ZoneRuleKind};
use y_core::dependencies::{parse_steam_appmanifest_full, parse_steam_libraryfolders_vdf};
use y_core::history::{ScanHistory, ScanScopeKind, recompute_report_growth_from_history};
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, GrowthDelta, InstalledApp, RegistryDiscoveryStats,
    ScannedUnit, ScannedUnitKind,
};
use y_core::pe::{is_plausible_main_exe_name, parse_pe_version_product_name};
use y_core::tree::decompose_scanned_tree_with_containers;

fn drive_fixture(letter: char, used: u64, free: u64, is_sys: bool, ts: Option<(u64, &str)>) -> DriveInfo {
    DriveInfo {
        letter,
        root: format!("{letter}:\\"),
        total_bytes: used + free,
        free_bytes: free,
        used_bytes: used,
        is_system_drive: is_sys,
        full_walk_enabled: true,
        exclusions: if is_sys { vec![format!("{letter}:\\Windows")] } else { vec![] },
        last_scanned_unix: ts.map(|(u, _)| u),
        last_scanned_display: ts.map(|(_, s)| s.to_string()),
    }
}

fn empty_stats(canonical: usize) -> RegistryDiscoveryStats {
    RegistryDiscoveryStats {
        raw_display_name_entries: canonical,
        raw_with_install_location: canonical,
        canonical_apps: canonical,
        canonical_with_direct_install_location: canonical,
        canonical_with_inferred_install_location: 0,
    }
}

fn build_synthetic_pe_with_version_product_name(product_name: &str) -> Vec<u8> {
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

    let mut prod_val = Vec::new();
    push_utf16z(&mut prod_val, product_name);
    let prod_words = (prod_val.len() / 2) as u16;
    let str_blk = build_block("ProductName", prod_words, 1, &prod_val, &[]);
    let st_blk = build_block("040904b0", 0, 1, &[], &str_blk);
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
    // DataDirectory[2] (Resource) at opt + 112 + 16 = opt + 128
    buf[opt + 128..opt + 132].copy_from_slice(&0x2000u32.to_le_bytes());
    buf[opt + 132..opt + 136].copy_from_slice(&0x800u32.to_le_bytes());

    let sec = opt + 240;
    buf[sec + 8..sec + 12].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 12..sec + 16].copy_from_slice(&0x2000u32.to_le_bytes());
    buf[sec + 16..sec + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 20..sec + 24].copy_from_slice(&0x400u32.to_le_bytes());

    // Build 3-level RT_VERSION tree inside raw .rsrc at 0x400 (RVA 0x2000)
    // Level 1 (0x400): 1 ID entry (16 -> subdir 0x20)
    buf[0x40E..0x410].copy_from_slice(&1u16.to_le_bytes());
    buf[0x410..0x414].copy_from_slice(&16u32.to_le_bytes());
    buf[0x414..0x418].copy_from_slice(&0x8000_0020u32.to_le_bytes());

    // Level 2 (0x420): 1 ID entry (1 -> subdir 0x40)
    buf[0x42E..0x430].copy_from_slice(&1u16.to_le_bytes());
    buf[0x430..0x434].copy_from_slice(&1u32.to_le_bytes());
    buf[0x434..0x438].copy_from_slice(&0x8000_0040u32.to_le_bytes());

    // Level 3 (0x440): 1 ID entry (1033 -> data entry 0x60)
    buf[0x44E..0x450].copy_from_slice(&1u16.to_le_bytes());
    buf[0x450..0x454].copy_from_slice(&1033u32.to_le_bytes());
    buf[0x454..0x458].copy_from_slice(&0x0000_0060u32.to_le_bytes());

    // IMAGE_RESOURCE_DATA_ENTRY at 0x460 -> data RVA 0x2100 (file 0x500)
    buf[0x460..0x464].copy_from_slice(&0x2100u32.to_le_bytes());
    buf[0x464..0x468].copy_from_slice(&(vs_root.len() as u32).to_le_bytes());

    buf[0x500..0x500 + vs_root.len()].copy_from_slice(&vs_root);
    buf
}

#[test]
fn container_rule_claims_unowned_children_and_registry_beats_container_without_double_count() {
    let mut containers = ContainerRulebook::default();
    assert_eq!(containers.root_folder_names, DEFAULT_CONTAINER_NAMES);
    assert!(containers.add_explicit_path(r"D:\CustomSuite"));

    let installed = vec![InstalledApp {
        id: "reg:unity".into(),
        display_name: "Unity 6000.3.27f1".into(),
        display_version: "6000.3.27f1".into(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: vec!["Unity Technologies ApS".into()],
        direct_install_location: Some(r"Y:\Apps\Unity".into()),
        inferred_install_location: None,
        all_install_locations: vec![r"Y:\Apps\Unity".into()],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }];

    let tree_y = FolderNode {
        path: r"Y:\".into(),
        name: r"Y:\".into(),
        direct_file_bytes: 0,
        total_bytes: 1_500_000,
        children: vec![
            FolderNode {
                path: r"Y:\Apps".into(),
                name: "Apps".into(),
                direct_file_bytes: 0,
                total_bytes: 1_200_000,
                children: vec![
                    FolderNode {
                        path: r"Y:\Apps\Unity".into(),
                        name: "Unity".into(),
                        direct_file_bytes: 900_000,
                        total_bytes: 900_000,
                        children: vec![],
                    },
                    FolderNode {
                        path: r"Y:\Apps\Lossless.Scaling.v3.2.2_LinkNeverDie.Com".into(),
                        name: "Lossless.Scaling.v3.2.2_LinkNeverDie.Com".into(),
                        direct_file_bytes: 300_000,
                        total_bytes: 300_000,
                        children: vec![],
                    },
                ],
            },
            FolderNode {
                path: r"Y:\Repos".into(),
                name: "Repos".into(),
                direct_file_bytes: 300_000,
                total_bytes: 300_000,
                children: vec![],
            },
        ],
    };

    let mut units = decompose_scanned_tree_with_containers(&tree_y, &installed, &[], &[], &containers);
    units.push(ScannedUnit {
        path: r"D:\CustomSuite\ToolAlpha".into(),
        drive_letter: 'D',
        bytes: 45_000,
        kind: ScannedUnitKind::Regular,
        is_direct_install_root: false,
    });

    let report = attribute_scan_with_options(
        10,
        "2026-11-27 18:30:00".into(),
        10,
        vec![
            drive_fixture('D', 50_000, 50_000, false, Some((10, "2026-11-27 18:30:00"))),
            drive_fixture('Y', 1_500_000, 500_000, false, Some((10, "2026-11-27 18:30:00"))),
        ],
        empty_stats(1),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            containers: Some(&containers),
            steam_library_roots: &[],
            root_exe_candidates: None,
            ..AttributionOptions::default()
        },
    );

    // Registry owner keeps Y:\Apps\Unity and container rule NEVER double-claims it.
    let unity = report
        .apps
        .iter()
        .find(|a| a.display_name == "Unity 6000.3.27f1")
        .expect("Unity present");
    assert_eq!(unity.total_bytes, 900_000);
    assert_eq!(unity.bases, vec![AttributionBasis::InstallLocation]);
    assert_eq!(
        report
            .apps
            .iter()
            .filter(|a| a.items.iter().any(|i| i.path.eq_ignore_ascii_case(r"Y:\Apps\Unity")))
            .count(),
        1
    );

    // Unowned child inside Apps is claimed by container rule with cleaned name.
    let ls = report
        .apps
        .iter()
        .find(|a| a.display_name == "Lossless Scaling v3.2.2")
        .expect("container child claimed");
    assert_eq!(ls.total_bytes, 300_000);
    assert_eq!(ls.bases, vec![AttributionBasis::ContainerRule]);
    assert_eq!(ls.items[0].basis_label(), "layout rule: Apps");
    assert!(ls.publishers.is_empty());

    // Explicit path container rule claims child under D:\CustomSuite.
    let tool = report
        .apps
        .iter()
        .find(|a| a.display_name == "ToolAlpha")
        .expect("explicit path container child claimed");
    assert_eq!(tool.total_bytes, 45_000);
    assert_eq!(tool.items[0].basis_label(), r"layout rule: D:\CustomSuite");

    // Y:\Repos stays honestly Unattributed.
    assert_eq!(report.unattributed_total_bytes, 300_000);
    assert_eq!(report.unattributed[0].path, r"Y:\Repos");
}

#[test]
fn steam_vdf_and_appmanifest_discover_unregistered_steam_game() {
    let vdf = r#""libraryfolders"
{
    "0"
    {
        "path" "Z:\\Games\\Steam"
    }
    "1"
    {
        "path" "D:\\SteamLibrary"
    }
}"#;
    let libs = parse_steam_libraryfolders_vdf(vdf);
    assert_eq!(libs, vec![r"Z:\Games\Steam", r"D:\SteamLibrary"]);

    let acf = r#""AppState"
{
    "appid" "480"
    "name" "Spacewar"
    "installdir" "Spacewar"
}"#;
    let man = parse_steam_appmanifest_full(acf).unwrap();
    assert_eq!(man.appid, "480");
    assert_eq!(man.name, "Spacewar");
    assert_eq!(man.installdir, "Spacewar");

    let installed = vec![InstalledApp {
        id: format!("steam:app:{}", man.appid),
        display_name: man.name,
        display_version: String::new(),
        uninstall_guids: vec![],
        aliases: vec![],
        publishers: vec![],
        direct_install_location: Some(r"Z:\Games\Steam\steamapps\common\Spacewar".into()),
        inferred_install_location: None,
        all_install_locations: vec![r"Z:\Games\Steam\steamapps\common\Spacewar".into()],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }];
    let units = vec![ScannedUnit {
        path: r"Z:\Games\Steam\steamapps\common\Spacewar".into(),
        drive_letter: 'Z',
        bytes: 1_906_055,
        kind: ScannedUnitKind::Regular,
        is_direct_install_root: true,
    }];

    let report = attribute_scan_with_options(
        20,
        "2026-11-27 18:31:00".into(),
        5,
        vec![drive_fixture('Z', 2_000_000, 1_000_000, false, Some((20, "2026-11-27 18:31:00")))],
        empty_stats(1),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            containers: Some(&ContainerRulebook::default()),
            steam_library_roots: &libs,
            root_exe_candidates: None,
            ..AttributionOptions::default()
        },
    );
    let sw = report.apps.iter().find(|a| a.display_name == "Spacewar").unwrap();
    assert_eq!(sw.total_bytes, 1_906_055);
    assert_eq!(sw.bases, vec![AttributionBasis::SteamManifest]);
}

#[test]
fn executable_inference_pe_product_name_and_os_folder_exclusion_behave_conservatively() {
    let pe = build_synthetic_pe_with_version_product_name("Portable Sampler Pro");
    let prod = parse_pe_version_product_name(&mut Cursor::new(pe));
    assert_eq!(prod.as_deref(), Some("Portable Sampler Pro"));
    assert!(is_plausible_main_exe_name("sampler.exe"));
    assert!(!is_plausible_main_exe_name("unins000.exe"));
    assert!(!is_plausible_main_exe_name("vc_redist.x64.exe"));
    assert!(!is_plausible_main_exe_name("UnityCrashHandler64.exe"));

    let mut cands = HashMap::new();
    cands.insert(
        r"d:\portabletool".to_string(),
        RootExeCandidate {
            exe_path: r"D:\PortableTool\sampler.exe".into(),
            product_name: prod,
            company_name: None,
        },
    );

    let units = vec![
        ScannedUnit {
            path: r"D:\PortableTool".into(),
            drive_letter: 'D',
            bytes: 25_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: r"D:\Media".into(),
            drive_letter: 'D',
            bytes: 90_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: r"D:\$RECYCLE.BIN".into(),
            drive_letter: 'D',
            bytes: 12_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: r"D:\System Volume Information".into(),
            drive_letter: 'D',
            bytes: 8_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];

    let report = attribute_scan_with_options(
        30,
        "2026-11-27 18:32:00".into(),
        5,
        vec![drive_fixture('D', 150_000, 50_000, false, Some((30, "2026-11-27 18:32:00")))],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions {
            containers: Some(&ContainerRulebook::default()),
            steam_library_roots: &[],
            root_exe_candidates: Some(&cands),
            ..AttributionOptions::default()
        },
    );

    let exe_app = report
        .apps
        .iter()
        .find(|a| a.display_name == "Portable Sampler Pro")
        .expect("root executable app inferred with ProductName");
    assert_eq!(exe_app.total_bytes, 25_000);
    assert_eq!(exe_app.bases, vec![AttributionBasis::ExecutableScan]);
    assert_eq!(exe_app.items[0].basis_label(), "executable scan (inferred)");

    // D:\$RECYCLE.BIN and D:\System Volume Information stay in Windows & system, never apps or Unattributed.
    assert_eq!(report.windows_system[0].os_folders.len(), 2);
    assert_eq!(report.unattributed.len(), 1);
    assert_eq!(report.unattributed[0].path, r"D:\Media");
    assert_eq!(report.unattributed_total_bytes, 90_000);

    // Exact drive reconciliation.
    let apps_d = report.total_apps_bytes_on_drive('D');
    let un_d = report.total_unattributed_on_drive('D');
    let sys_d = report.windows_system[0].total_bytes;
    assert_eq!(apps_d + un_d + sys_d + 50_000, 200_000);
}

#[test]
fn per_drive_rescan_merges_cleanly_and_computes_deltas_only_against_same_drive_history() {
    let mut history = ScanHistory::default();

    // T=100: Initial C:\-only full scan. Must NEVER act as baseline for D:\-only apps!
    let r1_c = attribute_scan_with_options(
        100,
        "2026-11-27 10:00:00".into(),
        5,
        vec![
            drive_fixture('C', 50_000, 50_000, true, Some((100, "2026-11-27 10:00:00"))),
            DriveInfo {
                letter: 'D',
                root: r"D:\".into(),
                total_bytes: 100_000,
                free_bytes: 100_000,
                used_bytes: 0,
                is_system_drive: false,
                full_walk_enabled: false,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            },
        ],
        empty_stats(1),
        &[InstalledApp {
            id: "reg:c-app".into(),
            display_name: "CApp".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec![],
            direct_install_location: Some(r"C:\Apps\CApp".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"C:\Apps\CApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        }],
        &HashMap::new(),
        &[ScannedUnit {
            path: r"C:\Apps\CApp".into(),
            drive_letter: 'C',
            bytes: 10_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        }],
        None,
        &AttributionOptions::default(),
    );
    history.record_report_with_scope(&r1_c, ScanScopeKind::Full, &['C']);

    // T=200: User includes D:\ and scans D:\ only; CApp must keep 100 timestamp & data, DApp must be NoBaseline!
    let single_d1 = attribute_scan_with_options(
        200,
        "2026-11-27 11:00:00".into(),
        5,
        vec![drive_fixture('D', 30_000, 70_000, false, Some((200, "2026-11-27 11:00:00")))],
        empty_stats(1),
        &[InstalledApp {
            id: "reg:d-app".into(),
            display_name: "DApp".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec![],
            direct_install_location: Some(r"D:\Games\DApp".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"D:\Games\DApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        }],
        &HashMap::new(),
        &[ScannedUnit {
            path: r"D:\Games\DApp".into(),
            drive_letter: 'D',
            bytes: 20_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        }],
        None,
        &AttributionOptions::default(),
    );
    let merged_1 = merge_drive_rescan(&r1_c, 'D', single_d1, &history);
    history.record_report_with_scope(&merged_1, ScanScopeKind::Partial, &['D']);
    assert_eq!(history.snapshots.last().unwrap().scope_label(), r"Partial rescan (D:\)");

    let d_app_1 = merged_1.apps.iter().find(|a| a.display_name == "DApp").unwrap();
    assert_eq!(d_app_1.growth, GrowthDelta::NoBaseline, "first scan of D must never compare against C-only scan");
    assert_eq!(
        merged_1.drives.iter().find(|d| d.letter == 'C').unwrap().last_scanned_unix,
        Some(100)
    );
    assert_eq!(
        merged_1.drives.iter().find(|d| d.letter == 'D').unwrap().last_scanned_unix,
        Some(200)
    );

    // T=300: User rescans D:\ after DApp grew from 20_000 -> 25_000; CApp remains untouched, DApp shows +5_000!
    let single_d2 = attribute_scan_with_options(
        300,
        "2026-11-27 12:00:00".into(),
        5,
        vec![drive_fixture('D', 35_000, 65_000, false, Some((300, "2026-11-27 12:00:00")))],
        empty_stats(1),
        &[InstalledApp {
            id: "reg:d-app".into(),
            display_name: "DApp".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec![],
            direct_install_location: Some(r"D:\Games\DApp".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"D:\Games\DApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        }],
        &HashMap::new(),
        &[ScannedUnit {
            path: r"D:\Games\DApp".into(),
            drive_letter: 'D',
            bytes: 25_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        }],
        None,
        &AttributionOptions::default(),
    );
    let mut merged_2 = merge_drive_rescan(&merged_1, 'D', single_d2, &history);
    recompute_report_growth_from_history(&mut merged_2, &history);

    let c_app_2 = merged_2.apps.iter().find(|a| a.display_name == "CApp").unwrap();
    assert_eq!(c_app_2.total_bytes, 10_000);
    assert_eq!(c_app_2.growth, GrowthDelta::NoBaseline);

    let d_app_2 = merged_2.apps.iter().find(|a| a.display_name == "DApp").unwrap();
    assert_eq!(d_app_2.total_bytes, 25_000);
    assert_eq!(d_app_2.growth, GrowthDelta::Changed(5_000));
    assert_eq!(
        d_app_2.growth_for_drive('D'),
        GrowthDelta::Changed(5_000),
        "per_drive_growth must record D: delta independently"
    );

    // Scope staleness check
    let mut cfg = AppConfig::default();
    assert!(!r1_c.is_scope_stale(&cfg));
    cfg.zones
        .add_root_zone("PortableTools", ZoneRuleKind::AppContainer);
    assert!(merged_2.is_scope_stale(&cfg));
}

#[test]
fn excluded_drive_omits_apps_and_ignores_out_of_scope_units_and_install_locations() {
    let mut d_drv = drive_fixture('D', 100_000, 100_000, false, None);
    d_drv.full_walk_enabled = false;
    let drives = vec![
        drive_fixture('C', 50_000, 50_000, true, Some((100, "2026-11-27 18:00:00"))),
        d_drv,
    ];
    let installed = vec![
        InstalledApp {
            id: "reg:c-only".into(),
            display_name: "COnly".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["Pub".into()],
            direct_install_location: Some(r"C:\Program Files\COnly".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"C:\Program Files\COnly".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "reg:d-only".into(),
            display_name: "DOnly".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["EA".into()],
            direct_install_location: Some(r"D:\Games\FIFA 23".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"D:\Games\FIFA 23".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "reg:multi".into(),
            display_name: "MultiApp".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["Pub".into()],
            direct_install_location: Some(r"D:\Apps\MultiApp".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"D:\Apps\MultiApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
    ];
    // Even if stray D: units were supplied, attribution must discard excluded-drive units.
    let units = vec![
        ScannedUnit {
            path: r"C:\Program Files\COnly".into(),
            drive_letter: 'C',
            bytes: 12_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
        ScannedUnit {
            path: r"C:\Users\Win\AppData\Local\MultiApp".into(),
            drive_letter: 'C',
            bytes: 5_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: r"D:\Games\FIFA 23".into(),
            drive_letter: 'D',
            bytes: 100_000_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
    ];
    let report = attribute_scan_with_options(
        100,
        "2026-11-27 18:00:00".into(),
        10,
        drives,
        empty_stats(3),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    assert!(
        !report.apps.iter().any(|a| a.display_name == "DOnly"),
        "app whose locations all sit on excluded D: must be omitted"
    );
    let multi = report
        .apps
        .iter()
        .find(|a| a.display_name == "MultiApp")
        .expect("MultiApp keeps its in-scope C: footprint");
    assert_eq!(multi.total_bytes, 5_000);
    assert_eq!(multi.bytes_on_drive('C'), 5_000);
    assert_eq!(multi.bytes_on_drive('D'), 0);
    assert_eq!(multi.install_location, None, "excluded D: InstallLocation must not linger");
}
