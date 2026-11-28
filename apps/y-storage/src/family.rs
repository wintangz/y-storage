use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;
use y_core::discovery::normalize_app_token;
use y_core::model::{AppCategory, AttributedApp, GrowthDelta};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum FamilyBasis {
    Stem,
    Dependency,
    SharedRoot,
}

impl FamilyBasis {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stem => "stem",
            Self::Dependency => "dependency",
            Self::SharedRoot => "shared root",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppFamily {
    pub id: String,
    pub display_name: String,
    pub publisher: String,
    pub normalized_publisher: String,
    pub member_indices: Vec<usize>,
    pub total_bytes: u64,
    pub shared_category: Option<AppCategory>,
    pub bases: Vec<FamilyBasis>,
    pub basis_detail: String,
}

impl AppFamily {
    #[must_use]
    pub fn basis_label(&self) -> String {
        self.bases
            .iter()
            .map(|b| b.label())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    #[must_use]
    pub fn bytes_on_drive(&self, apps: &[AttributedApp], drive_letter: char) -> u64 {
        self.member_indices
            .iter()
            .map(|&idx| apps[idx].bytes_on_drive(drive_letter))
            .sum()
    }

    #[must_use]
    pub fn effective_bytes(&self, apps: &[AttributedApp], per_drive: Option<char>) -> u64 {
        per_drive.map_or(self.total_bytes, |dl| self.bytes_on_drive(apps, dl))
    }

    #[must_use]
    pub fn effective_growth(&self, apps: &[AttributedApp], per_drive: Option<char>) -> GrowthDelta {
        let mut any_changed = false;
        let mut changed_sum: i64 = 0;
        let mut any_new = false;
        let mut new_sum: u64 = 0;
        for &idx in &self.member_indices {
            let g = per_drive.map_or(apps[idx].growth, |dl| apps[idx].growth_for_drive(dl));
            match g {
                GrowthDelta::NoBaseline => {}
                GrowthDelta::New(b) => {
                    any_new = true;
                    new_sum = new_sum.saturating_add(b);
                }
                GrowthDelta::Changed(d) => {
                    any_changed = true;
                    changed_sum = changed_sum.saturating_add(d);
                }
            }
        }
        if any_changed {
            GrowthDelta::Changed(changed_sum)
        } else if any_new {
            GrowthDelta::New(new_sum)
        } else {
            GrowthDelta::NoBaseline
        }
    }
}

#[derive(Debug, Clone)]
pub enum LedgerEntry {
    Unit(usize),
    Family(AppFamily),
}

impl LedgerEntry {
    #[must_use]
    pub fn is_family(&self) -> bool {
        matches!(self, Self::Family(_))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn member_count(&self) -> usize {
        match self {
            Self::Unit(_) => 1,
            Self::Family(f) => f.member_indices.len(),
        }
    }

    #[must_use]
    pub fn effective_bytes(&self, apps: &[AttributedApp], per_drive: Option<char>) -> u64 {
        match self {
            Self::Unit(idx) => per_drive.map_or(apps[*idx].total_bytes, |dl| apps[*idx].bytes_on_drive(dl)),
            Self::Family(f) => f.effective_bytes(apps, per_drive),
        }
    }

    #[must_use]
    pub fn effective_growth(&self, apps: &[AttributedApp], per_drive: Option<char>) -> GrowthDelta {
        match self {
            Self::Unit(idx) => per_drive.map_or(apps[*idx].growth, |dl| apps[*idx].growth_for_drive(dl)),
            Self::Family(f) => f.effective_growth(apps, per_drive),
        }
    }

    #[must_use]
    pub fn display_name<'a>(&'a self, apps: &'a [AttributedApp]) -> &'a str {
        match self {
            Self::Unit(idx) => &apps[*idx].display_name,
            Self::Family(f) => &f.display_name,
        }
    }

    #[must_use]
    pub fn primary_publisher<'a>(&'a self, apps: &'a [AttributedApp]) -> Option<&'a str> {
        match self {
            Self::Unit(idx) => apps[*idx]
                .publishers
                .first()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty()),
            Self::Family(f) => {
                let t = f.publisher.trim();
                if t.is_empty() { None } else { Some(t) }
            }
        }
    }

