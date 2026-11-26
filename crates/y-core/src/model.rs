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
}

impl LocationQualifier {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::RoamingConfig => "Roaming · Config",
            Self::MachineShared => "ProgramData · Machine-shared",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AttributionBasis {
    InstallLocation,
    ExactName,
    Token,
    Publisher,
    PackageFallback,
    StandaloneFolder,
}

impl AttributionBasis {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::InstallLocation => "install location",
            Self::ExactName => "exact name match",
            Self::Token => "token match",
            Self::Publisher => "publisher match",
            Self::PackageFallback => "package name",
            Self::StandaloneFolder => "standalone folder",
        }
    }
}

pub const NO_KNOWN_DEPENDENCIES_LABEL: &str = "no known dependencies";

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegistryApp {
    pub hive_view: String,
    pub subkey_name: String,
    pub display_name: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub id: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub publishers: Vec<String>,
    pub direct_install_location: Option<String>,
    pub inferred_install_location: Option<String>,
    pub all_install_locations: Vec<String>,
    pub no_touch: Option<NoTouchCategory>,
    pub is_store_fallback: bool,
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
    pub no_touch: Option<NoTouchCategory>,
    pub drive_letter: char,
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
    pub no_touch: Option<NoTouchCategory>,
    pub bases: Vec<AttributionBasis>,
    pub items: Vec<BreakdownItem>,
    #[serde(default)]
    pub dependencies: Vec<DependencyRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDriveBreakdown {
    pub drive_letter: char,
    pub root_files: Vec<BreakdownItem>,
    pub remainder_bytes: u64,
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
    pub catalog_source: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl ScanReport {
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
