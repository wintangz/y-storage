use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::discovery::{is_subpath_of, normalize_path_key, normalize_windows_path, path_drive_letter};
use crate::model::{AppCategory, DriveInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePresetChoice {
    #[default]
    Blue,
    Pink,
    Purple,
    Green,
    Orange,
    Yellow,
    Red,
    Custom,
}

impl ThemePresetChoice {
    pub const PRESETS: [Self; 7] = [
        Self::Blue,
        Self::Pink,
        Self::Purple,
        Self::Green,
        Self::Orange,
        Self::Yellow,
        Self::Red,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Pink => "Pink",
            Self::Purple => "Purple",
            Self::Green => "Green",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Red => "Red",
            Self::Custom => "Custom",
        }
    }

    #[must_use]
    pub const fn base_rgb(self) -> [u8; 3] {
        match self {
            Self::Blue => [0x7C, 0xCB, 0xF2],   // #7CCBF2
            Self::Pink => [0xF2, 0xA9, 0xD6],   // #F2A9D6
            Self::Purple => [0x9B, 0x8A, 0xFB], // #9B8AFB
            Self::Green => [0x3E, 0xCF, 0x6E],  // #3ECF6E
            Self::Orange => [0xFB, 0x92, 0x3C], // #FB923C
            Self::Yellow => [0xFA, 0xCC, 0x15], // #FACC15
            Self::Red => [0xF8, 0x71, 0x71],    // #F87171
            Self::Custom => [0x7C, 0xCB, 0xF2],
        }
    }
}

