# Tasks: `visual-identity-1`

- [x] T001 Update `.specify/memory/constitution.md`, `DESIGN.md`, and `specs/010-visual-identity-1/` docs
- [x] T002 Add `CachedAppIcon`, `display_icon`, and `icon_rgba` in `crates/y-core` (`model.rs`, `discovery.rs`, `attribution.rs`) and base64 roundtrip test
- [x] T003 Implement Win32 shell icon extraction once per scan in `crates/y-platform-windows/src/icons.rs` and wire into `scanner.rs`, `win32_host.rs`, and `main.rs`
- [x] T004 Sync `ThemePalette`, accents, `input_bg`, and `apply_theme_visuals` with `z-controller` in `apps/y-storage/src/ui.rs`
- [x] T005 Register `Consolas` / `CascadiaMono` instrument numerals, render `18x18` icons + neutral fallback, `2px` proportion bar + `%` hover, and word-boundary `…` basis capping + unit test in `apps/y-storage/src/ui.rs`
- [x] T006 Add `--view hover-size` support in `apps/y-storage/src/main.rs`
- [x] T007 Run `cargo test --workspace`, `cargo build --release --workspace`, live CLI scan, and visually inspect all 4 `1020x720` screenshots
- [x] T008 Write `specs/010-visual-identity-1/verification.md`, deploy to `Y:\Apps\y-storage\y-storage.exe`, and commit on `main`
