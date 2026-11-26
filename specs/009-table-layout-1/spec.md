# Feature Specification: `table-layout-1`

**Feature Branch / Commit Scope**: `main` (building on `design-pass-1` `fcaeb01`)  
**Status**: Approved Autonomous Execution  
**Constraint**: Structural layout change ONLY. Zero changes to scan, attribution, dependency, history, sort/filter computation (`crates/y-core`, `crates/y-platform-windows`, `apps/y-storage/src/list_query.rs`), or computed byte totals.

## Scope & Deliverables

1. **Main List as Fixed-Column Table (`ui.rs`)**:
   - Render the main installed-app list inside an `8px`-radius hairline ledger panel with a sticky `26px` `panel_alt` column-header row fixed above the scrolling body.
   - Five fixed columns left to right across every row:
     1. Expand chevron (`16px`)
     2. `App name` (flexible primary share, `T3` `13.0pt` Regular, `NO-TOUCH` / `LOCKED` chip inline when flagged, ellipsis truncation + full tooltip)
     3. `Publisher` (fixed `208px`, `T4` `11.0pt` subtle meta tier, ellipsis truncation + full tooltip)
     4. `Growth delta` (`96px`, right-aligned semantic `+#`/`-#`/`0 B`/`new`)
     5. `Total size` (`96px`, right-aligned `T2` `13.5pt` Strong, byte tooltip on hover)
   - Collapsed rows remain borderless hairline-divided ledger rows (`30px` height) with subtle `widget_hover` row highlight.
   - Right edge of `Total size` matches the pinned reconciliation ledger size column below (`table_outer_w == aligned_w`).

2. **Sticky Column Headers as Sort Controls (`ui.rs`)**:
   - Clicking `App name`, `Growth delta`, or `Total size` column headers invokes `AppSortMode::{Name, Growth, Size}.click_transition(...)` and emits `UiAction::UpdateListView`.
   - Active column renders directional arrow via `draw_sort_arrow` next to its header label; inactive sortable headers show hover cue only.
   - Remove the standalone `Sort: [Size] [Name] [Growth]` segmented control bar from Header Row 3 while preserving the search input, `Clear` button, filter chip row (`All`, `Grew`, `NO-TOUCH`, `Standalone`, `Has C: footprint`), and live count/subtotal summary.

3. **Expanded Row Sub-Tables (`ui.rs`)**:
   - Expanding an app drawer (`2px` `accent_line` bar on `panel_alt` surface) renders two fixed-column hairline-divided sub-tables aligned to the same right column edge:
     - **`Locations (N)`**: `Zone` (`64px` pill) · `Path` (flex, monospace `11.5pt`, ellipsis + hover tooltip) · `Basis` (`170px`, `10.5pt` muted, ellipsis + hover tooltip) · `Size` (`96px`, right-aligned `12.0pt` monospace + byte tooltip).
     - **`Dependencies` (when non-empty)**: `Dependency name` (flex, `12.0pt`) · `Kind` (`140px`, quiet `11.0pt` muted text) · `Confidence` (`130px`, `proven`/`inferred` + `shared`/`missing` pills) · `Used by` (`84px`, `draw_z_button` reverse-dependency trigger) · `Own size` (`96px`, right-aligned quiet `11.5pt` muted). Below each dependency row, render `dep.basis` on a quieter full-width (`10.5pt` `subtle_text`) second line so columns stay uncrowded and aligned.

4. **Family Spec Alignment & Numbers Intact (`DESIGN.md`, Verification)**:
   - Update `DESIGN.md` with shared tabular header/column guidance (Section 3) and app-specific `y-storage` schemas (`C3`, `C5`, `C6`, `C7`, `C11`, `C12`).
   - Verify no regression in tests (`cargo test --workspace`), live CLI scan (`EA SPORTS™ FIFA 23 == 51,372,195,955 B`, `C:\ diff == 0 B`), and screenshots across themes/sorts/expanded drawer (`1020x720`, `96` DPI).
