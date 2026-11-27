# Tasks `024-ui-polish-4` (REVISED)

- [x] Read global `wt-ui-library` (`SKILL.md` + `WT_UI_LIBRARY.md`) and adopt dual-variant primitives (`BrandVariant::{Blue, Pink}` for `PrimaryOutline` & `resolve_checkbox_colors`) in `apps/y-storage/src/ui.rs`
- [x] Apply `y-storage` Scope Decision Table (`PrimaryOutline(Blue)` on header & per-pod `Rescan`, Status/Meters/OPEN rows preserved)
- [x] Keep `build_feathered_rounded_box_mesh` (`32` segs/quadrant, `bezier_tolerance = 0.015`, `feathering_size_in_pixels = 1.0`) with `45°` diagonal coverage plateau (`FIX 2`)
- [x] Keep unified pod control row (`11.0pt`/`10.5pt` compact, `"Hx"` baseline) and swept footer/drawer/Settings rows + leading `● diff 0 B` dot (`FIX 3`)
- [x] Update `DESIGN.md` (`WT_UI_LIBRARY` governance header, dual-variant `§2.2`/`§6`, Part II `§11.1 Scope Decision Table` with `OPEN` rows)
- [x] Run `cargo test --workspace` and `cargo build --release --bin y-storage` (0 warnings)
- [x] Re-render and visually inspect `after-*` screenshots and `4x` macro crops (`125%` scaling)
- [x] Deploy `Y:\Apps\y-storage\y-storage.exe`, run GUI smoke check, update `verification.md`, commit, and push `origin/main`
