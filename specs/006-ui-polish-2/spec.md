# Specification: `ui-polish-2`

**Canonical Slug**: `ui-polish-2`
**Type**: UI Polish & Visual Sync Pass (Strictly Read-Only, UI-Only — Zero Changes to Scan, Attribution, Dependency, or History Logic)
**Reference Design System**: `Y:\Repos\z-controller\apps\z-controller\src\ui.rs`

## Context & Problem Statement
Owner review of `ui-polish-1` screenshots identified five defects:
1. Header buttons (`Rescan`, `Settings`) rendered broken side-bracket outlines instead of one continuous rounded-rect frame, and the theme-toggle icon button rendered a doubled/fuzzy border at 96 DPI. Furthermore, button fill/stroke/corner styling must be synced directly with `z-controller`.
2. In `Settings`, exclusion `TextEdit` hint strings (`"C:\\FolderToExclude"`, etc.) rendered at full text contrast (`override_text_color = Some(pal.text)`), making untouched fields indistinguishable from real entered exclusions.
3. Drive bar fills in the top drive strip rendered too short and lacked a delineated bordered track, obscuring true `used_bytes / total_bytes` proportionality.
4. App rows with `GrowthDelta::Changed(0)` displayed `"0 B"` clutter in the delta column instead of remaining blank.
5. The vertical app-list scrollbar rendered as a stark high-contrast floating-like pill overlapping card right borders instead of a slim palette-coloured scrollbar sitting inside a clean gutter.

## Functional & Visual Requirements
- **FIX 1 — Buttons Synced with `z-controller` & Continuous 1px Rounded Outlines**:
  - Mirror `z-controller` button fill, border, corner radius (`Rounding::same(6.0)`), hover/pressed/disabled states, and the sun/crescent theme-toggle icon button for every button (`ThemeToggle`, `Rescan`, `Settings` / `Back to Apps`, `Add exclusion`, `Remove`, filter `Clear`, reverse overlay `All` / `Close`, and `used by N apps >`).
  - Paint every bordered control, button, chip, and card with physical-pixel-snapped outer/inner rounded rects (`paint_crisp_bordered_box` / `draw_crisp_frame`) so straight edges sample cleanly on pixel centers at 96 DPI and high DPI with zero broken horizontal lines, zero side brackets, and zero doubled/fuzzy outlines.
- **FIX 2 — Exclusion Input Hints**:
  - Remove `visuals.override_text_color` override and render exclusion hints in distinctly muted `pal.hint_text`, recessed inside a crisp input well. Hide hint text as soon as the field has real content or focus; render real entered text in crisp `pal.text`.
- **FIX 3 — Proportional Bordered Drive Bars**:
  - Explicit fill formula: `ratio = clamp(used_bytes / total_bytes, 0.0, 1.0)`, rendered against a full-card-width bordered track (`height = 6.0` px, `1.0` px `pal.border` frame over `pal.bg` well), with integer-pixel fill width `fill_px = round(inner_track_px * ratio)`.
  - Prove before (`ui-polish-1`) vs after (`ui-polish-2`) numbers from measured screenshot pixels against exact `used_bytes / total_bytes` across `C:\`, `D:\`, `Y:\`, and `Z:\`.
- **FIX 4 — Empty Zero Growth Delta**:
  - When `growth` is `GrowthDelta::Changed(0)`, `GrowthDelta::NoBaseline`, or `GrowthDelta::New(0)`, render the delta column completely empty (preserving alignment). Render only non-zero deltas (`+` grew pink/red, `-` shrank green) with exact byte tooltips on hover.
- **FIX 5 — Scrollbar Synced with `z-controller`**:
  - Configure a slim (`6.0` px), non-floating (`floating = false`, `foreground_color = false`) scrollbar with `8.0` px inner margin (`SPACE_8`) coloured from `pal.scrollbar_thumb` / `pal.scrollbar_thumb_hover`, sitting cleanly inside its own gutter with zero card border overlap in both themes.

## Logged Assumptions
- Assuming `settings-typed` headless screenshot flag (`--view settings-typed`) may pre-fill `C:\TempBuildCache` exclusively for visual verification of typed vs untouched exclusion fields, with zero persistence to `%APPDATA%\y-storage\config.json`.
- Assuming `crates/y-core` and `crates/y-platform-windows` remain completely untouched so computed scan, attribution, dependency, and history values are byte-for-byte invariant.

## Acceptance Criteria (Light Verification)
- `cargo test --workspace` passes all `18` existing tests with zero compiler warnings.
- Live CLI scan confirms `EA SPORTS™ FIFA 23 == 51,372,195,955 B` (or names any real on-disk delta) and `C:\` reconciliation `diff == 0 B`.
- Six screenshots captured in `specs/006-ui-polish-2/` and visually verified:
  1. `screenshot-main-dark.png` (1020x720, 96 DPI)
  2. `screenshot-main-light.png` (1020x720, 96 DPI)
  3. `screenshot-header-buttons-zoom-dark.png` (4x nearest-neighbor crop of header buttons)
  4. `screenshot-header-buttons-zoom-light.png` (4x nearest-neighbor crop of header buttons)
  5. `screenshot-settings-empty.png` (Settings Dark with untouched hints)
  6. `screenshot-settings-typed.png` (Settings Dark with `C:\TempBuildCache` typed in `C:\`)
- Drive-bar Before/After proportionality table proves rendered pixel % matches `used_bytes / total_bytes`.
- Release binary deployed to `Y:\Apps\y-storage\y-storage.exe` and changes committed to `main`.
