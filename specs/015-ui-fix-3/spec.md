# Feature Specification: `ui-fix-3`

**Feature Branch**: `main` (building on `372aee0`)
**Created**: 2026-11-26
**Status**: Active
**Input**: Owner hands-on review requesting `FIX 4 — WT Logo Lockup` (`ui-fix-3`): replace the old procedural 3-bar exe/window icon and text-only header with the original WT logo artwork (`assets/logo-mark.png`), adding a crisp `24px` header lockup across Main/Dependencies/Settings+About and documenting the shared WT Header Lockup & Application Icon standard in `DESIGN.md`.

## Requirements
- **FIX 4(a) — Application Icon & Embedded Asset**:
  - Move `logo-mark.png` unmodified into `assets/logo-mark.png` and embed it at compile time via `include_bytes!("../../../assets/logo-mark.png")` (`EMBEDDED_WT_LOGO_PNG`).
  - Replace all old procedural 3-bar placeholder generation in `apps/y-storage/build.rs` with premultiplied-alpha area-box downsampling from `assets/logo-mark.png` into `apps/y-storage/assets/y-storage.ico` (`[16, 20, 24, 32, 40, 48, 64, 128, 256]` px with proper 1-bit AND masks).
  - Build DPI-aware `HICON` (`ICON_SMALL` & `ICON_BIG`, refreshed on `WM_DPICHANGED`) directly from `EMBEDDED_WT_LOGO_PNG` in `win32_host.rs`.
- **FIX 4(b) — Header Lockup & `DESIGN.md` Standard**:
  - Left-align `[24x24 WT logo mark] + 8px + "y-storage" (16.0pt text) + 8px + "Read-only · Last scan: <timestamp> · <duration>s" (11.5pt muted)` vertically optically centered on the `28.0px` top header row across all views (`Main`, `Settings`, `Dependencies`).
  - Render original unmodified transparent WT mark (`alpha == 0` outer canvas trimmed cleanly without altering strokes, recoloring, or adding any backdrop tile or border).
  - Document the canonical **WT Header Lockup & Application Icon Standard** (`§6.4`), update `§9` (`FC10`), and document `y-storage` header + About lockups in `§11` of `DESIGN.md`.
- **FIX 4(c) — Settings / About Lockup**:
  - Include a compact `8px` `panel` About card at the bottom of `Settings` repeating the `24px` WT lockup (`"y-storage"` + `"v0.1.0 · WT family read-only storage & dependency inspector"`).
- **Verification**:
  - `cargo test --workspace`, `cargo build --release --workspace`, one live ephemeral scan confirming zero invariant changes (`FIFA 23 = 51,372,195,955 B` with `7` dependencies, `C:\ diff = 0 B`, `Epic Games Launcher -> Epic Online Services`), and visual inspection of `screenshot-main-dark.png`, `screenshot-main-light.png`, `screenshot-settings-dark.png`, `screenshot-settings-light.png`, and `screenshot-titlebar-crop.png`.
