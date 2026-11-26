use std::collections::HashMap;
use std::io::Cursor;

use y_core::attribution::attribute_scan;
use y_core::dependencies::{
    AppBinaryEvidence, DependencyCatalog, WindowsServiceEntry, analyze_report_dependencies,
    parse_deps_json_is_dotnet, parse_steam_appmanifest, resolve_catalog_targets,
};
use y_core::model::{
    AttributionBasis, DependencyConfidence, DriveInfo, InstalledApp, RegistryDiscoveryStats,
    ScannedUnit, ScannedUnitKind, ZoneKind,
};
use y_core::pe::parse_pe_imported_dlls;

fn build_synthetic_pe(is_64: bool, dll_names: &[&str]) -> Vec<u8> {
    let mut buf = vec![0u8; 0x1000];
    // DOS MZ + e_lfanew = 0x80
    buf[0..2].copy_from_slice(&0x5A4Du16.to_le_bytes());
    buf[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    // PE\0\0 at 0x80
    buf[0x80..0x84].copy_from_slice(&0x0000_4550u32.to_le_bytes());
    // NumberOfSections = 1, SizeOfOptionalHeader = 240 (0xF0)
    buf[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    buf[0x94..0x96].copy_from_slice(&240u16.to_le_bytes());

    let opt = 0x80 + 24;
    let magic: u16 = if is_64 { 0x20B } else { 0x10B };
    buf[opt..opt + 2].copy_from_slice(&magic.to_le_bytes());
    let import_dir_off = opt + if is_64 { 120 } else { 104 };
    // Import table at RVA 0x2000 (file offset 0x400)
    buf[import_dir_off..import_dir_off + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    buf[import_dir_off + 4..import_dir_off + 8].copy_from_slice(&200u32.to_le_bytes());

    let sec = opt + 240;
    buf[sec + 8..sec + 12].copy_from_slice(&0x1000u32.to_le_bytes()); // VirtualSize
    buf[sec + 12..sec + 16].copy_from_slice(&0x2000u32.to_le_bytes()); // VirtualAddress
    buf[sec + 16..sec + 20].copy_from_slice(&0x0800u32.to_le_bytes()); // SizeOfRawData
    buf[sec + 20..sec + 24].copy_from_slice(&0x0400u32.to_le_bytes()); // PointerToRawData

    // Write descriptors at 0x400 and DLL ASCII names starting at 0x600 (RVA 0x2200)
    let mut str_off = 0x600usize;
    for (idx, dll) in dll_names.iter().enumerate() {
        let desc = 0x400 + idx * 20;
        let name_rva = (0x2000 + (str_off - 0x400)) as u32;
        buf[desc..desc + 4].copy_from_slice(&1u32.to_le_bytes()); // OFT non-zero
        buf[desc + 12..desc + 16].copy_from_slice(&name_rva.to_le_bytes());
        let b = dll.as_bytes();
        buf[str_off..str_off + b.len()].copy_from_slice(b);
        buf[str_off + b.len()] = 0;
        str_off += b.len() + 1;
    }
    buf
}

#[test]
fn pe_import_parser_handles_pe32_and_pe64_without_full_read() {
    let pe64 = build_synthetic_pe(true, &["KERNEL32.dll", "VCRUNTIME140.dll", "D3DCOMPILER_47.dll"]);
    let imports64 = parse_pe_imported_dlls(&mut Cursor::new(pe64));
    assert!(imports64.contains(&"vcruntime140.dll".to_string()));
    assert!(imports64.contains(&"d3dcompiler_47.dll".to_string()));

    let pe32 = build_synthetic_pe(false, &["MSVCP140.dll", "d3d11.dll"]);
    let imports32 = parse_pe_imported_dlls(&mut Cursor::new(pe32));
    assert!(imports32.contains(&"msvcp140.dll".to_string()));
    assert!(imports32.contains(&"d3d11.dll".to_string()));
}

#[test]
fn deps_json_and_steam_manifest_parsers_extract_metadata() {
    let deps = r#"{ "runtimeTarget": { "name": ".NETCoreApp,Version=v8.0" }, "targets": {} }"#;
    assert_eq!(
        parse_deps_json_is_dotnet(deps).as_deref(),
        Some(".NETCoreApp,Version=v8.0")
    );

    let acf = r#""AppState"
{
    "appid" "1811260"
    "installdir" "FIFA 23"
    "InstallScripts" { "3340991" "EAappInstaller_installScript.vdf" }
}"#;
    let (appid, dir, ea_script) = parse_steam_appmanifest(acf).unwrap();
    assert_eq!(appid, "1811260");
    assert_eq!(dir, "FIFA 23");
    assert!(ea_script);
}

#[test]
fn attribution_coverage_2_routes_green_caches_dot_folders_and_flutter_preserving_zero_diff() {
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 100_000,
        free_bytes: 40_000,
        used_bytes: 60_000,
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
        id: "reg:discord".into(),
        display_name: "Discord".into(),
        aliases: vec![],
        publishers: vec!["Discord Inc.".into()],
        direct_install_location: Some("C:\\Users\\Win\\AppData\\Local\\Discord".into()),
        inferred_install_location: None,
        all_install_locations: vec!["C:\\Users\\Win\\AppData\\Local\\Discord".into()],
        no_touch: None,
        is_store_fallback: false,
        display_icon: None,
    }];
    let units = vec![
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
            drive_letter: 'C',
            bytes: 10_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Temp".into(),
            drive_letter: 'C',
            bytes: 5_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\NVIDIA\\DXCache".into(),
            drive_letter: 'C',
            bytes: 4_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\.gemini".into(),
            drive_letter: 'C',
            bytes: 3_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\flutter".into(),
            drive_letter: 'C',
            bytes: 2_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Roaming\\UnknownLeftover".into(),
            drive_letter: 'C',
            bytes: 1_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];

    let report = attribute_scan(
        1,
        "2026-11-26 09:30:00".into(),
        10,
        vec![drive],
        stats,
        &apps,
        &HashMap::new(),
        &units,
        None,
    );

    assert_eq!(report.shared_caches_total_bytes, 9_000);
    assert!(report.shared_caches.iter().all(|c| c.zone == Some(ZoneKind::Green)));
    assert_eq!(report.unattributed_total_bytes, 1_000);
    assert_eq!(report.unattributed[0].path, "C:\\Users\\Win\\AppData\\Roaming\\UnknownLeftover");

    let gemini = report.apps.iter().find(|a| a.display_name == ".gemini").unwrap();
    assert_eq!(gemini.total_bytes, 3_000);
    assert!(gemini.bases.contains(&AttributionBasis::StandaloneFolder));

    let flutter = report.apps.iter().find(|a| a.display_name == "flutter").unwrap();
    assert_eq!(flutter.total_bytes, 2_000);
    assert!(flutter.bases.contains(&AttributionBasis::StandaloneFolder));

    // Exact 0 B drive reconciliation
    let apps_c = report.total_apps_bytes_on_drive('C');
    let caches_c = report.total_shared_caches_on_drive('C');
    let yf_c = report.total_your_files_on_drive('C');
    let un_c = report.total_unattributed_on_drive('C');
    let sys_c = report.windows_system[0].total_bytes;
    assert_eq!(apps_c + caches_c + yf_c + un_c + sys_c + 40_000, 100_000);
}

#[test]
fn dependencies_invert_cleanly_without_inflating_app_totals() {
    let catalog = DependencyCatalog::load_from_disk_or_embedded();
    let drive = DriveInfo {
        letter: 'C',
        root: "C:\\".into(),
        total_bytes: 200_000,
        free_bytes: 100_000,
        used_bytes: 100_000,
        is_system_drive: true,
        full_walk_enabled: true,
        exclusions: vec!["C:\\Windows".into()],
    };
    let stats = RegistryDiscoveryStats {
        raw_display_name_entries: 3,
        raw_with_install_location: 2,
        canonical_apps: 3,
        canonical_with_direct_install_location: 2,
        canonical_with_inferred_install_location: 0,
    };
    let apps = vec![
        InstalledApp {
            id: "reg:fifa".into(),
            display_name: "EA SPORTS™ FIFA 23".into(),
            aliases: vec![],
            publishers: vec!["EA Canada".into()],
            direct_install_location: Some("D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into()),
            inferred_install_location: None,
            all_install_locations: vec!["D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "reg:ea".into(),
            display_name: "EA app".into(),
            aliases: vec![],
            publishers: vec!["Electronic Arts".into()],
            direct_install_location: Some("C:\\Program Files\\Electronic Arts".into()),
            inferred_install_location: None,
            all_install_locations: vec!["C:\\Program Files\\Electronic Arts".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "reg:note".into(),
            display_name: "PlainApp".into(),
            aliases: vec![],
            publishers: vec!["Solo".into()],
            direct_install_location: Some("C:\\Program Files\\PlainApp".into()),
            inferred_install_location: None,
            all_install_locations: vec!["C:\\Program Files\\PlainApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "store:msteams".into(),
            display_name: "Store: MSTeams".into(),
            aliases: vec![],
            publishers: vec!["Microsoft Store".into()],
            direct_install_location: None,
            inferred_install_location: None,
            all_install_locations: vec![],
            no_touch: None,
            is_store_fallback: true,
            display_icon: None,
        },
    ];
    let units = vec![
        ScannedUnit {
            path: "D:\\Games\\Steam\\steamapps\\common\\FIFA 23".into(),
            drive_letter: 'D',
            bytes: 50_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
        ScannedUnit {
            path: "C:\\Program Files\\Electronic Arts".into(),
            drive_letter: 'C',
            bytes: 12_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
        ScannedUnit {
            path: "C:\\Program Files\\PlainApp".into(),
            drive_letter: 'C',
            bytes: 2_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        },
        ScannedUnit {
            path: "C:\\Users\\Win\\AppData\\Local\\Packages\\MSTeams_8wekyb3d8bbwe".into(),
            drive_letter: 'C',
            bytes: 99_000,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        },
    ];
    let mut report = attribute_scan(
        1,
        "2026-11-26 09:30:00".into(),
        5,
        vec![drive],
        stats,
        &apps,
        &HashMap::new(),
        &units,
        None,
    );

    let services = vec![WindowsServiceEntry {
        service_name: "EAAntiCheatService".into(),
        display_name: "EAAntiCheatService".into(),
        service_type: 16,
        raw_image_path: "C:\\Program Files\\EA\\AC\\eaanticheat.gameservice.exe".into(),
        normalized_image_path: "C:\\Program Files\\EA\\AC\\eaanticheat.gameservice.exe".into(),
        depend_on_service: vec![],
    }];
    let statuses = resolve_catalog_targets(&catalog, &report.apps, &services, |_| true);
    assert_eq!(statuses.get("steam").unwrap().own_size_bytes, None);
    let mut ev_map = HashMap::new();
    ev_map.insert(
        "reg:fifa".to_string(),
        AppBinaryEvidence {
            imported_dlls: vec!["vcruntime140.dll".into(), "d3dcompiler_47.dll".into()],
            file_names_lower: vec!["eaanticheat.gameservicelauncher.exe".into()],
            steam_app_manifests: vec!["appmanifest_1811260.acf".into()],
            steam_ea_script_present: true,
            ..Default::default()
        },
    );

    analyze_report_dependencies(&mut report, &catalog, &services, &ev_map, &statuses);

    // Non-inflation guarantee: FIFA 23 total stays 50_000 despite EA app own_size_bytes == 12_000!
    let fifa = report.apps.iter().find(|a| a.id == "reg:fifa").unwrap();
    assert_eq!(fifa.total_bytes, 50_000);
    assert!(fifa.dependencies.iter().any(|d| d.target_id == "steam" && d.confidence == DependencyConfidence::Proven));
    assert!(fifa.dependencies.iter().any(|d| d.target_id == "ea-app" && d.own_size_bytes == Some(12_000)));
    assert!(fifa.dependencies.iter().any(|d| d.target_id == "vcpp-runtime" && d.imported_by_binary));
    assert!(fifa.dependencies.iter().any(|d| d.target_id == "directx-runtime" && d.imported_by_binary));

    let plain = report.apps.iter().find(|a| a.id == "reg:note").unwrap();
    assert!(plain.dependencies.is_empty());

    let rev_vcpp = report
        .reverse_dependencies
        .iter()
        .find(|r| r.target_id == "vcpp-runtime")
        .unwrap();
    assert_eq!(rev_vcpp.dependent_count, 1);
    assert_eq!(rev_vcpp.binary_importer_count, 1);
}
