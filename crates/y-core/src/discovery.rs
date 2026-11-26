use std::collections::{BTreeSet, HashMap};

use crate::model::{InstalledApp, NoTouchCategory, RawRegistryApp, RegistryDiscoveryStats};

pub struct RegistryCatalog {
    pub apps: Vec<InstalledApp>,
    pub guid_to_app_name: HashMap<String, String>,
    pub stats: RegistryDiscoveryStats,
}

#[must_use]
pub fn normalize_windows_path(raw: &str) -> String {
    let mut s = raw.trim().trim_matches('"').trim().replace('/', "\\");
    while s.contains("\\\\") && !s.starts_with("\\\\") {
        s = s.replace("\\\\", "\\");
    }
    if s.len() > 3 && s.ends_with('\\') {
        s = s.trim_end_matches('\\').to_string();
    }
    if s.len() == 2 && s.as_bytes()[1] == b':' {
        s.push('\\');
    }
    if let Some(first) = s.chars().next() {
        if s.len() >= 2 && s.as_bytes()[1] == b':' && first.is_ascii_lowercase() {
            let mut upper = String::with_capacity(s.len());
            upper.push(first.to_ascii_uppercase());
            upper.push_str(&s[1..]);
            s = upper;
        }
    }
    s
}

#[must_use]
pub fn normalize_path_key(path: &str) -> String {
    normalize_windows_path(path).to_ascii_lowercase()
}

#[must_use]
pub fn path_drive_letter(path: &str) -> Option<char> {
    let b = path.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        Some((b[0] as char).to_ascii_uppercase())
    } else {
        None
    }
}

#[must_use]
pub fn is_subpath_of(child: &str, parent: &str) -> bool {
    let c = normalize_path_key(child);
    let p = normalize_path_key(parent);
    if c == p {
        return true;
    }
    if p.ends_with('\\') {
        c.starts_with(&p)
    } else {
        c.starts_with(&format!("{p}\\"))
    }
}

#[must_use]
pub fn strip_wrapping_quotes(raw: &str) -> String {
    raw.trim().trim_matches('"').trim().to_string()
}

#[must_use]
pub fn extract_exe_path_from_command(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return Some(normalize_windows_path(&rest[..end]));
        }
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(idx) = lower.find(".exe") {
        let candidate = &trimmed[..idx + 4];
        return Some(normalize_windows_path(candidate));
    }
    if let Some(idx) = lower.find(".ico") {
        let candidate = &trimmed[..idx + 4];
        return Some(normalize_windows_path(candidate));
    }
    None
}

#[must_use]
pub fn infer_install_root_from_exe(raw_cmd_or_icon: &str) -> Option<String> {
    let exe = extract_exe_path_from_command(raw_cmd_or_icon)?;
    let lower = exe.to_ascii_lowercase();
    if lower.contains("\\windows\\")
        || lower.contains("msiexec.exe")
        || lower.contains("\\package cache\\")
        || lower.contains("\\windowsapps\\")
    {
        return None;
    }

    // Remove trailing filename if it has an extension.
    let dir = match exe.rfind('\\') {
        Some(pos) if exe[pos + 1..].contains('.') => &exe[..pos],
        _ => exe.as_str(),
    };
    if dir.len() <= 3 {
        return None;
    }

    let dir_lower = dir.to_ascii_lowercase();
    let container_markers = [
        ":\\program files\\",
        ":\\program files (x86)\\",
        "\\appdata\\local\\programs\\",
        "\\appdata\\local\\",
        "\\appdata\\roaming\\",
        ":\\apps\\",
        ":\\games\\",
        ":\\softwares\\",
        ":\\xboxgames\\",
        ":\\gog games\\",
        ":\\riot games\\",
        "\\steamapps\\common\\",
    ];

    for marker in container_markers {
        if let Some(idx) = dir_lower.find(marker) {
            let after_marker = &dir[idx + marker.len()..];
            let first_seg = after_marker.split('\\').next().unwrap_or("");
            if first_seg.is_empty() {
                return None;
            }
            let first_lower = first_seg.to_ascii_lowercase();
            // Avoid collapsing containers like Microsoft / Common Files / SteamLibrary directly.
            if matches!(
                first_lower.as_str(),
                "programs" | "common" | "common files" | "microsoft" | "steamlibrary" | "temp"
            ) {
                if let Some(second_seg) = after_marker.split('\\').nth(1) {
                    if !second_seg.is_empty() {
                        return Some(format!(
                            "{}{}{}\\{}",
                            &dir[..idx],
                            &dir[idx..idx + marker.len()],
                            first_seg,
                            second_seg
                        ));
                    }
                }
                return Some(dir.to_string());
            }
            return Some(format!(
                "{}{}{}",
                &dir[..idx],
                &dir[idx..idx + marker.len()],
                first_seg
            ));
        }
    }

    Some(dir.to_string())
}

