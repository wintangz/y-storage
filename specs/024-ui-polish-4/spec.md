# Spec `024-ui-polish-4` (`ui-polish-4`)

UI-only polish round from `ac4e9e4` addressing 3 owner-reported defects without modifying any scanner, attribution, or reconciliation engine code.

## Scope & Fixes (`FIX 1`–`FIX 3`)
1. **FIX 1 (Amended) — Scope-Split Primary Buttons (`PrimaryOutline` Blue Global vs `PodPrimaryOutline` Pink Local)**:
   - Record in `DESIGN.md §6` that buttons never carry gradient borders (gradients live strictly on container rims and meters) and `PrimaryOutline` splits by scope: global primary CTA (`Rescan` / `Rescan needed` in the masthead) uses `accent_1` blue (`#69CBFC` + `#253141` Dark; `#0090D9` + `#E2F4FC` Light), while repeated local primaries (`PodPrimaryOutline` `Rescan` on each drive pod) use `accent_2` pink (`#FA80CB` + `#382C3C` Dark; `#FA29AA` + `#FCECF6` Light; `widget_hover` + `accent_2` on hover).
   - Keep pod `Rescan` size, position, padding, click/hover tooltips, and original uncompressed 6-stop meter bar sampling unchanged.
2. **FIX 2 — Anti-Aliased Rounded Corners Across DPI Scales (`96` / `120` / `144` DPI)**:
   - Eliminate stair-stepped/dropout rounded corners on checkboxes (`4pt`), buttons (`6pt`), pills (`9pt`), and pod/card frames (`8pt`) with zero changes to radii, control sizes, or palette colours:
     - Configure `TessellationOptions` (`feathering = true`, `feathering_size_in_pixels = 1.0`, `bezier_tolerance = 0.015`) in `configure_fonts_and_theme`.
     - Replace `paint_crisp_ring_mesh` (which drew unfeathered 2-ring `Mesh` quads separately from `rect_filled`) and two-layer `draw_crisp_frame` rects with a single concentric physical-pixel-feathered rounded-box mesh builder (`build_feathered_rounded_box_mesh`, `32` segments per quadrant, `1px` edge feathering across `-0.5px .. +0.5px` around integer pixel boundaries so straight H/V edges remain needle-sharp while corner arcs blend smoothly into fill/backdrop).
     - Feather `build_cached_rim_geometry` outer perimeter across `1` physical pixel keyed on `(w, h, r, ppp)`, and render checkmarks as single connected polylines (`Shape::line`).
3. **FIX 3 — Pod Control Row Baseline Unification & Full UI Row Sweep**:
   - Unify `"Include"` (`draw_compact_pod_checkbox`) and `"Exclusions"` / `"Exclusions (N)"` (`draw_z_button`) on one exact font size (`11.0pt`, `10.5pt` compact) and one shared `"Hx"` optical center baseline (`0 px` deviation, strictly `<= 1 px`), removing top-aligned `ui.horizontal` / `egui::Label` wrapper drift.
   - Always probe `"Hx"` in `optical_galley_pos` (removing the `"x"`-only branch that shifted lowercase strings vertically off capitalized siblings) and apply `optical_galley_pos` + `draw_baseline_paired_text` consistently across collapsed ledger footer (`11.0pt` unified), reconciliation line (`11.0pt` unified), expanded ledger titles/subtitles, drawer Category & Shared Runtime strips, main Filter bar, and Settings drive/exclusion/zone/pin rows (`Layout::left_to_right(Align::Center)`).

## Assumptions Logged
1. Owner display scaling on this host is `AppliedDPI = 120` (**125% scaling**, `pixels_per_point = 1.25`), so all BEFORE/AFTER screenshots and `4x` nearest-neighbor macro crops are rendered at `--dpi 120` (`1275x900` physical px).
2. Per the owner's Process Note, `crates/y-core` and `crates/y-platform-windows` remain completely untouched and live disk scans are skipped; all visual proofs render deterministically from the cached 4-drive report JSON.
