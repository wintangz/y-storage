use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoTouchCategory {
    Driver,
    Service,
    AntiCheat,
    SystemRuntime,
    Registry,
}

impl NoTouchCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Driver => "NO-TOUCH · Driver",
            Self::Service => "NO-TOUCH · Service",
            Self::AntiCheat => "NO-TOUCH · Anti-Cheat",
            Self::SystemRuntime => "NO-TOUCH · Runtime",
            Self::Registry => "NO-TOUCH · Registry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZoneKind {
    Body,
    Red,
    Green,
}

impl ZoneKind {
    #[must_use]
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Body => "BODY",
            Self::Red => "RED",
            Self::Green => "GREEN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LocationQualifier {
    RoamingConfig,
    MachineShared,
    InstallLooseFiles,
    LooseFiles,
}

impl LocationQualifier {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::RoamingConfig => "Roaming · Config",
            Self::MachineShared => "ProgramData · Machine-shared",
            Self::InstallLooseFiles => "loose files in the install folder",
            Self::LooseFiles => "loose files",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AttributionBasis {
    InstallLocation,
    ExactName,
    Token,
    Publisher,
    PublisherProduct,
    PackageFallback,
    StandaloneFolder,
    SteamManifest,
    LauncherManifest,
    ContainerRule,
    ExecutableScan,
    ManualPin,
}

impl AttributionBasis {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::InstallLocation => "install location",
            Self::ExactName => "exact name match",
            Self::Token => "token match",
            Self::Publisher => "publisher match",
            Self::PublisherProduct => "publisher+folder match",
            Self::PackageFallback => "package name",
            Self::StandaloneFolder => "standalone folder",
            Self::SteamManifest => "Steam manifest",
            Self::LauncherManifest => "launcher manifest",
            Self::ContainerRule => "layout rule",
            Self::ExecutableScan => "executable scan (inferred)",
            Self::ManualPin => "manual pin",
        }
    }
}

pub const NO_KNOWN_DEPENDENCIES_LABEL: &str = "no known dependencies";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum AppCategory {
    #[serde(rename = "Games")]
    Games,
    #[serde(rename = "Platforms & Launchers")]
    PlatformsAndLaunchers,
    #[serde(rename = "Browsers")]
    Browsers,
    #[serde(rename = "Communication")]
    Communication,
    #[serde(rename = "Development")]
    Development,
    #[serde(rename = "Media & Creative")]
    MediaAndCreative,
    #[serde(rename = "Productivity")]
    Productivity,
    #[serde(rename = "Utilities")]
    Utilities,
    #[serde(rename = "System & Runtimes")]
    SystemAndRuntimes,
    #[default]
    #[serde(rename = "Uncategorized")]
    Uncategorized,
}

impl AppCategory {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Games => "Games",
            Self::PlatformsAndLaunchers => "Platforms & Launchers",
            Self::Browsers => "Browsers",
            Self::Communication => "Communication",
            Self::Development => "Development",
            Self::MediaAndCreative => "Media & Creative",
            Self::Productivity => "Productivity",
            Self::Utilities => "Utilities",
            Self::SystemAndRuntimes => "System & Runtimes",
            Self::Uncategorized => "Uncategorized",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DependencyKind {
    PlatformLauncher,
    AntiCheatServiceDriver,
    SharedRuntime,
    SharedData,
}

impl DependencyKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::PlatformLauncher => "platform/launcher client",
            Self::AntiCheatServiceDriver => "anti-cheat, service or driver",
            Self::SharedRuntime => "shared runtime",
            Self::SharedData => "shared data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DependencyConfidence {
    Proven,
    Inferred,
}

