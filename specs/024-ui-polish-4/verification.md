# Verification Report — `ui-polish-4` (REVISED · `WT_UI_LIBRARY` + FIX 2 + FIX 3)

## Summary Result — **PASS (WT_UI_LIBRARY Adoption · Scope Table · FIX 2 · FIX 3)**

| Item | Description | Status | Verification Evidence |
|---|---|---|---|
| **`WT_UI_LIBRARY` + Scope Table** | Govern Part I via global `WT_UI_LIBRARY.md`; provide dual **Blue** + **Pink** variants on library controls (`BrandVariant::{Blue, Pink}` on `PrimaryOutline` & `resolve_checkbox_colors`); embed `y-storage` Part II Scope Decision Table in `DESIGN.md §11.1` (Header & Per-Pod `Rescan` -> **Blue**; `"new"` & status components -> **§4B Status**; drive/proportion meters -> **Gradient sweep**; Checkbox checked state, active filter chip, active sort arrow, qualifier chips -> **OPEN** unchanged) | **PASS** | `before-main-{dark,light}.png` vs `after-main-{dark,light}.png`, `after-settings-dark.png`, `after-drawer-dark.png`, `ui::tests::button_hierarchy_reserves_accent_for_primary_active_and_hover`, `ui::tests::growth_noise_floor_blanks_under_one_mb_and_uses_new_pill` |
| **FIX 2** | Anti-aliased rounded corners across `100%` / `125%` / `150%` DPI with zero radius, size, or colour changes: `bezier_tolerance = 0.015`, `feathering_size_in_pixels = 1.0`, unified `32`-segment/quadrant `build_feathered_rounded_box_mesh` with `±0.42px` `45°` diagonal coverage plateau, `1px` outer rim feathering keyed on `(w, h, r, ppp)`, connected `Shape::line` checkmarks | **PASS** | `before-macro-{checkbox,pod-corner,button-corner}-dark.png` vs `after-macro-{checkbox,pod-corner,button-corner}-dark.png` (`4x` zoom at `--dpi 120`), `ui::tests::rounded_corners_emit_antialiased_intermediate_coverage_across_dpi_scales` |
| **FIX 3** | Unified pod control row (`Include` + `Exclusions (N)`) on `11.0pt` (`10.5pt` compact) and shared `"Hx"` optical center baseline (`0 px` drift, `<= 1 px`); removed `"x"`-only `optical_galley_pos` split, added `baseline_aligned_galley_pos`, swept ledger footer/reconciliation (`● diff 0 B` leading dot), drawer strips, filter bar, and Settings rows | **PASS** | `before-macro-pod-row-dark.png` vs `after-macro-pod-row-dark.png` (`Include` & `Exclusions (1)` both start at `minY=146` at `125%` DPI), `after-settings-dark.png`, `after-drawer-dark.png`, `ui::tests::pod_control_row_include_and_exclusions_share_font_size_and_baseline` |

---

## 1. Component-Instance Audit (`WT_UI_LIBRARY.md §7.4`)

| Screen / Place | Component Instance | Variant Used | Form Used | Authorising Scope Table Row (`DESIGN.md §11.1`) |
|---|---|---|---|---|
| Masthead (`Main` / `Settings` / `Dependencies`) | `Rescan` / `Rescan needed` button | **Blue** (`accent_line` + `pill_blue_fill`) | Outlined + soft fill (`PrimaryOutline(Blue)`) | Row 1 (Header `Rescan` -> Blue) |
| Drive pods (`Main`) | Per-pod `Rescan` button | **Blue** (`accent_line` + `pill_blue_fill`) | Outlined + soft fill (`PrimaryOutline(Blue)`) | Row 2 (Per-pod `Rescan` -> Blue) |
| Masthead / Pods / Settings / Drawers | `Settings`, `Back to Apps`, `ThemeToggle`, `Add exclusion`, `Add root zone`, `Clear`, `Locate`, `Web search`, `Exclusions` | Neutral / Quiet (`SecondaryOutline` / `SubtleOutline`) | Outlined neutral / Borderless quiet | Non-primary neutral & quiet tiers (`DESIGN.md §6.2`) |
| App table (`Growth delta`) | `"new"` growth indicator | **§4B Status (Green)** (`#28D66C` / `#0A7A3C`) | Soft status pill (`status_green` + `pill_green_fill`) | Row 4 (`"new"` -> §4B Green status pill) |
| Masthead, table, drawers, ledger, Settings | `NO-TOUCH`, `LOCKED`, `missing`, `● GREEN` / `● RED` zones, stale warning masthead (`status_amber`), `● diff 0 B` dot & signed deltas | **§4B Status (Green / Amber / Red)** | Status pill / dot / status text | Row 5 (Status components -> §4B Status only) |
| Drive pods & App table (`Size`) | Drive capacity bars & row proportion bars | **Gradient sweep** (logo 6 stops) | Meter / progress bar | Row 6 (Meters -> Gradient sweep as shipped) |
| Drive pods & Settings | `Include` / `Include in scope` checkboxes | **OPEN** *(current rendering kept)* | `OPEN` | Row 7 (OPEN — untouched) |
| Filter / Sort / Zone toggles | `All`, `Grew`, `App container`, etc. | **OPEN** *(current rendering kept)* | `OPEN` | Row 8 (OPEN — untouched) |
| Sticky table column header | Active sort arrow (`↓` / `↑`) | **OPEN** *(current rendering kept)* | `OPEN` | Row 9 (OPEN — untouched) |
| Expanded drawer & Ledger rows | `ProgramData · shared`, `loose files`, `HARDLINK`, `ESTIMATE` | **OPEN** *(current rendering kept)* | `OPEN` | Row 10 (OPEN — untouched) |

