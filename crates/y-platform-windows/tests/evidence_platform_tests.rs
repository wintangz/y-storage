use std::path::Path;

use y_platform_windows::app_paths::enumerate_app_paths;
use y_platform_windows::authenticode::extract_authenticode_signer_org;
use y_platform_windows::pe_version::read_pe_version_strings;
use y_platform_windows::services::enumerate_service_image_paths;
use y_platform_windows::start_menu::{enumerate_start_menu_shortcuts, resolve_lnk_target_exe};

const SIGNED_EXES: &[(&str, &str)] = &[
    (
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        "Microsoft Corporation",
    ),
    (
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        "Google LLC",
    ),
    (r"Y:\Apps\nodejs\node.exe", "OpenJS Foundation"),
];

const UNSIGNED_EXE: &str = r"Y:\Apps\y-storage\y-storage.exe";
const CATALOG_ONLY_EXE: &str = r"C:\Windows\System32\notepad.exe";

#[test]
fn live_authenticode_signer_extraction_signed_unsigned_and_catalog_only() {
    for (exe, expected_org) in SIGNED_EXES {
        assert!(Path::new(exe).is_file(), "expected live exe missing: {exe}");
        let got = extract_authenticode_signer_org(exe);
        println!("SIGNER {exe} => {got:?}");
        assert_eq!(
            got.as_deref(),
            Some(*expected_org),
            "unexpected Authenticode signer org for {exe}"
        );
    }

    assert!(
        Path::new(UNSIGNED_EXE).is_file(),
        "expected unsigned exe missing: {UNSIGNED_EXE}"
    );
    let unsigned_got = extract_authenticode_signer_org(UNSIGNED_EXE);
    println!("SIGNER {UNSIGNED_EXE} => {unsigned_got:?}");
    assert_eq!(unsigned_got, None, "unsigned exe must return None");

    assert!(
        Path::new(CATALOG_ONLY_EXE).is_file(),
        "expected catalog-only exe missing: {CATALOG_ONLY_EXE}"
    );
    let cat_got = extract_authenticode_signer_org(CATALOG_ONLY_EXE);
    println!("SIGNER {CATALOG_ONLY_EXE} => {cat_got:?}");
    assert_eq!(
        cat_got, None,
        "catalog-only exe without embedded PKCS7 signature must return None"
    );
}

#[test]
fn live_pe_version_strings_match_windows_properties() {
    let expected = [
        (
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            "Microsoft Corporation",
            "Microsoft Edge",
            "Microsoft Edge",
        ),
        (
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            "Google LLC",
            "Google Chrome",
            "Google Chrome",
        ),
        (
            r"Y:\Apps\nodejs\node.exe",
            "Node.js",
            "Node.js",
            "Node.js JavaScript Runtime",
        ),
    ];

    for (exe, company, product, desc) in expected {
        assert!(Path::new(exe).is_file(), "expected live exe missing: {exe}");
        let ver = read_pe_version_strings(exe);
        println!(
            "PEVER {exe} => company={:?} product={:?} desc={:?}",
            ver.company_name, ver.product_name, ver.file_description
        );
        assert_eq!(ver.company_name.as_deref(), Some(company));
        assert_eq!(ver.product_name.as_deref(), Some(product));
        assert_eq!(ver.file_description.as_deref(), Some(desc));
    }
}

#[test]
fn live_app_paths_enumeration_finds_registered_exes() {
    let entries = enumerate_app_paths();
    println!("APP_PATHS count = {}", entries.len());
    for e in entries.iter().take(5) {
        println!("APP_PATH sample: {} -> {}", e.exe_name, e.target_path);
    }
    assert!(
        !entries.is_empty(),
        "expected App Paths registry entries on live Windows machine"
    );
    assert!(
        entries.iter().any(|e| {
            e.exe_name.eq_ignore_ascii_case("chrome.exe")
                || e.exe_name.eq_ignore_ascii_case("msedge.exe")
                || Path::new(&e.target_path).is_file()
        }),
        "expected at least one existing target executable in App Paths"
    );
}

#[test]
fn live_start_menu_shortcuts_resolve_exe_targets() {
    let shortcuts = enumerate_start_menu_shortcuts();
    println!("START_MENU shortcuts count = {}", shortcuts.len());
    for s in shortcuts.iter().take(5) {
        println!(
            "SHORTCUT sample: {} -> {} ({})",
            s.display_name, s.target_path, s.shortcut_path
        );
    }
    assert!(
        !shortcuts.is_empty(),
        "expected Start Menu .lnk shortcuts on live Windows machine"
    );
    assert!(
        shortcuts
            .iter()
            .all(|s| s.target_path.to_ascii_lowercase().ends_with(".exe")),
        "every resolved Start Menu shortcut target must end with .exe"
    );
    let existing = shortcuts
        .iter()
        .find(|s| Path::new(&s.target_path).is_file())
        .expect("expected at least one Start Menu shortcut pointing to an existing exe");
    assert_eq!(
        resolve_lnk_target_exe(&existing.shortcut_path).as_deref(),
        Some(existing.target_path.as_str())
    );
}

#[test]
fn live_service_image_paths_enumeration_succeeds() {
    let svcs = enumerate_service_image_paths();
    println!("SERVICE_IMAGE_PATHS count = {}", svcs.len());
    for s in svcs.iter().take(5) {
        println!("SVC sample: {} -> {}", s.service_name, s.image_path);
    }
    assert!(
        !svcs.is_empty(),
        "expected Windows services with ImagePath"
    );
    assert!(
        svcs.iter()
            .all(|s| !s.service_name.is_empty() && !s.image_path.is_empty())
    );
    assert!(
        svcs.iter().any(|s| Path::new(&s.image_path).is_file()),
        "expected at least one service ImagePath pointing to an existing binary"
    );
}
