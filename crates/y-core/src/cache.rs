use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::ScanReport;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FolderSizeCache {
    pub updated_unix: u64,
    pub folder_bytes: BTreeMap<String, u64>,
}

impl FolderSizeCache {
    #[must_use]
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("y-storage").join("folder_cache.json")
    }

    pub fn update_from_report(&mut self, report: &ScanReport) {
        self.updated_unix = report.scan_timestamp_unix;
        for app in &report.apps {
            for item in &app.items {
                self.folder_bytes.insert(item.path.clone(), item.bytes);
            }
        }
        for item in &report.shared_caches {
            self.folder_bytes.insert(item.path.clone(), item.bytes);
        }
        for item in &report.your_files {
            self.folder_bytes.insert(item.path.clone(), item.bytes);
        }
        for item in &report.unattributed {
            self.folder_bytes.insert(item.path.clone(), item.bytes);
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
        let body = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, body).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

#[must_use]
pub fn default_last_scan_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("y-storage").join("last_scan.json")
}

pub fn load_last_scan(path: &Path) -> Option<ScanReport> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<ScanReport>(&raw).ok()
}

pub fn save_last_scan_atomic(path: &Path, report: &ScanReport) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string(report).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, body).map_err(|e| e.to_string())?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedExeEvidence {
    pub size: u64,
    pub mtime_unix_secs: u64,
    pub mtime_subsec_nanos: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_org: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_description: Option<String>,
}

/// Persistent cache of per-executable evidence (Authenticode signer + PE version strings),
/// keyed by normalized lowercase path and validated by `(size, mtime)`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExeEvidenceCache {
    #[serde(default)]
    pub entries: BTreeMap<String, CachedExeEvidence>,
}

impl ExeEvidenceCache {
    #[must_use]
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("y-storage").join("evidence_cache.json")
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
        let body = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, body).map_err(|e| e.to_string())?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    #[must_use]
    pub fn get_fresh(
        &self,
        path_key: &str,
        size: u64,
        mtime_unix_secs: u64,
        mtime_subsec_nanos: u32,
    ) -> Option<&CachedExeEvidence> {
        self.entries.get(path_key).filter(|e| {
            e.size == size
                && e.mtime_unix_secs == mtime_unix_secs
                && e.mtime_subsec_nanos == mtime_subsec_nanos
        })
    }

    pub fn upsert(&mut self, path_key: String, entry: CachedExeEvidence) {
        self.entries.insert(path_key, entry);
    }
}
