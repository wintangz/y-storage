use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{GrowthDelta, ScanReport};

pub type HistoryEntryRef = BTreeMap<String, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScanScopeKind {
    #[default]
    Full,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanHistorySnapshot {
    pub timestamp_unix: u64,
    pub timestamp_display: String,
    #[serde(default)]
    pub kind: ScanScopeKind,
    #[serde(default)]
    pub refreshed_drives: Vec<char>,
    pub per_app_bytes: BTreeMap<String, u64>,
    #[serde(default)]
    pub per_drive_app_bytes: BTreeMap<char, BTreeMap<String, u64>>,
}

impl ScanHistorySnapshot {
    #[must_use]
    pub fn scope_label(&self) -> String {
        let list: String = if self.refreshed_drives.is_empty() {
            "C:\\".to_string()
        } else {
            self.refreshed_drives
                .iter()
                .map(|d| format!("{}:\\", d.to_ascii_uppercase()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self.kind {
            ScanScopeKind::Full => format!("Full scan ({list})"),
            ScanScopeKind::Partial => format!("Partial rescan ({list})"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanHistory {
    pub snapshots: Vec<ScanHistorySnapshot>,
}

impl ScanHistory {
    #[must_use]
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("y-storage").join("scan_history.json")
    }

    #[must_use]
    pub fn latest_per_app(&self) -> Option<&BTreeMap<String, u64>> {
        self.snapshots.last().map(|s| &s.per_app_bytes)
    }

    /// Returns the most recent per-app byte map specifically recorded for `drive_letter`.
    /// Never matches snapshots that did not include `drive_letter`.
    #[must_use]
    pub fn latest_for_drive(&self, drive_letter: char) -> Option<&BTreeMap<String, u64>> {
        self.latest_for_drive_before(drive_letter, u64::MAX)
    }

    /// Returns the most recent per-app byte map for `drive_letter` strictly before `before_ts_unix`
    /// (or at `u64::MAX` to inspect the latest snapshot before recording a new scan).
    #[must_use]
    pub fn latest_for_drive_before(
        &self,
        drive_letter: char,
        before_ts_unix: u64,
    ) -> Option<&BTreeMap<String, u64>> {
        let target = drive_letter.to_ascii_uppercase();
        for snap in self.snapshots.iter().rev() {
            if snap.timestamp_unix >= before_ts_unix {
                continue;
            }
            if snap
                .refreshed_drives
                .iter()
                .any(|d| d.to_ascii_uppercase() == target)
            {
                if let Some(map) = snap.per_drive_app_bytes.get(&target) {
                    return Some(map);
                }
            }
            // Legacy snapshots written prior to per-drive tracking covered C:\ only.
            if snap.refreshed_drives.is_empty() && snap.kind == ScanScopeKind::Full && target == 'C'
            {
                return Some(&snap.per_app_bytes);
            }
        }
        None
    }

    pub fn record_report(&mut self, report: &ScanReport) {
        let refreshed: Vec<char> = report
            .drives
            .iter()
            .filter(|d| d.full_walk_enabled)
            .map(|d| d.letter.to_ascii_uppercase())
            .collect();
        self.record_report_with_scope(report, ScanScopeKind::Full, &refreshed);
    }

    pub fn record_report_with_scope(
        &mut self,
        report: &ScanReport,
        kind: ScanScopeKind,
        refreshed_drives: &[char],
    ) {
        let mut refreshed_upper: Vec<char> = refreshed_drives
            .iter()
            .map(|d| d.to_ascii_uppercase())
            .collect();
        refreshed_upper.sort_unstable();
        refreshed_upper.dedup();
        let refreshed_set: HashSet<char> = refreshed_upper.iter().copied().collect();

        let mut per_app_bytes = BTreeMap::new();
        let mut per_drive_app_bytes: BTreeMap<char, BTreeMap<String, u64>> = BTreeMap::new();
        for &d in &refreshed_upper {
            per_drive_app_bytes.insert(d, BTreeMap::new());
        }
        for app in &report.apps {
            per_app_bytes.insert(app.display_name.clone(), app.total_bytes);
            for item in &app.items {
                let dl = item.drive_letter.to_ascii_uppercase();
                if refreshed_set.contains(&dl) {
                    let drv_map = per_drive_app_bytes.entry(dl).or_default();
                    *drv_map.entry(app.display_name.clone()).or_insert(0) += item.bytes;
                }
            }
        }
        self.snapshots.push(ScanHistorySnapshot {
            timestamp_unix: report.scan_timestamp_unix,
            timestamp_display: report.scan_timestamp_display.clone(),
            kind,
            refreshed_drives: refreshed_upper,
            per_app_bytes,
            per_drive_app_bytes,
        });
        if self.snapshots.len() > 60 {
            let drain_count = self.snapshots.len() - 60;
            self.snapshots.drain(0..drain_count);
        }
    }

    pub fn load_or_default(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str::<Self>(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, body).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

/// Recomputes each app's `previous_bytes` and `growth` per drive against the most recent history
/// snapshot that actually included each drive (`None` when a drive was not previously scanned).
pub fn recompute_report_growth_from_history(report: &mut ScanReport, history: &ScanHistory) {
    let mut baselines: BTreeMap<char, Option<&BTreeMap<String, u64>>> = BTreeMap::new();
    for drv in &report.drives {
        if drv.full_walk_enabled {
            let dl = drv.letter.to_ascii_uppercase();
            let ts = drv.last_scanned_unix.unwrap_or(report.scan_timestamp_unix);
            baselines.insert(dl, history.latest_for_drive_before(dl, ts));
        }
    }

    for app in &mut report.apps {
        let mut cur_by_drive: BTreeMap<char, u64> = BTreeMap::new();
        for item in &app.items {
            let dl = item.drive_letter.to_ascii_uppercase();
            *cur_by_drive.entry(dl).or_insert(0) += item.bytes;
        }
        app.per_drive_growth.clear();
        for (&dl, &cur_b) in &cur_by_drive {
            let drv_growth = match baselines.get(&dl).and_then(|o| *o) {
                None => GrowthDelta::NoBaseline,
                Some(base_map) => {
                    compute_growth(true, base_map.get(&app.display_name).copied(), cur_b)
                }
            };
            app.per_drive_growth.insert(dl, drv_growth);
        }

        let has_any_drive_baseline = cur_by_drive
            .keys()
            .any(|d| baselines.get(d).is_some_and(Option::is_some));
        if !has_any_drive_baseline {
            app.previous_bytes = None;
            app.growth = GrowthDelta::NoBaseline;
            continue;
        }

        let mut cur_known = 0u64;
        let mut prev_known = 0u64;
        let mut any_prev_entry = false;
        for (&dl, base_opt) in &baselines {
            let Some(base_map) = base_opt else {
                continue;
            };
            cur_known = cur_known.saturating_add(cur_by_drive.get(&dl).copied().unwrap_or(0));
            if let Some(&prev_b) = base_map.get(&app.display_name) {
                any_prev_entry = true;
                prev_known = prev_known.saturating_add(prev_b);
            }
        }
        if !any_prev_entry {
            app.previous_bytes = None;
            app.growth = GrowthDelta::New(cur_known);
        } else {
            app.previous_bytes = Some(prev_known);
            app.growth = compute_growth(true, Some(prev_known), cur_known);
        }
    }
}

#[must_use]
pub fn compute_growth(
    has_baseline: bool,
    previous_bytes: Option<u64>,
    current_bytes: u64,
) -> GrowthDelta {
    if !has_baseline {
        return GrowthDelta::NoBaseline;
    }
    match previous_bytes {
        None => GrowthDelta::New(current_bytes),
        Some(prev) => {
            let diff = current_bytes as i128 - prev as i128;
            GrowthDelta::Changed(diff.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
        }
    }
}
