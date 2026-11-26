use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use y_core::dependencies::{
    AppBinaryEvidence, DependencyCatalog, analyze_report_dependencies, parse_deps_json_is_dotnet,
    parse_steam_appmanifest, resolve_catalog_targets,
};
use y_core::discovery::{is_subpath_of, normalize_path_key, normalize_windows_path};
use y_core::model::{AttributedApp, ScanReport, ZoneKind};
use y_core::pe::parse_pe_imported_dlls;

use crate::services::enumerate_windows_services;

#[derive(Default)]
struct SteamManifestIndex {
    // key: (normalized_steamapps_common_path_key, installdir_lower) -> (appid, filename, has_ea_script)
    by_install_path: HashMap<String, Vec<(String, bool)>>,
}

fn build_steam_manifest_index(apps: &[AttributedApp]) -> SteamManifestIndex {
    let mut steamapps_dirs: BTreeSet<String> = BTreeSet::new();
    for app in apps {
        for item in &app.items {
            let p_norm = normalize_windows_path(&item.path);
            let p_lower = p_norm.to_ascii_lowercase();
            if let Some(idx) = p_lower.find("\\steamapps\\common\\") {
                let sa = p_norm[..idx + "\\steamapps".len()].to_string();
                steamapps_dirs.insert(sa);
            } else if p_lower.ends_with("\\steam") || p_lower.ends_with("\\steamlibrary") {
                let cand = format!("{p_norm}\\steamapps");
                if Path::new(&cand).is_dir() {
                    steamapps_dirs.insert(cand);
                }
            }
        }
    }

    let mut index = SteamManifestIndex::default();
    for sa_dir in steamapps_dirs {
        let Ok(rd) = fs::read_dir(&sa_dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let fname = ent.file_name().to_string_lossy().to_string();
            let fname_lower = fname.to_ascii_lowercase();
            if !fname_lower.starts_with("appmanifest_") || !fname_lower.ends_with(".acf") {
                continue;
            }
            let Ok(text) = fs::read_to_string(ent.path()) else {
                continue;
            };
            if let Some((_appid, installdir, has_ea_script)) = parse_steam_appmanifest(&text) {
                let full_game_dir = format!("{sa_dir}\\common\\{installdir}");
                index
                    .by_install_path
                    .entry(normalize_path_key(&full_game_dir))
                    .or_default()
                    .push((fname, has_ea_script));
            }
        }
    }
    index
}

fn parse_appx_manifest(xml_text: &str) -> Vec<(String, String)> {
    let lower = xml_text.to_ascii_lowercase();
    let mut out = Vec::new();
    let checks = [
        ("microsoft.vclibs", "vcpp-runtime", "AppxManifest: Microsoft.VCLibs"),
        (
            "microsoft.directxruntime",
            "directx-runtime",
            "AppxManifest: Microsoft.DirectXRuntime",
        ),
        (
            "microsoft.windowsappruntime",
            "dotnet-runtime",
            "AppxManifest: WindowsAppRuntime",
        ),
    ];
    for (needle, tid, basis) in checks {
        if lower.contains(needle) {
            out.push((tid.to_string(), basis.to_string()));
        }
    }
    out
}

fn parse_installerdata_xml(xml_text: &str) -> Vec<(String, String)> {
    let lower = xml_text.to_ascii_lowercase();
    let mut out = Vec::new();
    if lower.contains("eaanticheat") {
        out.push((
            "ea-anticheat".to_string(),
            "manifest: __Installer/installerdata.xml (EAAntiCheat)".to_string(),
        ));
    }
    if lower.contains("-req:vc") || lower.contains("visual c++") {
        out.push((
            "vcpp-runtime".to_string(),
            "manifest: __Installer/installerdata.xml (VC++ redist)".to_string(),
        ));
    }
    if lower.contains("-req:dotnet") || lower.contains("dotnet") {
        out.push((
            "dotnet-runtime".to_string(),
            "manifest: __Installer/installerdata.xml (.NET)".to_string(),
        ));
    }
    out
}