#[must_use]
pub fn extract_guid(input: &str) -> Option<String> {
    let start = input.find('{')?;
    let rest = &input[start..];
    let end = rest.find('}')?;
    let candidate = &rest[..=end];
    if candidate.len() >= 34 && candidate.contains('-') {
        Some(candidate.to_ascii_lowercase())
    } else {
        None
    }
}

#[must_use]
pub fn classify_no_touch(display_name: &str, publisher: &str) -> Option<NoTouchCategory> {
    let n = display_name.to_ascii_lowercase();
    let p = publisher.to_ascii_lowercase();

    if n.contains("anticheat")
        || n.contains("anti-cheat")
        || n.contains("easyanticheat")
        || n.contains("battleye")
        || n.contains("vanguard")
    {
        return Some(NoTouchCategory::AntiCheat);
    }

    if n.contains("driver")
        || n.contains("physx")
        || n.contains("chipset software")
        || n.contains("hidhide")
        || n.contains("vigembus")
        || n.contains("virtual gamepad")
        || n.contains("wireless adapter")
        || n.contains("wacom tablet")
    {
        return Some(NoTouchCategory::Driver);
    }

    if n.contains("visual c++")
        || n.contains(".net runtime")
        || n.contains(".net desktop runtime")
        || n.contains("windows desktop runtime")
        || n.contains(".net sdk")
        || n.contains("directx")
        || n.contains("openal")
        || n.contains("webview2 runtime")
        || n.contains("windows software development kit")
        || n.contains("windows sdk")
        || n.contains("application compatibility fix database")
        || n.contains("microsoft gameinput")
        || n.contains("epic online services")
        || n.contains("rockstar games sdk")
        || n.contains("frameview sdk")
    {
        return Some(NoTouchCategory::SystemRuntime);
    }

    if n.ends_with(" service")
        || n.contains("framework service")
        || n.contains("gamesdk service")
        || n.contains("rog live service")
        || n.contains("xbox service")
        || n.contains("update health tools")
        || n.contains("aura service")
        || n.contains("aura lighting effect")
        || (p.contains("asus") && (n.contains("service") || n.contains("lighting")))
    {
        return Some(NoTouchCategory::Service);
    }

    None
}

#[must_use]
pub fn normalize_app_token(name: &str) -> String {
    let lower = name
        .to_ascii_lowercase()
        .replace('™', "")
        .replace('®', "")
        .replace('©', "");
    let mut out = String::with_capacity(lower.len());
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch.is_ascii_whitespace() || ch == '-' || ch == '_' || ch == '.' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        }
    }
    let words: Vec<&str> = out
        .split_whitespace()
        .filter(|w| {
            let is_ver = w.starts_with('v') && w[1..].chars().all(|c| c.is_ascii_digit());
            let is_pure_num = w.chars().all(|c| c.is_ascii_digit());
            !is_ver
                && !is_pure_num
                && !matches!(
                    *w,
                    "version" | "64" | "bit" | "x64" | "x86" | "user" | "edition" | "update"
                )
        })
        .collect();
    words.join(" ").trim().to_string()
}

