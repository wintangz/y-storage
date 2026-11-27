use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::discovery::{is_subpath_of, normalize_app_token, normalize_path_key};
use crate::model::{
    AttributedApp, DependencyConfidence, DependencyKind, DependencyRecord, ReverseConsumer,
    ReverseDependencyEntry, ScanReport,
};

pub const EMBEDDED_DEPENDENCY_CATALOG: &str =
    include_str!("../../../data/dependency_catalog.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogTarget {
    pub id: String,
    pub name: String,
    pub kind: DependencyKind,
    pub shared: bool,
    #[serde(default)]
    pub registry_matches: Vec<String>,
    #[serde(default)]
    pub service_names: Vec<String>,
    #[serde(default)]
    pub presence_paths: Vec<String>,
    #[serde(default)]
    pub size_folder_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogDllRule {
    pub target_id: String,
    pub dlls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFingerprintRule {
    pub target_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogAppRule {
    #[serde(default)]
    pub app_matches: Vec<String>,
    #[serde(default)]
    pub publisher_matches: Vec<String>,
    #[serde(default)]
    pub install_path_contains: Option<String>,
    pub target_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyCatalog {
    #[serde(default)]
    pub targets: Vec<CatalogTarget>,
    #[serde(default)]
    pub dll_imports: Vec<CatalogDllRule>,
    #[serde(default)]
    pub fingerprints: Vec<CatalogFingerprintRule>,
    #[serde(default)]
    pub app_rules: Vec<CatalogAppRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatalogSourceKind {
    Embedded,
    ExternalOverride,
    EmbeddedFallbackAfterError,
}

#[derive(Debug, Clone)]
pub struct CatalogLoadResult {
    pub catalog: DependencyCatalog,
    pub source_kind: CatalogSourceKind,
    pub source_label: String,
    pub override_path: Option<std::path::PathBuf>,
    pub warning: Option<String>,
}

impl DependencyCatalog {
    /// Parses and validates TOML text for the dependency catalog.
    pub fn parse_and_validate(raw: &str) -> Result<Self, String> {
        let cat = toml::from_str::<Self>(raw).map_err(|e| format!("invalid TOML schema: {e}"))?;
        if cat.targets.is_empty() {
            return Err("catalog defines zero [[targets]]".to_string());
        }
        Ok(cat)
    }

    /// Loads the embedded catalog, or an optional valid override next to `exe_dir`.
    pub fn load_for_exe_dir(exe_dir: Option<&Path>) -> CatalogLoadResult {
        let embedded = Self::parse_and_validate(EMBEDDED_DEPENDENCY_CATALOG)
            .expect("embedded data/dependency_catalog.toml must parse");

        if let Some(dir) = exe_dir {
            let candidates = [
                dir.join("dependency_catalog.toml"),
                dir.join("data").join("dependency_catalog.toml"),
            ];
            for cand in candidates {
                if cand.exists() {
                    match std::fs::read_to_string(&cand)
                        .map_err(|e| format!("read error: {e}"))
                        .and_then(|raw| Self::parse_and_validate(&raw))
                    {
                        Ok(cat) => {
                            return CatalogLoadResult {
                                catalog: cat,
                                source_kind: CatalogSourceKind::ExternalOverride,
                                source_label: format!("override: {}", cand.display()),
                                override_path: Some(cand),
                                warning: None,
                            };
                        }
                        Err(err) => {
                            let warn_msg = format!(
                                "Failed to load dependency catalog override '{}': {}; falling back to embedded catalog.",
                                cand.display(),
                                err
                            );
                            return CatalogLoadResult {
                                catalog: embedded,
                                source_kind: CatalogSourceKind::EmbeddedFallbackAfterError,
                                source_label: format!(
                                    "embedded (fallback after invalid override: {})",
                                    cand.display()
                                ),
                                override_path: Some(cand),
                                warning: Some(warn_msg),
                            };
                        }
                    }
                }
            }
        }

        CatalogLoadResult {
            catalog: embedded,
            source_kind: CatalogSourceKind::Embedded,
            source_label: "embedded".to_string(),
            override_path: None,
            warning: None,
        }
    }

    pub fn load_from_current_exe() -> CatalogLoadResult {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        Self::load_for_exe_dir(exe_dir.as_deref())
    }

    pub fn load_from_disk_or_embedded() -> Self {
        Self::load_from_current_exe().catalog
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsServiceEntry {
    pub service_name: String,
    pub display_name: String,
    pub service_type: u32,
    pub raw_image_path: String,
    pub normalized_image_path: String,
    pub depend_on_service: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AppBinaryEvidence {
    pub imported_dlls: Vec<String>,
    pub file_names_lower: Vec<String>,
    pub companion_dir_tokens: Vec<String>,
    pub has_deps_json: bool,
    pub deps_json_files: Vec<String>,
    pub dotnet_major_versions: Vec<u32>,
    pub manifest_targets: Vec<(String, String)>, // (target_id, basis)
    pub steam_app_manifests: Vec<String>,       // e.g. appmanifest_1811260.acf
    pub steam_ea_script_present: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedTargetStatus {
    pub target: CatalogTarget,
    pub present: bool,
    pub own_size_bytes: Option<u64>,
}

pub fn contains_token_boundary(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let mut start = 0usize;
    while let Some(pos) = haystack[start..].find(needle) {
        let abs = start + pos;
        let end = abs + needle.len();
        let left_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let right_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

pub fn resolve_catalog_targets<F>(
    catalog: &DependencyCatalog,
    apps: &[AttributedApp],
    services: &[WindowsServiceEntry],
    path_exists: F,
) -> HashMap<String, ResolvedTargetStatus>
where
    F: Fn(&str) -> bool,
{
    resolve_catalog_targets_with_registry(catalog, apps, &[], services, path_exists)
}

pub fn resolve_catalog_targets_with_registry<F>(
    catalog: &DependencyCatalog,
    apps: &[AttributedApp],
    installed_registry_names: &[String],
    services: &[WindowsServiceEntry],
    path_exists: F,
) -> HashMap<String, ResolvedTargetStatus>
where
    F: Fn(&str) -> bool,
{
    let svc_names_lower: BTreeSet<String> = services
        .iter()
        .map(|s| s.service_name.to_ascii_lowercase())
        .collect();
    let reg_names_lower: Vec<String> = installed_registry_names
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut out = HashMap::new();
    for target in &catalog.targets {
        let mut matched_size_sum: u64 = 0;
        let mut matched_any_app = false;
        for app in apps {
            let dn = app.display_name.to_ascii_lowercase();
            if target
                .registry_matches
                .iter()
                .any(|m| contains_token_boundary(&dn, &m.to_ascii_lowercase()))
            {
                matched_any_app = true;
                matched_size_sum = matched_size_sum.saturating_add(app.total_bytes);
            }
        }
        let matched_in_registry = reg_names_lower.iter().any(|rn| {
            target
                .registry_matches
                .iter()
                .any(|m| contains_token_boundary(rn, &m.to_ascii_lowercase()))
        });

        let mut hint_size: Option<u64> = None;
        if let Some(hint) = &target.size_folder_hint {
            let hk = normalize_path_key(hint);
            for app in apps {
                for item in &app.items {
                    if normalize_path_key(&item.path) == hk {
                        hint_size = Some(hint_size.unwrap_or(0).saturating_add(item.bytes));
                    }
                }
            }
        }

        let svc_present = target
            .service_names
            .iter()
            .any(|s| svc_names_lower.contains(&s.to_ascii_lowercase()));
        let path_present = target.presence_paths.iter().any(|p| path_exists(p));

        let own_size_bytes = if let Some(hs) = hint_size {
            Some(hs)
        } else if matched_any_app {
            Some(matched_size_sum)
        } else {
            None
        };

        out.insert(
            target.id.clone(),
            ResolvedTargetStatus {
                target: target.clone(),
                present: matched_any_app || matched_in_registry || svc_present || path_present,
                own_size_bytes,
            },
        );
    }
    out
}

#[derive(Clone)]
struct PendingDep {
    target_id: String,
    target_name: String,
    kind: DependencyKind,
    present: bool,
    own_size_bytes: Option<u64>,
    shared: bool,
    confidence: DependencyConfidence,
    bases: Vec<String>,
    imported_by_binary: bool,
}

fn upsert_dep(map: &mut BTreeMap<String, PendingDep>, cand: PendingDep) {
    if let Some(existing) = map.get_mut(&cand.target_id) {
        if cand.confidence == DependencyConfidence::Proven {
            existing.confidence = DependencyConfidence::Proven;
        }
        existing.imported_by_binary |= cand.imported_by_binary;
        for b in cand.bases {
            if !existing.bases.contains(&b) {
                existing.bases.push(b);
            }
        }
    } else {
        map.insert(cand.target_id.clone(), cand);
    }
}

fn is_self_dependency(app_name: &str, target: &CatalogTarget) -> bool {
    let al = app_name.to_ascii_lowercase();
    if target
        .registry_matches
        .iter()
        .any(|m| contains_token_boundary(&al, &m.to_ascii_lowercase()))
    {
        return true;
    }
    if target.id.starts_with("dotnet-")
        && !al.contains("visual studio")
        && (al.contains(".net") || al.contains("desktop runtime"))
    {
        return true;
    }
    if target.id.starts_with("vcpp-") && al.contains("visual c++") && al.contains("redistributable") {
        return true;
    }
    let a = normalize_app_token(app_name);
    let t = normalize_app_token(&target.name);
    !a.is_empty() && !t.is_empty() && (a == t || a.starts_with(&format!("{t} ")))
}

fn normalize_vendor_key(raw: &str) -> String {
    let mut s = raw.trim().to_ascii_lowercase();
    for pat in [", inc.", " inc.", ", inc", " inc", ", llc", " llc", ", ltd.", " ltd.", " ltd", " corporation", " corp.", " corp", " aps", " gmbh"] {
        if let Some(stripped) = s.strip_suffix(pat) {
            s = stripped.trim().to_string();
        }
    }
    s.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ' ')
        .trim()
        .to_string()
}

fn build_target_dep(
    status: &ResolvedTargetStatus,
    confidence: DependencyConfidence,
    basis: String,
    imported_by_binary: bool,
) -> PendingDep {
    PendingDep {
        target_id: status.target.id.clone(),
        target_name: status.target.name.clone(),
        kind: status.target.kind,
        present: status.present,
        own_size_bytes: status.own_size_bytes,
        shared: status.target.shared,
        confidence,
        bases: vec![basis],
        imported_by_binary,
    }
}

pub fn parse_deps_json_is_dotnet(json_text: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_text).ok()?;
    let target = v
        .get("runtimeTarget")
        .and_then(|rt| rt.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string);
    if target.is_some() || v.get("targets").is_some() {
        Some(target.unwrap_or_else(|| ".NET Core / .NET".to_string()))
    } else {
        None
    }
}

pub fn parse_runtimeconfig_json_major(json_text: &str) -> Option<u32> {
    let v: serde_json::Value = serde_json::from_str(json_text).ok()?;
    let ro = v.get("runtimeOptions")?;
    if let Some(tfm) = ro.get("tfm").and_then(|x| x.as_str()) {
        if let Some(maj) = extract_dotnet_major_from_tfm(tfm) {
            return Some(maj);
        }
    }
    if let Some(ver) = ro.get("framework").and_then(|f| f.get("version")).and_then(|x| x.as_str()) {
        if let Some(maj) = ver.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
            return Some(maj);
        }
    }
    if let Some(arr) = ro.get("frameworks").and_then(|f| f.as_array()) {
        for fw in arr {
            if let Some(ver) = fw.get("version").and_then(|x| x.as_str()) {
                if let Some(maj) = ver.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
                    return Some(maj);
                }
            }
        }
    }
    None
}

pub fn extract_dotnet_major_from_tfm(raw: &str) -> Option<u32> {
    let lower = raw.to_ascii_lowercase();
    if let Some(idx) = lower.find("version=v") {
        let rest = &lower[idx + "version=v".len()..];
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(m) = num_str.parse::<u32>() {
            return Some(m);
        }
    }
    if let Some(rest) = lower.strip_prefix("net") {
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(m) = num_str.parse::<u32>() {
            return Some(m);
        }
    }
    None
}

pub fn parse_steam_appmanifest(acf_text: &str) -> Option<(String, String, bool)> {
    let mut installdir: Option<String> = None;
    let mut appid: Option<String> = None;
    let mut has_ea_script = false;

    for line in acf_text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("eaappinstaller_installscript.vdf") {
            has_ea_script = true;
        }
        let tokens: Vec<&str> = line.split('"').collect();
        if tokens.len() >= 4 {
            let key = tokens[1].trim();
            let val = tokens[3].trim();
            if key.eq_ignore_ascii_case("installdir") {
                installdir = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("appid") {
                appid = Some(val.to_string());
            }
        }
    }
    Some((appid?, installdir?, has_ea_script))
}

pub fn analyze_report_dependencies(
    report: &mut ScanReport,
    catalog: &DependencyCatalog,
    services: &[WindowsServiceEntry],
    evidence_by_app_id: &HashMap<String, AppBinaryEvidence>,
    target_statuses: &HashMap<String, ResolvedTargetStatus>,
) {
    // Invert DLL -> target_id for O(1) lookup.
    let mut dll_to_target: HashMap<String, String> = HashMap::new();
    for rule in &catalog.dll_imports {
        for dll in &rule.dlls {
            dll_to_target.insert(dll.to_ascii_lowercase(), rule.target_id.clone());
        }
    }

    // Invert Service name -> catalog target_id if service belongs to a catalog target,
    // and index Windows services by lowercase service name for quick lookup.
    let mut svc_to_target: HashMap<String, String> = HashMap::new();
    for target in &catalog.targets {
        for s in &target.service_names {
            svc_to_target.insert(s.to_ascii_lowercase(), target.id.clone());
        }
    }
    let svc_by_name: HashMap<String, &WindowsServiceEntry> = services
        .iter()
        .map(|s| (s.service_name.to_ascii_lowercase(), s))
        .collect();

    // Index normalized publisher keys of installed apps matching each catalog target for companion kinship checks.
    let mut target_sibling_vendors: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (tid, st) in target_statuses {
        let mut vendors = BTreeSet::new();
        for a in &report.apps {
            let dn = a.display_name.to_ascii_lowercase();
            if st
                .target
                .registry_matches
                .iter()
                .any(|m| contains_token_boundary(&dn, &m.to_ascii_lowercase()))
            {
                for p in &a.publishers {
                    let v = normalize_vendor_key(p);
                    if v.len() >= 3 {
                        vendors.insert(v);
                    }
                }
            }
        }
        target_sibling_vendors.insert(tid.clone(), vendors);
    }

    for app in &mut report.apps {
        let mut deps_map: BTreeMap<String, PendingDep> = BTreeMap::new();
        let app_ev = evidence_by_app_id.get(&app.id);

        // Source (1): PE imports, .deps.json, AppxManifest / installerdata.xml (PROVEN).
        if let Some(ev) = app_ev {
            let mut matched_dlls_by_target: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for dll in &ev.imported_dlls {
                if let Some(tid) = dll_to_target.get(&dll.to_ascii_lowercase()) {
                    matched_dlls_by_target
                        .entry(tid.clone())
                        .or_default()
                        .push(dll.clone());
                }
            }
            for (tid, mut dlls) in matched_dlls_by_target {
                if let Some(st) = target_statuses.get(&tid) {
                    if !is_self_dependency(&app.display_name, &st.target) {
                        dlls.sort();
                        dlls.dedup();
                        let preview = dlls.into_iter().take(4).collect::<Vec<_>>().join(", ");
                        upsert_dep(
                            &mut deps_map,
                            build_target_dep(
                                st,
                                DependencyConfidence::Proven,
                                format!("PE import: {preview}"),
                                true,
                            ),
                        );
                    }
                }
            }

            if ev.has_deps_json {
                let mut attached_versioned = false;
                for &maj in &ev.dotnet_major_versions {
                    let vtid = format!("dotnet-{maj}");
                    if let Some(st) = target_statuses.get(&vtid) {
                        if !is_self_dependency(&app.display_name, &st.target) {
                            let f = ev
                                .deps_json_files
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "*.deps.json".into());
                            upsert_dep(
                                &mut deps_map,
                                build_target_dep(
                                    st,
                                    DependencyConfidence::Proven,
                                    format!(".NET metadata: {f} (.NET {maj})"),
                                    true,
                                ),
                            );
                            attached_versioned = true;
                        }
                    }
                }
                if !attached_versioned {
                    if let Some(st) = target_statuses.get("dotnet-runtime") {
                        if !is_self_dependency(&app.display_name, &st.target) {
                            let f = ev
                                .deps_json_files
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "*.deps.json".into());
                            upsert_dep(
                                &mut deps_map,
                                build_target_dep(
                                    st,
                                    DependencyConfidence::Proven,
                                    format!(".deps.json: {f}"),
                                    true,
                                ),
                            );
                        }
                    }
                }
            }

            for (raw_tid, basis) in &ev.manifest_targets {
                let resolved_tid = if raw_tid == "vcpp-generic" {
                    deps_map
                        .keys()
                        .find(|k| k.starts_with("vcpp-"))
                        .cloned()
                        .unwrap_or_else(|| "vcpp-v14".to_string())
                } else {
                    raw_tid.clone()
                };
                if let Some(st) = target_statuses.get(&resolved_tid) {
                    if !is_self_dependency(&app.display_name, &st.target) {
                        upsert_dep(
                            &mut deps_map,
                            build_target_dep(st, DependencyConfidence::Proven, basis.clone(), false),
                        );
                    }
                }
            }
        }

        // Source (2): Windows Services / Drivers whose ImagePath points into app's folders (PROVEN).
        let app_folder_keys: Vec<String> = app
            .items
            .iter()
            .filter(|i| !i.path.ends_with("[files]"))
            .map(|i| normalize_path_key(&i.path))
            .collect();
        for svc in services {
            if svc.normalized_image_path.is_empty() {
                continue;
            }
            let img_key = normalize_path_key(&svc.normalized_image_path);
            let points_into_app = app_folder_keys
                .iter()
                .any(|fk| fk.len() > 3 && is_subpath_of(&img_key, fk));
            if !points_into_app {
                continue;
            }

            let dep_on = if svc.depend_on_service.is_empty() {
                String::new()
            } else {
                format!(" (DependOnService: {})", svc.depend_on_service.join(", "))
            };
            let basis = format!("service: {}{}", svc.service_name, dep_on);

            if let Some(tid) = svc_to_target.get(&svc.service_name.to_ascii_lowercase()) {
                if let Some(st) = target_statuses.get(tid) {
                    if !is_self_dependency(&app.display_name, &st.target) {
                        upsert_dep(
                            &mut deps_map,
                            build_target_dep(st, DependencyConfidence::Proven, basis, false),
                        );
                    }
                }
            } else {
                let kind_lbl = if svc.service_type <= 8 { "driver" } else { "service" };
                upsert_dep(
                    &mut deps_map,
                    PendingDep {
                        target_id: format!("svc:{}", svc.service_name.to_ascii_lowercase()),
                        target_name: if svc.display_name.is_empty() {
                            svc.service_name.clone()
                        } else {
                            format!("{} ({})", svc.display_name, svc.service_name)
                        },
                        kind: DependencyKind::AntiCheatServiceDriver,
                        present: true,
                        own_size_bytes: None,
                        shared: false,
                        confidence: DependencyConfidence::Proven,
                        bases: vec![format!("{kind_lbl}: {}{}", svc.service_name, dep_on)],
                        imported_by_binary: false,
                    },
                );
            }
        }

        // Source (2b): Companion Windows service + install-folder/publisher kinship (PROVEN).
        if app.no_touch.is_none()
            && !app
                .bases
                .contains(&crate::model::AttributionBasis::StandaloneFolder)
        {
            let app_vendors: BTreeSet<String> = app
                .publishers
                .iter()
                .map(|p| normalize_vendor_key(p))
                .filter(|v| v.len() >= 3)
                .collect();
            let app_is_launcher = app.display_name.to_ascii_lowercase().contains("launcher")
                || target_statuses.iter().any(|(_, s)| {
                    s.target.kind == DependencyKind::PlatformLauncher
                        && is_self_dependency(&app.display_name, &s.target)
                });
            let mut item_tokens_lower: Vec<String> = app
                .items
                .iter()
                .map(|it| it.path.to_ascii_lowercase())
                .collect();
            if let Some(ev) = app_ev {
                item_tokens_lower.extend(ev.companion_dir_tokens.iter().cloned());
            }

            for (tid, st) in target_statuses {
                if is_self_dependency(&app.display_name, &st.target) {
                    continue;
                }
                let registered_svcs: Vec<&WindowsServiceEntry> = st
                    .target
                    .service_names
                    .iter()
                    .filter_map(|sn| svc_by_name.get(&sn.to_ascii_lowercase()).copied())
                    .collect();
                if registered_svcs.is_empty() {
                    continue;
                }

                let target_compact = normalize_app_token(&st.target.name).replace(' ', "");
                let folder_kinship = (!target_compact.is_empty()
                    && item_tokens_lower
                        .iter()
                        .any(|tok| tok.replace(' ', "").contains(&target_compact)))
                    || (tid == "epic-online-services"
                        && item_tokens_lower.iter().any(|tok| tok.contains("eosoverlay")))
                    || registered_svcs.iter().any(|svc| {
                        let sk = svc.service_name.to_ascii_lowercase();
                        sk.len() >= 6 && item_tokens_lower.iter().any(|tok| tok.contains(&sk))
                    });

                let same_pub_launcher_kinship = app_is_launcher
                    && target_sibling_vendors
                        .get(tid)
                        .is_some_and(|tv| app_vendors.iter().any(|v| tv.contains(v)));

                if folder_kinship || same_pub_launcher_kinship {
                    let svc = registered_svcs[0];
                    let mut basis_parts = vec![format!("service: {}", svc.service_name)];
                    if folder_kinship {
                        basis_parts.push("companion folder kinship".to_string());
                    }
                    if same_pub_launcher_kinship {
                        basis_parts.push("same-publisher companion service".to_string());
                    }
                    upsert_dep(
                        &mut deps_map,
                        build_target_dep(
                            st,
                            DependencyConfidence::Proven,
                            basis_parts.join(" + "),
                            false,
                        ),
                    );
                }
            }

            // Source (2c): Split-install own registered service (e.g., MSI cache on C:, service exe on Y:).
            let app_compact = normalize_app_token(&app.display_name).replace(' ', "");
            if app_compact.len() >= 5 {
                for svc in services {
                    if svc.normalized_image_path.is_empty()
                        || svc
                            .normalized_image_path
                            .to_ascii_lowercase()
                            .contains("\\windows\\")
                    {
                        continue;
                    }
                    let svc_compact = normalize_app_token(&svc.service_name).replace(' ', "");
                    if svc_compact == app_compact {
                        let svc_tid = svc_to_target
                            .get(&svc.service_name.to_ascii_lowercase())
                            .cloned()
                            .unwrap_or_else(|| format!("svc:{}", svc.service_name.to_ascii_lowercase()));
                        if let Some(st) = target_statuses.get(&svc_tid) {
                            if !is_self_dependency(&app.display_name, &st.target) {
                                upsert_dep(
                                    &mut deps_map,
                                    build_target_dep(
                                        st,
                                        DependencyConfidence::Proven,
                                        format!("service: {}", svc.service_name),
                                        false,
                                    ),
                                );
                            }
                        } else if !deps_map.contains_key(&svc_tid) {
                            let kind_lbl = if svc.service_type <= 8 { "driver" } else { "service" };
                            upsert_dep(
                                &mut deps_map,
                                PendingDep {
                                    target_id: svc_tid,
                                    target_name: if svc.display_name.is_empty() {
                                        svc.service_name.clone()
                                    } else {
                                        format!("{} ({})", svc.display_name, svc.service_name)
                                    },
                                    kind: DependencyKind::AntiCheatServiceDriver,
                                    present: true,
                                    own_size_bytes: None,
                                    shared: false,
                                    confidence: DependencyConfidence::Proven,
                                    bases: vec![format!("{kind_lbl}: {}", svc.service_name)],
                                    imported_by_binary: false,
                                },
                            );
                        }
                    }
                }
            }
        }

        // Source (3): Steam appmanifest_*.acf (PROVEN) & folder fingerprints (INFERRED).
        if let Some(ev) = app_ev {
            if !ev.steam_app_manifests.is_empty() {
                if let Some(st) = target_statuses.get("steam") {
                    if !is_self_dependency(&app.display_name, &st.target) {
                        upsert_dep(
                            &mut deps_map,
                            build_target_dep(
                                st,
                                DependencyConfidence::Proven,
                                format!("manifest: {}", ev.steam_app_manifests.join(", ")),
                                false,
                            ),
                        );
                    }
                }
                if ev.steam_ea_script_present {
                    if let Some(st) = target_statuses.get("ea-app") {
                        if !is_self_dependency(&app.display_name, &st.target) {
                            upsert_dep(
                                &mut deps_map,
                                build_target_dep(
                                    st,
                                    DependencyConfidence::Proven,
                                    format!(
                                        "manifest: {} (EAappInstaller_installScript.vdf)",
                                        ev.steam_app_manifests[0]
                                    ),
                                    false,
                                ),
                            );
                        }
                    }
                }
            }

            let files_set: BTreeSet<&str> = ev.file_names_lower.iter().map(String::as_str).collect();
            for fp in &catalog.fingerprints {
                let matched: Vec<&str> = fp
                    .files
                    .iter()
                    .filter(|f| files_set.contains(f.to_ascii_lowercase().as_str()))
                    .map(String::as_str)
                    .collect();
                if !matched.is_empty() {
                    if let Some(st) = target_statuses.get(&fp.target_id) {
                        if !is_self_dependency(&app.display_name, &st.target) {
                            upsert_dep(
                                &mut deps_map,
                                build_target_dep(
                                    st,
                                    DependencyConfidence::Inferred,
                                    format!("fingerprint: {}", matched.join(", ")),
                                    false,
                                ),
                            );
                        }
                    }
                }
            }
        }

        // Source (4): Curated dependency catalog rules (INFERRED).
        let app_name_lower = app.display_name.to_ascii_lowercase();
        let pub_lower: Vec<String> = app
            .publishers
            .iter()
            .map(|p| p.to_ascii_lowercase())
            .collect();
        let install_lower = app
            .install_location
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();

        for rule in &catalog.app_rules {
            let name_ok = rule.app_matches.is_empty()
                || rule
                    .app_matches
                    .iter()
                    .any(|m| app_name_lower.contains(&m.to_ascii_lowercase()));
            let pub_ok = rule.publisher_matches.is_empty()
                || rule.publisher_matches.iter().any(|pm| {
                    let needle = pm.to_ascii_lowercase();
                    pub_lower.iter().any(|p| p.contains(&needle))
                });
            let path_ok = rule
                .install_path_contains
                .as_deref()
                .is_none_or(|sub| install_lower.contains(&sub.to_ascii_lowercase()));

            if name_ok && pub_ok && path_ok {
                if let Some(st) = target_statuses.get(&rule.target_id) {
                    if !is_self_dependency(&app.display_name, &st.target) {
                        upsert_dep(
                            &mut deps_map,
                            build_target_dep(
                                st,
                                DependencyConfidence::Inferred,
                                format!("catalog: {}", rule.reason),
                                false,
                            ),
                        );
                    }
                }
            }
        }

        for (tid, pending) in &mut deps_map {
            let already_has_svc = pending
                .bases
                .iter()
                .any(|b| b.starts_with("service: ") || b.starts_with("driver: "));
            if !already_has_svc {
                if let Some(st) = target_statuses.get(tid) {
                    let mut svc_notes = Vec::new();
                    for sname in &st.target.service_names {
                        if let Some(svc) = svc_by_name.get(&sname.to_ascii_lowercase()) {
                            let kind_lbl = if svc.service_type <= 8 {
                                "driver"
                            } else {
                                "service"
                            };
                            let dep_on = if svc.depend_on_service.is_empty() {
                                String::new()
                            } else {
                                format!(" (DependOnService: {})", svc.depend_on_service.join(", "))
                            };
                            svc_notes.push(format!("{kind_lbl}: {}{}", svc.service_name, dep_on));
                        }
                    }
                    if !svc_notes.is_empty() {
                        pending
                            .bases
                            .push(format!("registered {}", svc_notes.join(", ")));
                    }
                }
            }
        }

        let mut list: Vec<DependencyRecord> = deps_map
            .into_values()
            .map(|p| DependencyRecord {
                target_id: p.target_id,
                target_name: p.target_name,
                kind: p.kind,
                present: p.present,
                own_size_bytes: p.own_size_bytes,
                shared: p.shared,
                basis: p.bases.join(" + "),
                confidence: p.confidence,
                imported_by_binary: p.imported_by_binary,
            })
            .collect();

        list.sort_by(|a, b| {
            a.confidence
                .cmp(&b.confidence)
                .then_with(|| a.target_name.cmp(&b.target_name))
        });
        app.dependencies = list;
    }

    // Build reverse dependency view by inverting the forward graph across all apps.
    let mut rev_map: HashMap<String, ReverseDependencyEntry> = HashMap::new();
    for app in &report.apps {
        for dep in &app.dependencies {
            let entry = rev_map
                .entry(dep.target_id.clone())
                .or_insert_with(|| ReverseDependencyEntry {
                    target_id: dep.target_id.clone(),
                    target_name: dep.target_name.clone(),
                    kind: dep.kind,
                    present: dep.present,
                    own_size_bytes: dep.own_size_bytes,
                    dependent_count: 0,
                    binary_importer_count: 0,
                    consumers: Vec::new(),
                });
            entry.consumers.push(ReverseConsumer {
                app_id: app.id.clone(),
                app_name: app.display_name.clone(),
                app_total_bytes: app.total_bytes,
                confidence: dep.confidence,
                basis: dep.basis.clone(),
                imported_by_binary: dep.imported_by_binary,
            });
        }
    }

    let mut reverse_list: Vec<ReverseDependencyEntry> = rev_map
        .into_values()
        .map(|mut e| {
            e.consumers.sort_by(|a, b| {
                b.app_total_bytes
                    .cmp(&a.app_total_bytes)
                    .then_with(|| a.app_name.cmp(&b.app_name))
            });
            e.dependent_count = e.consumers.len();
            e.binary_importer_count = e.consumers.iter().filter(|c| c.imported_by_binary).count();
            e
        })
        .collect();

    reverse_list.sort_by(|a, b| {
        b.dependent_count
            .cmp(&a.dependent_count)
            .then_with(|| a.target_name.cmp(&b.target_name))
    });
    report.reverse_dependencies = reverse_list;
}
