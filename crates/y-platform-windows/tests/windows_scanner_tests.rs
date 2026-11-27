use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use y_core::config::{AppConfig, DriveScanTarget};
use y_core::model::{DriveInfo, InstalledApp};
use y_core::tree::decompose_scanned_tree;
use y_platform_windows::scanner::{
    execute_scoped_fixture_scan_for_test, scan_directory_tree_for_test,
};

fn test_tmp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-tmp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_junction(link_path: &Path, target_path: &Path) -> bool {
    let status = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link_path.to_string_lossy(),
            &target_path.to_string_lossy(),
        ])
        .output();
    matches!(status, Ok(out) if out.status.success())
}

#[test]
fn scanner_skips_junctions_and_hard_excludes_windows_and_winsxs() {
    let root = test_tmp_dir("win-scanner-reparse");
    let app_dir = root.join("Program Files").join("SampleApp");
    let win_dir = root.join("Windows");
    let winsxs_dir = win_dir.join("WinSxS");
    let junction_dir = root.join("Program Files").join("SampleJunction");

    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&winsxs_dir).unwrap();
    std::fs::write(app_dir.join("bin.dat"), vec![1u8; 4096]).unwrap();
    std::fs::write(winsxs_dir.join("huge.dat"), vec![2u8; 8192]).unwrap();

    let linked = create_junction(&junction_dir, &app_dir);
    assert!(linked, "mklink /J must succeed inside target/test-tmp");

    let win_str = win_dir.to_string_lossy().to_string();
    // Pass empty user exclusions to verify constitutional hard-exclusion of sys_win_key + WinSxS.
    let node = scan_directory_tree_for_test(&root.to_string_lossy(), &[], &win_str);
    assert_eq!(node.total_bytes, 4096, "junction & Windows/WinSxS must not be double-counted or walked");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scanner_config_default_enables_only_system_drive_with_windows_excluded() {
    let drives = vec![
        DriveInfo {
            letter: 'C',
            root: "C:\\".into(),
            total_bytes: 1_000,
            free_bytes: 400,
            used_bytes: 600,
            is_system_drive: true,
            full_walk_enabled: true,
            exclusions: vec!["C:\\Windows".into()],
            last_scanned_unix: None,
            last_scanned_display: None,
        },
        DriveInfo {
            letter: 'D',
            root: "D:\\".into(),
            total_bytes: 2_000,
            free_bytes: 1_500,
            used_bytes: 500,
            is_system_drive: false,
            full_walk_enabled: false,
            exclusions: vec![],
            last_scanned_unix: None,
            last_scanned_display: None,
        },
    ];
    let cfg = AppConfig::for_detected_drives(&drives, "C:\\Windows");
    assert!(cfg.is_drive_enabled('C'));
    assert!(!cfg.is_drive_enabled('D'));
    assert_eq!(cfg.exclusions_for_drive('C'), vec!["C:\\Windows"]);

    // Verify decompose_scanned_tree handles empty tree cleanly without panics.
    let root = y_core::model::FolderNode {
        path: "C:\\".into(),
        name: "C:\\".into(),
        direct_file_bytes: 0,
        total_bytes: 0,
        children: vec![],
    };
    let units = decompose_scanned_tree(&root, &[] as &[InstalledApp], &[], &[]);
    assert!(units.is_empty());
    let _ = HashMap::<String, String>::new();
}

#[test]
fn excluded_drive_apps_are_omitted_and_install_folders_are_never_sized() {
    let c_root = test_tmp_dir("scoped-drive-c");
    let d_root = test_tmp_dir("scoped-drive-d");
    let c_app_dir = c_root.join("Program Files").join("InScopeApp");
    let d_app_dir = d_root.join("Games").join("ExcludedDriveApp");
    std::fs::create_dir_all(&c_app_dir).unwrap();
    std::fs::create_dir_all(&d_app_dir).unwrap();
    std::fs::write(c_app_dir.join("app.bin"), vec![1u8; 4096]).unwrap();
    std::fs::write(d_app_dir.join("huge.bin"), vec![2u8; 65536]).unwrap();

    let drives = vec![
        (
            DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 1_000_000,
                free_bytes: 900_000,
                used_bytes: 100_000,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            },
            c_root.to_string_lossy().to_string(),
        ),
        (
            DriveInfo {
                letter: 'D',
                root: "D:\\".into(),
                total_bytes: 2_000_000,
                free_bytes: 1_500_000,
                used_bytes: 500_000,
                is_system_drive: false,
                full_walk_enabled: false,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            },
            d_root.to_string_lossy().to_string(),
        ),
    ];

    let mut cfg = AppConfig::default();
    cfg.targets = vec![
        DriveScanTarget {
            letter: 'C',
            enabled: true,
            exclusions: vec![],
        },
        DriveScanTarget {
            letter: 'D',
            enabled: false,
            exclusions: vec![],
        },
    ];
    let installed = vec![
        InstalledApp {
            id: "reg:c-in-scope".into(),
            display_name: "InScopeApp".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["InScopePub".into()],
            direct_install_location: Some(r"C:\Program Files\InScopeApp".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"C:\Program Files\InScopeApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
        InstalledApp {
            id: "reg:d-excluded".into(),
            display_name: "ExcludedDriveApp".into(),
            display_version: String::new(),
            uninstall_guids: vec![],
            aliases: vec![],
            publishers: vec!["ExcludedPub".into()],
            direct_install_location: Some(r"D:\Games\ExcludedDriveApp".into()),
            inferred_install_location: None,
            all_install_locations: vec![r"D:\Games\ExcludedDriveApp".into()],
            no_touch: None,
            is_store_fallback: false,
            display_icon: None,
        },
    ];

    let (report, walked) =
        execute_scoped_fixture_scan_for_test(drives, installed, r"C:\Windows", &cfg);

    assert_eq!(walked, vec!['C'], "excluded D: root must never be walked or sized");
    assert_eq!(report.apps.len(), 1, "only C: app should remain in scoped report");
    assert_eq!(report.apps[0].display_name, "InScopeApp");
    assert_eq!(report.apps[0].total_bytes, 4096);
    assert!(
        !report.apps.iter().any(|a| a.display_name == "ExcludedDriveApp"),
        "app whose InstallLocation sits on excluded D: must be omitted cleanly"
    );

    let _ = std::fs::remove_dir_all(&c_root);
    let _ = std::fs::remove_dir_all(&d_root);
}
