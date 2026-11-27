use std::collections::HashMap;

use y_core::attribution::{AttributionOptions, attribute_scan_with_options};
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, InstalledApp, NoTouchCategory, RegistryDiscoveryStats,
    ScanReport, ZoneKind,
};
use y_core::tree::decompose_scanned_tree;

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
        last_scanned_unix: Some(1_795_800_000),
        last_scanned_display: Some("2026-11-28 02:40:00".into()),
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
    no_touch: Option<NoTouchCategory>,
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
        no_touch,
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
    assert_eq!(
        total, sys.drive_total_bytes,
        "reconciliation diff must be 0 on {drive}:\\"
    );
}

#[test]
fn a1_store_minecraft_uwp_backup_folder_merges_into_single_app() {
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
                        vec![dir_node(
                            r"C:\Users\Win\AppData\Local\Packages",
                            0,
                            vec![
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe",
                                    16_446,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_bak",
                                    62,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_old",
                                    10,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_backup",
                                    20,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_copy",
                                    30,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_archived",
                                    40,
                                    vec![],
                                ),
                            ],
                        )],
                    )],
                )],
            )],
        )],
    );

    let units = decompose_scanned_tree(&tree_c, &[], &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000, true)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let mc_apps: Vec<_> = report
        .apps
        .iter()
        .filter(|a| a.display_name == "Store: Microsoft.MinecraftUWP")
        .collect();
    assert_eq!(mc_apps.len(), 1, "expected 1 merged Store: Microsoft.MinecraftUWP row");
    let app = mc_apps[0];
    assert_eq!(app.id, "store:microsoft.minecraftuwp_8wekyb3d8bbwe");
    assert_eq!(app.items.len(), 6);
    assert_eq!(app.total_bytes, 16_446 + 62 + 10 + 20 + 30 + 40);
    assert!(app.items.iter().all(|it| it.zone == Some(ZoneKind::Red)));
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn a2_normal_two_segment_store_package_resolves_unchanged() {
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
                        vec![dir_node(
                            r"C:\Users\Win\AppData\Local\Packages",
                            0,
                            vec![dir_node(
                                r"C:\Users\Win\AppData\Local\Packages\MSTeams_8wekyb3d8bbwe",
                                16_384,
                                vec![],
                            )],
                        )],
                    )],
                )],
            )],
        )],
    );

    let units = decompose_scanned_tree(&tree_c, &[], &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', 16_384, 1_000_000, true)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let teams = report
        .apps
        .iter()
        .find(|a| a.id == "store:msteams_8wekyb3d8bbwe")
        .expect("MSTeams synthetic app");
    assert_eq!(teams.display_name, "Store: MSTeams");
    assert_eq!(teams.total_bytes, 16_384);
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn a3_windowsapps_five_segment_package_resolves_unchanged() {
    let tree_c = dir_node(
        r"C:\",
        0,
        vec![
            dir_node(
                r"C:\Program Files",
                0,
                vec![dir_node(
                    r"C:\Program Files\WindowsApps",
                    0,
                    vec![dir_node(
                        r"C:\Program Files\WindowsApps\MSTeams_25017.203.3370.1174_x64__8wekyb3d8bbwe",
                        279_412_182,
                        vec![],
                    )],
                )],
            ),
            dir_node(
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
                            vec![dir_node(
                                r"C:\Users\Win\AppData\Local\Packages",
                                0,
                                vec![dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\MSTeams_8wekyb3d8bbwe",
                                    16_384,
                                    vec![],
                                )],
                            )],
                        )],
                    )],
                )],
            ),
        ],
    );

    let units = decompose_scanned_tree(&tree_c, &[], &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', 279_428_566, 1_000_000, true)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let teams_rows: Vec<_> = report
        .apps
        .iter()
        .filter(|a| a.display_name == "Store: MSTeams")
        .collect();
    assert_eq!(teams_rows.len(), 1);
    assert_eq!(teams_rows[0].id, "store:msteams_8wekyb3d8bbwe");
    assert_eq!(teams_rows[0].total_bytes, 279_428_566);
    assert_eq!(teams_rows[0].items.len(), 2);
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn a4_same_stem_with_different_publisher_ids_stay_separate() {
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
                        vec![dir_node(
                            r"C:\Users\Win\AppData\Local\Packages",
                            0,
                            vec![
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Contoso.Widget_8wekyb3d8bbwe",
                                    10_000,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Local\Packages\Contoso.Widget_cw5n1h2txyewy",
                                    20_000,
                                    vec![],
                                ),
                            ],
                        )],
                    )],
                )],
            )],
        )],
    );

    let units = decompose_scanned_tree(&tree_c, &[], &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', 30_000, 1_000_000, true)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let a = report
        .apps
        .iter()
        .find(|x| x.id == "store:contoso.widget_8wekyb3d8bbwe")
        .expect("first publisher");
    let b = report
        .apps
        .iter()
        .find(|x| x.id == "store:contoso.widget_cw5n1h2txyewy")
        .expect("second publisher");
    assert_eq!(a.total_bytes, 10_000);
    assert_eq!(b.total_bytes, 20_000);
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn b1_five_minecraft_paths_route_to_bedrock_and_launcher_never_xbox_service() {
    let installed = vec![installed_app(
        "reg:xbox-service",
        "Xbox Service",
        Some("Microsoft Corporation"),
        None,
        Some(NoTouchCategory::Service),
    )];

    // C:\Users appears alphabetically before C:\XboxGames, mirroring live NTFS unit ordering.
    let tree_c = dir_node(
        r"C:\",
        0,
        vec![
            dir_node(
                r"C:\Users",
                0,
                vec![dir_node(
                    r"C:\Users\Win",
                    0,
                    vec![dir_node(
                        r"C:\Users\Win\AppData",
                        0,
                        vec![
                            dir_node(
                                r"C:\Users\Win\AppData\Local",
                                0,
                                vec![
                                    dir_node(
                                        r"C:\Users\Win\AppData\Local\MinecraftInstaller",
                                        36,
                                        vec![],
                                    ),
                                    dir_node(
                                        r"C:\Users\Win\AppData\Local\Packages",
                                        0,
                                        vec![
                                            dir_node(
                                                r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe",
                                                16_446,
                                                vec![],
                                            ),
                                            dir_node(
                                                r"C:\Users\Win\AppData\Local\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_bak",
                                                62,
                                                vec![],
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                            dir_node(
                                r"C:\Users\Win\AppData\Roaming",
                                0,
                                vec![
                                    dir_node(
                                        r"C:\Users\Win\AppData\Roaming\.minecraft",
                                        4_808_965_584,
                                        vec![],
                                    ),
                                    dir_node(
                                        r"C:\Users\Win\AppData\Roaming\Minecraft Bedrock",
                                        1_219_685_530,
                                        vec![],
                                    ),
                                ],
                            ),
                        ],
                    )],
                )],
            ),
            dir_node(
                r"C:\XboxGames",
                0,
                vec![
                    dir_node(
                        r"C:\XboxGames\Minecraft for Windows",
                        2_060_589_255,
                        vec![],
                    ),
                    dir_node(r"C:\XboxGames\Minecraft Launcher", 6_476_104, vec![]),
                ],
            ),
        ],
    );

    let units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let xbox = report.apps.iter().find(|a| a.display_name == "Xbox Service");
    assert!(
        xbox.is_none(),
        "Xbox Service must own zero Minecraft paths, got {:?}",
        xbox.map(|a| &a.items)
    );

    let bedrock = report
        .apps
        .iter()
        .find(|a| a.id == "store:microsoft.minecraftuwp_8wekyb3d8bbwe")
        .expect("Store: Microsoft.MinecraftUWP");
    assert_eq!(bedrock.display_name, "Store: Microsoft.MinecraftUWP");
    assert_eq!(bedrock.items.len(), 4);
    assert_eq!(
        bedrock.total_bytes,
        16_446 + 62 + 2_060_589_255 + 1_219_685_530
    );

    let launcher = report
        .apps
        .iter()
        .find(|a| a.display_name == "Minecraft Launcher")
        .expect("Minecraft Launcher");
    assert_eq!(launcher.items.len(), 3);
    assert_eq!(launcher.total_bytes, 4_808_965_584 + 6_476_104 + 36);
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn b2_other_xboxgames_title_still_becomes_xbox_game_leaf_entry() {
    let tree_c = dir_node(
        r"C:\",
        0,
        vec![dir_node(
            r"C:\XboxGames",
            0,
            vec![dir_node(r"C:\XboxGames\Forza Horizon 5", 50_000_000, vec![])],
        )],
    );

    let units = decompose_scanned_tree(&tree_c, &[], &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', 50_000_000, 1_000_000_000, true)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let fh5 = report
        .apps
        .iter()
        .find(|a| a.id == "xbox:forza horizon 5")
        .expect("Xbox Game entry");
    assert_eq!(fh5.display_name, "Xbox Game — forza horizon 5");
    assert_eq!(fh5.total_bytes, 50_000_000);
    assert_eq!(fh5.bases, vec![AttributionBasis::Token]);
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn b3_without_minecraft_launcher_dot_minecraft_does_not_land_on_xbox_service() {
    let installed = vec![installed_app(
        "reg:xbox-service",
        "Xbox Service",
        Some("Microsoft Corporation"),
        Some(r"C:\Program Files\XboxService"),
        Some(NoTouchCategory::Service),
    )];

    let tree_c = dir_node(
        r"C:\",
        0,
        vec![
            dir_node(
                r"C:\Program Files",
                0,
                vec![dir_node(r"C:\Program Files\XboxService", 850_668, vec![])],
            ),
            dir_node(
                r"C:\Users",
                0,
                vec![dir_node(
                    r"C:\Users\Win",
                    0,
                    vec![dir_node(
                        r"C:\Users\Win\AppData",
                        0,
                        vec![
                            dir_node(
                                r"C:\Users\Win\AppData\Local",
                                0,
                                vec![dir_node(
                                    r"C:\Users\Win\AppData\Local\MinecraftInstaller",
                                    36,
                                    vec![],
                                )],
                            ),
                            dir_node(
                                r"C:\Users\Win\AppData\Roaming",
                                0,
                                vec![dir_node(
                                    r"C:\Users\Win\AppData\Roaming\.minecraft",
                                    4_808_965_584,
                                    vec![],
                                )],
                            ),
                        ],
                    )],
                )],
            ),
        ],
    );

    let units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let xbox = report
        .apps
        .iter()
        .find(|a| a.display_name == "Xbox Service")
        .expect("Xbox Service");
    assert_eq!(xbox.total_bytes, 850_668);
    assert_eq!(xbox.items.len(), 1);
    assert!(!report.apps.iter().any(|a| a.display_name == "Minecraft Launcher"));
    assert!(report
        .unattributed
        .iter()
        .any(|u| u.path.eq_ignore_ascii_case(r"C:\Users\Win\AppData\Roaming\.minecraft")));
    assert!(report
        .unattributed
        .iter()
        .any(|u| u.path.eq_ignore_ascii_case(r"C:\Users\Win\AppData\Local\MinecraftInstaller")));
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn d1_with_ea_app_present_frostbite_and_origin_land_on_ea_app() {
    let installed = vec![
        installed_app(
            "reg:ea-app",
            "EA app",
            Some("Electronic Arts"),
            Some(r"C:\Program Files\Electronic Arts"),
            None,
        ),
        installed_app(
            "reg:ea-sports-fifa",
            "EA SPORTS™ FIFA 23",
            Some("Electronic Arts"),
            Some(r"D:\Games\Steam\steamapps\common\FIFA 23"),
            None,
        ),
    ];

    let tree_c = dir_node(
        r"C:\",
        0,
        vec![
            dir_node(
                r"C:\Program Files",
                0,
                vec![dir_node(
                    r"C:\Program Files\Electronic Arts",
                    678_450_152,
                    vec![],
                )],
            ),
            dir_node(
                r"C:\ProgramData",
                0,
                vec![
                    dir_node(r"C:\ProgramData\Frostbite", 50_000_000, vec![]),
                    dir_node(r"C:\ProgramData\Origin", 12_148, vec![]),
                ],
            ),
        ],
    );
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Games",
            0,
            vec![dir_node(
                r"D:\Games\Steam",
                0,
                vec![dir_node(
                    r"D:\Games\Steam\steamapps",
                    0,
                    vec![dir_node(
                        r"D:\Games\Steam\steamapps\common",
                        0,
                        vec![dir_node(
                            r"D:\Games\Steam\steamapps\common\FIFA 23",
                            107_199_710_486,
                            vec![],
                        )],
                    )],
                )],
            )],
        )],
    );

    let mut units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    units.extend(decompose_scanned_tree(&tree_d, &installed, &[], &[]));

    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![
            drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true),
            drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false),
        ],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let ea = report
        .apps
        .iter()
        .find(|a| a.id == "reg:ea-app")
        .expect("EA app");
    assert_eq!(ea.total_bytes, 678_450_152 + 50_000_000 + 12_148);
    assert!(ea
        .items
        .iter()
        .any(|it| it.path.eq_ignore_ascii_case(r"C:\ProgramData\Frostbite")));
    assert!(ea
        .items
        .iter()
        .any(|it| it.path.eq_ignore_ascii_case(r"C:\ProgramData\Origin")));

    let fifa = report
        .apps
        .iter()
        .find(|a| a.id == "reg:ea-sports-fifa")
        .expect("FIFA 23");
    assert_eq!(fifa.total_bytes, 107_199_710_486);
    assert_eq!(fifa.items.len(), 1);
    assert_drive_diff_zero(&report, 'C');
    assert_drive_diff_zero(&report, 'D');
}

#[test]
fn d2_without_ea_app_frostbite_and_origin_do_not_land_on_fifa_or_ea_sports() {
    let installed = vec![installed_app(
        "reg:ea-sports-fifa",
        "EA SPORTS™ FIFA 23",
        Some("Electronic Arts"),
        Some(r"D:\Games\Steam\steamapps\common\FIFA 23"),
        None,
    )];

    let tree_c = dir_node(
        r"C:\",
        0,
        vec![dir_node(
            r"C:\ProgramData",
            0,
            vec![
                dir_node(r"C:\ProgramData\Frostbite", 50_000_000, vec![]),
                dir_node(r"C:\ProgramData\Origin", 12_148, vec![]),
            ],
        )],
    );
    let tree_d = dir_node(
        r"D:\",
        0,
        vec![dir_node(
            r"D:\Games",
            0,
            vec![dir_node(
                r"D:\Games\Steam",
                0,
                vec![dir_node(
                    r"D:\Games\Steam\steamapps",
                    0,
                    vec![dir_node(
                        r"D:\Games\Steam\steamapps\common",
                        0,
                        vec![dir_node(
                            r"D:\Games\Steam\steamapps\common\FIFA 23",
                            107_199_710_486,
                            vec![],
                        )],
                    )],
                )],
            )],
        )],
    );

    let mut units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    units.extend(decompose_scanned_tree(&tree_d, &installed, &[], &[]));

    let report = attribute_scan_with_options(
        10,
        "2026-11-28 02:40:00".into(),
        1,
        vec![
            drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true),
            drive_fixture('D', tree_d.total_bytes, 1_000_000_000, false),
        ],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let fifa = report
        .apps
        .iter()
        .find(|a| a.id == "reg:ea-sports-fifa")
        .expect("FIFA 23");
    assert_eq!(fifa.total_bytes, 107_199_710_486);
    assert_eq!(fifa.items.len(), 1);
    assert!(report
        .unattributed
        .iter()
        .any(|u| u.path.eq_ignore_ascii_case(r"C:\ProgramData\Frostbite")));
    assert!(report
        .unattributed
        .iter()
        .any(|u| u.path.eq_ignore_ascii_case(r"C:\ProgramData\Origin")));
    assert_drive_diff_zero(&report, 'C');
    assert_drive_diff_zero(&report, 'D');
}
