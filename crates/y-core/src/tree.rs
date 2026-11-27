use std::collections::{HashMap, HashSet};

use crate::attribution::RootExeCandidate;
use crate::config::{ContainerRulebook, PinKind, ZoneRulebook};
use crate::discovery::{
    AppTokenMatchKind, is_adobe_product_app, is_subpath_of, match_adobe_product_subfolder,
    normalize_app_token, normalize_path_key, path_drive_letter, split_alnum_words,
};
use crate::model::{FolderNode, InstalledApp, ManifestAppEntry, ScannedUnit, ScannedUnitKind};

const ROOT_SYSTEM_FILE_NAMES: &[&str] = &[
    "hiberfil.sys",
    "pagefile.sys",
    "swapfile.sys",
    "dumpstack.log.tmp",
    "dumpstack.log",
];

#[derive(Debug, Clone, Default)]
pub struct DecompositionOptions<'a> {
    pub zones: Option<&'a ZoneRulebook>,
    pub manifest_entries: &'a [ManifestAppEntry],
    pub exe_candidates: Option<&'a HashMap<String, RootExeCandidate>>,
}

#[must_use]
pub fn is_root_system_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ROOT_SYSTEM_FILE_NAMES.contains(&lower.as_str())
}

#[must_use]
pub fn is_personal_shell_folder(path: &str, shell_folders: &[String]) -> bool {
    let pk = normalize_path_key(path);
    shell_folders.iter().any(|sf| normalize_path_key(sf) == pk)
}

#[must_use]
pub fn is_program_files_inbox_folder(path_lower: &str, leaf_lower: &str) -> bool {
    (path_lower.contains(":\\program files\\") || path_lower.contains(":\\program files (x86)\\"))
        && matches!(
            leaf_lower,
            "common files"
                | "windowsapps"
                | "windows defender"
                | "windows mail"
                | "windows media player"
                | "windows nt"
                | "windows photo viewer"
                | "windows sidebar"
                | "windowspowershell"
                | "internet explorer"
                | "iis"
                | "iis express"
                | "modifiablewindowsapps"
                | "uninstall information"
                | "reference assemblies"
                | "microsoft"
                | "msbuild"
                | "ruxim"
                | "application verifier"
                | "xboxservice"
                | "activation-renewal"
        )
}

fn is_drive_root_child_key(pk: &str) -> bool {
    let trimmed = pk.trim_end_matches('\\');
    let parts: Vec<&str> = trimmed.split('\\').collect();
    parts.len() == 2 && parts[0].len() == 2 && parts[0].ends_with(':')
}

fn is_known_os_drive_root_folder(pk: &str) -> bool {
    if !is_drive_root_child_key(pk) {
        return false;
    }
    let leaf = pk.trim_end_matches('\\').rsplit('\\').next().unwrap_or("");
    leaf.starts_with("$recycle.bin")
        || leaf == "system volume information"
        || leaf == "recovery"
        || leaf == "$winreagent"
        || leaf == "config.msi"
        || leaf == "$windows.~bt"
        || leaf == "$windows.~ws"
        || leaf == "$sysreset"
        || leaf == "perflogs"
        || leaf == "msocache"
        || leaf == ".gamingroot"
        || leaf == "documents and settings"
        || leaf == "windows"
        || (leaf.starts_with("found.") && leaf[6..].chars().all(|c| c.is_ascii_digit()))
}

/// Returns `true` for standard architecture, version, binary, or asset subdirectories inside an app
/// that must never be counted as independent products during group-node detection.
#[must_use]
pub fn is_non_app_internal_subdir_name(name_lower: &str) -> bool {
    if matches!(
        name_lower,
        "x86"
            | "x64"
            | "amd64"
            | "arm64"
            | "win32"
            | "win64"
            | "i386"
            | "bin"
            | "binaries"
            | "lib"
            | "libs"
            | "ux"
            | "ui"
            | "common"
            | "redist"
            | "_commonredist"
            | "support"
            | "locales"
            | "resources"
            | "plugins"
            | "modules"
            | "data"
            | "assets"
            | "cache"
            | "logs"
            | "temp"
            | "tmp"
            | "drivers"
            | "updater"
            | "update"
    ) {
        return true;
    }
    if let Some(rest) = name_lower.strip_prefix('v') {
        if !rest.is_empty()
            && rest.as_bytes()[0].is_ascii_digit()
            && rest
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
        {
            return true;
        }
    }
    false
}

