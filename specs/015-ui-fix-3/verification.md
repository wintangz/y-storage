# Verification Report: `ui-fix-3`

**Result**: **`FIX 4: PASS`**
**Commit Parent**: `372aee0` (`main`)

## 1. Canonical Asset & Multi-Size Icon Verification (`FIX 4a`)
- **Unmodified Artwork**: `assets/logo-mark.png` (`84,018` bytes, `1200x1200` 8-bit RGBA, transparent background, SHA-256 `44B1C44843127655B6B716935AC9AF4E17441D2B6CB5808AFAF10A0A1ED05DCA`).
- **Embedded Asset**: `EMBEDDED_WT_LOGO_PNG` (`include_bytes!("../../../assets/logo-mark.png")`) decoded cleanly in `logo.rs` (`alpha_bbox = (283, 276)..=(952, 935)`).
- **Generated `.ico`**: `apps/y-storage/assets/y-storage.ico` (`381,038` bytes, `type = 1`, `9` 32-bpp entries: `16, 20, 24, 32, 40, 48, 64, 128, 256` px with 1-bit AND masks).
- **Live Window Icons**: `win32_host::load_app_icons_for_dpi` builds `ICON_SMALL` & `ICON_BIG` directly from `EMBEDDED_WT_LOGO_PNG` and refreshes both on `WM_DPICHANGED`, cleaned up once via `WindowHostState::drop`.

## 2. Build, Tests & Live Scan Invariants
- **`cargo test --workspace`**: **`33 passed, 0 failed, 0 ignored`** (`0` warnings):
  - `y-core`: `16 passed` (`core_tests`: `6`, `round2_tests`: `6`, `round3_tests`: `4`)
  - `y-platform-windows`: `2 passed` (`windows_scanner_tests`)
  - `y-storage`: `15 passed` (including `logo::tests::embedded_wt_logo_decodes_and_downsamples_cleanly`)
- **`cargo build --release --workspace`**: Succeeded cleanly (`17.04s`, `0` warnings); deployed to `Y:\Apps\y-storage\y-storage.exe`.
- **Ephemeral Live Scan (`--scan-cli ... --ephemeral`)**:
  - `259` attributed apps
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`**, **`7` dependencies**
  - `Epic Games Launcher`: links `["Easy Anti-Cheat", "Epic Online Services"]`
  - `C:\` reconciliation: **`diff = 0 B`**

## 3. Header Lockup, Settings/About & `DESIGN.md` (`FIX 4b` & `FIX 4c`)
- **`DESIGN.md`**: Added Part I `§6.4` (*WT Header Lockup & Application Icon Standard*), `§9` `FC10` (*WT Brand Lockup & Unmodified Mark*), updated `FC4` badge radius wording, and documented `y-storage` header + About lockups in `§11`.
- **Visual Inspection**:
  - [screenshot-main-dark.png](file:///Y:/Repos/y-storage/specs/015-ui-fix-3/screenshot-main-dark.png) — `24px` unmodified WT mark + `8px` + `y-storage` (`16.0pt`) + `8px` + muted `"Read-only · Last scan..."` caption optically centered in the `28px` Dark header row.
  - [screenshot-main-light.png](file:///Y:/Repos/y-storage/specs/015-ui-fix-3/screenshot-main-light.png) — Smooth premultiplied-alpha anti-aliasing against Light `#EEF4FA` canvas with zero dark halo or added tile.
  - [screenshot-settings-dark.png](file:///Y:/Repos/y-storage/specs/015-ui-fix-3/screenshot-settings-dark.png) — Settings view + compact bottom `panel` About card repeating the `24px` WT lockup.
  - [screenshot-settings-light.png](file:///Y:/Repos/y-storage/specs/015-ui-fix-3/screenshot-settings-light.png) — Light-mode Settings view + About card lockup.
  - [screenshot-titlebar-crop.png](file:///Y:/Repos/y-storage/specs/015-ui-fix-3/screenshot-titlebar-crop.png) — Live Win32 non-client title bar (`16px` WT `HICON`) + `24px` client header lockup captured across Dark and Light modes.

## 4. Logged Assumptions
- **Asset Location (`assets/logo-mark.png`)**: Relocated the owner-provided `logo-mark.png` from the repository root into `assets/logo-mark.png` (byte-for-byte identical, SHA-256 unchanged) and updated `include_bytes!` and `build.rs` accordingly so source assets stay organized cleanly under `assets/`.
