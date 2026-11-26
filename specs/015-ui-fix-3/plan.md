# Implementation Plan: `ui-fix-3`

1. Move unmodified `logo-mark.png` (`84,018` bytes, `1200x1200` RGBA, SHA-256 `44B1C44843127655B6B716935AC9AF4E17441D2B6CB5808AFAF10A0A1ED05DCA`) to `assets/logo-mark.png` and register `ui-fix-3` in `.specify/memory/constitution.md`.
2. Add `apps/y-storage/src/logo_png.rs` (zero-dependency pure-Rust PNG RGBA decoder + RFC-1951 inflate + premultiplied-alpha area-box downsampler) and `apps/y-storage/src/logo.rs` (`EMBEDDED_WT_LOGO_PNG` + `OnceLock` cache + unit tests).
3. Update `apps/y-storage/build.rs` to generate multi-size `assets/y-storage.ico` strictly from `assets/logo-mark.png`.
4. Update `apps/y-storage/src/win32_host.rs` to create DPI-aware `HICON` handles from `EMBEDDED_WT_LOGO_PNG` (updating on `WM_DPICHANGED`) and add `--capture-titlebar` window capture across Dark & Light themes.
5. Update `apps/y-storage/src/ui.rs` to add `draw_wt_header_lockup`, apply it to the main/global header row and Settings About card, and update `DESIGN.md` (`§6.4`, `§9 FC10`, `§11`).
6. Run `cargo test --workspace`, `cargo build --release --workspace`, one live `--scan-cli --ephemeral` invariant check, render & inspect all 5 PNGs in `specs/015-ui-fix-3/`, write `verification.md`, mark `tasks.md` `[x]`, deploy to `Y:\Apps\y-storage\y-storage.exe`, and commit on `main`.
