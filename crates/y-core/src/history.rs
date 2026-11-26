use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{GrowthDelta, ScanReport};

pub type HistoryEntryRef = BTreeMap<String, u64>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanHistorySnapshot {
    pub timestamp_unix: u64,
    pub timestamp_display: String,
    pub per_app_bytes: BTreeMap<String, u64>,
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

    pub fn record_report(&mut self, report: &ScanReport) {
        let mut per_app_bytes = BTreeMap::new();
        for app in &report.apps {
            per_app_bytes.insert(app.display_name.clone(), app.total_bytes);
        }
        self.snapshots.push(ScanHistorySnapshot {
            timestamp_unix: report.scan_timestamp_unix,
            timestamp_display: report.scan_timestamp_display.clone(),
            per_app_bytes,
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
