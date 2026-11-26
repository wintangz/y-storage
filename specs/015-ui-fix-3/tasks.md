# Tasks: `ui-fix-3`

- [x] T001 Relocate unmodified `logo-mark.png` to `assets/logo-mark.png`, verify SHA-256, append `ui-fix-3` to `.specify/memory/constitution.md`, and create `specs/015-ui-fix-3/{spec,plan,tasks}.md`
- [x] T002 Implement pure-Rust PNG RGBA decoder + premultiplied area-box downsampler (`logo_png.rs`), `EMBEDDED_WT_LOGO_PNG` cache (`logo.rs`), multi-size `.ico` builder (`build.rs`), and DPI-aware `HICON` + `--capture-titlebar` (`win32_host.rs`)
- [x] T003 Add `24px` `draw_wt_header_lockup` in `ui.rs` across header and Settings/About surfaces and update `DESIGN.md` (`§6.4`, `§9 FC4/FC10`, `§11`)
- [x] T004 Run `cargo test --workspace`, release build, live ephemeral scan invariant check, capture/render and visually inspect all 5 PNGs (`screenshot-main-{dark,light}.png`, `screenshot-settings-{dark,light}.png`, `screenshot-titlebar-crop.png`), write `verification.md`, deploy binary, and commit on `main`