struct ClaimIndex {
    install_owners_by_key: HashMap<String, HashSet<String>>,
    all_claim_keys: Vec<String>,
    installed_composed_names: HashSet<Vec<String>>,
    installed_exact_tokens: HashSet<String>,
    adobe_product_apps: Vec<InstalledApp>,
}

impl ClaimIndex {
    fn build(
        apps: &[InstalledApp],
        zones: &ZoneRulebook,
        manifest_entries: &[ManifestAppEntry],
    ) -> Self {
        let mut install_owners_by_key: HashMap<String, HashSet<String>> = HashMap::new();
        let mut key_set: HashSet<String> = HashSet::new();
        let mut installed_composed_names: HashSet<Vec<String>> = HashSet::new();
        let mut installed_exact_tokens: HashSet<String> = HashSet::new();

        for app in apps {
            let name_words = split_alnum_words(&app.display_name);
            if name_words.len() >= 2 {
                installed_composed_names.insert(name_words);
            }
            if !app.is_store_fallback {
                let dn_tok = normalize_app_token(&app.display_name);
                if dn_tok.len() >= 3 {
                    installed_exact_tokens.insert(dn_tok);
                }
                for alias in &app.aliases {
                    let atok = normalize_app_token(alias);
                    if atok.len() >= 3 {
                        installed_exact_tokens.insert(atok);
                    }
                }
            }
            for alias in &app.aliases {
                let aw = split_alnum_words(alias);
                if aw.len() >= 2 {
                    installed_composed_names.insert(aw);
                }
            }
            for loc in &app.all_install_locations {
                let lk = normalize_path_key(loc);
                if lk.len() > 3 {
                    install_owners_by_key
                        .entry(lk.clone())
                        .or_default()
                        .insert(app.id.clone());
                    key_set.insert(lk);
                }
            }
        }

        for m in manifest_entries {
            let ik = normalize_path_key(&m.install_dir);
            if ik.len() > 3 {
                install_owners_by_key
                    .entry(ik.clone())
                    .or_default()
                    .insert(m.id.clone());
                key_set.insert(ik);
            }
            let ck = normalize_path_key(&m.container_dir);
            if ck.len() > 3 {
                key_set.insert(ck);
            }
        }

        for ez in &zones.explicit_zones {
            let ek = normalize_path_key(&ez.path);
            if ek.len() > 3 {
                key_set.insert(ek);
            }
        }
        for pin in &zones.pins {
            let pk = normalize_path_key(&pin.path);
            if pk.len() > 3 {
                key_set.insert(pk);
            }
        }

        Self {
            install_owners_by_key,
            all_claim_keys: key_set.into_iter().collect(),
            installed_composed_names,
            installed_exact_tokens,
            adobe_product_apps: apps
                .iter()
                .filter(|app| is_adobe_product_app(app))
                .cloned()
                .collect(),
        }
    }

    fn has_adobe_product_child(&self, node: &FolderNode) -> bool {
        if self.adobe_product_apps.is_empty() {
            return false;
        }
        node.children.iter().any(|ch| {
            self.adobe_product_apps.iter().any(|app| {
                matches!(
                    match_adobe_product_subfolder(app, &ch.name),
                    Some(AppTokenMatchKind::ExactFullToken | AppTokenMatchKind::WholeWordSlice)
                )
            })
        })
    }

    fn has_own_app_claim(&self, node: &FolderNode, pk: &str) -> bool {
        if self.has_exact_claim(pk) {
            return true;
        }
        let tok = normalize_app_token(&node.name);
        tok.len() >= 3 && self.installed_exact_tokens.contains(&tok)
    }