    #[must_use]
    pub fn shared_category(&self, apps: &[AttributedApp]) -> Option<AppCategory> {
        match self {
            Self::Unit(idx) => Some(apps[*idx].category),
            Self::Family(f) => f.shared_category,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn entry_id<'a>(&'a self, apps: &'a [AttributedApp]) -> &'a str {
        match self {
            Self::Unit(idx) => &apps[*idx].id,
            Self::Family(f) => &f.id,
        }
    }
}

const SINGLE_TOKEN_STOP_STEMS: &[&str] = &[
    "windows",
    "microsoft",
    "visual",
    "system",
    "app",
    "apps",
    "software",
    "service",
    "services",
    "driver",
    "drivers",
    "runtime",
    "sdk",
    "tool",
    "tools",
    "client",
    "host",
    "core",
    "common",
    "shared",
    "update",
    "updater",
    "installer",
    "launcher",
    "helper",
    "manager",
    "control",
    "panel",
    "audio",
    "video",
    "graphics",
    "media",
    "game",
    "games",
    "gaming",
    "security",
    "network",
    "cloud",
    "web",
    "desktop",
    "mobile",
    "pro",
    "plus",
    "home",
    "studio",
    "suite",
    "kit",
    "pack",
    "data",
    "files",
    "lib",
    "libs",
    "x64",
    "x86",
    "arm64",
    "win32",
    "win64",
    "inc",
    "corp",
    "corporation",
    "llc",
    "ltd",
    "ii",
];

const GENERIC_PAIR_WORDS: &[&str] = &[
    "windows",
    "microsoft",
    "system",
    "app",
    "client",
    "host",
    "service",
    "services",
    "driver",
    "drivers",
    "software",
    "tool",
    "tools",
    "update",
    "updater",
    "installer",
    "helper",
    "common",
    "shared",
    "core",
    "corporation",
    "inc",
    "ii",
];

const LEGAL_PUBLISHER_SUFFIXES: &[&str] = &[
    "inc",
    "incorporated",
    "corp",
    "corporation",
    "llc",
    "ltd",
    "limited",
    "co",
    "company",
    "gmbh",
    "aps",
    "ag",
    "sa",
    "sarl",
    "bv",
    "nv",
    "oy",
    "ab",
    "pty",
    "plc",
    "lp",
    "llp",
    "sro",
    "eu",
    "ii",
];

const GENERIC_INSTALL_PARENTS: &[&str] = &[
    "program files",
    "program files (x86)",
    "common files",
    "windowsapps",
    "xboxgames",
    "programdata",
    "users",
    "appdata",
    "local",
    "roaming",
    "locallow",
    "installshield installation information",
    "apps",
    "games",
    "game",
    "programs",
    "software",
    "softwares",
    "portableapps",
    "common",
    "epic games",
];

fn split_camel_case(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len() + 8);
    for (idx, &ch) in chars.iter().enumerate() {
        if idx > 0 {
            let prev = chars[idx - 1];
            let next = chars.get(idx + 1).copied();
            let lower_to_upper = prev.is_ascii_lowercase() && ch.is_ascii_uppercase();
            let acronym_to_word = prev.is_ascii_uppercase()
                && ch.is_ascii_uppercase()
                && next.is_some_and(|n| n.is_ascii_lowercase());
            let digit_to_letter = prev.is_ascii_digit() && ch.is_ascii_alphabetic();
            if lower_to_upper || acronym_to_word || digit_to_letter {
                out.push(' ');
            }
        }
        out.push(ch);
    }
    out
}

fn normalize_publisher_string(raw: &str, allow_camel_split: bool) -> Option<String> {
    let pre = raw
        .replace("s.r.o.", " ")
        .replace("S.R.O.", " ")
        .replace("e.U.", " ")
        .replace("E.U.", " ");
    let spaced = if allow_camel_split && !pre.contains(' ') {
        split_camel_case(&pre)
    } else {
        pre
    };
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in spaced.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    while let Some(last) = tokens.last() {
        if LEGAL_PUBLISHER_SUFFIXES.contains(&last.as_str()) {
            tokens.pop();
        } else {
            break;
        }
    }
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

fn store_vendor_prefix(app: &AttributedApp) -> Option<&str> {
    let is_store = app.id.starts_with("store:")
        || app
            .display_name
            .trim_start()
            .get(..6)
            .is_some_and(|p| p.eq_ignore_ascii_case("store:"));
    if !is_store {
        return None;
    }
    let body = app
        .display_name
        .trim_start()
        .strip_prefix("Store:")
        .or_else(|| app.display_name.trim_start().strip_prefix("store:"))
        .unwrap_or(&app.display_name)
        .trim();
    let vendor = body.split('.').next()?.trim();
    if vendor.is_empty() { None } else { Some(vendor) }
}

fn is_hex_or_numeric(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Computes the normalised publisher key from an app's registration publisher.
/// Units without a valid registration publisher return `None` and never family-group.
#[must_use]
pub fn normalized_publisher_key(app: &AttributedApp) -> Option<String> {
    if app.id.starts_with("dot:") || app.id.starts_with("standalone:") {
        return None;
    }
    let raw_pub = app
        .publishers
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())?;

    let lower = raw_pub.to_ascii_lowercase();
    if lower == "user profile dot-folder"
        || lower.starts_with("standalone toolchain")
        || lower == "program files"
        || lower == "program files (x86)"
    {
        return None;
    }

    if lower == "microsoft store" {
        let vendor = store_vendor_prefix(app)?;
        let vendor_l = vendor.to_ascii_lowercase();
        if is_hex_or_numeric(vendor)
            || matches!(
                vendor_l.as_str(),
                "windows" | "cr" | "resources" | "deletedalluserpackages"
            )
        {
            return None;
        }
        return normalize_publisher_string(vendor, true);
    }

    normalize_publisher_string(raw_pub, false)
}

fn is_store_unit(app: &AttributedApp) -> bool {
    app.id.starts_with("store:")
        || app
            .display_name
            .trim_start()
            .get(..6)
            .is_some_and(|p| p.eq_ignore_ascii_case("store:"))
}

fn tokenize_name_for_stem(app: &AttributedApp, norm_pub_tokens: &[&str]) -> Vec<String> {
    let is_store = is_store_unit(app);
    let mut name = app.display_name.trim();
    if let Some(rest) = name.strip_prefix("Store:").or_else(|| name.strip_prefix("store:")) {
        name = rest.trim();
    }
    if is_store {
        // Strip the dot-delimited Store vendor prefix (e.g. `MicrosoftCorporationII.` or `AppleInc.`)
        // before tokenizing the product name so legal words like `Corporation II` / `Inc` never become stems.
        if let Some((_, after_vendor)) = name.split_once('.') {
            if !after_vendor.trim().is_empty() {
                name = after_vendor.trim();
            }
        }
    }
    let prepared = if is_store {
        split_camel_case(name)
    } else {
        name.to_string()
    };
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in prepared.chars() {
        if ch.is_ascii_alphanumeric() || ch == '+' || ch == '#' {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }

    // Strip leading publisher prefix only when all tokens of `norm_pub` match the prefix
    // (repeatedly so e.g. `Microsoft.MicrosoftEdge.Stable` reduces to `["edge", "stable"]`).
    if !norm_pub_tokens.is_empty() {
        loop {
            if tokens.len() <= norm_pub_tokens.len() {
                break;
            }
            let matches_prefix = tokens[..norm_pub_tokens.len()]
                .iter()
                .zip(norm_pub_tokens.iter())
                .all(|(t, p)| t == p);
            if matches_prefix {
                tokens.drain(..norm_pub_tokens.len());
            } else {
                break;
            }
        }
    }

    tokens
}

fn is_valid_shared_stem(stem: &[String], norm_pub_tokens: &[&str]) -> bool {
    if stem.is_empty() {
        return false;
    }
    let has_alpha = stem
        .iter()
        .any(|t| t.chars().any(|c| c.is_ascii_alphabetic()));
    if !has_alpha {
        return false;
    }
    match stem.len() {
        1 => {
            let t = stem[0].as_str();
            let is_pub_brand_only = norm_pub_tokens.first().is_some_and(|&p| p == t);
            t.len() >= 3 && !is_pub_brand_only && !SINGLE_TOKEN_STOP_STEMS.contains(&t)
        }
        2 => stem
            .iter()
            .any(|t| !GENERIC_PAIR_WORDS.contains(&t.as_str())),
        _ => true,
    }
}

fn common_token_prefix(a: &[String], b: &[String]) -> Vec<String> {
    let n = a.len().min(b.len());
    let mut k = 0;
    while k < n && a[k] == b[k] {
        k += 1;
    }
    a[..k].to_vec()
}

fn strip_id_prefix(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, rest)| rest)
}

fn has_dependency_edge(a: &AttributedApp, b: &AttributedApp) -> bool {
    let b_id_stripped = strip_id_prefix(&b.id);
    let b_tok = normalize_app_token(b_id_stripped);
    a.dependencies.iter().any(|dep| {
        if dep.target_id.eq_ignore_ascii_case(&b.id)
            || dep.target_id.eq_ignore_ascii_case(b_id_stripped)
        {
            return true;
        }
        if b_tok.len() >= 3 {
            let d_tok = normalize_app_token(strip_id_prefix(&dep.target_id));
            if !d_tok.is_empty() && d_tok == b_tok {
                return true;
            }
        }
        !dep.target_name.is_empty() && dep.target_name.eq_ignore_ascii_case(&b.display_name)
    })
}

/// Returns `(normalized_parent_key, display_parent_path)` when `install_location` sits under a
/// dedicated publisher-group parent folder (at least 2 path segments below the drive root and not a
/// generic OS/container/store folder).
fn publisher_group_install_parent(app: &AttributedApp) -> Option<(String, String)> {
    let raw = app.install_location.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    let trimmed = raw.trim_end_matches(['\\', '/']);
    let (parent, _leaf) = trimmed.rsplit_once(['\\', '/'])?;
    let parent_trimmed = parent.trim_end_matches(['\\', '/']);
    if parent_trimmed.len() <= 2 || parent_trimmed.ends_with(':') {
        return None;
    }
    let parent_norm = parent_trimmed.to_ascii_lowercase().replace('/', "\\");
    if parent_norm.ends_with("\\steamapps\\common") {
        return None;
    }
    let segs: Vec<&str> = parent_norm.split('\\').filter(|s| !s.is_empty()).collect();
    // Require `<drive>:`, `<container_or_program_files>`, `<publisher_or_suite>` (>= 3 segments total).
    if segs.len() < 3 {
        return None;
    }
    let parent_leaf = *segs.last()?;
    if GENERIC_INSTALL_PARENTS.contains(&parent_leaf) {
        return None;
    }
    Some((parent_norm, parent_trimmed.to_string()))
}

struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[rb] = ra;
        }
    }
}