fn collect_app_evidence(
    app: &AttributedApp,
    steam_idx: &SteamManifestIndex,
) -> AppBinaryEvidence {
    let mut ev = AppBinaryEvidence::default();
    let mut body_roots: Vec<PathBuf> = Vec::new();

    for item in &app.items {
        let is_body_candidate = item.zone == Some(ZoneKind::Body)
            || app
                .install_location
                .as_deref()
                .is_some_and(|il| is_subpath_of(&item.path, il));
        if is_body_candidate && !item.path.ends_with("[files]") {
            let p = PathBuf::from(&item.path);
            if p.is_dir() {
                body_roots.push(p);
            }
        }
        if let Some(entries) = steam_idx.by_install_path.get(&normalize_path_key(&item.path)) {
            for (fname, has_ea) in entries {
                if !ev.steam_app_manifests.contains(fname) {
                    ev.steam_app_manifests.push(fname.clone());
                }
                ev.steam_ea_script_present |= *has_ea;
            }
        }
    }
    if let Some(il) = &app.install_location {
        let p = PathBuf::from(il);
        if p.is_dir() && !body_roots.iter().any(|b| normalize_path_key(&b.to_string_lossy()) == normalize_path_key(il)) {
            body_roots.push(p);
        }
        if let Some(entries) = steam_idx.by_install_path.get(&normalize_path_key(il)) {
            for (fname, has_ea) in entries {
                if !ev.steam_app_manifests.contains(fname) {
                    ev.steam_app_manifests.push(fname.clone());
                }
                ev.steam_ea_script_present |= *has_ea;
            }
        }
    }

    if body_roots.is_empty() {
        return ev;
    }

    let mut queue: VecDeque<(PathBuf, usize)> = body_roots.into_iter().map(|p| (p, 0)).collect();
    let pe_limit: usize = if app.id.starts_with("store:") { 4 } else { 24 };
    let mut pe_parsed_count = 0usize;
    let mut dlls_set: BTreeSet<String> = BTreeSet::new();
    let mut names_set: BTreeSet<String> = BTreeSet::new();

    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs = Vec::new();
        for ent in rd.flatten() {
            let Ok(ft) = ent.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let path = ent.path();
            let name = ent.file_name().to_string_lossy().to_string();
            let name_lower = name.to_ascii_lowercase();

            if ft.is_dir() {
                if depth < 1
                    && !matches!(
                        name_lower.as_str(),
                        "data"
                            | "patch"
                            | "locales"
                            | "resources"
                            | "node_modules"
                            | ".git"
                            | "cache"
                            | "toolchains"
                            | "packages"
                            | "logs"
                            | "shader"
                            | "shaders"
                    )
                {
                    subdirs.push((path, depth + 1));
                }
                continue;
            }

            if depth <= 1 {
                names_set.insert(name_lower.clone());
            }

            if (name_lower.ends_with(".exe") || name_lower.ends_with(".dll"))
                && pe_parsed_count < pe_limit
            {
                let file_len = ent.metadata().map(|m| m.len()).unwrap_or(0);
                // Avoid Defender/AV stalls on multi-hundred-MB packed game executables when smaller PEs exist.
                if file_len > 0 && file_len <= 64 * 1024 * 1024 {
                    if let Ok(mut f) = File::open(&path) {
                        for dll in parse_pe_imported_dlls(&mut f) {
                            dlls_set.insert(dll);
                        }
                        pe_parsed_count += 1;
                    }
                }
            } else if name_lower.ends_with(".deps.json") {
                if let Ok(txt) = fs::read_to_string(&path) {
                    if parse_deps_json_is_dotnet(&txt).is_some() {
                        ev.has_deps_json = true;
                        ev.deps_json_files.push(name);
                    }
                }
            } else if name_lower == "appxmanifest.xml" {
                if let Ok(txt) = fs::read_to_string(&path) {
                    ev.manifest_targets.extend(parse_appx_manifest(&txt));
                }
            } else if name_lower == "installerdata.xml" {
                if let Ok(txt) = fs::read_to_string(&path) {
                    ev.manifest_targets.extend(parse_installerdata_xml(&txt));
                }
            }
        }
        for sd in subdirs {
            queue.push_back(sd);
        }
    }

    ev.imported_dlls = dlls_set.into_iter().collect();
    ev.file_names_lower = names_set.into_iter().collect();
    ev
}

fn append_warning_log(unix_ts: u64, warn_msg: &str) {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let log_dir = base.join("y-storage");
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("y-storage.log");
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_path) {
        use std::io::Write;
        let _ = writeln!(f, "[{unix_ts}] WARN: {warn_msg}");
    }
}

pub fn enrich_report_with_dependencies(report: &mut ScanReport) {
    let loaded = DependencyCatalog::load_from_current_exe();
    report.catalog_source = loaded.source_label;
    if let Some(warn) = loaded.warning {
        eprintln!("WARN [y-storage]: {warn}");
        append_warning_log(report.scan_timestamp_unix, &warn);
        report.warnings.push(warn);
    }
    let catalog = loaded.catalog;
    let services = enumerate_windows_services();
    let target_statuses =
        resolve_catalog_targets(&catalog, &report.apps, &services, |p| Path::new(p).exists());
    let steam_idx = Arc::new(build_steam_manifest_index(&report.apps));

    let app_indices: Mutex<VecDeque<usize>> =
        Mutex::new((0..report.apps.len()).collect());
    let evidence_map: Mutex<HashMap<String, AppBinaryEvidence>> =
        Mutex::new(HashMap::with_capacity(report.apps.len()));

    let apps_ref = &report.apps;
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get().clamp(2, 6));

    std::thread::scope(|s| {
        for _ in 0..threads {
            let q = &app_indices;
            let out = &evidence_map;
            let si = Arc::clone(&steam_idx);
            s.spawn(move || loop {
                let idx = q.lock().unwrap().pop_front();
                let Some(i) = idx else { break };
                let app = &apps_ref[i];
                let ev = collect_app_evidence(app, &si);
                out.lock().unwrap().insert(app.id.clone(), ev);
            });
        }
    });

    let ev_map = evidence_map.into_inner().unwrap();
    analyze_report_dependencies(report, &catalog, &services, &ev_map, &target_statuses);
}