pub fn build_registry_catalog(
    raw_entries: &[RawRegistryApp],
    steam_path: Option<&str>,
) -> RegistryCatalog {
    let raw_display_name_entries = raw_entries.len();
    let mut raw_with_install_location = 0usize;
    let mut guid_to_app_name: HashMap<String, String> = HashMap::new();

    for entry in raw_entries {
        let loc = normalize_windows_path(&entry.install_location);
        if !loc.is_empty() {
            raw_with_install_location += 1;
        }
        if let Some(g) = extract_guid(&entry.subkey_name) {
            guid_to_app_name
                .entry(g)
                .or_insert_with(|| entry.display_name.trim().to_string());
        }
        if let Some(g) = extract_guid(&entry.uninstall_string) {
            guid_to_app_name
                .entry(g)
                .or_insert_with(|| entry.display_name.trim().to_string());
        }
        if let Some(g) = extract_guid(&entry.display_icon) {
            guid_to_app_name
                .entry(g)
                .or_insert_with(|| entry.display_name.trim().to_string());
        }
    }

    let mut apps: Vec<InstalledApp> = Vec::new();
    let mut index_by_location: HashMap<String, usize> = HashMap::new();
    let mut index_by_name_key: HashMap<String, usize> = HashMap::new();

    for entry in raw_entries {
        let release_lower = entry.release_type.trim().to_ascii_lowercase();
        let is_update = release_lower.contains("update")
            || release_lower.contains("hotfix")
            || release_lower.contains("security");
        if entry.system_component || !entry.parent_key_name.trim().is_empty() || is_update {
            continue;
        }

        let display_name = entry.display_name.trim().to_string();
        if display_name.is_empty() {
            continue;
        }

        let direct_loc = {
            let norm = normalize_windows_path(&entry.install_location);
            if norm.len() >= 3 { Some(norm) } else { None }
        };

        let mut inferred_loc = if direct_loc.is_none() {
            infer_install_root_from_exe(&entry.uninstall_string)
                .or_else(|| infer_install_root_from_exe(&entry.display_icon))
        } else {
            None
        };

        if direct_loc.is_none()
            && inferred_loc.is_none()
            && display_name.eq_ignore_ascii_case("Steam")
        {
            if let Some(sp) = steam_path {
                let norm = normalize_windows_path(sp);
                if norm.len() >= 3 {
                    inferred_loc = Some(norm);
                }
            }
        }

        let effective_loc = direct_loc.as_ref().or(inferred_loc.as_ref());
        let loc_key = effective_loc.map(|p| normalize_path_key(p));
        let name_key = normalize_app_token(&display_name);

        let existing_idx = loc_key
            .as_ref()
            .and_then(|k| index_by_location.get(k).copied())
            .or_else(|| {
                if !name_key.is_empty() {
                    index_by_name_key.get(&name_key).copied()
                } else {
                    None
                }
            });

        let publisher_clean = entry.publisher.trim().to_string();
        if let Some(idx) = existing_idx {
            let existing = &mut apps[idx];
            if existing.display_name != display_name && !existing.aliases.contains(&display_name) {
                existing.aliases.push(display_name.clone());
            }
            if !publisher_clean.is_empty() && !existing.publishers.contains(&publisher_clean) {
                existing.publishers.push(publisher_clean);
            }
            if existing.direct_install_location.is_none() && direct_loc.is_some() {
                existing.direct_install_location = direct_loc.clone();
            }
            if existing.inferred_install_location.is_none() && inferred_loc.is_some() {
                existing.inferred_install_location = inferred_loc.clone();
            }
            if let Some(loc) = effective_loc {
                if !existing
                    .all_install_locations
                    .iter()
                    .any(|p| normalize_path_key(p) == normalize_path_key(loc))
                {
                    existing.all_install_locations.push(loc.clone());
                }
            }
            if existing.no_touch.is_none() {
                existing.no_touch = classify_no_touch(&display_name, &entry.publisher);
            }
            continue;
        }

        let mut all_locs = Vec::new();
        if let Some(loc) = effective_loc {
            all_locs.push(loc.clone());
        }
        let publishers = if publisher_clean.is_empty() {
            Vec::new()
        } else {
            vec![publisher_clean]
        };
        let no_touch = classify_no_touch(&display_name, &entry.publisher);
        let id = if !name_key.is_empty() {
            format!("reg:{}", name_key.replace(' ', "-"))
        } else {
            format!("reg:{}", apps.len())
        };

        let idx = apps.len();
        if let Some(k) = loc_key {
            index_by_location.insert(k, idx);
        }
        if !name_key.is_empty() {
            index_by_name_key.insert(name_key, idx);
        }
        apps.push(InstalledApp {
            id,
            display_name,
            aliases: Vec::new(),
            publishers,
            direct_install_location: direct_loc,
            inferred_install_location: inferred_loc,
            all_install_locations: all_locs,
            no_touch,
            is_store_fallback: false,
        });
    }

    // Ensure unique IDs if normalized tokens collided on distinct locations.
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for (i, app) in apps.iter_mut().enumerate() {
        if !seen_ids.insert(app.id.clone()) {
            app.id = format!("{}-{i}", app.id);
            seen_ids.insert(app.id.clone());
        }
    }

    let canonical_with_direct = apps
        .iter()
        .filter(|a| a.direct_install_location.is_some())
        .count();
    let canonical_with_inferred = apps
        .iter()
        .filter(|a| a.direct_install_location.is_none() && a.inferred_install_location.is_some())
        .count();

    RegistryCatalog {
        stats: RegistryDiscoveryStats {
            raw_display_name_entries,
            raw_with_install_location,
            canonical_apps: apps.len(),
            canonical_with_direct_install_location: canonical_with_direct,
            canonical_with_inferred_install_location: canonical_with_inferred,
        },
        apps,
        guid_to_app_name,
    }
}
