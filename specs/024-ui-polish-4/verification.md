# Verification Report — `ui-polish-4` (`specs/024-ui-polish-4`)

## Summary Result — **PASS (3 / 3 Fixes Verified, UI-Only Round)**

| Item | Description | Status | Verification Evidence |
|---|---|---|---|
| **FIX 1 (Amended)** | Scope-split `PrimaryOutline` (`accent_1` blue `#69CBFC` / `#0090D9` global CTA) vs `PodPrimaryOutline` (`accent_2` pink `#FA80CB` on `#382C3C` Dark / `#FA29AA` on `#FCECF6` Light per-pod `Rescan`); no button gradient borders; meters unchanged; recorded in `DESIGN.md §6` | **PASS** | `before-main-{dark,light}.png` vs `after-main-{dark,light}.png`, `ui::tests::button_hierarchy_reserves_accent_for_primary_active_and_hover`, `ui::tests::growth_noise_floor_blanks_under_one_mb_and_uses_new_pill` |
| **FIX 2** | Anti-aliased rounded corners (`100%` / `125%` / `150%` DPI) with zero radius, size, or colour changes: `bezier_tolerance = 0.015`, `feathering_size_in_pixels = 1.0`, unified `32`-segment/quadrant `build_feathered_rounded_box_mesh`, `1px` outer rim feathering in `build_cached_rim_geometry`, connected `Shape::line` checkmarks | **PASS** | `before-macro-{checkbox,pod-corner,button-corner}-dark.png` vs `after-macro-{checkbox,pod-corner,button-corner}-dark.png` (`4x` zoom at `--dpi 120`), `ui::tests::rounded_corners_emit_antialiased_intermediate_coverage_across_dpi_scales` |
| **FIX 3** | Unified pod control row (`Include` + `Exclusions (N)`) on `11.0pt` (`10.5pt` compact) and shared `"Hx"` optical center baseline (`0 px` drift, `<= 1 px`); removed `"x"`-only `optical_galley_pos` split and swept collapsed/expanded ledger footer, reconciliation lines, drawer strips, filter bar, and Settings rows | **PASS** | `before-macro-pod-row-dark.png` vs `after-macro-pod-row-dark.png` (`Include` & `Exclusions (1)` both start at `minY=146` at `125%` DPI), `after-settings-dark.png`, `after-drawer-dark.png`, `ui::tests::pod_control_row_include_and_exclusions_share_font_size_and_baseline` |

---

## 1. FIX 2 Root Cause & Pipeline Changes (`apps/y-storage/src/ui.rs`)
- **Root Causes**:
  1. `y-storage` software-rasterizes `egui 0.29` meshes via a `1x` point-sampled CPU GDI rasterizer (`apps/y-storage/src/sw_painter.rs`, no hardware MSAA).
  2. `paint_crisp_bordered_box` called `paint_crisp_ring_mesh`, emitting a raw 2-ring `egui::Shape::Mesh` (`16` corner segments, `alpha = 255` on both edges); `egui::Context::tessellate` does not feather `Shape::Mesh`, producing binary stair-steps and `1px` corner dropouts against the separate `rect_filled` interior.
  3. `configure_fonts_and_theme` left `TessellationOptions::bezier_tolerance` at `egui` default `0.1`, beveling `8pt` pod corners, while `draw_crisp_frame` and the table outer container stacked two separate `Shape::rect_filled` rectangles.
- **Changes Made**:
  1. `configure_fonts_and_theme`: enabled `opts.feathering = true`, `opts.feathering_size_in_pixels = 1.0`, `opts.bezier_tolerance = 0.015`.
  2. Increased `corner_unit_arcs` to `32` segments per quadrant (`132` fixed vertices via `sample_rounded_rect_ring`) and replaced `paint_crisp_ring_mesh` / two-layer `draw_crisp_frame` / table `outer_border_slot` rects with `build_feathered_rounded_box_mesh` (`-0.5px .. +0.5px` physical-pixel edge feathering on corners while snapping straight H/V edges onto pixel centers).
  3. Feathered `build_cached_rim_geometry`'s outer band across `1` physical pixel keyed on `(w, h, r, ppp)`, and drew checkmarks with a single connected `Shape::line`.

## 2. FIX 3 Row Sweep (`apps/y-storage/src/ui.rs`)
- Pod control strip: unified `"Include"` (`draw_compact_pod_checkbox`) and `"Exclusions (N)"` (`draw_z_button`) on `ctrl_font_sz` (`11.0pt`, `10.5pt` compact) and shared `24.0pt` `"Hx"` optical center baseline (`minY=146` on both `"I"` and `"E"` in `after-main-dark.png` at `125%` DPI).
- Swept rows:
  - `optical_galley_pos`: unified on `"Hx"` probe across all text strings.
  - `draw_crisp_checkbox`, `draw_word_capped_text_cached`, `draw_truncated_text`, `draw_right_aligned_size`, and `draw_baseline_paired_text`: optical/baseline alignment in bounded horizontal rows.
  - Collapsed ledger footer (`11.0pt`), reconciliation lines (`11.0pt`), expanded ledger group headers (baseline-paired `12.0pt`/`11.0pt`), drawer `Category` (`10.5pt`) & `Used by` (`11.5pt`) strips, main `Filter:` bar (`12.0pt`), and Settings drive headers, exclusions (`11.0pt`), zone chips (`11.0pt`), and manual pin rows (`Align::Center`).

## 3. Build, Test, Deployment & Scope Check
- **Engine untouched**: zero changes to `crates/y-core` or `crates/y-platform-windows`; live disk scans skipped per Process Note.
- **`cargo test --workspace`**: **77 passed, 0 failed, 0 warnings** (`2 + 6 + 6 + 8 + 5 + 11` in `y-core`, `3` in `y-platform-windows`, `36` in `y-storage`).
- **`cargo build --release --bin y-storage`**: compiled cleanly with **0 warnings**.
- **Deployment & Smoke Check**:
  - Release & deployed [`Y:\Apps\y-storage\y-storage.exe`](file:///Y:/Apps/y-storage/y-storage.exe) SHA-256: `EF19393A4AF2C2194C89B3B9B32E1D9F3E8203FD77E3CE02A10117668EDED9CD` (previous running instance swapped safely via NTFS rename to `Y:\Apps\y-storage\y-storage.prev.exe` because PID `26912` held an open handle).
  - `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500` exited `0` (`working_set_bytes: 34,050,048`, `private_bytes: 16,015,360`, `apps_count: 283`, `has_cached_report: true`).

## 4. Before / After Artifact Index (`specs/024-ui-polish-4/`, `--dpi 120` = `125%` scaling)
- Main views (`1275x900`): `before-main-dark.png`, `before-main-light.png`, `after-main-dark.png`, `after-main-light.png`
- `4x` Macro crops: `before-macro-checkbox-dark.png` -> `after-macro-checkbox-dark.png`, `before-macro-pod-corner-dark.png` -> `after-macro-pod-corner-dark.png`, `before-macro-button-corner-dark.png` -> `after-macro-button-corner-dark.png`, `before-macro-pod-row-dark.png` -> `after-macro-pod-row-dark.png`
- Row-sweep proof (`1275x900`): `before-settings-dark.png` -> `after-settings-dark.png`, `after-drawer-dark.png`
