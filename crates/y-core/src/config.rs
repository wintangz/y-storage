use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::discovery::normalize_path_key;
use crate::model::DriveInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
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
    pub targets: Vec<DriveScanTarget>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            theme_mode: ThemeMode::Dark,
            window: None,
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
