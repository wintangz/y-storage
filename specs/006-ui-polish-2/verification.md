# Verification Report: `ui-polish-2`

**Status**: PASS (Light UI Verification + Live Scan Regression Check + Pixel-Level Inspection)
**Reference Design System**: `Y:\Repos\z-controller\apps\z-controller\src\ui.rs`

## 1. Automated Tests & Build
- `cargo test --workspace`: **18 passed, 0 failed, 0 warnings**
  - `y-core` `core_tests`: 5 passed
  - `y-core` `round2_tests`: 4 passed
  - `y-core` `round3_tests`: 4 passed
  - `y-platform-windows` `windows_scanner_tests`: 2 passed
  - `y-storage` unit tests: 3 passed
- `cargo build --release --workspace`: **Finished cleanly (0 warnings)**

## 2. Live Scan Regression Check
- `apps`: **262** installed/standalone app entries
- `EA SPORTS™ FIFA 23`: **51,372,195,955 B** (exact match to prior baselines)
- `C:\` reconciliation:
  - Apps on `C:\`: `83,995,086,494 B`
  - Shared & system caches (`GREEN`): `11,826,078,743 B`
  - Your files: `3,210,724,335 B`
  - Unattributed: `1,479,599,992 B`
  - Windows & system (estimate + root files): `46,119,031,268 B`
  - Free space: `101,602,230,272 B` (`drive_used = 146,630,520,832 B`, `drive_total = 248,232,751,104 B`)
  - Reconciliation discrepancy: **`diff 0 B`**
- Note on normal live Windows `C:\` movement since `ui-polish-1`: minor background activity (`Microsoft Edge +1.65 MB`, `NVIDIA App +22.37 KB`, plus browser/OS/temp churn) shows up accurately in non-zero growth deltas while `253` unchanged apps have `delta == 0`.

## 3. Drive-Bar Proportionality Proof (FIX 3)
Fill formula implemented in `apps/y-storage/src/ui.rs`:
`ratio = clamp(used_bytes as f64 / total_bytes as f64, 0.0, 1.0)`
`fill_px = round(inner_track_px * ratio)` (inside a `6.0` px bordered track `paint_crisp_bordered_box` -> `inner_track_px = 219 px` per drive tile at `1020x720`, `96` DPI).

| Drive | `used_bytes` | `total_bytes` | Exact `used / total` | Before (`ui-polish-1`, hidden `76px` / `227px` card) | After (`ui-polish-2`, `219px` bordered inner track, `y=82` PNG) | Error vs Exact |
|---|---:|---:|---:|---:|---:|---:|
| `C:\` | `146,630,520,832` | `248,232,751,104` | **59.07%** | `44 px` (`57.89%` of hidden track / `19.38%` of card) | **`130 / 219 px = 59.36%`** | `+0.29%` |
| `D:\` | `383,636,369,408` | `1,000,168,484,864` | **38.36%** | `28 px` (`36.84%` of hidden track / `12.33%` of card) | **`84 / 219 px = 38.36%`** | `0.00%` |
| `Y:\` | `51,823,345,664` | `256,059,109,376` | **20.24%** | `14 px` (`18.42%` of hidden track / `6.17%` of card) | **`44 / 219 px = 20.09%`** | `-0.15%` |
| `Z:\` | `202,792,300,544` | `262,142,947,328` | **77.36%** | `57 px` (`75.00%` of hidden track / `25.11%` of card) | **`170 / 219 px = 77.63%`** | `+0.27%` |

## 4. Per-Fix Visual Inspection & Screenshots (`1020x720`, `96` DPI)
- **FIX 1 (Buttons synced with `z-controller` + continuous 1px outline)**: **PASS**. Inspected in 4x nearest-neighbor crops `screenshot-header-buttons-zoom-dark.png` and `screenshot-header-buttons-zoom-light.png` — sun/crescent theme toggle, `Rescan`, and `Settings` (as well as `Add exclusion`, `Back to Apps`, chips, and cards) all render one unbroken, even `1px` rounded-rect outline with zero side brackets and zero doubled/fuzzy edges.
- **FIX 2 (Exclusion input hints)**: **PASS**. Removed `override_text_color`; untouched fields in `screenshot-settings-empty.png` show dim muted placeholder hints (`#56586E` Dark) in recessed wells, while `screenshot-settings-typed.png` shows `C:\TempBuildCache` in crisp full-strength `#EDEDF2` text directly above untouched muted hints on `D:\`, `Y:\`, `Z:\`.
- **FIX 3 (Drive bar proportionality)**: **PASS** (table above; measured from `screenshot-main-dark.png` and verified visually in both Dark and Light main views).
- **FIX 4 (Empty zero deltas)**: **PASS**. Verified in `screenshot-main-dark.png` and `screenshot-main-light.png`: zero-delta apps (`FIFA 23`, `Unity`, `Xbox Service`, `CPM 2`, etc.) render completely empty delta cells while non-zero deltas (`Microsoft Edge +1.65 MB`, `NVIDIA App +22.37 KB`) retain semantic colour and hover tooltips.
- **FIX 5 (Scrollbar synced with `z-controller`)**: **PASS**. Verified in Dark & Light main views: `6px` slim palette-coloured thumb inside an `8px` (`SPACE_8`) gutter, zero overlap onto card borders.

Screenshots:
- [screenshot-main-dark.png](file:///Y:/Repos/y-storage/specs/006-ui-polish-2/screenshot-main-dark.png)
- [screenshot-main-light.png](file:///Y:/Repos/y-storage/specs/006-ui-polish-2/screenshot-main-light.png)
- [screenshot-header-buttons-zoom-dark.png](file:///Y:/Repos/y-storage/specs/006-ui-polish-2/screenshot-header-buttons-zoom-dark.png)
- [screenshot-header-buttons-zoom-light.png](file:///Y:/Repos/y-storage/specs/006-ui-polish-2/screenshot-header-buttons-zoom-light.png)
- [screenshot-settings-empty.png](file:///Y:/Repos/y-storage/specs/006-ui-polish-2/screenshot-settings-empty.png)
- [screenshot-settings-typed.png](file:///Y:/Repos/y-storage/specs/006-ui-polish-2/screenshot-settings-typed.png)