fn title_case_stem(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|t| {
            let mut chars = t.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut s = String::new();
                    s.push(first.to_ascii_uppercase());
                    s.extend(chars);
                    s
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn reconstruct_stem_display_name(
    stem_tokens: &[String],
    norm_pub_tokens: &[&str],
    apps: &[AttributedApp],
    member_indices: &[usize],
) -> String {
    // Prefer the largest non-Store member, falling back to the largest member.
    let rep_idx = member_indices
        .iter()
        .copied()
        .find(|&idx| !is_store_unit(&apps[idx]))
        .unwrap_or(member_indices[0]);
    let rep_name = apps[rep_idx].display_name.trim();
    let rep_clean = rep_name
        .strip_prefix("Store:")
        .or_else(|| rep_name.strip_prefix("store:"))
        .unwrap_or(rep_name)
        .trim();

    if !is_store_unit(&apps[rep_idx]) {
        // Extract original-cased words from `rep_clean` (preserving `+` / `#` / `.`).
        let raw_words: Vec<&str> = rep_clean
            .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | ':' | ',' | '/' | '\\' | '™' | '®' | '©'))
            .filter(|w| !w.is_empty())
            .collect();
        let lower_words: Vec<String> = raw_words
            .iter()
            .map(|w| w.trim_start_matches('.').to_ascii_lowercase())
            .collect();

        // Find where `stem_tokens` starts in `lower_words` (either at 0 or right after `norm_pub_tokens`).
        let offset = if lower_words.len() >= stem_tokens.len()
            && lower_words[..stem_tokens.len()] == *stem_tokens
        {
            Some(0)
        } else if !norm_pub_tokens.is_empty()
            && lower_words.len() >= norm_pub_tokens.len() + stem_tokens.len()
            && lower_words[..norm_pub_tokens.len()]
                .iter()
                .zip(norm_pub_tokens.iter())
                .all(|(a, b)| a == b)
            && lower_words[norm_pub_tokens.len()..norm_pub_tokens.len() + stem_tokens.len()]
                == *stem_tokens
        {
            Some(norm_pub_tokens.len())
        } else {
            None
        };
        if let Some(off) = offset {
            // Preserve the leading publisher prefix if the representative name carries it
            // (e.g. "Microsoft Edge", "Microsoft Visual Studio").
            let end = off + stem_tokens.len();
            return raw_words[..end].join(" ");
        }
    }

    title_case_stem(stem_tokens)
}

