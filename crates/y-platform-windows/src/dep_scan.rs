use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use y_core::dependencies::{
    AppBinaryEvidence, DependencyCatalog, analyze_report_dependencies, extract_dotnet_major_from_tfm,
    parse_deps_json_is_dotnet, parse_runtimeconfig_json_major, parse_steam_appmanifest,
    resolve_catalog_targets_with_registry,
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
    if lower.contains("microsoft.vclibs.110") {
        out.push(("vcpp-2012".to_string(), "AppxManifest: Microsoft.VCLibs.110".to_string()));
    } else if lower.contains("microsoft.vclibs.120") {
        out.push(("vcpp-2013".to_string(), "AppxManifest: Microsoft.VCLibs.120".to_string()));
    } else if lower.contains("microsoft.vclibs") {
        out.push(("vcpp-v14".to_string(), "AppxManifest: Microsoft.VCLibs.140".to_string()));
    }
    if lower.contains("microsoft.directxruntime") {
        out.push((
            "directx-runtime".to_string(),
            "AppxManifest: Microsoft.DirectXRuntime".to_string(),
        ));
    }
    if lower.contains("microsoft.windowsappruntime") {
        out.push((
            "dotnet-runtime".to_string(),
            "AppxManifest: WindowsAppRuntime".to_string(),
        ));
    }
    if lower.contains("microsoft.ui.xaml") {
        out.push((
            "winui-xaml".to_string(),
            "AppxManifest: Microsoft.UI.Xaml".to_string(),
        ));
    }
    if lower.contains("microsoft.net.native") {
        out.push((
            "dotnet-native".to_string(),
            "AppxManifest: Microsoft.NET.Native".to_string(),
        ));
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
            "vcpp-generic".to_string(),
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

fn should_descend_binary_subdir(depth: usize, name_lower: &str) -> bool {
    if depth >= 4 {
        return false;
    }
    if matches!(
        name_lower,
        "data"
            | "patch"
            | "locales"
            | "resources"
            | "node_modules"
            | ".git"
            | "cache"
            | "caches"
            | "code cache"
            | "gpucache"
            | "shadercache"
            | "dxcache"
            | "glcache"
            | "toolchains"
            | "packages"
            | "logs"
            | "shader"
            | "shaders"
            | "doc"
            | "docs"
            | "help"
            | "legal"
            | "streamingassets"
            | "assets"
            | "locale"
    ) {
        return false;
    }
    true
}

fn collect_app_evidence(
    app: &AttributedApp,
    steam_idx: &SteamManifestIndex,
) -> AppBinaryEvidence {
    let mut ev = AppBinaryEvidence::default();
    let mut body_roots: Vec<PathBuf> = Vec::new();
    let mut fallback_roots: Vec<PathBuf> = Vec::new();
    let mut companion_tokens: BTreeSet<String> = BTreeSet::new();

    for item in &app.items {
        let is_body_candidate = item.zone == Some(ZoneKind::Body)
            || app
                .install_location
                .as_deref()
                .is_some_and(|il| is_subpath_of(&item.path, il));
        if !item.path.ends_with("[files]") {
            let p = PathBuf::from(&item.path);
            let p_lower = item.path.to_ascii_lowercase();
            let is_cache_only = item.zone == Some(ZoneKind::Green)
                || p_lower.contains("\\cache")
                || p_lower.contains("\\temp")
                || p_lower.contains("\\package cache\\");
            if is_body_candidate && p.is_dir() {
                body_roots.push(p);
            } else if p.is_dir() {
                if !is_cache_only || p_lower.contains("\\ms-playwright") {
                    fallback_roots.push(p);
                }
                if let Ok(rd) = fs::read_dir(&item.path) {
                    for ent in rd.flatten() {
                        if let Ok(ft) = ent.file_type() {
                            if ft.is_dir() {
                                companion_tokens.insert(ent.file_name().to_string_lossy().to_ascii_lowercase());
                            }
                        }
                    }
                }
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

    let using_fallback = body_roots.is_empty();
    let scan_roots = if using_fallback { fallback_roots } else { body_roots };
    if scan_roots.is_empty() {
        return ev;
    }

    let mut queue: VecDeque<(PathBuf, usize)> = scan_roots.into_iter().map(|p| (p, 0)).collect();
    let pe_limit: usize = if using_fallback {
        16
    } else if app.id.starts_with("store:") {
        12
    } else {
        40
    };
    let mut pe_parsed_count = 0usize;
    let mut dlls_set: BTreeSet<String> = BTreeSet::new();
    let mut names_set: BTreeSet<String> = BTreeSet::new();
    let mut dotnet_majors: BTreeSet<u32> = BTreeSet::new();

    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs = Vec::new();
        let mut pe_candidates: Vec<(bool, PathBuf, u64)> = Vec::new();
        for ent in rd.flatten() {
            let Ok(ft) = ent.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let path = ent.path();
            let name = ent.file_name().to_string_lossy().to_string();
            let name_lower = name.to_ascii_lowercase();

            if ft.is_dir() {
                if should_descend_binary_subdir(depth, &name_lower) {
                    subdirs.push((path, depth + 1));
                }
                continue;
            }

            if depth <= 3 {
                names_set.insert(name_lower.clone());
            }

            if name_lower.ends_with(".exe") || name_lower.ends_with(".dll") {
                let file_len = ent.metadata().map(|m| m.len()).unwrap_or(0);
                // parse_pe_imported_dlls only seeks and reads tiny header/import slices (<12 KB).
                if file_len > 0 && file_len <= 512 * 1024 * 1024 {
                    pe_candidates.push((name_lower.ends_with(".exe"), path, file_len));
                }
            } else if name_lower.ends_with(".deps.json") {
                if let Ok(txt) = fs::read_to_string(&path) {
                    if let Some(rt) = parse_deps_json_is_dotnet(&txt) {
                        ev.has_deps_json = true;
                        ev.deps_json_files.push(name);
                        if let Some(m) = extract_dotnet_major_from_tfm(&rt) {
                            dotnet_majors.insert(m);
                        }
                    }
                }
            } else if name_lower.ends_with(".runtimeconfig.json") {
                if let Ok(txt) = fs::read_to_string(&path) {
                    if let Some(m) = parse_runtimeconfig_json_major(&txt) {
                        ev.has_deps_json = true;
                        if ev.deps_json_files.is_empty() {
                            ev.deps_json_files.push(name);
                        }
                        dotnet_majors.insert(m);
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
        // Prioritize .exe files before .dll files inside each directory so PE limit catches entry points first.
        pe_candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.2.cmp(&b.2)));
        for (_, path, _) in pe_candidates {
            if pe_parsed_count >= pe_limit {
                break;
            }
            if let Ok(mut f) = File::open(&path) {
                for dll in parse_pe_imported_dlls(&mut f) {
                    dlls_set.insert(dll);
                }
                pe_parsed_count += 1;
            }
        }
        for sd in subdirs {
            queue.push_back(sd);
        }
    }

    ev.imported_dlls = dlls_set.into_iter().collect();
    ev.file_names_lower = names_set.into_iter().collect();
    ev.companion_dir_tokens = companion_tokens.into_iter().collect();
    ev.dotnet_major_versions = dotnet_majors.into_iter().collect();
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
    let target_statuses = resolve_catalog_targets_with_registry(
        &catalog,
        &report.apps,
        &report.installed_registry_names,
        &services,
        |p| {
            if let Some(prefix_base) = p.strip_suffix(".*") {
                if let Some((parent, leaf_prefix)) = prefix_base.rsplit_once('\\') {
                    let lp = leaf_prefix.to_ascii_lowercase();
                    if let Ok(rd) = fs::read_dir(parent) {
                        return rd.flatten().any(|e| {
                            e.file_name()
                                .to_string_lossy()
                                .to_ascii_lowercase()
                                .starts_with(&format!("{lp}."))
                        });
                    }
                }
                false
            } else {
                Path::new(p).exists()
            }
        },
    );
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