impl DependencyConfidence {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::Inferred => "inferred",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRecord {
    pub target_id: String,
    pub target_name: String,
    pub kind: DependencyKind,
    pub present: bool,
    pub own_size_bytes: Option<u64>,
    pub shared: bool,
    pub basis: String,
    pub confidence: DependencyConfidence,
    #[serde(default)]
    pub imported_by_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseConsumer {
    pub app_id: String,
    pub app_name: String,
    pub app_total_bytes: u64,
    pub confidence: DependencyConfidence,
    pub basis: String,
    #[serde(default)]
    pub imported_by_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseDependencyEntry {
    pub target_id: String,
    pub target_name: String,
    pub kind: DependencyKind,
    pub present: bool,
    pub own_size_bytes: Option<u64>,
    pub dependent_count: usize,
    pub binary_importer_count: usize,
    pub consumers: Vec<ReverseConsumer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub letter: char,
    pub root: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub used_bytes: u64,
    pub is_system_drive: bool,
    pub full_walk_enabled: bool,
    pub exclusions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scanned_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scanned_display: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegistryApp {
    pub hive_view: String,
    pub subkey_name: String,
    pub display_name: String,
    #[serde(default)]
    pub display_version: String,
    pub publisher: String,
    pub install_location: String,
    pub uninstall_string: String,
    pub display_icon: String,
    pub system_component: bool,
    pub parent_key_name: String,
    pub release_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryDiscoveryStats {
    pub raw_display_name_entries: usize,
    pub raw_with_install_location: usize,
    pub canonical_apps: usize,
    pub canonical_with_direct_install_location: usize,
    pub canonical_with_inferred_install_location: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedAppIcon {
    pub width: u16,
    pub height: u16,
    pub rgba_b64: String,
}

impl CachedAppIcon {
    #[must_use]
    pub fn from_rgba(width: u16, height: u16, rgba: &[u8]) -> Self {
        Self {
            width,
            height,
            rgba_b64: encode_base64(rgba),
        }
    }

    #[must_use]
    pub fn decode_rgba(&self) -> Option<Vec<u8>> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let expected_len = (self.width as usize)
            .checked_mul(self.height as usize)?
            .checked_mul(4)?;
        let decoded = decode_base64(&self.rgba_b64)?;
        if decoded.len() == expected_len {
            Some(decoded)
        } else {
            None
        }
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) & 0x3F] as char);
        out.push(TABLE[(n >> 12) & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let raw = input.trim().as_bytes();
    if raw.is_empty() {
        return Some(Vec::new());
    }
    if raw.len() % 4 != 0 {
        return None;
    }
    let val = |b: u8| -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity((raw.len() / 4) * 3);
    for (idx, chunk) in raw.chunks_exact(4).enumerate() {
        let is_last = idx + 1 == raw.len() / 4;
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let pad_c = chunk[2] == b'=';
        let pad_d = chunk[3] == b'=';
        if (pad_c && !pad_d) || ((pad_c || pad_d) && !is_last) {
            return None;
        }
        let c = if pad_c { 0 } else { val(chunk[2])? };
        let d = if pad_d { 0 } else { val(chunk[3])? };
        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | (d as u32);
        out.push(((n >> 16) & 0xFF) as u8);
        if !pad_c {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if !pad_d {
            out.push((n & 0xFF) as u8);
        }
    }
    Some(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub display_version: String,
    #[serde(default)]
    pub uninstall_guids: Vec<String>,
    pub aliases: Vec<String>,
    pub publishers: Vec<String>,
    pub direct_install_location: Option<String>,
    pub inferred_install_location: Option<String>,
    pub all_install_locations: Vec<String>,
    pub no_touch: Option<NoTouchCategory>,
    pub is_store_fallback: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_icon: Option<String>,
}

impl InstalledApp {
    #[must_use]
    pub fn primary_install_location(&self) -> Option<&str> {
        self.direct_install_location
            .as_deref()
            .or(self.inferred_install_location.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderNode {
    pub path: String,
    pub name: String,
    pub direct_file_bytes: u64,
    pub total_bytes: u64,
    pub children: Vec<FolderNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScannedUnitKind {
    Regular,
    PersonalShell,
    RootSystemFile,
    RegistryUserHive,
    ContainerLooseFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestAppEntry {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    pub install_dir: String,
    pub container_dir: String,
    pub basis: AttributionBasis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedUnit {
    pub path: String,
    pub drive_letter: char,
    pub bytes: u64,
    pub kind: ScannedUnitKind,
    pub is_direct_install_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakdownItem {
    pub path: String,
    pub bytes: u64,
    pub zone: Option<ZoneKind>,
    pub qualifier: Option<LocationQualifier>,
    pub basis: Option<AttributionBasis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis_detail: Option<String>,
    pub no_touch: Option<NoTouchCategory>,
    pub drive_letter: char,
}

impl BreakdownItem {
    #[must_use]
    pub fn basis_label(&self) -> &str {
        self.basis_detail
            .as_deref()
            .or_else(|| self.basis.map(AttributionBasis::label))
            .unwrap_or("—")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowthDelta {
    NoBaseline,
    New(u64),
    Changed(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributedApp {
    pub id: String,
    pub display_name: String,
    pub publishers: Vec<String>,
    pub install_location: Option<String>,
    pub total_bytes: u64,
    pub previous_bytes: Option<u64>,
    pub growth: GrowthDelta,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_drive_growth: BTreeMap<char, GrowthDelta>,
    pub no_touch: Option<NoTouchCategory>,
    pub bases: Vec<AttributionBasis>,
    pub items: Vec<BreakdownItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carved_out_apps: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<DependencyRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_rgba: Option<CachedAppIcon>,
    #[serde(default)]
    pub category: AppCategory,
    #[serde(default)]
    pub category_basis: String,
}

impl AttributedApp {
    /// Returns the sum of this app's OWN location bytes on `drive_letter` (never dependencies).
    #[must_use]
    pub fn bytes_on_drive(&self, drive_letter: char) -> u64 {
        let target = drive_letter.to_ascii_uppercase();
        self.items
            .iter()
            .filter(|it| it.drive_letter.to_ascii_uppercase() == target)
            .map(|it| it.bytes)
            .sum()
    }

    /// Returns the recorded growth delta for `drive_letter`, falling back to global `growth`
    /// only when legacy snapshots omit `per_drive_growth` and every location is on `drive_letter`.
    #[must_use]
    pub fn growth_for_drive(&self, drive_letter: char) -> GrowthDelta {
        let target = drive_letter.to_ascii_uppercase();
        if let Some(&g) = self.per_drive_growth.get(&target) {
            return g;
        }
        if !self.items.is_empty()
            && self
                .items
                .iter()
                .all(|it| it.drive_letter.to_ascii_uppercase() == target)
        {
            return self.growth;
        }
        GrowthDelta::NoBaseline
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDriveBreakdown {
    pub drive_letter: char,
    pub root_files: Vec<BreakdownItem>,
    #[serde(default)]
    pub os_folders: Vec<BreakdownItem>,
    pub remainder_bytes: u64,
    #[serde(default)]
    pub hardlink_overlap_bytes: u64,
    pub total_bytes: u64,
    pub scanned_non_system_bytes: u64,
    pub drive_used_bytes: u64,
    pub drive_free_bytes: u64,
    pub drive_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnattributedItem {
    pub path: String,
    pub bytes: u64,
    pub drive_letter: char,
    pub zone_hint: Option<ZoneKind>,
    #[serde(default)]
    pub is_loose_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub scan_timestamp_unix: u64,
    pub scan_timestamp_display: String,
    pub duration_ms: u64,
    pub drives: Vec<DriveInfo>,
    pub registry_stats: RegistryDiscoveryStats,
    pub apps: Vec<AttributedApp>,
    #[serde(default)]
    pub shared_caches: Vec<BreakdownItem>,
    #[serde(default)]
    pub shared_caches_total_bytes: u64,
    pub your_files: Vec<BreakdownItem>,
    pub your_files_total_bytes: u64,
    pub windows_system: Vec<SystemDriveBreakdown>,
    pub windows_system_total_bytes: u64,
    pub unattributed: Vec<UnattributedItem>,
    pub unattributed_total_bytes: u64,
    #[serde(default)]
    pub reverse_dependencies: Vec<ReverseDependencyEntry>,
    #[serde(default)]
    pub installed_registry_names: Vec<String>,
    #[serde(default)]
    pub catalog_source: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_containers: Option<crate::config::ContainerRulebook>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_zones: Option<crate::config::ZoneRulebook>,
}

impl ScanReport {
    /// Returns `true` when drive scope, drive exclusions, or zone rules in `config`
    /// differ from the configuration that produced this report.
    #[must_use]
    pub fn is_scope_stale(&self, config: &crate::config::AppConfig) -> bool {
        for drv in &self.drives {
            let letter = drv.letter.to_ascii_uppercase();
            if config.is_drive_enabled(letter) != drv.full_walk_enabled {
                return true;
            }
            let mut report_ex: Vec<String> = drv
                .exclusions
                .iter()
                .map(|e| crate::discovery::normalize_path_key(e))
                .filter(|s| !s.is_empty())
                .collect();
            let mut cfg_ex: Vec<String> = config
                .exclusions_for_drive(letter)
                .into_iter()
                .map(|e| crate::discovery::normalize_path_key(&e))
                .filter(|s| !s.is_empty())
                .collect();
            report_ex.sort();
            report_ex.dedup();
            cfg_ex.sort();
            cfg_ex.dedup();
            if report_ex != cfg_ex {
                return true;
            }
        }
        if let Some(ref applied_zones) = self.applied_zones {
            if applied_zones != &config.zones {
                return true;
            }
        } else if let Some(ref applied) = self.applied_containers {
            if applied != &config.containers {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn total_apps_bytes_on_drive(&self, drive_letter: char) -> u64 {
        let target = drive_letter.to_ascii_uppercase();
        self.apps
            .iter()
            .flat_map(|a| a.items.iter())
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target)
            .map(|i| i.bytes)
            .sum()
    }

    #[must_use]
    pub fn total_shared_caches_on_drive(&self, drive_letter: char) -> u64 {
        let target = drive_letter.to_ascii_uppercase();
        self.shared_caches
            .iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target)
            .map(|i| i.bytes)
            .sum()
    }

    #[must_use]
    pub fn total_your_files_on_drive(&self, drive_letter: char) -> u64 {
        let target = drive_letter.to_ascii_uppercase();
        self.your_files
            .iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target)
            .map(|i| i.bytes)
            .sum()
    }

    #[must_use]
    pub fn total_unattributed_on_drive(&self, drive_letter: char) -> u64 {
        let target = drive_letter.to_ascii_uppercase();
        self.unattributed
            .iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target)
            .map(|i| i.bytes)
            .sum()
    }
}

#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[must_use]
pub fn format_growth_delta(delta: GrowthDelta) -> String {
    match delta {
        GrowthDelta::NoBaseline => "—".to_string(),
        GrowthDelta::New(bytes) => {
            if bytes == 0 {
                "NEW".to_string()
            } else {
                format!("NEW {}", format_bytes(bytes))
            }
        }
        GrowthDelta::Changed(0) => "0 B".to_string(),
        GrowthDelta::Changed(d) if d > 0 => format!("+{}", format_bytes(d as u64)),
        GrowthDelta::Changed(d) => format!("-{}", format_bytes(d.unsigned_abs())),
    }
}
