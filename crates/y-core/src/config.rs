use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::discovery::normalize_path_key;
use crate::model::{AppCategory, DriveInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub theme_mode: ThemeMode,
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
    pub targets: Vec<DriveScanTarget>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            theme_mode: ThemeMode::Dark,
            window: None,
            sort_mode: AppSortMode::Size,
            sort_direction: SortDirection::Desc,
            filter_chip: AppFilterChip::All,
            search_query: String::new(),
            facets: FacetFilters::default(),
            ledger_collapsed: false,
            ledger_height_px: None,
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
            window: None,
            sort_mode: AppSortMode::Size,
            sort_direction: SortDirection::Desc,
            filter_chip: AppFilterChip::All,
            search_query: String::new(),
            facets: FacetFilters::default(),
            ledger_collapsed: false,
            ledger_height_px: None,
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
            Ok(raw) => toml::from_str::<Self>(&raw)
                .unwrap_or_else(|_| Self::for_detected_drives(drives, system_windows_dir)),
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
