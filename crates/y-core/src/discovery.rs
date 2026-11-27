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
    let raw_lower = raw_cmd_or_icon.trim().to_ascii_lowercase();
    if raw_lower.contains("://") {
        return None;
    }
    let exe = extract_exe_path_from_command(raw_cmd_or_icon)?;
    let lower = exe.to_ascii_lowercase();
    if lower.contains("\\windows\\")
        || lower.contains("msiexec.exe")
        || lower.contains("\\package cache\\")
        || lower.contains("\\windowsapps\\")
        || lower.contains("\\steam\\games\\")
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
pub fn split_alnum_words(s: &str) -> Vec<String> {
    let lower = s
        .to_ascii_lowercase()
        .replace('™', "")
        .replace('®', "")
        .replace('©', "");
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch);
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn is_release_patch_token(w: &str) -> bool {
    let bytes = w.as_bytes();
    let Some(first_alpha) = bytes.iter().position(|b| b.is_ascii_alphabetic()) else {
        return false;
    };
    if first_alpha == 0 {
        return false;
    }
    let prefix = &w[..first_alpha];
    let rest = &w[first_alpha..];
    if !prefix.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    for tag in ["rc", "a", "b", "f", "p", "r"] {
        if let Some(tail) = rest.strip_prefix(tag) {
            return !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppTokenMatchKind {
    WholeWordSlice = 1,
    ExactFullToken = 2,
}

/// Token-boundary app/publisher predicate matcher: matches only when `predicate` equals the
/// candidate's normalized token identity or appears as a contiguous whole-word token slice —
/// never as a substring inside a longer alphanumeric token (e.g. `"steam"` in `"msteams"`).
#[must_use]
pub fn match_app_predicate(haystack: &str, predicate: &str) -> Option<AppTokenMatchKind> {
    let pt_lower = predicate.trim().to_ascii_lowercase();
    if let Some(first_dot_tok) = pt_lower.split_whitespace().next() {
        if first_dot_tok.starts_with('.') {
            let dot_stem = first_dot_tok.trim_end_matches('.');
            if !haystack.to_ascii_lowercase().contains(dot_stem) {
                return None;
            }
        }
    }
    let h_words = split_alnum_words(haystack);
    let p_words = split_alnum_words(predicate);
    if h_words.is_empty() || p_words.is_empty() {
        return None;
    }
    if h_words == p_words {
        return Some(AppTokenMatchKind::ExactFullToken);
    }
    let has_digits = predicate.chars().any(|c| c.is_ascii_digit());
    let h_norm = if has_digits {
        String::new()
    } else {
        normalize_app_token(haystack)
    };
    let p_norm = if has_digits {
        String::new()
    } else {
        normalize_app_token(predicate)
    };
    if !has_digits
        && !p_norm.is_empty()
        && (h_norm == p_norm || h_norm.strip_prefix("store ") == Some(p_norm.as_str()))
    {
        return Some(AppTokenMatchKind::ExactFullToken);
    }
    if h_words.len() >= p_words.len() && h_words.windows(p_words.len()).any(|w| w == p_words.as_slice())
    {
        return Some(AppTokenMatchKind::WholeWordSlice);
    }
    if !has_digits && !p_norm.is_empty() {
        let hn_words: Vec<&str> = h_norm.split_whitespace().collect();
        let pn_words: Vec<&str> = p_norm.split_whitespace().collect();
        if !pn_words.is_empty()
            && hn_words.len() >= pn_words.len()
            && hn_words.windows(pn_words.len()).any(|w| w == pn_words.as_slice())
        {
            return Some(AppTokenMatchKind::WholeWordSlice);
        }
    }
    None
}

#[must_use]
pub fn contains_whole_token_sequence(haystack: &str, predicate: &str) -> bool {
    match_app_predicate(haystack, predicate).is_some()
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
                && !is_release_patch_token(w)
                && !matches!(
                    *w,
                    "version"
                        | "64"
                        | "bit"
                        | "x64"
                        | "x86"
                        | "arm64"
                        | "amd64"
                        | "user"
                        | "edition"
                        | "update"
                )
        })
        .collect();
    words.join(" ").trim().to_string()
}

fn extract_arch_tag(lower: &str) -> &'static str {
    if lower.contains("x64") || lower.contains("amd64") || lower.contains("64-bit") {
        "x64"
    } else if lower.contains("arm64") {
        "arm64"
    } else if lower.contains("x86") || lower.contains("32-bit") {
        "x86"
    } else {
        "neutral"
    }
}

fn extract_first_dotted_version(lower: &str) -> Option<String> {
    for raw_tok in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')) {
        let tok = raw_tok.trim_start_matches('v').trim_matches('.');
        if tok.contains('.') && tok.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            let valid = tok.chars().all(|c| c.is_ascii_digit() || c == '.');
            if valid {
                return Some(tok.to_string());
            }
        }
    }
    None
}

fn extract_vcpp_era(lower: &str) -> Option<&'static str> {
    for year in ["2005", "2008", "2010", "2012", "2013", "2015", "2017", "2019", "2022"] {
        if lower.contains(year) {
            return Some(match year {
                "2015" | "2017" | "2019" | "2022" => "v14",
                other => other,
            });
        }
    }
    if lower.contains("v14") {
        return Some("v14");
    }
    None
}

