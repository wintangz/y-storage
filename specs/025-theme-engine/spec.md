# Spec 025: Monochrome Theme Engine (`WT_UI_LIBRARY §0 v2`) & UI Lag Root-Cause Fix

- **Slug**: `theme-engine`
- **Status**: Implemented & Verified
- **Base Commit**: `6a97de6` (`ui-polish-4` + baselines)
- **Standard**: `C:\Users\Win\.gemini\config\skills\wt-ui-library\references\WT_UI_LIBRARY.md` **§0 (v2 — Monochrome Theme Engine)**

## 1. Scope & Hard Boundaries

- **Included**: UI styling, theme engine module (`apps/y-storage/src/theme.rs`), theme persistence in `AppConfig` (`crates/y-core/src/config.rs`), `Appearance & Theme` Settings section (`apps/y-storage/src/ui.rs`), software rasterizer/host UI latency optimizations (`sw_painter.rs`, `win32_host.rs`), `DESIGN.md` Part I & Part II §11.1 alignment, and `--render-screenshot` CLI theme/crop options (`main.rs`).
- **Excluded (Untouched)**: All scan and attribution engine files (`attribution.rs`, `categories.rs`, `dependencies.rs`, `discovery.rs`, `history.rs`, `pe.rs`, `tree.rs`, `zones.rs`, `crates/y-platform-windows/`).

## 2. Monochrome Theme Engine Architecture (`WT_UI_LIBRARY §0`)

1. **Single Hue Family Per Screen (`apps/y-storage/src/theme.rs`)**:
   - Replaces dual-accent Blue/Pink tokens (`BrandVariant`, `accent_2`, `pill_blue_fill`, `pill_pink_fill`, `pill_pink_text`, `proven_text`, `WT_RIM_GRADIENT_STOPS`) with one cohesive 4-step shade ladder (`ThemeLadder::generate`) producing `ThemePalette::from_config(mode, preset, custom_rgb)`.
   - Role mapping:
     - `accent_line` (`ink` step on Light, boosted accent on Dark): `PrimaryOutline` label/border, checkboxes, active icons.
     - `accent_border`: active/hover/pressed `1px` perimeter strokes.
     - `accent_soft_fill`: soft button/chip/selected-row backdrop.
     - `accent_pill_text`: high-contrast pill text over `accent_soft_fill`.
     - `accent_sweep: [Color32; 4]`: within-family monochrome stops sampled by `pal.sample_sweep(t)` for drive meter bars, app row proportion bars, and active/expanded drawer rims (`CachedRimMesh` keyed by `theme_key`).
   - **Pure Neutral Backgrounds (`§0.5`)**: All neutral surfaces enforce `R == G == B` in both modes (`#0A0A0A` / `#121212` / `#161616` / `#1C1C1C` / `#262626` / `#2E2E2E` Dark; `#F2F2F2` / `#F7F7F7` / `#FFFFFF` / `#F4F4F4` / `#EAEAEA` / `#DADADA` Light). Clear framebuffer constants (`CLEAR_BG_DARK_RGB = 0x000A_0A0A`, `CLEAR_BG_LIGHT_RGB = 0x00F2_F2F2`) match `pal.bg` identically.
   - **WT Logo Mark Exception (`§0.6`)**: Only the embedded 24px WT brand mark (`assets/logo-mark.png`) retains the original cross-hue blue→pink gradient. No button ever carries a gradient border or fill.

2. **App-Level Theme Setting (`AppConfig`, Settings `Appearance & Theme`)**:
   - `ThemePresetChoice`: `Blue` (`#7CCBF2`, default), `Pink` (`#F2A9D6`), `Purple` (`#9B8AFB`), `Green` (`#3ECF6E`), `Orange` (`#FB923C`), `Yellow` (`#FACC15`), `Red` (`#F87171`), and `Custom` (`theme_custom_rgb: [u8; 3]`), persisted atomically with `#[serde(default)]` backward compatibility for legacy TOML files.
   - **Custom Contrast Normalisation (`§0.7`)**: Arbitrary `Custom` inputs (including `#FACC15` Yellow in Light mode or `#081018` in Dark mode) preserve hue while adjusting HSL lightness/saturation so Dark `accent_line`/`accent_pill_text` and Light `ink_light` achieve WCAG `>= 4.5:1` contrast against `panel`, `bg`, and `accent_soft_fill`. Presets `Blue` (`#0090D9` Light ink) and `Pink` (`#FA29AA` Light ink) retain their exact locked `WT_UI_LIBRARY §0.3 / §2.1` tokens.
   - **Settings UI**: `Appearance & Theme` card at top of Settings renders Mode chips (`Dark` / `Light`), all 7 colour swatches + `Custom`, live `#RRGGBB` input + R/G/B sliders (`visuals.selection` themed to the active ladder), live 4-step normalised shade ladder preview with contrast ratio readout, and a `Near status hue` reminder when applicable.