    fn has_strict_nested_claim(&self, pk: &str) -> bool {
        let owners_here = self.install_owners_by_key.get(pk);
        for tk in &self.all_claim_keys {
            if tk == pk || !is_subpath_of(tk, pk) {
                continue;
            }
            if let Some(child_owners) = self.install_owners_by_key.get(tk) {
                let only_same_owner = owners_here.is_some_and(|here| {
                    child_owners.iter().all(|co| here.contains(co))
                });
                if only_same_owner {
                    // Different sub-install location or same app root doesn't require self-carve unless multiple owners exist,
                    // wait: if `pk` is NOT an install root or another owner lives inside `tk`, split!
                    if owners_here.is_some() && child_owners.len() <= 1 {
                        continue;
                    }
                }
            }
            return true;
        }
        false
    }

    fn has_exact_or_nested_claim(&self, pk: &str) -> bool {
        self.all_claim_keys
            .iter()
            .any(|tk| tk == pk || is_subpath_of(tk, pk))
    }

    fn has_exact_claim(&self, pk: &str) -> bool {
        self.install_owners_by_key.contains_key(pk)
    }

    fn should_split_for_two_segment(&self, node: &FolderNode, pk: &str) -> bool {
        if self.has_exact_claim(pk) || node.children.is_empty() {
            return false;
        }
        let parent_key = pk
            .trim_end_matches('\\')
            .rsplit_once('\\')
            .map(|(p, _)| p)
            .unwrap_or("");
        let in_publisher_parent = parent_key.ends_with("\\appdata\\local")
            || parent_key.ends_with("\\appdata\\roaming")
            || parent_key.ends_with("\\appdata\\locallow")
            || parent_key.ends_with(":\\programdata")
            || parent_key.ends_with(":\\program files")
            || parent_key.ends_with(":\\program files (x86)")
            || parent_key.ends_with("\\common files");
        if !in_publisher_parent {
            return false;
        }
        let p_words = split_alnum_words(&node.name);
        if p_words.is_empty() {
            return false;
        }
        node.children.iter().any(|ch| {
            let l_words = split_alnum_words(&ch.name);
            if l_words.is_empty() {
                return false;
            }
            let mut combined = p_words.clone();
            combined.extend(l_words);
            self.installed_composed_names.contains(&combined)
        })
    }
}

fn is_structural_container_dir(
    path: &str,
    pk: &str,
    zones: &ZoneRulebook,
    manifest_containers: &HashSet<String>,
) -> bool {
    // Drive roots (e.g. c:\, d:\)
    if pk.len() == 3 && pk.ends_with(":\\") {
        return true;
    }

    // User-configured or default zone rulebook (`AppContainer`) or strict ancestor of explicit rule/pin.
    if zones.matches_app_container_dir(path).is_some()
        || zones.is_strict_ancestor_of_rule_or_pin(path)
    {
        return true;
    }

    if manifest_containers.contains(pk) {
        return true;
    }

    let suffix_containers = [
        "\\users",
        "\\appdata",
        "\\appdata\\local",
        "\\appdata\\roaming",
        "\\appdata\\locallow",
        "\\appdata\\local\\programs",
        "\\appdata\\local\\programs\\common",
        "\\appdata\\local\\packages",
        "\\appdata\\local\\microsoft",
        "\\appdata\\roaming\\microsoft",
        "\\appdata\\locallow\\microsoft",
        "\\appdata\\local\\google",
        "\\appdata\\roaming\\google",
        "\\appdata\\local\\jetbrains",
        "\\appdata\\roaming\\jetbrains",
        "\\appdata\\locallow\\colossal order",
        "\\appdata\\local\\microsoft\\windows",
        "\\appdata\\local\\nvidia",
        "\\appdata\\local\\nvidia corporation",
        "\\appdata\\local\\amd",
        "\\program files",
        "\\program files (x86)",
        "\\program files\\common files",
        "\\program files (x86)\\common files",
        "\\program files\\windowsapps",
        "\\programdata",
        "\\programdata\\microsoft",
        "\\programdata\\package cache",
        "\\programdata\\nvidia",
        "\\programdata\\nvidia corporation",
        "\\xboxgames",
        "\\gog games",
        "\\riot games",
        "\\steam",
        "\\steam\\steamapps",
        "\\steam\\steamapps\\common",
        "\\steamlibrary",
        "\\steamlibrary\\steamapps",
        "\\steamlibrary\\steamapps\\common",
        "\\epic games",
    ];
    if suffix_containers
        .iter()
        .any(|sfx| pk.ends_with(sfx) || (pk.len() > 2 && &pk[2..] == *sfx))
    {
        return true;
    }

    if pk.ends_with("\\steam")
        || pk.ends_with("\\steam\\steamapps")
        || pk.ends_with("\\steam\\steamapps\\common")
        || pk.ends_with("\\steamlibrary")
        || pk.ends_with("\\steamlibrary\\steamapps")
        || pk.ends_with("\\steamlibrary\\steamapps\\common")
    {
        return true;
    }

    // C:\Users\<Username> (3 path segments) and C:\Users\<Username>\Microsoft (4 segments)
    let parts: Vec<&str> = pk.trim_end_matches('\\').split('\\').collect();
    if parts.len() == 3 && parts[1] == "users" {
        return true;
    }
    if parts.len() == 4 && parts[1] == "users" && parts[3] == "microsoft" {
        return true;
    }

    false
}

