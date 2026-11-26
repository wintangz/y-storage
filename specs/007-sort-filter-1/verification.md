# Verification Report: `sort-filter-1`

**Status**: PASS (Light Verification — Pure Unit Tests + Release Build + Live Scan Regression + Screenshot Visual Inspection)

## 1. Automated Tests & Build
- `cargo test --workspace`: **23 passed, 0 failed, 0 warnings**
  - `y-core` `core_tests`: 5 passed
  - `y-core` `round2_tests`: 4 passed
  - `y-core` `round3_tests`: 4 passed
  - `y-platform-windows` `windows_scanner_tests`: 2 passed
  - `y-storage` unit & query tests: 8 passed (`5` new pure tests in `list_query.rs` covering `Size` desc/asc byte-for-byte default preservation, case-insensitive `Name` `A–Z`/`Z–A` & click flips, `Growth` desc/asc placing `NoBaseline` and `New` after all delta entries ordered by size, single-select filter chips combined with search `AND`, and `AppConfig` TOML roundtrip + legacy config compatibility; plus the `3` existing UI/DPI/painter tests).
- `cargo build --release --workspace`: **Finished cleanly (0 warnings)**

## 2. Live Scan Regression Check
- `apps`: **262** installed/standalone entries
- `EA SPORTS™ FIFA 23`: **51,372,195,955 B** (exact match; no scan/attribution/dependency logic modified)
- `C:\` reconciliation:
  - Apps on `C:\`: `84,035,441,422 B`
  - Shared & system caches (`GREEN`): `11,826,078,743 B`
  - Your files: `3,210,724,335 B`
  - Unattributed: `1,479,738,420 B`
  - Windows & system (estimate + root files): `46,091,845,816 B`
  - Reconciliation discrepancy: **`diff 0 B`**
- Real on-disk movement since `ui-polish-2` (captured accurately by `grew_count = 7`): `.gemini +39,820,971 B`, `Store: Microsoft.StartExperiencesApp +362,113 B`, `Microsoft Visual Studio Code (User) +169,784 B`, `Store: Microsoft.MinecraftUWP +16,384 B`, `Microsoft Edge +1,916 B`, `AMD Software +143 B`, `NVIDIA App 11.0.9.585 +1 B`.

## 3. Feature Checklist & Visual Evidence (`1020x720`, `96` DPI)
- **Sort segmented control (`Size`, `Name`, `Growth`, click active to flip direction, default `Size ↓`)**: **PASS**.
- **Growth no-baseline / `New` rule (`"new"` muted cell, sorts after all baseline deltas ordered by size)**: **PASS** (verified via pure comparator + `growth_sort_places_no_baseline_and_new_after_all_deltas_ordered_by_size` test and `format_delta_display`).
- **Single-select filter chip row (`All`, `Grew`, `NO-TOUCH`, `Standalone`, `Has C: footprint`) + `AND` search + live `"N of 262 apps"` count**: **PASS**.
- **Pinned bottom groups & reconciliation invariance**: **PASS** (untouched across all sort/filter states).
- **`AppConfig` view-state persistence (`sort_mode`, `sort_direction`, `filter_chip`, `search_query`)**: **PASS** (persisted in `AppConfig`, saved on `UpdateListView`, preserved across `SaveConfig`, and restored in `StorageUiState::from_config`).

Screenshots:
- [screenshot-growth-grew.png](file:///Y:/Repos/y-storage/specs/007-sort-filter-1/screenshot-growth-grew.png) — `Growth ↓` sort + `Grew` chip active + `7 of 262 apps` + `150.24 GB attributed` + pinned bottom groups & `diff 0 B`.
- [screenshot-name-search-chip.png](file:///Y:/Repos/y-storage/specs/007-sort-filter-1/screenshot-name-search-chip.png) — `Name A-Z ↑` sort + `NO-TOUCH` chip + `"microsoft"` search (`AND`) + `Clear` + `75 of 262 apps` + slim palette scrollbar.
