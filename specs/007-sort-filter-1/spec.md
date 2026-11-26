# Specification: `sort-filter-1`

**Canonical Slug**: `sort-filter-1`
**Type**: View-Only List Controls (Zero Changes to Scan, Attribution, Dependency, or History Computation)

## Requirements
1. **Sort Segmented Control** (app-list header area, styled with shared WT / `ui-polish-2` button & chip primitives):
   - **Size**: descending by `total_bytes` (**default**, identical order to prior builds).
   - **Name**: ascending `A–Z`, case-insensitive.
   - **Growth**: descending by growth delta vs previous scan (biggest grower first). Apps with no baseline (`GrowthDelta::NoBaseline` or `GrowthDelta::New(_)`) have no delta: they ALWAYS sort AFTER every app that has a delta (ordered by size descending among themselves), and their delta cell renders a muted `"new"` marker instead of a number. Apps with `GrowthDelta::Changed(0)` still keep an empty delta cell (`ui-polish-2` Fix 4).
   - Clicking the already-active sort mode flips its direction (`Size`/`Growth`: `desc <-> asc`; `Name`: `A-Z <-> Z-A`). Active mode and direction are visible at a glance via highlighted WT styling + vector direction arrow (`↓` / `↑`).
2. **Single-Select Filter Chip Row** (radio behaviour under the search box, exactly one active, default `All`):
   - `All`
   - `Grew` (`GrowthDelta::Changed(d)` with `d > 0`)
   - `NO-TOUCH` (`no_touch.is_some()`)
   - `Standalone` (`AttributionBasis::StandaloneFolder` or `standalone:` / `dot:` IDs, not registry apps)
   - `Has C: footprint` (uses actual system drive letter; app has at least one breakdown item on the system drive)
   - Combines with text search (`AND`).
   - Displays live `"N of M apps"` count right beside the chips so filters never silently hide the list.
3. **Scope & Persistence Rules**:
   - Sort and filter affect **only** the app list; pinned bottom groups (`Windows & system`, `Your files`, `Shared & system caches`, `Unattributed`), attributed byte total, and the reconciliation bar never change or hide.
   - `Size desc` + `All` + empty search preserves exact byte-for-byte order of `ScanReport.apps`.
   - `sort_mode`, `sort_direction`, `filter_chip`, and `search_query` persist in `AppConfig` (`%APPDATA%\y-storage\config.toml`) alongside `theme_mode` and `window` and restore on launch.

## Logged Assumptions
- `Standalone` filter matches user-profile dot-folders and standalone SDK/toolchain entries (`AttributionBasis::StandaloneFolder` or `standalone:` / `dot:` ID prefixes), excluding registry apps.
- `Grew` filter includes apps with strictly positive delta vs the previous scan (`GrowthDelta::Changed(d)` where `d > 0`); newly appeared apps (`GrowthDelta::New`) have no baseline delta and sort with the `"new"` no-baseline group in Growth sort.

## Acceptance Criteria (Light Verification)
- Pure unit tests for sort comparators (including Growth no-baseline rule) and filter predicates (`AND` combination + `AppConfig` roundtrip) pass alongside all existing workspace tests (`cargo test --workspace`).
- Live scan confirms `EA SPORTS™ FIFA 23 == 51,372,195,955 B` and `C:\` reconciliation `diff == 0 B`.
- Two screenshots at `1020x720` (`96` DPI) in `specs/007-sort-filter-1/`:
  1. `screenshot-growth-grew.png` (Growth sort + Grew filter active, count line visible)
  2. `screenshot-name-search-chip.png` (Name sort + search text combined with a chip filter)
- Release exe deployed to `Y:\Apps\y-storage\y-storage.exe` and committed to `main`.