fn should_split_for_inference(
    node: &FolderNode,
    pk: &str,
    zones: &ZoneRulebook,
    claims: &ClaimIndex,
    opts: &DecompositionOptions<'_>,
) -> bool {
    if node.children.is_empty() {
        return false;
    }
    match zones.pin_for_path(&node.path) {
        Some(PinKind::Group) => return true,
        Some(PinKind::App | PinKind::Personal | PinKind::Ignore) => return false,
        None => {}
    }

    let Some(exe_map) = opts.exe_candidates else {
        return false;
    };
    let parent_trimmed = pk.trim_end_matches('\\');
    let parent_path = parent_trimmed
        .rsplit_once('\\')
        .map(|(p, _)| p)
        .unwrap_or("");
    let is_app_container_child = zones.matches_app_container_dir(parent_path).is_some();
    let is_root_candidate = is_drive_root_child_key(pk) && !is_known_os_drive_root_folder(pk);
    if !is_root_candidate && !is_app_container_child {
        return false;
    }
    let leaf_lower = node.name.to_ascii_lowercase();
    if is_app_container_child && is_program_files_inbox_folder(pk, &leaf_lower) {
        return false;
    }

    // Part B 5(a): own-level app evidence (registered root/exact app or direct exe) stops inference descent.
    if claims.has_own_app_claim(node, pk) || exe_map.contains_key(pk) {
        return false;
    }

    // Part B 5(b): count immediate non-internal children that pass (a); >= 2 means Group node -> split!
    let exe_child_count = node
        .children
        .iter()
        .filter(|ch| {
            !is_non_app_internal_subdir_name(&ch.name.to_ascii_lowercase())
                && exe_map.contains_key(&normalize_path_key(&ch.path))
        })
        .count();
    exe_child_count >= 2
}

fn emit_loose_remainder_if_any(
    node: &FolderNode,
    pk: &str,
    drive_letter: char,
    claims: &ClaimIndex,
    out: &mut Vec<ScannedUnit>,
) {
    if node.direct_file_bytes == 0 {
        return;
    }
    let parts: Vec<&str> = pk.trim_end_matches('\\').split('\\').collect();
    let is_registry_hive_container = (parts.len() == 3 && parts[1] == "users")
        || pk.ends_with("\\appdata\\local\\microsoft\\windows");
    let kind = if is_registry_hive_container {
        ScannedUnitKind::RegistryUserHive
    } else {
        ScannedUnitKind::ContainerLooseFiles
    };
    out.push(ScannedUnit {
        path: node.path.clone(),
        drive_letter,
        bytes: node.direct_file_bytes,
        kind,
        is_direct_install_root: claims.has_exact_claim(pk),
    });
}

