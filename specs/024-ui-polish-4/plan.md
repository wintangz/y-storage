# Plan `024-ui-polish-4` (REVISED)

1. **Pre-Round Baseline (`ac4e9e4`)**: Keep `before-main-{dark,light}.png`, `before-settings-dark.png`, and `4x` `before-macro-{checkbox,pod-corner,button-corner,pod-row}-dark.png` at `--dpi 120` (`125%`).
2. **WT UI Library Dual Variants, Scope Table Implementation, FIX 2 & FIX 3 (`apps/y-storage/src/ui.rs`)**:
   - Define `BrandVariant::{Blue, Pink}` with full Dark & Light dual-variant support on `ButtonVariant::PrimaryOutline(BrandVariant)` and `resolve_checkbox_colors(BrandVariant, ...)`.
   - Set header `Rescan` / `Rescan needed` and per-pod `Rescan` to `ButtonVariant::PrimaryOutline(BrandVariant::Blue)` per `y-storage`'s Scope Decision Table (`DESIGN.md §11.1`); keep all Status (`§4B`), Meter, and `OPEN` rows unchanged.
   - Stabilize `45°` corner AA density in `build_feathered_rounded_box_mesh` (`diag_pad = ±0.42 * px` transitioning smoothly to `0` on straight H/V tangents) and fix `● diff 0 B` leading dot order in `right_to_left` footer rows.
   - Update unit tests to assert both Blue and Pink library variants across both Dark and Light themes plus DPI corner AA and `<= 1px` pod-row baseline alignment.
3. **Design System & Spec Docs**: Update `DESIGN.md` to declare Part I governed by `WT_UI_LIBRARY.md`, embed `y-storage`'s Part II Scope Decision Table (`§11.1`) including explicit `OPEN` rows, and update `specs/024-ui-polish-4/`.
4. **Verify, Re-Render AFTER Proof, Deploy & Push**: Run `cargo test --workspace` + `cargo build --release --bin y-storage` (0 warnings), render updated `after-*.png` and `4x` `after-macro-*.png`, visually inspect via `read_file`, deploy to `Y:\Apps\y-storage\y-storage.exe`, run `--smoke-gui-ms 500`, commit, and push `origin/main`.
