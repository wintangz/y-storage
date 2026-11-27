# Tasks `024-ui-polish-4`

- [x] Capture `--dpi 120` (`125%`) before PNGs and `4x` nearest-neighbor macro crops from `ac4e9e4`
- [x] Split `PodPrimaryOutline` to `accent_2` pink (`#FA80CB`/`#382C3C` Dark, `#FA29AA`/`#FCECF6` Light) keeping header `PrimaryOutline` blue and meters unchanged (`FIX 1` amended)
- [x] Implement concentric physical-pixel-feathered rounded-box meshes (`32` segs/quadrant) & `bezier_tolerance = 0.015` across DPI scales (`FIX 2`)
- [x] Unify pod control row typography (`11.0pt`/`10.5pt` compact) & `"Hx"` optical baselines across pod row, footer, drawer, filter bar, and Settings (`FIX 3`)
- [x] Add multi-DPI corner AA and pod-row baseline alignment unit tests
- [x] Update `DESIGN.md` (`§2.2`, `§6`, `§6.1`, `§6.2`, `§9 FC6`, `§11`) and `specs/024-ui-polish-4/` docs
- [x] Run `cargo test --workspace` and `cargo build --release --bin y-storage` with 0 warnings
- [x] Render and visually inspect `after-*` screenshots and `4x` macro crops (`125%` scaling)
- [x] Deploy `Y:\Apps\y-storage\y-storage.exe`, run GUI smoke check, write `verification.md`, commit, and push `origin/main`