---

## 2. FIX 2 Root Cause & Pipeline Changes (`apps/y-storage/src/ui.rs`)
- **Root Causes**:
  1. `y-storage` software-rasterizes `egui 0.29` meshes via a `1x` point-sampled Win32 GDI CPU rasterizer (`apps/y-storage/src/sw_painter.rs`, no hardware MSAA).
  2. `paint_crisp_bordered_box` called `paint_crisp_ring_mesh`, emitting a raw 2-ring `egui::Shape::Mesh` (`16` corner segments, `alpha = 255` on both edges); `egui::Context::tessellate` does not feather `Shape::Mesh`, producing binary stair-steps and `1px` corner dropouts against the separately shrunk `rect_filled`.
  3. `configure_fonts_and_theme` left `TessellationOptions::bezier_tolerance` at `egui` default `0.1` (beveling `8pt` pod corners), and `draw_crisp_frame` / table containers stacked two separate `Shape::rect_filled` shapes.
- **Changes Made**:
  1. `configure_fonts_and_theme`: set `opts.feathering = true`, `opts.feathering_size_in_pixels = 1.0`, `opts.bezier_tolerance = 0.015`.
  2. Increased `corner_unit_arcs` to `32` segments per quadrant (`132` fixed vertices via `sample_rounded_rect_ring`) and replaced `paint_crisp_ring_mesh` / two-layer `draw_crisp_frame` / table `outer_border_slot` rects with `build_feathered_rounded_box_mesh` (`-0.5px .. +0.5px` edge feathering + `±0.42px` `45°` diagonal stroke plateau keeping straight H/V edges pixel-snapped while smoothing curved corners).
  3. Feathered `build_cached_rim_geometry`'s outer band across `1` physical pixel keyed on `(w, h, r, ppp)`, and drew checkmarks with a single connected `Shape::line`.

## 3. FIX 3 Row Sweep (`apps/y-storage/src/ui.rs`)
- **Pod control strip**: Unified `"Include"` (`draw_compact_pod_checkbox`) and `"Exclusions (N)"` (`draw_z_button`) on `ctrl_font_sz` (`11.0pt`, `10.5pt` compact) and shared `24.0pt` `"Hx"` optical center baseline (`minY=146` on both `"I"` and `"E"` in `after-main-dark.png` at `125%` DPI).
- **App-wide sweep**: Unified `optical_galley_pos` on the `"Hx"` probe across all strings, added `baseline_aligned_galley_pos` for intentional mixed-size pairs, and applied consistent center/optical alignment to `draw_crisp_checkbox`, `draw_word_capped_text_cached`, `draw_truncated_text`, `draw_right_aligned_size`, collapsed ledger footer (`11.0pt`, `● diff 0 B` leading dot), reconciliation lines (`11.0pt`, leading dot), expanded ledger headers, drawer `Category` (`10.5pt`) & `Used by` (`11.5pt`) strips, main `Filter:` bar (`12.0pt`), and Settings drive/exclusion/zone/pin rows (`Layout::left_to_right(Align::Center)`).

## 4. Build, Test, Deployment & Scope Check
- **Engine untouched**: Zero changes to `crates/y-core` or `crates/y-platform-windows`; live disk scans skipped per UI-only Process rule.
- **`cargo test --workspace`**: **77 passed, 0 failed, 0 warnings** (`2 + 6 + 6 + 8 + 5 + 11` in `y-core`, `3` in `y-platform-windows`, `36` in `y-storage`).
- **`cargo build --release --bin y-storage`**: Compiled cleanly with **0 warnings**.
- **Deployment & Smoke Check**:
  - Release & deployed [`Y:\Apps\y-storage\y-storage.exe`](file:///Y:/Apps/y-storage/y-storage.exe) SHA-256: `6786F55CC30AE3B56C4454DD487E814480A654DA863C953F04E9FE780E4CFD88`.
  - `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500` exited `0` (`working_set_bytes: 34,009,088`, `private_bytes: 15,962,112`, `apps_count: 283`, `has_cached_report: true`).

## 5. Before / After Artifact Index (`specs/024-ui-polish-4/`, `--dpi 120` = `125%` scaling)
- Main views (`1275x900`): `before-main-dark.png`, `before-main-light.png` -> `after-main-dark.png`, `after-main-light.png`
- `4x` Macro crops: `before-macro-checkbox-dark.png` -> `after-macro-checkbox-dark.png`, `before-macro-pod-corner-dark.png` -> `after-macro-pod-corner-dark.png`, `before-macro-button-corner-dark.png` -> `after-macro-button-corner-dark.png`, `before-macro-pod-row-dark.png` -> `after-macro-pod-row-dark.png`
- Row-sweep proof (`1275x900`): `before-settings-dark.png` -> `after-settings-dark.png`, `after-drawer-dark.png`