fn carve_inside_personal(
    node: &FolderNode,
    drive_letter: char,
    shell_folders: &[String],
    zones: &ZoneRulebook,
    manifest_containers: &HashSet<String>,
    claims: &ClaimIndex,
    opts: &DecompositionOptions<'_>,
    personal_accum: &mut u64,
    out: &mut Vec<ScannedUnit>,
) {
    let pk = normalize_path_key(&node.path);
    *personal_accum = personal_accum.saturating_add(node.direct_file_bytes);
    for ch in &node.children {
        let ck = normalize_path_key(&ch.path);
        if !claims.has_exact_or_nested_claim(&ck) {
            *personal_accum = personal_accum.saturating_add(ch.total_bytes);
        } else if claims.has_exact_claim(&ck) && !claims.has_strict_nested_claim(&ck) {
            visit_node(
                ch,
                drive_letter,
                shell_folders,
                zones,
                manifest_containers,
                claims,
                opts,
                out,
            );
        } else if claims.has_exact_claim(&ck) {
            visit_node(
                ch,
                drive_letter,
                shell_folders,
                zones,
                manifest_containers,
                claims,
                opts,
                out,
            );
        } else {
            carve_inside_personal(
                ch,
                drive_letter,
                shell_folders,
                zones,
                manifest_containers,
                claims,
                opts,
                personal_accum,
                out,
            );
        }
    }
    let _ = pk;
}

fn is_adobe_shared_container_key(pk: &str) -> bool {
    let trimmed = pk.trim_end_matches('\\');
    trimmed.ends_with("\\common files\\adobe")
        || trimmed.ends_with("\\appdata\\roaming\\adobe")
        || trimmed.ends_with("\\appdata\\local\\adobe")
        || trimmed.ends_with("\\appdata\\locallow\\adobe")
}

#[allow(clippy::too_many_arguments)]
fn visit_node(
    node: &FolderNode,
    drive_letter: char,
    shell_folders: &[String],
    zones: &ZoneRulebook,
    manifest_containers: &HashSet<String>,
    claims: &ClaimIndex,
    opts: &DecompositionOptions<'_>,
    out: &mut Vec<ScannedUnit>,
) {
    let pk = normalize_path_key(&node.path);

    if zones.matches_personal_dir(&node.path, shell_folders) {
        if !claims.has_strict_nested_claim(&pk) || node.children.is_empty() {
            out.push(ScannedUnit {
                path: node.path.clone(),
                drive_letter,
                bytes: node.total_bytes,
                kind: ScannedUnitKind::PersonalShell,
                is_direct_install_root: false,
            });
        } else {
            let mut personal_accum = 0u64;
            carve_inside_personal(
                node,
                drive_letter,
                shell_folders,
                zones,
                manifest_containers,
                claims,
                opts,
                &mut personal_accum,
                out,
            );
            if personal_accum > 0 {
                out.push(ScannedUnit {
                    path: node.path.clone(),
                    drive_letter,
                    bytes: personal_accum,
                    kind: ScannedUnitKind::PersonalShell,
                    is_direct_install_root: false,
                });
            }
        }
        return;
    }

    let must_split = !node.children.is_empty()
        && (claims.has_strict_nested_claim(&pk)
            || is_structural_container_dir(&node.path, &pk, zones, manifest_containers)
            || (is_adobe_shared_container_key(&pk) && claims.has_adobe_product_child(node))
            || claims.should_split_for_two_segment(node, &pk)
            || should_split_for_inference(node, &pk, zones, claims, opts));

    if must_split {
        emit_loose_remainder_if_any(node, &pk, drive_letter, claims, out);
        for child in &node.children {
            visit_node(
                child,
                drive_letter,
                shell_folders,
                zones,
                manifest_containers,
                claims,
                opts,
                out,
            );
        }
        return;
    }

    out.push(ScannedUnit {
        path: node.path.clone(),
        drive_letter,
        bytes: node.total_bytes,
        kind: ScannedUnitKind::Regular,
        is_direct_install_root: claims.has_exact_claim(&pk),
    });
}

pub fn decompose_scanned_tree(
    root: &FolderNode,
    apps: &[InstalledApp],
    shell_folders: &[String],
    root_system_files: &[(String, u64)],
) -> Vec<ScannedUnit> {
    decompose_scanned_tree_with_options(
        root,
        apps,
        shell_folders,
        root_system_files,
        &DecompositionOptions::default(),
    )
}

