# Plan `024-ui-polish-4`

1. **Before Baseline & Macro Crops (`ac4e9e4`)**: Capture `--dpi 120` (`125%`) Dark/Light main and Settings screenshots and extract `4x` nearest-neighbor macro crops (`checkbox`, `pod-corner`, `button-corner`, `pod-row`).
2. **Amended FIX 1, FIX 2 AA, FIX 3 Baselines (`apps/y-storage/src/ui.rs`)**:
   - Update `resolve_button_colors` (`PodPrimaryOutline` -> `pal.pill_pink_fill` / `pal.accent_2` / `pal.accent_2`; `PrimaryOutline` stays `accent_1` blue; zero button gradients; meter bars unchanged).
   - Add `build_feathered_rounded_box_mesh` (`32` segments/quadrant, `1px` edge feathering) + `TessellationOptions` (`bezier_tolerance = 0.015`, `feathering_size_in_pixels = 1.0`) for `paint_crisp_bordered_box`, `draw_crisp_frame`, table outer panel, and `build_cached_rim_geometry`.
   - Unify pod control row `"Include"` & `"Exclusions"` on `11.0pt` (`10.5pt` compact) and shared `"Hx"` optical center baseline; sweep ledger footer, reconciliation lines, drawer strips, filter bar, and Settings rows.
   - Update/add unit tests for scope-split buttons, multi-DPI corner AA, and `<= 1px` pod row baseline alignment.
3. **Design System & Spec Docs**: Update `DESIGN.md` (`§2.2`, `§6`, `§6.1`, `§6.2`, `§9 FC6`, `§11`) and author `specs/024-ui-polish-4/{spec.md,plan.md,tasks.md,verification.md}`.
4. **Verify, Render AFTER Proof, Deploy & Push**: Run `cargo test --workspace` + `cargo build --release --bin y-storage` (0 warnings), render AFTER PNGs & `4x` crops, visually inspect all before/after PNGs & macro crops via `read_file`, deploy to `Y:\Apps\y-storage\y-storage.exe`, run `--smoke-gui-ms 500`, commit, and push `origin/main`.
