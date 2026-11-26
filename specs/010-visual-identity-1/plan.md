# Implementation Plan: `visual-identity-1`

See approved plan:
1. Update `DESIGN.md`, `.specify/memory/constitution.md`, and `specs/010-visual-identity-1/`.
2. Extend `crates/y-core` (`model.rs`, `discovery.rs`, `attribution.rs`) with `CachedAppIcon` + optional `display_icon` and `icon_rgba`.
3. Add `crates/y-platform-windows/src/icons.rs` (`SHDefExtractIconW` / `ExtractIconExW` -> `18x18` RGBA) invoked once per scan in `scanner.rs` (and lazily backfilled on legacy `last_scan.json` loads in `win32_host.rs` / `main.rs`).
4. Update `apps/y-storage/src/ui.rs` & `main.rs`: synced `z-controller` palette/accents, `Consolas` monospace numerals, `18x18` app icons + neutral folder-tile fallback inside `cols.name`, `2px` proportion bar + `%` share hover in `cols.size`, word-boundary `…` basis capping, and `--view hover-size` screenshot capture.
5. Verify tests, release build (0 warnings), live CLI scan (`FIFA 23 == 51,372,195,955 B`, `C:\ diff == 0 B`), and visually inspect all 4 `1020x720` screenshots before deploying and committing.