3. **Semantic Status Disambiguation (`WT_UI_LIBRARY §0.8 & §4B`)**:
   - Added `StatusKind::{Success, Warning, Danger}`, crisp vector `draw_status_glyph` (`✓`, `!`, `✕`), and `paint_status_chip` so green/amber/red status states never depend on colour alone:
     - `✓ new`, `✓ GREEN`, `✕ RED`, `✕ NO-TOUCH`, `✕ LOCKED`, `✕ missing`, `✕ ROOT`, `✕ SYSTEM`, `✓ diff 0 B`, `✕ diff N B`, `✓ Shared & system caches` header, `! Scope changed — rescan needed`, `[✕ Warning]`, `✕ Read error`, and signed `+/-` deltas.

## 3. UI Lag Root-Cause & Fix Summary

Measured via `--bench-perf` (`1275×900`, `120 DPI / 1.25x`, `283` real apps) before any code changes:
- **Root Cause**: Background scan/attribution was NOT running (`idle_5s = 0.00% CPU, 0 WM_PAINTs`, `lock_clone_ms = 0.009 ms`). Instead, `91.4%` (`18.798 ms / 20.565 ms`) of active frame time was inside `raster_blit_ms`:
  1. `build_feathered_rounded_box_mesh` filled large rounded panels (`~1230×450 px`) using a `132`-triangle full-span `add_ring_center_fan`, forcing barycentric edge/scanline math across hundreds of thousands of pixels instead of hitting `sw_painter`'s axis-aligned `slice::fill` fast path (`fill_solid_rect`).
  2. `sw_painter` stepped indices in fixed 6-index strides (`i += 6`) without recovering quad alignment after odd 3-index triangles, causing textured glyphs and quads after an odd triangle to drop to per-triangle barycentric + float bilinear sampling.
  3. Redundant full-window `pal.bg` fills in `masthead_panel` + `CentralPanel` over the already-cleared framebuffer (`~1.15M` pixels/frame).
  4. Per-frame `"Hx"` font probe allocations (`optical_galley_pos` / `baseline_aligned_galley_pos`), per-frame `has_prop_strong` probes, per-scroll Unattributed filter/sort recomputation, synchronous `config.save_atomic` during filter/drag typing, and `Ep::Rounding::ZERO` `Shape::rect_filled` calls on flat separators/rows.
- **Fixes Applied**:
  - Reduced corner tessellation to `CORNER_ARC_PTS = 17` (`16` segments/quadrant) with 9-slice interior (`3` axis-aligned `WHITE_UV` quads + `4` localized corner fans on containers `>= 96×48 px`) while keeping `add_ring_center_fan` on small buttons (`Rescan`) so `1px` corner AA stays defect-free.
  - Added `AlignedTexQuad` 1:1 texel-aligned integer blit fast path + `while i + 3 <= len` quad alignment recovery + `PaintStats` telemetry in `sw_painter.rs`.
  - Switched `masthead_panel` and `CentralPanel` fills to `Color32::TRANSPARENT` over the DIB clear (`#0A0A0A` / `#F2F2F2`).
  - Added `cached_hx_metrics`, cached `has_prop_strong` on `applied_theme_key` change, `CachedUnattributedList`, `paint_solid_px_rect` / `add_solid_rect_quad` for flat quads, same-frame `ensure_report_and_filter_caches` on `list_view_dirty`, and `350 ms` debounced `TIMER_SAVE_CONFIG_ID` in `win32_host.rs` (flushed on `WM_CLOSE`).

## 4. Assumptions Logged

- **`apply_theme_visuals` backwards compatibility**: No external crate or module outside `apps/y-storage` calls `apply_theme_visuals`, so `apply_theme_visuals_with(ctx, mode, preset, custom_rgb)` replaces it cleanly (`configure_fonts_and_theme` remains as a Blue-default wrapper for non-preset test/capture helpers).
- **`ui-polish-4` FIX 2 & FIX 3 preservation**: Kept `1px` physical-pixel corner AA feathering with `±0.42px` diagonal compensation and `"Hx"` optical vertical centering across all controls while only decomposing container fills (`>= 96×48`) into 9-slice quads.
- **`0.8` unicode symbols in prose**: Because `Segoe UI` Regular lacks clean vector coverage for Unicode `✓`/`✕` (rendering square tofu boxes in body copy), prose text avoids raw Unicode `✓`/`✕` while all live status badges and ledger indicators draw crisp custom vector `draw_status_glyph` shapes.