fn root_folder_display_name(path: &str) -> String {
    let segs: Vec<&str> = path
        .trim_end_matches(['\\', '/'])
        .split(['\\', '/'])
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(&last) = segs.last() {
        let lower = last.to_ascii_lowercase();
        if matches!(lower.as_str(), "installer2" | "bin" | "app" | "application")
            && segs.len() >= 2
        {
            return segs[segs.len() - 2].to_string();
        }
        return last.to_string();
    }
    path.to_string()
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "group".to_string()
    } else {
        out
    }
}

/// Derives view-only product families over a finished `ScanReport.apps` slice.
/// Pure presentation function: unit ownership, unit bytes, and drive reconciliation remain untouched.
#[must_use]
pub fn derive_app_families(apps: &[AttributedApp]) -> Vec<LedgerEntry> {
    let mut entries: Vec<LedgerEntry> = Vec::with_capacity(apps.len());
    let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    for (idx, app) in apps.iter().enumerate() {
        if let Some(norm_pub) = normalized_publisher_key(app) {
            buckets.entry(norm_pub).or_default().push(idx);
        } else {
            entries.push(LedgerEntry::Unit(idx));
        }
    }

    let mut used_family_ids: HashMap<String, usize> = HashMap::new();

    for (norm_pub, bucket) in buckets {
        if bucket.len() < 2 {
            for idx in bucket {
                entries.push(LedgerEntry::Unit(idx));
            }
            continue;
        }

        let norm_pub_tokens: Vec<&str> = norm_pub.split_whitespace().collect();
        let tokens_per_member: Vec<Vec<String>> = bucket
            .iter()
            .map(|&idx| tokenize_name_for_stem(&apps[idx], &norm_pub_tokens))
            .collect();
        let parents_per_member: Vec<Option<(String, String)>> = bucket
            .iter()
            .map(|&idx| publisher_group_install_parent(&apps[idx]))
            .collect();

        let n = bucket.len();
        let mut dsu = DisjointSet::new(n);
        let mut stem_edges: Vec<(usize, usize, Vec<String>)> = Vec::new();
        let mut dep_edges: Vec<(usize, usize)> = Vec::new();
        let mut root_edges: Vec<(usize, usize, String)> = Vec::new();

        for i in 0..n {
            for j in (i + 1)..n {
                let mut connected = false;
                let lcp = common_token_prefix(&tokens_per_member[i], &tokens_per_member[j]);
                if is_valid_shared_stem(&lcp, &norm_pub_tokens) {
                    stem_edges.push((i, j, lcp));
                    connected = true;
                }
                let app_i = &apps[bucket[i]];
                let app_j = &apps[bucket[j]];
                if has_dependency_edge(app_i, app_j) || has_dependency_edge(app_j, app_i) {
                    dep_edges.push((i, j));
                    connected = true;
                }
                if let (Some((pi_key, pi_disp)), Some((pj_key, _))) =
                    (&parents_per_member[i], &parents_per_member[j])
                {
                    if pi_key == pj_key {
                        root_edges.push((i, j, pi_disp.clone()));
                        connected = true;
                    }
                }
                if connected {
                    dsu.union(i, j);
                }
            }
        }

        let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for local_idx in 0..n {
            let root = dsu.find(local_idx);
            components.entry(root).or_default().push(local_idx);
        }

        for (_root, local_members) in components {
            if local_members.len() < 2 {
                entries.push(LedgerEntry::Unit(bucket[local_members[0]]));
                continue;
            }

            let member_set: BTreeSet<usize> = local_members.iter().copied().collect();
            let mut bases_set: BTreeSet<FamilyBasis> = BTreeSet::new();
            let mut comp_stems: Vec<Vec<String>> = Vec::new();
            let mut comp_roots: Vec<String> = Vec::new();

            for (i, j, stem) in &stem_edges {
                if member_set.contains(i) && member_set.contains(j) {
                    bases_set.insert(FamilyBasis::Stem);
                    comp_stems.push(stem.clone());
                }
            }
            for (i, j) in &dep_edges {
                if member_set.contains(i) && member_set.contains(j) {
                    bases_set.insert(FamilyBasis::Dependency);
                }
            }
            for (i, j, root_disp) in &root_edges {
                if member_set.contains(i) && member_set.contains(j) {
                    bases_set.insert(FamilyBasis::SharedRoot);
                    comp_roots.push(root_disp.clone());
                }
            }

            let mut member_indices: Vec<usize> =
                local_members.iter().map(|&li| bucket[li]).collect();
            member_indices.sort_by(|&ia, &ib| {
                apps[ib]
                    .total_bytes
                    .cmp(&apps[ia].total_bytes)
                    .then_with(|| apps[ia].display_name.cmp(&apps[ib].display_name))
                    .then_with(|| apps[ia].id.cmp(&apps[ib].id))
            });

            let bases: Vec<FamilyBasis> = bases_set.into_iter().collect();
            let publisher = member_indices
                .iter()
                .copied()
                .filter(|&idx| !is_store_unit(&apps[idx]))
                .chain(member_indices.iter().copied())
                .find_map(|idx| {
                    apps[idx]
                        .publishers
                        .first()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or_else(|| norm_pub.clone());

            let best_stem = comp_stems
                .iter()
                .min_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
                .cloned();
            let first_root = comp_roots.into_iter().next();

            let display_name = if let Some(ref stem) = best_stem {
                reconstruct_stem_display_name(stem, &norm_pub_tokens, apps, &member_indices)
            } else if let Some(ref root_path) = first_root {
                root_folder_display_name(root_path)
            } else {
                apps[member_indices[0]].display_name.clone()
            };

            let basis_parts: Vec<String> = bases
                .iter()
                .map(|b| match b {
                    FamilyBasis::Stem => {
                        let s_txt = best_stem
                            .as_ref()
                            .map(|s| title_case_stem(s))
                            .unwrap_or_else(|| display_name.clone());
                        format!("shared name stem \"{s_txt}\"")
                    }
                    FamilyBasis::Dependency => "same-publisher dependency".to_string(),
                    FamilyBasis::SharedRoot => {
                        let r_txt = first_root.as_deref().unwrap_or("shared folder");
                        format!("shared install parent ({r_txt})")
                    }
                })
                .collect();
            let basis_detail = format!("{} ({publisher})", basis_parts.join(" + "));

            let total_bytes: u64 = member_indices.iter().map(|&idx| apps[idx].total_bytes).sum();
            let first_cat = apps[member_indices[0]].category;
            let shared_category = if member_indices
                .iter()
                .all(|&idx| apps[idx].category == first_cat)
            {
                Some(first_cat)
            } else {
                None
            };

            let base_id = format!("family:{}:{}", slugify(&norm_pub), slugify(&display_name));
            let occ = used_family_ids.entry(base_id.clone()).or_insert(0);
            *occ += 1;
            let id = if *occ == 1 {
                base_id
            } else {
                format!("{base_id}-{}", *occ)
            };

            entries.push(LedgerEntry::Family(AppFamily {
                id,
                display_name,
                publisher,
                normalized_publisher: norm_pub.clone(),
                member_indices,
                total_bytes,
                shared_category,
                bases,
                basis_detail,
            }));
        }
    }

    entries.sort_by(|a, b| {
        b.effective_bytes(apps, None)
            .cmp(&a.effective_bytes(apps, None))
            .then_with(|| a.display_name(apps).cmp(b.display_name(apps)))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use y_core::model::{
        AttributionBasis, BreakdownItem, DependencyConfidence, DependencyKind, DependencyRecord,
        NoTouchCategory, ZoneKind,
    };

    #[allow(clippy::too_many_arguments)]
    fn make_unit(
        id: &str,
        name: &str,
        publishers: &[&str],
        install_location: Option<&str>,
        total_bytes: u64,
        category: AppCategory,
        no_touch: Option<NoTouchCategory>,
        deps: Vec<DependencyRecord>,
    ) -> AttributedApp {
        AttributedApp {
            id: id.to_string(),
            display_name: name.to_string(),
            publishers: publishers.iter().map(|s| (*s).to_string()).collect(),
            install_location: install_location.map(str::to_string),
            total_bytes,
            previous_bytes: None,
            growth: GrowthDelta::Changed(0),
            per_drive_growth: Default::default(),
            no_touch,
            bases: vec![AttributionBasis::InstallLocation],
            items: vec![BreakdownItem {
                path: install_location.unwrap_or(r"C:\Apps\Fallback").to_string(),
                bytes: total_bytes,
                zone: Some(ZoneKind::Body),
                qualifier: None,
                basis: Some(AttributionBasis::InstallLocation),
                basis_detail: None,
                no_touch,
                drive_letter: 'C',
            }],
            carved_out_apps: Vec::new(),
            dependencies: deps,
            display_icon: None,
            icon_rgba: None,
            category,
            category_basis: String::new(),
        }
    }

    fn make_dep(target_id: &str, target_name: &str) -> DependencyRecord {
        DependencyRecord {
            target_id: target_id.to_string(),
            target_name: target_name.to_string(),
            kind: DependencyKind::SharedRuntime,
            present: true,
            own_size_bytes: None,
            shared: true,
            basis: "test dependency".to_string(),
            confidence: DependencyConfidence::Proven,
            imported_by_binary: false,
        }
    }

    #[test]
    fn fixture_1_browser_runtime_and_two_satellites_group_with_exact_total() {
        let browser_b = 10_000_000_000_u64;
        let runtime_b = 2_400_000_000_u64;
        let sat1_b = 2_500_000_u64;
        let sat2_b = 78_000_u64;
        let apps = vec![
            make_unit(
                "reg:aurora-browser",
                "Aurora Browser",
                &["Northwind Systems Corp."],
                Some(r"C:\Program Files\Northwind\Aurora\Browser"),
                browser_b,
                AppCategory::Browsers,
                None,
                vec![],
            ),
            make_unit(
                "sys:aurora-runtime",
                "Aurora Browser WebRuntime",
                &["Northwind Systems"],
                Some(r"C:\Program Files (x86)\Northwind\WebRuntime"),
                runtime_b,
                AppCategory::SystemAndRuntimes,
                Some(NoTouchCategory::SystemRuntime),
                vec![],
            ),
            make_unit(
                "reg:aurora-helper",
                "Aurora Browser Helper",
                &["Northwind Systems Inc."],
                Some(r"C:\Program Files\Northwind\Helper"),
                sat1_b,
                AppCategory::Utilities,
                None,
                vec![],
            ),
            make_unit(
                "store:northwindsystems.aurorabrowserdevtools_8wekyb3d8bbwe",
                "Store: NorthwindSystems.AuroraBrowserDevTools",
                &["Microsoft Store"],
                None,
                sat2_b,
                AppCategory::Development,
                None,
                vec![],
            ),
        ];

        let entries = derive_app_families(&apps);
        assert_eq!(entries.len(), 1);
        let LedgerEntry::Family(fam) = &entries[0] else {
            panic!("expected single family entry");
        };
        assert_eq!(entries[0].member_count(), 4);
        assert!(entries[0].entry_id(&apps).starts_with("family:"));
        assert_eq!(fam.member_indices.len(), 4);
        assert_eq!(
            fam.total_bytes,
            browser_b + runtime_b + sat1_b + sat2_b
        );
        assert!(fam.bases.contains(&FamilyBasis::Stem));
        assert_eq!(fam.basis_label(), "stem");
        // Mixed categories across members -> family header shows no category
        assert_eq!(fam.shared_category, None);
        // NO-TOUCH badge on the runtime member stays intact inside the family
        let rt_member = fam
            .member_indices
            .iter()
            .map(|&i| &apps[i])
            .find(|a| a.id == "sys:aurora-runtime")
            .expect("runtime member present");
        assert_eq!(rt_member.no_touch, Some(NoTouchCategory::SystemRuntime));
    }

    #[test]
    fn fixture_2_same_publisher_different_stems_without_dep_or_root_stay_separate() {
        let apps = vec![
            make_unit(
                "reg:acme-photo",
                "Acme Photo",
                &["Acme Software LLC"],
                Some(r"C:\Program Files\Acme Photo"),
                500_000_000,
                AppCategory::MediaAndCreative,
                None,
                vec![],
            ),
            make_unit(
                "reg:acme-ledger",
                "Acme Ledger",
                &["Acme Software LLC"],
                Some(r"D:\Apps\Acme Ledger"),
                300_000_000,
                AppCategory::Productivity,
                None,
                vec![],
            ),
        ];

        let entries = derive_app_families(&apps);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| matches!(e, LedgerEntry::Unit(_))));
    }

    #[test]
    fn fixture_3_cross_publisher_dependency_does_not_group() {
        let apps = vec![
            make_unit(
                "reg:alpha-game",
                "Alpha Game",
                &["Alpha Studio Inc."],
                Some(r"D:\Games\Alpha Game"),
                4_000_000_000,
                AppCategory::Games,
                None,
                vec![make_dep("beta-anticheat", "Beta AntiCheat")],
            ),
            make_unit(
                "sys:beta-anticheat",
                "Beta AntiCheat",
                &["Beta Security Corp."],
                Some(r"C:\Program Files (x86)\Beta AntiCheat"),
                120_000_000,
                AppCategory::SystemAndRuntimes,
                Some(NoTouchCategory::AntiCheat),
                vec![],
            ),
        ];

        let entries = derive_app_families(&apps);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| matches!(e, LedgerEntry::Unit(_))));
    }

    #[test]
    fn fixture_4_single_unit_and_publisherless_units_create_no_family() {
        let apps = vec![
            make_unit(
                "reg:solo",
                "Solo Tool",
                &["Solo Publisher"],
                Some(r"C:\Program Files\Solo Tool"),
                150_000_000,
                AppCategory::Utilities,
                None,
                vec![],
            ),
            make_unit(
                "container:d:\\apps\\suite\\tool-a",
                "Nebula Tool A",
                &[],
                Some(r"D:\Apps\Suite\ToolA"),
                200_000_000,
                AppCategory::Utilities,
                None,
                vec![],
            ),
            make_unit(
                "container:d:\\apps\\suite\\tool-b",
                "Nebula Tool B",
                &[],
                Some(r"D:\Apps\Suite\ToolB"),
                100_000_000,
                AppCategory::Utilities,
                None,
                vec![],
            ),
        ];

        let entries = derive_app_families(&apps);
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| matches!(e, LedgerEntry::Unit(_))));
    }

    #[test]
    fn fixture_5_family_sum_always_equals_member_sum_and_conserves_bytes() {
        let apps = vec![
            // Stem + Dependency transitive closure (same publisher)
            make_unit(
                "reg:orion-core",
                "Orion Studio",
                &["Orion Labs Inc."],
                Some(r"C:\Program Files\Orion Studio"),
                1_234_567_890,
                AppCategory::Development,
                None,
                vec![],
            ),
            make_unit(
                "reg:orion-cli",
                "Orion Studio CLI",
                &["Orion Labs"],
                Some(r"C:\Program Files\Orion CLI"),
                98_765_432,
                AppCategory::Development,
                None,
                vec![make_dep("orion-daemon", "Background Daemon")],
            ),
            make_unit(
                "sys:orion-daemon",
                "Background Daemon",
                &["Orion Labs LLC"],
                Some(r"C:\Program Files\Background Daemon"),
                11_223_344,
                AppCategory::Development,
                Some(NoTouchCategory::Service),
                vec![],
            ),
            // Shared publisher-group install parent
            make_unit(
                "reg:art-photo",
                "PhotoCraft",
                &["Learning Machines LLC"],
                Some(r"Y:\Apps\ArtCraft\PhotoCraft"),
                295_710_886,
                AppCategory::MediaAndCreative,
                None,
                vec![],
            ),
            make_unit(
                "reg:art-film",
                "FilmCraft",
                &["Learning Machines LLC"],
                Some(r"Y:\Apps\ArtCraft\FilmCraft"),
                224_034_289,
                AppCategory::MediaAndCreative,
                None,
                vec![],
            ),
            // Standalone ungrouped unit
            make_unit(
                "reg:lone-utility",
                "Lone Utility",
                &["Independent Dev"],
                Some(r"Z:\Apps\Lone Utility"),
                42_000_000,
                AppCategory::Utilities,
                None,
                vec![],
            ),
        ];

        let entries = derive_app_families(&apps);
        let expected_total: u64 = apps.iter().map(|a| a.total_bytes).sum();
        let entry_total: u64 = entries.iter().map(|e| e.effective_bytes(&apps, None)).sum();
        assert_eq!(entry_total, expected_total);

        let mut seen_indices = Vec::new();
        let mut family_count = 0usize;
        for entry in &entries {
            match entry {
                LedgerEntry::Unit(idx) => seen_indices.push(*idx),
                LedgerEntry::Family(fam) => {
                    family_count += 1;
                    let member_sum: u64 = fam.member_indices.iter().map(|&i| apps[i].total_bytes).sum();
                    assert_eq!(fam.total_bytes, member_sum);
                    assert!(fam.member_indices.len() >= 2);
                    seen_indices.extend_from_slice(&fam.member_indices);
                }
            }
        }
        assert_eq!(family_count, 2);
        seen_indices.sort_unstable();
        assert_eq!(seen_indices, (0..apps.len()).collect::<Vec<_>>());

        // Check unanimous category + bases on both families
        let orion = entries
            .iter()
            .find_map(|e| match e {
                LedgerEntry::Family(f) if f.normalized_publisher == "orion labs" => Some(f),
                _ => None,
            })
            .expect("orion family");
        assert_eq!(orion.member_indices.len(), 3);
        assert_eq!(orion.shared_category, Some(AppCategory::Development));
        assert_eq!(orion.bases, vec![FamilyBasis::Stem, FamilyBasis::Dependency]);

        let art = entries
            .iter()
            .find_map(|e| match e {
                LedgerEntry::Family(f) if f.normalized_publisher == "learning machines" => Some(f),
                _ => None,
            })
            .expect("artcraft family");
        assert_eq!(art.bases, vec![FamilyBasis::SharedRoot]);
        assert_eq!(art.display_name, "ArtCraft");
        assert_eq!(art.shared_category, Some(AppCategory::MediaAndCreative));
    }
}
