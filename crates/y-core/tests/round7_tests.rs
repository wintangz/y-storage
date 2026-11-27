use std::collections::HashMap;

use y_core::attribution::{AttributionOptions, attribute_scan_with_options};
use y_core::dependencies::WindowsServiceEntry;
use y_core::discovery::build_registry_catalog_with_services;
use y_core::model::{
    DriveInfo, FolderNode, InstalledApp, NoTouchCategory, RawRegistryApp, RegistryDiscoveryStats,
    ScanReport,
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
        last_scanned_unix: Some(1_795_810_000),
        last_scanned_display: Some("2026-11-28 03:00:00".into()),
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
    version: &str,
    publisher: Option<&str>,
    install_loc: Option<&str>,
    uninstall_guid: Option<&str>,
) -> InstalledApp {
    InstalledApp {
        id: id.to_string(),
        display_name: name.to_string(),
        display_version: version.to_string(),
        uninstall_guids: uninstall_guid.into_iter().map(str::to_string).collect(),
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
fn e1_adobe_product_subfolder_lands_on_installed_adobe_product_app() {
    let installed = vec![installed_app(
        "reg:adobe-photoshop",
        "Adobe Photoshop 2024",
        "25.0",
        Some("Adobe Inc."),
        Some(r"C:\Program Files\Adobe\Adobe Photoshop 2024"),
        None,
    )];

    let tree_c = dir_node(
        r"C:\",
        0,
        vec![
            dir_node(
                r"C:\Program Files",
                0,
                vec![
                    dir_node(
                        r"C:\Program Files\Adobe",
                        0,
                        vec![dir_node(
                            r"C:\Program Files\Adobe\Adobe Photoshop 2024",
                            2_000_000_000,
                            vec![],
                        )],
                    ),
                    dir_node(
                        r"C:\Program Files\Common Files",
                        0,
                        vec![dir_node(
                            r"C:\Program Files\Common Files\Adobe",
                            0,
                            vec![
                                dir_node(
                                    r"C:\Program Files\Common Files\Adobe\Photoshop",
                                    50_000_000,
                                    vec![],
                                ),
                                dir_node(
                                    r"C:\Program Files\Common Files\Adobe\CEP",
                                    10_000_000,
                                    vec![],
                                ),
                            ],
                        )],
                    ),
                ],
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
                            r"C:\Users\Win\AppData\Roaming",
                            0,
                            vec![dir_node(
                                r"C:\Users\Win\AppData\Roaming\Adobe",
                                0,
                                vec![
                                    dir_node(
                                        r"C:\Users\Win\AppData\Roaming\Adobe\Adobe Photoshop 2024",
                                        80_000_000,
                                        vec![],
                                    ),
                                    dir_node(
                                        r"C:\Users\Win\AppData\Roaming\Adobe\UXP",
                                        20_000_000,
                                        vec![],
                                    ),
                                ],
                            )],
                        )],
                    )],
                )],
            ),
        ],
    );

    let units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 03:00:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let ps = report
        .apps
        .iter()
        .find(|a| a.display_name == "Adobe Photoshop 2024")
        .expect("Photoshop app");
    assert_eq!(ps.total_bytes, 2_000_000_000 + 50_000_000 + 80_000_000);
    assert!(ps
        .items
        .iter()
        .any(|it| it.path.eq_ignore_ascii_case(r"C:\Program Files\Common Files\Adobe\Photoshop")));
    assert!(ps.items.iter().any(|it| {
        it.path
            .eq_ignore_ascii_case(r"C:\Users\Win\AppData\Roaming\Adobe\Adobe Photoshop 2024")
    }));

    let pool = report
        .apps
        .iter()
        .find(|a| a.id == "app:adobe-shared-data")
        .expect("Adobe shared data pool");
    assert_eq!(pool.display_name, "Adobe (shared data)");
    assert_eq!(pool.total_bytes, 30_000_000);
    assert!(!report
        .apps
        .iter()
        .any(|a| a.display_name == "Adobe Creative Cloud"));
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn e2_unidentifiable_adobe_residue_pools_under_shared_family_label_not_product_name() {
    let tree_c = dir_node(
        r"C:\",
        0,
        vec![
            dir_node(
                r"C:\Program Files (x86)",
                0,
                vec![dir_node(
                    r"C:\Program Files (x86)\Common Files",
                    0,
                    vec![dir_node(
                        r"C:\Program Files (x86)\Common Files\Adobe",
                        152_212_941,
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
                        vec![
                            dir_node(
                                r"C:\Users\Win\AppData\Roaming",
                                0,
                                vec![
                                    dir_node(
                                        r"C:\Users\Win\AppData\Roaming\Adobe",
                                        148_754_220,
                                        vec![],
                                    ),
                                    dir_node(
                                        r"C:\Users\Win\AppData\Roaming\com.adobe.dunamis",
                                        42_064_409,
                                        vec![],
                                    ),
                                ],
                            ),
                            dir_node(
                                r"C:\Users\Win\AppData\Local",
                                0,
                                vec![dir_node(
                                    r"C:\Users\Win\AppData\Local\Adobe",
                                    11_391_219,
                                    vec![],
                                )],
                            ),
                        ],
                    )],
                )],
            ),
        ],
    );

    let units = decompose_scanned_tree(&tree_c, &[], &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 03:00:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true)],
        empty_stats(0),
        &[],
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let pool = report
        .apps
        .iter()
        .find(|a| a.id == "app:adobe-shared-data")
        .expect("Adobe shared data pool");
    assert!(
        pool.display_name.to_ascii_lowercase().contains("shared"),
        "pool label must include 'shared', got {:?}",
        pool.display_name
    );
    assert_eq!(pool.publishers, vec!["Adobe Inc.".to_string()]);
    assert_eq!(
        pool.total_bytes,
        152_212_941 + 148_754_220 + 42_064_409 + 11_391_219
    );
    assert!(!report
        .apps
        .iter()
        .any(|a| a.display_name == "Adobe Creative Cloud"));
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn e3_registered_adobe_creative_cloud_receives_residue_instead_of_synthetic_pool() {
    let installed = vec![
        installed_app(
            "reg:adobe-creative-cloud",
            "Adobe Creative Cloud",
            "6.4",
            Some("Adobe Inc."),
            Some(r"C:\Program Files\Adobe\Adobe Creative Cloud"),
            None,
        ),
        installed_app(
            "reg:adobe-premiere-pro",
            "Adobe Premiere Pro 2024",
            "24.0",
            Some("Adobe Inc."),
            Some(r"C:\Program Files\Adobe\Adobe Premiere Pro 2024"),
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
                    r"C:\Program Files\Adobe",
                    0,
                    vec![
                        dir_node(
                            r"C:\Program Files\Adobe\Adobe Creative Cloud",
                            300_000_000,
                            vec![],
                        ),
                        dir_node(
                            r"C:\Program Files\Adobe\Adobe Premiere Pro 2024",
                            3_000_000_000,
                            vec![],
                        ),
                    ],
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
                            r"C:\Users\Win\AppData\Roaming",
                            0,
                            vec![
                                dir_node(
                                    r"C:\Users\Win\AppData\Roaming\Adobe",
                                    0,
                                    vec![
                                        dir_node(
                                            r"C:\Users\Win\AppData\Roaming\Adobe\Premiere Pro",
                                            45_000_000,
                                            vec![],
                                        ),
                                        dir_node(
                                            r"C:\Users\Win\AppData\Roaming\Adobe\UXP",
                                            25_000_000,
                                            vec![],
                                        ),
                                    ],
                                ),
                                dir_node(
                                    r"C:\Users\Win\AppData\Roaming\com.adobe.dunamis",
                                    15_000_000,
                                    vec![],
                                ),
                            ],
                        )],
                    )],
                )],
            ),
        ],
    );

    let units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 03:00:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &HashMap::new(),
        &units,
        None,
        &AttributionOptions::default(),
    );

    let pr = report
        .apps
        .iter()
        .find(|a| a.display_name == "Adobe Premiere Pro 2024")
        .expect("Premiere Pro");
    assert_eq!(pr.total_bytes, 3_045_000_000);

    let cc = report
        .apps
        .iter()
        .find(|a| a.id == "reg:adobe-creative-cloud")
        .expect("registered Creative Cloud");
    assert_eq!(cc.total_bytes, 300_000_000 + 25_000_000 + 15_000_000);
    assert!(!report
        .apps
        .iter()
        .any(|a| a.id == "app:adobe-shared-data"));
    assert_drive_diff_zero(&report, 'C');
}