pub fn decompose_scanned_tree_with_containers(
    root: &FolderNode,
    apps: &[InstalledApp],
    shell_folders: &[String],
    root_system_files: &[(String, u64)],
    containers: &ContainerRulebook,
) -> Vec<ScannedUnit> {
    let zones = ZoneRulebook::from_legacy_containers(containers);
    decompose_scanned_tree_with_options(
        root,
        apps,
        shell_folders,
        root_system_files,
        &DecompositionOptions {
            zones: Some(&zones),
            manifest_entries: &[],
            exe_candidates: None,
        },
    )
}

pub fn decompose_scanned_tree_with_options(
    root: &FolderNode,
    apps: &[InstalledApp],
    shell_folders: &[String],
    root_system_files: &[(String, u64)],
    opts: &DecompositionOptions<'_>,
) -> Vec<ScannedUnit> {
    let default_zones = ZoneRulebook::default();
    let zones = opts.zones.unwrap_or(&default_zones);
    let claims = ClaimIndex::build(apps, zones, opts.manifest_entries);
    let manifest_containers: HashSet<String> = opts
        .manifest_entries
        .iter()
        .map(|m| normalize_path_key(&m.container_dir))
        .filter(|s| s.len() > 3)
        .collect();

    let mut units = Vec::new();
    let drive_letter = path_drive_letter(&root.path).unwrap_or('C');

    for (path, bytes) in root_system_files {
        units.push(ScannedUnit {
            path: path.clone(),
            drive_letter: path_drive_letter(path).unwrap_or(drive_letter),
            bytes: *bytes,
            kind: ScannedUnitKind::RootSystemFile,
            is_direct_install_root: false,
        });
    }

    // Drive-root direct non-system loose files keep the real drive root path (never "[root files]").
    if root.direct_file_bytes > 0 {
        units.push(ScannedUnit {
            path: root.path.clone(),
            drive_letter,
            bytes: root.direct_file_bytes,
            kind: ScannedUnitKind::ContainerLooseFiles,
            is_direct_install_root: false,
        });
    }

    for child in &root.children {
        visit_node(
            child,
            drive_letter,
            shell_folders,
            zones,
            &manifest_containers,
            &claims,
            opts,
            &mut units,
        );
    }

    units
}

/// Deduplicates secondary-drive `InstallLocation` folders when one install path is a strict
/// subdirectory of another (e.g. `Z:\Games\Steam` containing `Z:\Games\Steam\steamapps\common\Slay the Spire 2`),
/// subtracting nested child bytes from the outer directory so zero bytes are double-counted.
pub fn deduplicate_secondary_install_trees(raw_trees: &[(String, u64)]) -> Vec<ScannedUnit> {
    let mut unique: Vec<(String, u64)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (p, bytes) in raw_trees {
        let k = normalize_path_key(p);
        if seen.insert(k) {
            unique.push((p.clone(), *bytes));
        }
    }

    unique.sort_by_key(|(p, _)| p.len());

    let mut adjusted_bytes: Vec<u64> = unique.iter().map(|(_, b)| *b).collect();
    for i in 0..unique.len() {
        let parent_key = normalize_path_key(&unique[i].0);
        let mut child_indices: Vec<usize> = Vec::new();
        for j in (i + 1)..unique.len() {
            let child_key = normalize_path_key(&unique[j].0);
            if child_key != parent_key && is_subpath_of(&child_key, &parent_key) {
                let shadowed_by_earlier_child = child_indices
                    .iter()
                    .any(|&ci| is_subpath_of(&child_key, &normalize_path_key(&unique[ci].0)));
                if !shadowed_by_earlier_child {
                    child_indices.push(j);
                }
            }
        }
        let sub_sum: u64 = child_indices.iter().map(|&ci| unique[ci].1).sum();
        adjusted_bytes[i] = unique[i].1.saturating_sub(sub_sum);
    }

    unique
        .into_iter()
        .zip(adjusted_bytes)
        .filter(|(_, b)| *b > 0)
        .map(|((path, _), bytes)| ScannedUnit {
            drive_letter: path_drive_letter(&path).unwrap_or('D'),
            path,
            bytes,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: true,
        })
        .collect()
}
