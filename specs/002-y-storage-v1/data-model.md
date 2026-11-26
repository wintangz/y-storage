# Data Model: `y-storage` V1

## Core Entities

- **`DriveInfo`**: `letter`, `root`, `total_bytes`, `free_bytes`, `used_bytes`, `is_system_drive`, `full_walk_enabled`, `exclusions`.
- **`InstalledApp`**: `id`, `display_name`, `aliases`, `publishers`, `direct_install_location`, `inferred_install_location`, `all_install_locations`, `no_touch: Option<NoTouchCategory>`, `is_store_fallback`.
- **`BreakdownItem`**: `path`, `bytes`, `zone: Option<ZoneKind>`, `qualifier: Option<LocationQualifier>`, `basis: Option<AttributionBasis>`, `no_touch: Option<NoTouchCategory>`, `drive_letter`.
- **`AttributedApp`**: `id`, `display_name`, `publishers`, `install_location`, `total_bytes`, `previous_bytes`, `growth: GrowthDelta`, `no_touch`, `bases`, `items`.
- **`SystemDriveBreakdown`**: `drive_letter`, `root_files`, `remainder_bytes`, `total_bytes`, `scanned_non_system_bytes`, `drive_used_bytes`, `drive_free_bytes`, `drive_total_bytes`.
- **`ScanReport`**: `scan_timestamp_unix`, `scan_timestamp_display`, `duration_ms`, `drives`, `registry_stats`, `apps`, `your_files`, `windows_system`, `unattributed`.
- **`AppConfig`** (`%APPDATA%\y-storage\config.toml`): `version`, `theme_mode` (`Dark` | `Light`), `targets: Vec<DriveScanTarget>`.
- **`ScanHistory`** (`%LOCALAPPDATA%\y-storage\scan_history.json`): `snapshots: Vec<ScanHistorySnapshot>`.
- **`FolderSizeCache`** (`%LOCALAPPDATA%\y-storage\folder_cache.json`): `updated_unix`, `folder_bytes: BTreeMap<String, u64>`.