/// Returns a distinct canonical identity key for independently installable/uninstallable
/// side-by-side versioned product families (VC++ Redistributables, .NET runtimes/SDKs,
/// Windows SDKs, Python, Java). Returns `None` for non-versioned apps and components so
/// location/token deduplication continues unchanged for normal apps.
#[must_use]
pub fn versioned_family_identity_key(display_name: &str, display_version: &str) -> Option<String> {
    let lower = display_name.trim().to_ascii_lowercase();
    let ver_clean = display_version.trim();

    if lower.contains("visual c++") && lower.contains("redistributable") {
        let era = extract_vcpp_era(&lower).unwrap_or("other");
        let arch = match extract_arch_tag(&lower) {
            "neutral" => "x86",
            a => a,
        };
        let ver = extract_first_dotted_version(&lower)
            .or_else(|| (!ver_clean.is_empty()).then(|| ver_clean.to_ascii_lowercase()))
            .unwrap_or_else(|| era.to_string());
        return Some(format!("vcpp-redist:{era}:{arch}:{ver}"));
    }

    if !lower.contains("framework")
        && (lower.contains("windows desktop runtime")
            || lower.contains(".net desktop runtime")
            || lower.contains(".net runtime")
            || lower.contains("asp.net core")
            || lower.contains(".net sdk"))
    {
        let kind = if lower.contains("windows desktop runtime") || lower.contains(".net desktop runtime") {
            "dotnet-desktop"
        } else if lower.contains("asp.net core") {
            "aspnetcore"
        } else if lower.contains(".net sdk") {
            "dotnet-sdk"
        } else {
            "dotnet-core"
        };
        let arch = extract_arch_tag(&lower);
        let ver = extract_first_dotted_version(&lower)
            .or_else(|| (!ver_clean.is_empty()).then(|| ver_clean.to_ascii_lowercase()))
            .unwrap_or_else(|| "0".to_string());
        return Some(format!("{kind}:{arch}:{ver}"));
    }

    if lower.starts_with("windows software development kit") {
        let ver = extract_first_dotted_version(&lower)
            .or_else(|| (!ver_clean.is_empty()).then(|| ver_clean.to_ascii_lowercase()))
            .unwrap_or_else(|| "0".to_string());
        return Some(format!("winsdk:{ver}"));
    }

    if lower.starts_with("python ") && !lower.contains("launcher") {
        if let Some(ver) = extract_first_dotted_version(&lower) {
            let arch = extract_arch_tag(&lower);
            return Some(format!("python:{arch}:{ver}"));
        }
    }

    if (lower.starts_with("java ") || lower.starts_with("java(tm) se"))
        && !lower.contains("auto updater")
    {
        let arch = extract_arch_tag(&lower);
        let ver = if !ver_clean.is_empty() {
            ver_clean.to_ascii_lowercase()
        } else {
            lower
                .split_whitespace()
                .filter(|w| w.chars().any(|c| c.is_ascii_digit()))
                .collect::<Vec<_>>()
                .join("-")
        };
        if !ver.is_empty() {
            return Some(format!("java:{arch}:{ver}"));
        }
    }

    None
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
    let mut seen_hkcu_mirror: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();

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
        let display_version = entry.display_version.trim().to_string();

        // HKCU Uninstall is shared across 64-bit & 32-bit registry views; collapse exact mirror reflections.
        if entry.hive_view.starts_with("HKCU-") && !entry.hive_view.contains("WOW") {
            let key = (
                entry.subkey_name.trim().to_ascii_lowercase(),
                display_name.to_ascii_lowercase(),
                display_version.to_ascii_lowercase(),
            );
            if !seen_hkcu_mirror.insert(key) {
                continue;
            }
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
        let family_key = versioned_family_identity_key(&display_name, &display_version);
        let name_key = family_key
            .clone()
            .unwrap_or_else(|| normalize_app_token(&display_name));

        let mut entry_guids = Vec::new();
        for src in [&entry.subkey_name, &entry.uninstall_string, &entry.display_icon] {
            if let Some(g) = extract_guid(src) {
                if !entry_guids.contains(&g) {
                    entry_guids.push(g);
                }
            }
        }

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
        let display_icon_clean = {
            let trimmed = entry.display_icon.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };
        if let Some(idx) = existing_idx {
            let existing = &mut apps[idx];
            if existing.display_name != display_name && !existing.aliases.contains(&display_name) {
                existing.aliases.push(display_name.clone());
            }
            if existing.display_version.is_empty() && !display_version.is_empty() {
                existing.display_version = display_version;
            }
            for g in entry_guids {
                if !existing.uninstall_guids.contains(&g) {
                    existing.uninstall_guids.push(g);
                }
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
            if existing.display_icon.is_none() && display_icon_clean.is_some() {
                existing.display_icon = display_icon_clean.clone();
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
            format!("reg:{}", name_key.replace([' ', ':'], "-"))
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
            display_version,
            uninstall_guids: entry_guids,
            aliases: Vec::new(),
            publishers,
            direct_install_location: direct_loc,
            inferred_install_location: inferred_loc,
            all_install_locations: all_locs,
            no_touch,
            is_store_fallback: false,
            display_icon: display_icon_clean,
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
