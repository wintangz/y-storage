# Implementation Plan: `sort-filter-1`

## File Changes
1. `crates/y-core/src/config.rs`:
   - Add `AppSortMode`, `SortDirection`, `AppFilterChip` with `default_direction`, `click_transition`, `flipped`.
   - Persist `sort_mode`, `sort_direction`, `filter_chip`, `search_query` (with `#[serde(default)]`) in `AppConfig`.
2. `apps/y-storage/src/list_query.rs`:
   - Pure functions: `growth_delta_bytes`, `is_standalone_entry`, `app_matches_chip`, `app_matches_search`, `app_matches_filter`, `compare_apps`, `filter_and_sort_apps`.
   - Unit tests covering default byte-for-byte identity, `Size`/`Name`/`Growth` sort directions + no-baseline `"new"` rule, single-select chips + search `AND` composition, and config persistence roundtrip.
3. `apps/y-storage/src/ui.rs`:
   - Store `sort_mode`, `sort_direction`, `filter_chip` on `StorageUiState` + `StorageUiState::from_config`.
   - Emit `UiAction::UpdateListView` on sort, chip, or search edits.
   - Show `"new"` muted label for `GrowthDelta::NoBaseline | GrowthDelta::New(_)` while keeping `GrowthDelta::Changed(0)` blank.
   - Render segmented `Sort:` buttons (`Size`, `Name`, `Growth` with vector arrow + direction flip) right-aligned in the filter bar, and the 5 radio filter chips + `"N of M apps"` live count row underneath.
   - Apply `filter_and_sort_apps` exclusively inside `draw_main_app_list` (keeping bottom pinned groups & reconciliation untouched).
4. `apps/y-storage/src/win32_host.rs`:
   - Restore `StorageUiState::from_config(&init_cfg)` at startup, handle `UiAction::UpdateListView`, and preserve list view fields during `UiAction::SaveConfig`.
5. `apps/y-storage/src/main.rs`:
   - Declare `mod list_query;`; add `--sort`, `--sort-dir`, `--filter`, `--search` CLI screenshot flags for reproducible screenshot capture.