#[must_use]
pub const fn default_custom_theme_rgb() -> [u8; 3] {
    [0x7C, 0xCB, 0xF2]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppSortMode {
    #[default]
    Size,
    Name,
    Publisher,
    Category,
    Growth,
}

impl AppSortMode {
    #[must_use]
    pub const fn default_direction(self) -> SortDirection {
        match self {
            Self::Size | Self::Growth => SortDirection::Desc,
            Self::Name | Self::Publisher | Self::Category => SortDirection::Asc,
        }
    }

    #[must_use]
    pub const fn click_transition(
        self,
        active_mode: Self,
        active_dir: SortDirection,
    ) -> (Self, SortDirection) {
        if self as u8 == active_mode as u8 {
            (self, active_dir.flipped())
        } else {
            (self, self.default_direction())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortDirection {
    #[default]
    Desc,
    Asc,
}

impl SortDirection {
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Desc => Self::Asc,
            Self::Asc => Self::Desc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppFilterChip {
    #[default]
    All,
    Grew,
    NoTouch,
    Standalone,
    HasSystemDriveFootprint,
}

pub const NO_PUBLISHER_FACET_KEY: &str = "__no_publisher__";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SizeBandFilter {
    #[default]
    All,
    Ge10Gb,
    Between1And10Gb,
    Ge1Gb,
    Between100MbAnd1Gb,
    Lt100Mb,
}

impl SizeBandFilter {
    pub const ALL_OPTIONS: [Self; 6] = [
        Self::All,
        Self::Ge10Gb,
        Self::Between1And10Gb,
        Self::Ge1Gb,
        Self::Between100MbAnd1Gb,
        Self::Lt100Mb,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All sizes",
            Self::Ge10Gb => "≥ 10 GB",
            Self::Between1And10Gb => "1–10 GB",
            Self::Ge1Gb => "≥ 1 GB",
            Self::Between100MbAnd1Gb => "100 MB – 1 GB",
            Self::Lt100Mb => "< 100 MB",
        }
    }

    #[must_use]
    pub const fn matches_bytes(self, total_bytes: u64) -> bool {
        const MIB: u64 = 1024 * 1024;
        const GIB: u64 = 1024 * MIB;
        match self {
            Self::All => true,
            Self::Ge10Gb => total_bytes >= 10 * GIB,
            Self::Between1And10Gb => total_bytes >= GIB && total_bytes < 10 * GIB,
            Self::Ge1Gb => total_bytes >= GIB,
            Self::Between100MbAnd1Gb => total_bytes >= 100 * MIB && total_bytes < GIB,
            Self::Lt100Mb => total_bytes < 100 * MIB,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DepStateFilter {
    #[default]
    All,
    HasDependencies,
    NoKnownDependencies,
    SharedUsedByOthers,
}

impl DepStateFilter {
    pub const ALL_OPTIONS: [Self; 4] = [
        Self::All,
        Self::HasDependencies,
        Self::NoKnownDependencies,
        Self::SharedUsedByOthers,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All dependency states",
            Self::HasDependencies => "Has dependencies",
            Self::NoKnownDependencies => "No known dependencies",
            Self::SharedUsedByOthers => "Shared — used by other apps",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FootprintZoneFilter {
    #[default]
    All,
    InstallFolder,
    AppData,
    ProgramData,
    ProfileOrDotFolder,
}

impl FootprintZoneFilter {
    pub const ALL_OPTIONS: [Self; 5] = [
        Self::All,
        Self::InstallFolder,
        Self::AppData,
        Self::ProgramData,
        Self::ProfileOrDotFolder,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All zones",
            Self::InstallFolder => "Install folder",
            Self::AppData => "AppData",
            Self::ProgramData => "ProgramData",
            Self::ProfileOrDotFolder => "Profile / dot-folder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GrowthFacetFilter {
    #[default]
    All,
    Grew,
    Shrank,
    Unchanged,
}

impl GrowthFacetFilter {
    pub const ALL_OPTIONS: [Self; 4] = [
        Self::All,
        Self::Grew,
        Self::Shrank,
        Self::Unchanged,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All growth",
            Self::Grew => "Grew",
            Self::Shrank => "Shrank",
            Self::Unchanged => "Unchanged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FacetFilters {
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub category: Option<AppCategory>,
    #[serde(default)]
    pub drive: Option<char>,
    #[serde(default)]
    pub size_band: SizeBandFilter,
    #[serde(default)]
    pub dep_state: DepStateFilter,
    #[serde(default)]
    pub footprint_zone: FootprintZoneFilter,
    #[serde(default)]
    pub growth: GrowthFacetFilter,
}

impl FacetFilters {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.publisher.is_none()
            && self.category.is_none()
            && self.drive.is_none()
            && self.size_band == SizeBandFilter::All
            && self.dep_state == DepStateFilter::All
            && self.footprint_zone == FootprintZoneFilter::All
            && self.growth == GrowthFacetFilter::All
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveScanTarget {
    pub letter: char,
    pub enabled: bool,
    pub exclusions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPlacementConfig {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub maximized: bool,
}

pub const DEFAULT_CONTAINER_NAMES: &[&str] = &[
    "Apps",
    "Games",
    "Game",
    "Programs",
    "Program Files",
    "Software",
    "Softwares",
    "PortableApps",
];

pub const DEFAULT_PERSONAL_NAMES: &[&str] = &[
    "Documents",
    "Downloads",
    "Pictures",
    "Videos",
    "Music",
    "Desktop",
];

fn default_container_names() -> Vec<String> {
    DEFAULT_CONTAINER_NAMES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZoneRuleKind {
    AppContainer,
    PersonalFiles,
}

impl ZoneRuleKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AppContainer => "App container",
            Self::PersonalFiles => "Personal files",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneRootEntry {
    pub name: String,
    pub kind: ZoneRuleKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZonePathEntry {
    pub path: String,
    pub kind: ZoneRuleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinKind {
    App,
    Group,
    Personal,
    Ignore,
}

impl PinKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::App => "Pin: App",
            Self::Group => "Pin: Group",
            Self::Personal => "Pin: Personal",
            Self::Ignore => "Pin: Ignore",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::App => "App",
            Self::Group => "Group",
            Self::Personal => "Personal",
            Self::Ignore => "Ignore",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderPin {
    pub path: String,
    pub kind: PinKind,
}

fn default_root_zones() -> Vec<ZoneRootEntry> {
    let mut entries: Vec<ZoneRootEntry> = DEFAULT_CONTAINER_NAMES
        .iter()
        .map(|name| ZoneRootEntry {
            name: (*name).to_string(),
            kind: ZoneRuleKind::AppContainer,
        })
        .collect();
    entries.extend(DEFAULT_PERSONAL_NAMES.iter().map(|name| ZoneRootEntry {
        name: (*name).to_string(),
        kind: ZoneRuleKind::PersonalFiles,
    }));
    entries
}

/// Unified user-editable attribution rulebook: combines root/explicit `AppContainer` and
/// `PersonalFiles` zones plus per-folder `PinKind` overrides (`App`, `Group`, `Personal`, `Ignore`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneRulebook {
    #[serde(default = "default_root_zones")]
    pub root_zones: Vec<ZoneRootEntry>,
    #[serde(default)]
    pub explicit_zones: Vec<ZonePathEntry>,
    #[serde(default)]
    pub pins: Vec<FolderPin>,
}

impl Default for ZoneRulebook {
    fn default() -> Self {
        Self {
            root_zones: default_root_zones(),
            explicit_zones: Vec::new(),
            pins: Vec::new(),
        }
    }
}

impl ZoneRulebook {
    #[must_use]
    pub fn from_legacy_containers(c: &ContainerRulebook) -> Self {
        let mut root_zones: Vec<ZoneRootEntry> = c
            .root_folder_names
            .iter()
            .map(|name| ZoneRootEntry {
                name: name.clone(),
                kind: ZoneRuleKind::AppContainer,
            })
            .collect();
        for name in DEFAULT_PERSONAL_NAMES {
            if !root_zones
                .iter()
                .any(|rz| rz.name.eq_ignore_ascii_case(name))
            {
                root_zones.push(ZoneRootEntry {
                    name: (*name).to_string(),
                    kind: ZoneRuleKind::PersonalFiles,
                });
            }
        }
        let explicit_zones = c
            .explicit_paths
            .iter()
            .map(|p| ZonePathEntry {
                path: normalize_windows_path(p),
                kind: ZoneRuleKind::AppContainer,
            })
            .collect();
        Self {
            root_zones,
            explicit_zones,
            pins: Vec::new(),
        }
    }

    #[must_use]
    pub fn pin_for_path(&self, path: &str) -> Option<PinKind> {
        let pk = normalize_path_key(path);
        if pk.is_empty() {
            return None;
        }
        self.pins
            .iter()
            .find(|p| normalize_path_key(&p.path) == pk)
            .map(|p| p.kind)
    }

    /// Returns the matching app-container label if `path` is either an explicit `AppContainer`
    /// path (`Some("D:\\Tools")`) or a drive-root folder matching an `AppContainer` entry
    /// (`D:\Games` -> `Some("Games")`). Explicit zones and pins override root defaults cleanly.
    #[must_use]
    pub fn matches_app_container_dir(&self, path: &str) -> Option<String> {
        let norm = normalize_windows_path(path);
        let pk = normalize_path_key(&norm);
        if pk.is_empty() || pk.len() <= 3 {
            return None;
        }
        if let Some(pin) = self.pin_for_path(&norm) {
            if pin != PinKind::Group {
                return None;
            }
        }
        if let Some(ez) = self
            .explicit_zones
            .iter()
            .find(|ep| normalize_path_key(&ep.path) == pk)
        {
            return (ez.kind == ZoneRuleKind::AppContainer)
                .then(|| normalize_windows_path(&ez.path));
        }
        let trimmed = norm.trim_end_matches('\\');
        let parts: Vec<&str> = trimmed.split('\\').collect();
        if parts.len() == 2 && path_drive_letter(trimmed).is_some() {
            let folder_name = parts[1];
            if let Some(matched) = self.root_zones.iter().find(|rz| {
                rz.kind == ZoneRuleKind::AppContainer
                    && rz.name.trim().eq_ignore_ascii_case(folder_name)
            }) {
                return Some(matched.name.trim().to_string());
            }
        }
        None
    }

    /// Returns true if `path` belongs to a `PersonalFiles` zone (via pin, explicit zone path,
    /// drive-root personal folder name, user profile subfolder, or active OS shell folder).
    #[must_use]
    pub fn matches_personal_dir(&self, path: &str, os_shell_folders: &[String]) -> bool {
        let norm = normalize_windows_path(path);
        let pk = normalize_path_key(&norm);
        if pk.is_empty() || pk.len() <= 3 {
            return false;
        }
        if let Some(pin) = self.pin_for_path(&norm) {
            return pin == PinKind::Personal;
        }
        if let Some(ez) = self
            .explicit_zones
            .iter()
            .find(|ep| normalize_path_key(&ep.path) == pk)
        {
            return ez.kind == ZoneRuleKind::PersonalFiles;
        }
        let trimmed = pk.trim_end_matches('\\');
        let parts: Vec<&str> = trimmed.split('\\').collect();
        if parts.len() == 2 && path_drive_letter(trimmed).is_some() {
            let folder_name = parts[1];
            if let Some(rz) = self
                .root_zones
                .iter()
                .find(|r| r.name.trim().eq_ignore_ascii_case(folder_name))
            {
                return rz.kind == ZoneRuleKind::PersonalFiles;
            }
        }
        if parts.len() == 4 && parts[1] == "users" {
            let folder_name = parts[3];
            if let Some(rz) = self
                .root_zones
                .iter()
                .find(|r| r.name.trim().eq_ignore_ascii_case(folder_name))
            {
                return rz.kind == ZoneRuleKind::PersonalFiles;
            }
        }
        if os_shell_folders
            .iter()
            .any(|sf| normalize_path_key(sf) == pk)
        {
            let leaf = parts.last().copied().unwrap_or("");
            if DEFAULT_PERSONAL_NAMES
                .iter()
                .any(|d| d.eq_ignore_ascii_case(leaf))
            {
                return self.root_zones.iter().any(|r| {
                    r.kind == ZoneRuleKind::PersonalFiles && r.name.trim().eq_ignore_ascii_case(leaf)
                });
            }
            return true;
        }
        false
    }

    #[must_use]
    pub fn is_strict_ancestor_of_rule_or_pin(&self, path: &str) -> bool {
        let pk = normalize_path_key(path);
        if pk.is_empty() {
            return false;
        }
        self.explicit_zones
            .iter()
            .map(|ep| normalize_path_key(&ep.path))
            .chain(self.pins.iter().map(|p| normalize_path_key(&p.path)))
            .any(|tk| !tk.is_empty() && tk != pk && is_subpath_of(&tk, &pk))
    }

    pub fn add_root_zone(&mut self, raw: &str, kind: ZoneRuleKind) -> bool {
        let clean = raw
            .trim()
            .trim_matches(['\\', '/', '"'])
            .trim()
            .to_string();
        if clean.is_empty() || clean.contains('\\') || clean.contains('/') || clean.contains(':') {
            return false;
        }
        if let Some(existing) = self
            .root_zones
            .iter_mut()
            .find(|z| z.name.eq_ignore_ascii_case(&clean))
        {
            if existing.kind == kind {
                return false;
            }
            existing.kind = kind;
            return true;
        }
        self.root_zones.push(ZoneRootEntry { name: clean, kind });
        true
    }

    pub fn remove_root_zone(&mut self, idx: usize) -> bool {
        if idx < self.root_zones.len() {
            self.root_zones.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn add_explicit_zone(&mut self, raw: &str, kind: ZoneRuleKind) -> bool {
        let norm = normalize_windows_path(raw);
        let pk = normalize_path_key(&norm);
        if norm.len() <= 3 || path_drive_letter(&norm).is_none() {
            return false;
        }
        if let Some(existing) = self
            .explicit_zones
            .iter_mut()
            .find(|p| normalize_path_key(&p.path) == pk)
        {
            if existing.kind == kind {
                return false;
            }
            existing.kind = kind;
            return true;
        }
        self.explicit_zones.push(ZonePathEntry { path: norm, kind });
        true
    }

    pub fn remove_explicit_zone(&mut self, idx: usize) -> bool {
        if idx < self.explicit_zones.len() {
            self.explicit_zones.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn set_pin(&mut self, raw: &str, kind: PinKind) -> bool {
        let norm = normalize_windows_path(raw);
        let pk = normalize_path_key(&norm);
        if norm.len() <= 3 || path_drive_letter(&norm).is_none() {
            return false;
        }
        if let Some(existing) = self
            .pins
            .iter_mut()
            .find(|p| normalize_path_key(&p.path) == pk)
        {
            if existing.kind == kind {
                return false;
            }
            existing.kind = kind;
            return true;
        }
        self.pins.push(FolderPin { path: norm, kind });
        true
    }

    pub fn remove_pin(&mut self, idx: usize) -> bool {
        if idx < self.pins.len() {
            self.pins.remove(idx);
            true
        } else {
            false
        }
    }
}

/// User-editable app container rulebook: matches top-level drive-root folder names or explicit full paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRulebook {
    #[serde(default = "default_container_names")]
    pub root_folder_names: Vec<String>,
    #[serde(default)]
    pub explicit_paths: Vec<String>,
}

impl Default for ContainerRulebook {
    fn default() -> Self {
        Self {
            root_folder_names: default_container_names(),
            explicit_paths: Vec::new(),
        }
    }
}

impl ContainerRulebook {
    /// Returns the matching container label if `path` is either a configured drive-root container
    /// directory (e.g. `D:\Games` -> `Some("Games")`) or an explicit container path (`Some("D:\\Tools")`).
    #[must_use]
    pub fn matches_container_dir(&self, path: &str) -> Option<String> {
        let norm = normalize_windows_path(path);
        let pk = normalize_path_key(&norm);
        if pk.is_empty() || pk.len() <= 3 {
            return None;
        }
        for ep in &self.explicit_paths {
            let ek = normalize_path_key(ep);
            if !ek.is_empty() && ek == pk {
                return Some(normalize_windows_path(ep));
            }
        }
        let trimmed = norm.trim_end_matches('\\');
        let parts: Vec<&str> = trimmed.split('\\').collect();
        if parts.len() == 2 && path_drive_letter(trimmed).is_some() {
            let folder_name = parts[1];
            if let Some(matched) = self
                .root_folder_names
                .iter()
                .find(|name| name.trim().eq_ignore_ascii_case(folder_name))
            {
                return Some(matched.trim().to_string());
            }
        }
        None
    }

    /// Returns true when `path` is a strict ancestor of an explicit full-path container rule,
    /// ensuring tree decomposition splits intermediate directories to reach it.
    #[must_use]
    pub fn is_ancestor_of_explicit(&self, path: &str) -> bool {
        let pk = normalize_path_key(path);
        if pk.is_empty() {
            return false;
        }
        self.explicit_paths.iter().any(|ep| {
            let ek = normalize_path_key(ep);
            !ek.is_empty() && ek != pk && is_subpath_of(&ek, &pk)
        })
    }

    pub fn add_root_name(&mut self, raw: &str) -> bool {
        let clean = raw
            .trim()
            .trim_matches(['\\', '/', '"'])
            .trim()
            .to_string();
        if clean.is_empty() || clean.contains('\\') || clean.contains('/') {
            return false;
        }
        if self
            .root_folder_names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&clean))
        {
            return false;
        }
        self.root_folder_names.push(clean);
        true
    }

    pub fn remove_root_name(&mut self, idx: usize) -> bool {
        if idx < self.root_folder_names.len() {
            self.root_folder_names.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn add_explicit_path(&mut self, raw: &str) -> bool {
        let norm = normalize_windows_path(raw);
        let pk = normalize_path_key(&norm);
        if norm.len() <= 3 || path_drive_letter(&norm).is_none() {
            return false;
        }
        if self
            .explicit_paths
            .iter()
            .any(|p| normalize_path_key(p) == pk)
        {
            return false;
        }
        self.explicit_paths.push(norm);
        true
    }

    pub fn remove_explicit_path(&mut self, idx: usize) -> bool {
        if idx < self.explicit_paths.len() {
            self.explicit_paths.remove(idx);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UnattributedLocationZone {
    #[default]
    All,
    AppDataLocal,
    AppDataRoaming,
    AppDataLocalLow,
    ProgramData,
    ProgramFiles,
    UserProfileRoot,
    OtherDriveLocation,
}

impl UnattributedLocationZone {
    pub const ALL_OPTIONS: [Self; 8] = [
        Self::All,
        Self::AppDataLocal,
        Self::AppDataRoaming,
        Self::AppDataLocalLow,
        Self::ProgramData,
        Self::ProgramFiles,
        Self::UserProfileRoot,
        Self::OtherDriveLocation,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All locations",
            Self::AppDataLocal => "AppData Local",
            Self::AppDataRoaming => "AppData Roaming",
            Self::AppDataLocalLow => "AppData LocalLow",
            Self::ProgramData => "ProgramData",
            Self::ProgramFiles => "Program Files",
            Self::UserProfileRoot => "User profile root",
            Self::OtherDriveLocation => "Other drive location",
        }
    }

    #[must_use]
    pub fn classify_path(path: &str) -> Self {
        let p = normalize_path_key(path);
        if p.contains("\\appdata\\locallow\\") || p.ends_with("\\appdata\\locallow") {
            return Self::AppDataLocalLow;
        }
        if p.contains("\\appdata\\roaming\\") || p.ends_with("\\appdata\\roaming") {
            return Self::AppDataRoaming;
        }
        if p.contains("\\appdata\\local\\") || p.ends_with("\\appdata\\local") {
            return Self::AppDataLocal;
        }
        if p.contains(":\\programdata\\") || p.ends_with(":\\programdata") {
            return Self::ProgramData;
        }
        if p.contains(":\\program files\\")
            || p.contains(":\\program files (x86)\\")
            || p.ends_with(":\\program files")
            || p.ends_with(":\\program files (x86)")
        {
            return Self::ProgramFiles;
        }
        let parts: Vec<&str> = p.trim_end_matches('\\').split('\\').collect();
        if parts.len() >= 3 && parts[1] == "users" {
            return Self::UserProfileRoot;
        }
        Self::OtherDriveLocation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnattributedFacetFilters {
    pub drive: Option<char>,
    pub size_band: SizeBandFilter,
    pub location_zone: UnattributedLocationZone,
}

impl UnattributedFacetFilters {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.drive.is_none()
            && self.size_band == SizeBandFilter::All
            && self.location_zone == UnattributedLocationZone::All
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub theme_preset: ThemePresetChoice,
    #[serde(default = "default_custom_theme_rgb")]
    pub theme_custom_rgb: [u8; 3],
    #[serde(default)]
    pub window: Option<WindowPlacementConfig>,
    #[serde(default)]
    pub sort_mode: AppSortMode,
    #[serde(default)]
    pub sort_direction: SortDirection,
    #[serde(default)]
    pub filter_chip: AppFilterChip,
    #[serde(default)]
    pub search_query: String,
    #[serde(default)]
    pub facets: FacetFilters,
    #[serde(default)]
    pub ledger_collapsed: bool,
    #[serde(default)]
    pub ledger_height_px: Option<f32>,
    #[serde(default)]
    pub containers: ContainerRulebook,
    #[serde(default)]
    pub zones: ZoneRulebook,
    pub targets: Vec<DriveScanTarget>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            theme_mode: ThemeMode::Dark,
            theme_preset: ThemePresetChoice::Blue,
            theme_custom_rgb: default_custom_theme_rgb(),
            window: None,
            sort_mode: AppSortMode::Size,
            sort_direction: SortDirection::Desc,
            filter_chip: AppFilterChip::All,
            search_query: String::new(),
            facets: FacetFilters::default(),
            ledger_collapsed: false,
            ledger_height_px: None,
            containers: ContainerRulebook::default(),
            zones: ZoneRulebook::default(),
            targets: vec![DriveScanTarget {
                letter: 'C',
                enabled: true,
                exclusions: vec!["C:\\Windows".to_string()],
            }],
        }
    }
}

impl AppConfig {
    #[must_use]
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("y-storage").join("config.toml")
    }

    pub fn for_detected_drives(drives: &[DriveInfo], system_windows_dir: &str) -> Self {
        let sys_win_clean = system_windows_dir.trim_end_matches('\\').to_string();
        let targets = drives
            .iter()
            .map(|d| {
                let letter = d.letter.to_ascii_uppercase();
                let exclusions = if d.is_system_drive {
                    vec![if sys_win_clean.is_empty() {
                        format!("{letter}:\\Windows")
                    } else {
                        sys_win_clean.clone()
                    }]
                } else {
                    Vec::new()
                };
                DriveScanTarget {
                    letter,
                    enabled: d.is_system_drive,
                    exclusions,
                }
            })
            .collect();

        Self {
            version: 1,
            theme_mode: ThemeMode::Dark,
            theme_preset: ThemePresetChoice::Blue,
            theme_custom_rgb: default_custom_theme_rgb(),
            window: None,
            sort_mode: AppSortMode::Size,
            sort_direction: SortDirection::Desc,
            filter_chip: AppFilterChip::All,
            search_query: String::new(),
            facets: FacetFilters::default(),
            ledger_collapsed: false,
            ledger_height_px: None,
            containers: ContainerRulebook::default(),
            zones: ZoneRulebook::default(),
            targets,
        }
    }

    pub fn sync_with_drives(&mut self, drives: &[DriveInfo], system_windows_dir: &str) {
        let sys_win_clean = system_windows_dir.trim_end_matches('\\').to_string();
        for d in drives {
            let letter = d.letter.to_ascii_uppercase();
            if let Some(existing) = self
                .targets
                .iter_mut()
                .find(|t| t.letter.to_ascii_uppercase() == letter)
            {
                if d.is_system_drive {
                    let expected = if sys_win_clean.is_empty() {
                        format!("{letter}:\\Windows")
                    } else {
                        sys_win_clean.clone()
                    };
                    let expected_key = normalize_path_key(&expected);
                    if !existing
                        .exclusions
                        .iter()
                        .any(|e| normalize_path_key(e) == expected_key)
                    {
                        existing.exclusions.insert(0, expected);
                    }
                }
            } else {
                let exclusions = if d.is_system_drive {
                    vec![if sys_win_clean.is_empty() {
                        format!("{letter}:\\Windows")
                    } else {
                        sys_win_clean.clone()
                    }]
                } else {
                    Vec::new()
                };
                self.targets.push(DriveScanTarget {
                    letter,
                    enabled: d.is_system_drive,
                    exclusions,
                });
            }
        }
        self.targets.sort_by_key(|t| t.letter);
    }

    #[must_use]
    pub fn is_drive_enabled(&self, letter: char) -> bool {
        let target = letter.to_ascii_uppercase();
        self.targets
            .iter()
            .find(|t| t.letter.to_ascii_uppercase() == target)
            .is_some_and(|t| t.enabled)
    }

    #[must_use]
    pub fn exclusions_for_drive(&self, letter: char) -> Vec<String> {
        let target = letter.to_ascii_uppercase();
        self.targets
            .iter()
            .find(|t| t.letter.to_ascii_uppercase() == target)
            .map(|t| t.exclusions.clone())
            .unwrap_or_default()
    }

    pub fn load_or_default(path: &Path, drives: &[DriveInfo], system_windows_dir: &str) -> Self {
        let mut cfg = match fs::read_to_string(path) {
            Ok(raw) => {
                let has_zones_section = raw.lines().any(|line| {
                    let t = line.trim();
                    t == "[zones]" || t.starts_with("[[zones.")
                });
                let mut parsed = toml::from_str::<Self>(&raw)
                    .unwrap_or_else(|_| Self::for_detected_drives(drives, system_windows_dir));
                if !has_zones_section {
                    parsed.zones = ZoneRulebook::from_legacy_containers(&parsed.containers);
                }
                parsed
            }
            Err(_) => Self::for_detected_drives(drives, system_windows_dir),
        };
        cfg.sync_with_drives(drives, system_windows_dir);
        cfg
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, body).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_preset_and_custom_rgb_persist_and_legacy_toml_defaults_to_blue() {
        let legacy_toml = "version = 1\ntheme_mode = \"Light\"\n[[targets]]\nletter = \"C\"\nenabled = true\nexclusions = []\n";
        let parsed: AppConfig = toml::from_str(legacy_toml).expect("parse legacy config");
        assert_eq!(parsed.theme_mode, ThemeMode::Light);
        assert_eq!(parsed.theme_preset, ThemePresetChoice::Blue);
        assert_eq!(parsed.theme_custom_rgb, default_custom_theme_rgb());

        let mut cfg = AppConfig::default();
        cfg.theme_mode = ThemeMode::Dark;
        cfg.theme_preset = ThemePresetChoice::Custom;
        cfg.theme_custom_rgb = [0x9B, 0x8A, 0xFB];
        let encoded = toml::to_string_pretty(&cfg).expect("serialize config");
        let roundtrip: AppConfig = toml::from_str(&encoded).expect("round-trip config");
        assert_eq!(roundtrip.theme_preset, ThemePresetChoice::Custom);
        assert_eq!(roundtrip.theme_custom_rgb, [0x9B, 0x8A, 0xFB]);
    }
}
