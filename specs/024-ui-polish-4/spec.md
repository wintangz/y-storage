# Spec `024-ui-polish-4` (`ui-polish-4`, REVISED)

UI-only polish round from `main` adopting the canonical **`WT_UI_LIBRARY.md`** (`wt-ui-library` skill v1.1), recording `y-storage`'s Part II Scope Decision Table in `DESIGN.md §11.1`, and fixing FIX 2 (pixelated rounded corners) and FIX 3 (pod control row baseline misalignment + app-wide row sweep) without modifying any scanner, attribution, or reconciliation engine code.

## Scope & Fixes
1. **WT UI Library Adoption & Scope Decision Table (`WT_UI_LIBRARY.md §1–§7`, `DESIGN.md §11.1`)**:
   - Govern Part I by `WT_UI_LIBRARY.md` (dual **Blue** and **Pink** library variants for every component touched; zero gradient on buttons ever; logo 6-stop sweep strictly on container rims & meter/progress bars; `§4B` Status family strictly in green/amber/red).
   - Add `y-storage`'s Part II Scope Decision Table (`DESIGN.md §11.1`):
     - Labelled primary buttons (header `Rescan`, per-pod `Rescan`, future labelled CTAs) → **Blue** (`PrimaryOutline(Blue)`).
     - `"new"` growth state → **§4B Status (Green)** (`status_green` + `pill_green_fill`).
     - Status components (`NO-TOUCH`, `LOCKED`, `missing`, `● GREEN`/`● RED` zones, stale masthead `status_amber`, reconciliation `diff` dot/indicator, signed deltas, errors) → **§4B Status** only.
     - Drive meters / proportion bars → **Gradient sweep** (logo stops, as currently rendered).
     - **OPEN** rows (kept unchanged, never guessed): checkbox checked state, active filter chip, active sort arrow, qualifier chips.
2. **FIX 2 — Anti-Aliased Rounded Corners Across DPI Scales (`96` / `120` / `144` DPI)**:
   - Eliminate stair-stepped/dropout rounded corners on checkboxes (`4pt`), buttons (`6pt`), pills (`9pt`), and pod/card frames (`8pt`) with zero changes to radii, control sizes, or palette colours:
     - Configure `TessellationOptions` (`feathering = true`, `feathering_size_in_pixels = 1.0`, `bezier_tolerance = 0.015`) in `configure_fonts_and_theme`.
     - Replace `paint_crisp_ring_mesh` and stacked two-layer `draw_crisp_frame` / table container rects with a single concentric physical-pixel-feathered rounded-box mesh builder (`build_feathered_rounded_box_mesh`, `32` segments/quadrant, `-0.5px .. +0.5px` edge feathering + `±0.42px` `45°` diagonal stroke stabilization so straight H/V edges stay needle-sharp while corner arcs blend smoothly with even weight across DPI scales).
     - Feather `build_cached_rim_geometry` outer perimeter across `1` physical pixel keyed on `(w, h, r, ppp)`, and draw checkmarks with a single connected `Shape::line`.
3. **FIX 3 — Pod Control Row Baseline Unification & Full UI Row Sweep**:
   - Unify `"Include"` (`draw_compact_pod_checkbox`) and `"Exclusions"` / `"Exclusions (N)"` (`draw_z_button`) on one exact font size (`11.0pt`, `10.5pt` compact) and shared `"Hx"` optical center baseline (`0 px` drift, strictly `<= 1 px`), removing top-aligned `ui.horizontal` / `egui::Label` wrapper drift.
   - Unify `optical_galley_pos` on the `"Hx"` probe across all strings, apply `optical_galley_pos` / `draw_baseline_paired_text` consistently across collapsed/expanded ledger footers, reconciliation lines (`● diff 0 B` leading dot order), drawer `Category` / `Used by` strips, main `Filter:` bar, and Settings drive/exclusion/zone/pin rows (`Align::Center`).

## Assumptions Logged
1. Owner display scaling on this host is `AppliedDPI = 120` (**125% scaling**, `pixels_per_point = 1.25`), so all BEFORE/AFTER screenshots and `4x` nearest-neighbor macro crops use `--dpi 120` (`1275x900` physical px).
2. `crates/y-core` and `crates/y-platform-windows` remain completely untouched and live disk scans are skipped per the Process rule; screenshots render deterministically from the cached 4-drive report JSON.
