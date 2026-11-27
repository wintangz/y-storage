use std::collections::HashSet;

use crate::config::ContainerRulebook;
use crate::discovery::{is_subpath_of, normalize_path_key, path_drive_letter};
use crate::model::{FolderNode, InstalledApp, ScannedUnit, ScannedUnitKind};

const ROOT_SYSTEM_FILE_NAMES: &[&str] = &[
    "hiberfil.sys",
    "pagefile.sys",
    "swapfile.sys",
    "dumpstack.log.tmp",
    "dumpstack.log",
];

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

fn is_container_dir(
    path: &str,
    apps: &[InstalledApp],
    shell_folders: &[String],
    containers: &ContainerRulebook,
) -> bool {
    let pk = normalize_path_key(path);

    // Never split personal shell folders: they stay atomic under "Your files".
    if is_personal_shell_folder(&pk, shell_folders) {
        return false;
    }

    // Drive roots (e.g. c:\, d:\)
    if pk.len() == 3 && pk.ends_with(":\\") {
        return true;
    }

    // User-configured or default container rulebook match (drive-root name or explicit path) or strict ancestor.
    if containers.matches_container_dir(path).is_some() || containers.is_ancestor_of_explicit(path) {
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

    // Also split nested Steam library folders (`*\Steam\steamapps`, `*\Steam\steamapps\common`, `*\SteamLibrary\...`)
    // even if a parent directory like `D:\Games` was removed from the container rulebook.
    if pk.ends_with("\\steam")
        || pk.ends_with("\\steam\\steamapps")
        || pk.ends_with("\\steam\\steamapps\\common")
        || pk.ends_with("\\steamlibrary")
        || pk.ends_with("\\steamlibrary\\steamapps")
        || pk.ends_with("\\steamlibrary\\steamapps\\common")
    {
        return true;
    }

    // C:\Users\<Username> (3 path segments) and C:\Users\<Username>\Microsoft (4 segments: e.g. Power BI Desktop Store App)
    let parts: Vec<&str> = pk.trim_end_matches('\\').split('\\').collect();
    if parts.len() == 3 && parts[1] == "users" {
        return true;
    }
    if parts.len() == 4 && parts[1] == "users" && parts[3] == "microsoft" {
        return true;
    }

    // Multi-app InstallLocation strict ancestor rule: split any directory strictly containing >= 2 distinct app InstallLocations
    // OR strictly containing >= 1 distinct app InstallLocation when this directory itself is NOT an app's InstallLocation.
    let mut distinct_contained = HashSet::new();
    let mut is_self_install_loc = false;
    for app in apps {
        for loc in &app.all_install_locations {
            let lk = normalize_path_key(loc);
            if lk == pk {
                is_self_install_loc = true;
            } else if is_subpath_of(&lk, &pk) {
                distinct_contained.insert(app.id.as_str());
            }
        }
    }
    if distinct_contained.len() >= 2 || (!is_self_install_loc && !distinct_contained.is_empty()) {
        return true;
    }

    false
}

pub fn decompose_scanned_tree(
    root: &FolderNode,
    apps: &[InstalledApp],
    shell_folders: &[String],
    root_system_files: &[(String, u64)],
) -> Vec<ScannedUnit> {
    decompose_scanned_tree_with_containers(
        root,
        apps,
        shell_folders,
        root_system_files,
        &ContainerRulebook::default(),
    )
}

pub fn decompose_scanned_tree_with_containers(
    root: &FolderNode,
    apps: &[InstalledApp],
    shell_folders: &[String],
    root_system_files: &[(String, u64)],
    containers: &ContainerRulebook,
) -> Vec<ScannedUnit> {
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

    let install_keys: HashSet<String> = apps
        .iter()
        .flat_map(|a| a.all_install_locations.iter())
        .map(|p| normalize_path_key(p))
        .collect();

    fn visit(
        node: &FolderNode,
        drive_letter: char,
        apps: &[InstalledApp],
        shell_folders: &[String],
        containers: &ContainerRulebook,
        install_keys: &HashSet<String>,
        out: &mut Vec<ScannedUnit>,
    ) {
        let pk = normalize_path_key(&node.path);
        if is_personal_shell_folder(&pk, shell_folders) {
            out.push(ScannedUnit {
                path: node.path.clone(),
                drive_letter,
                bytes: node.total_bytes,
                kind: ScannedUnitKind::PersonalShell,
                is_direct_install_root: false,
            });
            return;
        }

        if is_container_dir(&node.path, apps, shell_folders, containers) && !node.children.is_empty() {
            if node.direct_file_bytes > 0 {
                let parts: Vec<&str> = pk.trim_end_matches('\\').split('\\').collect();
                let is_registry_hive_container = (parts.len() == 3 && parts[1] == "users")
                    || pk.ends_with("\\appdata\\local\\microsoft\\windows");
                let kind = if is_registry_hive_container {
                    ScannedUnitKind::RegistryUserHive
                } else {
                    ScannedUnitKind::Regular
                };
                out.push(ScannedUnit {
                    path: format!("{}\\[files]", node.path.trim_end_matches('\\')),
                    drive_letter,
                    bytes: node.direct_file_bytes,
                    kind,
                    is_direct_install_root: false,
                });
            }
            for child in &node.children {
                visit(
                    child,
                    drive_letter,
                    apps,
                    shell_folders,
                    containers,
                    install_keys,
                    out,
                );
            }
            return;
        }

        // Leaf attribution unit (if an install_location has direct_file_bytes AND children inside, keep whole folder intact).
        out.push(ScannedUnit {
            path: node.path.clone(),
            drive_letter,
            bytes: node.total_bytes,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: install_keys.contains(&pk),
        });
    }

    // Drive root direct non-system files (excluding root system files already emitted above)
    if root.direct_file_bytes > 0 {
        units.push(ScannedUnit {
            path: format!("{}\\[root files]", root.path.trim_end_matches('\\')),
            drive_letter,
            bytes: root.direct_file_bytes,
            kind: ScannedUnitKind::Regular,
            is_direct_install_root: false,
        });
    }
    for child in &root.children {
        visit(
            child,
            drive_letter,
            apps,
            shell_folders,
            containers,
            &install_keys,
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

    // Sort shortest path first so parents precede children.
    unique.sort_by_key(|(p, _)| p.len());

    let mut adjusted_bytes: Vec<u64> = unique.iter().map(|(_, b)| *b).collect();
    for i in 0..unique.len() {
        let parent_key = normalize_path_key(&unique[i].0);
        // Find immediate/non-overlapping contained children to subtract from parent.
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