#[test]
fn f1_widget_service_without_service_registration_is_not_no_touch() {
    let raw = vec![RawRegistryApp {
        hive_view: "HKLM-64".into(),
        subkey_name: "WidgetService".into(),
        display_name: "Widget Service".into(),
        display_version: "1.0.0".into(),
        publisher: "Contoso Ltd.".into(),
        install_location: r"C:\Program Files\Contoso\Widget Service".into(),
        uninstall_string: String::new(),
        display_icon: String::new(),
        system_component: false,
        parent_key_name: String::new(),
        release_type: String::new(),
    }];

    let cat = build_registry_catalog_with_services(&raw, None, &[]);
    assert_eq!(cat.apps.len(), 1);
    assert_eq!(
        cat.apps[0].no_touch, None,
        "Widget Service without Windows service registration must NOT be NO-TOUCH"
    );
}

#[test]
fn f2_app_with_real_windows_service_registration_is_no_touch_service_without_false_positives() {
    let raw = vec![
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "WidgetService".into(),
            display_name: "Widget Service".into(),
            display_version: "1.0.0".into(),
            publisher: "Contoso Ltd.".into(),
            install_location: r"C:\Program Files\Contoso\Widget Service".into(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "AcmeTelemetryAgent".into(),
            display_name: "Acme Telemetry Agent".into(),
            display_version: "2.1.0".into(),
            publisher: "Acme Corp".into(),
            install_location: r"C:\Program Files\Acme\Telemetry".into(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
        RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: "TeamViewer".into(),
            display_name: "TeamViewer".into(),
            display_version: "15.0".into(),
            publisher: "TeamViewer".into(),
            install_location: r"C:\Program Files\TeamViewer".into(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        },
    ];

    let services = vec![
        WindowsServiceEntry {
            service_name: "WidgetService".into(),
            display_name: "Widget Service".into(),
            service_type: 16,
            raw_image_path: r#""C:\Program Files\Contoso\Widget Service\WidgetHost.exe""#.into(),
            normalized_image_path: r"C:\Program Files\Contoso\Widget Service\WidgetHost.exe".into(),
            depend_on_service: vec![],
        },
        WindowsServiceEntry {
            service_name: "AcmeTelemetrySvc".into(),
            display_name: "Acme Telemetry".into(),
            service_type: 16,
            raw_image_path: r#""C:\Program Files\Acme\Telemetry\AcmeTelemetryAgent.exe""#.into(),
            normalized_image_path: r"C:\Program Files\Acme\Telemetry\AcmeTelemetryAgent.exe".into(),
            depend_on_service: vec![],
        },
        // Desktop GUI app bundling a helper service daemon must NOT turn the entire desktop app into NO-TOUCH Service.
        WindowsServiceEntry {
            service_name: "TeamViewer".into(),
            display_name: "TeamViewer".into(),
            service_type: 16,
            raw_image_path: r#""C:\Program Files\TeamViewer\TeamViewer_Service.exe""#.into(),
            normalized_image_path: r"C:\Program Files\TeamViewer\TeamViewer_Service.exe".into(),
            depend_on_service: vec![],
        },
    ];

    let cat = build_registry_catalog_with_services(&raw, None, &services);
    let widget = cat
        .apps
        .iter()
        .find(|a| a.display_name == "Widget Service")
        .unwrap();
    let acme = cat
        .apps
        .iter()
        .find(|a| a.display_name == "Acme Telemetry Agent")
        .unwrap();
    let tv = cat
        .apps
        .iter()
        .find(|a| a.display_name == "TeamViewer")
        .unwrap();
    assert_eq!(widget.no_touch, Some(NoTouchCategory::Service));
    assert_eq!(acme.no_touch, Some(NoTouchCategory::Service));
    assert_eq!(tv.no_touch, None);
}

#[test]
fn f3_explicit_curated_service_names_classify_without_registration_evidence() {
    let curated = [
        ("Xbox Service", "Microsoft Corporation"),
        ("ASUS Framework Service", "ASUSTeK Computer Inc."),
        ("Armoury Crate Service", "ASUSTeK COMPUTER INC."),
        ("ROG Live Service", "ASUSTek COMPUTER INC."),
        ("GameSDK Service", "ASUSTek COMPUTER INC."),
        ("AURA Service", "ASUSTeK COMPUTER INC."),
        ("AURA lighting effect add-on x64", "ASUSTek COMPUTER INC."),
        ("Microsoft Update Health Tools", "Microsoft Corporation"),
    ];
    let raw: Vec<RawRegistryApp> = curated
        .iter()
        .enumerate()
        .map(|(i, (name, pub_name))| RawRegistryApp {
            hive_view: "HKLM-64".into(),
            subkey_name: format!("curated-{i}"),
            display_name: (*name).into(),
            display_version: String::new(),
            publisher: (*pub_name).into(),
            install_location: String::new(),
            uninstall_string: String::new(),
            display_icon: String::new(),
            system_component: false,
            parent_key_name: String::new(),
            release_type: String::new(),
        })
        .collect();

    let cat = build_registry_catalog_with_services(&raw, None, &[]);
    assert_eq!(cat.apps.len(), curated.len());
    for app in &cat.apps {
        assert_eq!(
            app.no_touch,
            Some(NoTouchCategory::Service),
            "{} must remain NO-TOUCH Service from explicit curated list",
            app.display_name
        );
    }
}

#[test]
fn g2_dotnet_package_cache_msi_major_and_manifest_band_route_to_correct_sdk_buckets() {
    let installed = vec![
        installed_app(
            "reg:dotnet-sdk-7",
            "Microsoft .NET SDK 7.0.407 (x64)",
            "7.4.724.11803",
            Some("Microsoft Corporation"),
            Some(r"C:\Program Files\dotnet"),
            Some("{cadaf741-75eb-48f0-b122-9d02683d185c}"),
        ),
        installed_app(
            "reg:dotnet-sdk-10",
            "Microsoft .NET SDK 10.0.403 (x64)",
            "10.4.326.52412",
            Some("Microsoft Corporation"),
            None,
            Some("{d9d858ef-b7ee-45be-866d-b1095d9341e1}"),
        ),
    ];

    let mut guid_to_name = HashMap::new();
    guid_to_name.insert(
        "{0ee1109f-86ec-46c8-9f8f-c505f10c18ba}".into(),
        "Microsoft .NET Toolset 7.0.407 (x64)".into(),
    );
    guid_to_name.insert(
        "{a638efae-5229-46a8-9a18-d0ff9d9827d2}".into(),
        "Microsoft .NET Runtime - 7.0.17 (x64)".into(),
    );
    guid_to_name.insert(
        "{46705ee9-f242-4e3d-9db6-314a2f7ec673}".into(),
        "Microsoft .NET Toolset 10.0.403 (x64)".into(),
    );
    guid_to_name.insert(
        "{941b1f64-fcd9-4c40-b927-e75f49dd96d8}".into(),
        "Microsoft .NET Runtime - 10.0.14 (x64)".into(),
    );
    guid_to_name.insert(
        "{99b1b215-9315-462d-8c49-89cf843e73af}".into(),
        "Microsoft.NET.Workload.Mono.Toolchain.net7.Manifest-10.0.100 (x64)".into(),
    );

    let tree_c = dir_node(
        r"C:\",
        0,
        vec![dir_node(
            r"C:\ProgramData",
            0,
            vec![dir_node(
                r"C:\ProgramData\Package Cache",
                0,
                vec![
                    dir_node(
                        r"C:\ProgramData\Package Cache\{0EE1109F-86EC-46C8-9F8F-C505F10C18BA}v28.7.53051",
                        129_302_528,
                        vec![],
                    ),
                    dir_node(
                        r"C:\ProgramData\Package Cache\{A638EFAE-5229-46A8-9A18-D0FF9D9827D2}v56.68.10360",
                        20_000_000,
                        vec![],
                    ),
                    dir_node(
                        r"C:\ProgramData\Package Cache\{46705EE9-F242-4E3D-9DB6-314A2F7EC673}v40.11.64284",
                        120_438_784,
                        vec![],
                    ),
                    dir_node(
                        r"C:\ProgramData\Package Cache\{941B1F64-FCD9-4C40-B927-E75F49DD96D8}v80.56.61678",
                        29_446_144,
                        vec![],
                    ),
                    dir_node(
                        r"C:\ProgramData\Package Cache\{99B1B215-9315-462D-8C49-89CF843E73AF}v81.200.61678",
                        643_072,
                        vec![],
                    ),
                ],
            )],
        )],
    );

    let units = decompose_scanned_tree(&tree_c, &installed, &[], &[]);
    let report = attribute_scan_with_options(
        10,
        "2026-11-28 03:00:00".into(),
        1,
        vec![drive_fixture('C', tree_c.total_bytes, 1_000_000_000, true)],
        empty_stats(installed.len()),
        &installed,
        &guid_to_name,
        &units,
        None,
        &AttributionOptions::default(),
    );

    let sdk7 = report
        .apps
        .iter()
        .find(|a| a.id == "reg:dotnet-sdk-7")
        .expect(".NET SDK 7");
    let sdk10 = report
        .apps
        .iter()
        .find(|a| a.id == "reg:dotnet-sdk-10")
        .expect(".NET SDK 10");
    assert_eq!(sdk7.total_bytes, 129_302_528 + 20_000_000);
    assert_eq!(sdk10.total_bytes, 120_438_784 + 29_446_144 + 643_072);
    assert!(sdk10
        .items
        .iter()
        .any(|it| it.path.contains("99B1B215-9315-462D-8C49-89CF843E73AF")));
    assert_drive_diff_zero(&report, 'C');
}
