# Verification Report: `theme-engine` (`025`)

- **Date**: 2026-11-28
- **Commit Scope**: UI + `AppConfig` theme persistence + `DESIGN.md` + `specs/025-theme-engine/` (zero scan/attribution engine edits confirmed via `git diff --name-only`).

## 1. Automated Test Suite & Release Build

Ran `cargo test --workspace` and `cargo build --release --bin y-storage` after the final edit:

- `y-core` unit tests: **3 passed, 0 failed** (`theme_preset_and_custom_rgb_persist_and_legacy_toml_defaults_to_blue`, `dependency_kind_splits_anticheat_from_ordinary_services_and_deserializes_legacy`, `format_service_dep_name_deduplicates_trailing_service_suffix`)
- `y-core` integration tests (`core_tests`, `round2_tests`, `round3_tests`, `round4_tests`, `round5_tests`): **6 + 6 + 8 + 5 + 11 = 36 passed, 0 failed**
- `y-platform-windows` integration tests (`windows_scanner_tests`): **3 passed, 0 failed**
- `y-storage` unit tests: **41 passed, 0 failed**, including:
  - `theme::tests::default_blue_and_pink_presets_match_locked_tokens_with_neutral_backgrounds`
  - `theme::tests::custom_extremes_normalise_for_wcag_contrast_in_both_modes`
  - `theme::tests::hex_parser_and_formatter_round_trip`
  - `sw_painter::tests::aligned_tex_and_solid_quads_recover_alignment_after_single_triangle`
  - `ui::tests::monochrome_theme_engine_presets_custom_contrast_and_status_tokens`
  - `ui::tests::rounded_box_9slice_uses_fast_interior_quads_and_preserves_corner_aa`
  - `ui::tests::rounded_corners_emit_antialiased_intermediate_coverage_across_dpi_scales`
  - `ui::tests::button_hierarchy_reserves_accent_for_primary_active_and_hover`
- **Compiler Warnings**: **0 warnings** in both test and release builds (`cargo build --release --bin y-storage`).

## 2. Live Performance Benchmark (`--bench-perf`, Before vs After)

Test rig: `1275×900` client pixels (`120 DPI` / `ppp = 1.25`), `283` real installed apps, release build.

| Metric | Baseline (`6a97de6`) | Final (`theme-engine`) | Improvement |
|---|---|---|---|
| **`idle_5s` CPU / `WM_PAINT`** | `0.00%` / `0` | `0.00%` / `0` | Unchanged (`0%` idle) |
| **`scroll_collapsed_5s` avg / p50 / p95** | `20.565 / 20.282 / 25.255 ms` | **`9.330 / 8.806 / 12.289 ms`** | **`2.20×` faster** (`-54.6%`) |
| **`scroll_collapsed_5s` raster+blit / egui / tess** | `18.798 / 1.122 / 0.608 ms` | **`8.225 / 0.720 / 0.354 ms`** | `-56.2%` raster+blit |
| **`scroll_expanded_5s` avg / p50 / p95** | `22.917 / 22.746 / 27.378 ms` | **`11.182 / 10.711 / 14.134 ms`** | **`2.05×` faster** (`-51.2%`) |
| **`scroll_expanded_5s` raster+blit / egui / tess** | `21.139 / 1.187 / 0.556 ms` | **`9.951 / 0.830 / 0.372 ms`** | `-52.9%` raster+blit |
| **`resize_drag_5s` avg / p50 / p95** | `24.077 / 24.092 / 30.027 ms` | **`12.090 / 11.443 / 17.142 ms`** | **`1.99×` faster** (`-49.8%`) |
| **`resize_drag_5s` raster+blit / egui / tess** | `22.163 / 1.218 / 0.660 ms` | **`10.617 / 1.006 / 0.434 ms`** | `-52.1%` raster+blit |

Sustained throughput during synthetic 5-second scroll climbed from `240` frames (`48 FPS`) to `528` frames (`105+ FPS`) collapsed, and from `216` to `443` frames expanded.

## 3. Visual Screenshot Inspection (`specs/025-theme-engine/`, 100% DPI)

All 8 verification screenshots were rendered via `--render-screenshot` on the final release binary and visually inspected:

| Screenshot | Result | Observed Verification |
|---|---|---|
| [main-blue-dark-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/main-blue-dark-100.png) | PASS | Pure neutral `#0A0A0A`/`#121212`/`#161616` surfaces, monochrome WT Blue controls/bars/drawer rim, original WT mark intact, `✕ RED` / `✓ diff 0 B` icons+labels |
| [main-blue-light-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/main-blue-light-100.png) | PASS | Pure neutral `#F2F2F2`/`#F7F7F7`/`#FFFFFF` surfaces, `#0090D9` ink + `#E2F4FC` soft fills, crisp badges and monochrome drawer rim |
| [main-pink-dark-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/main-pink-dark-100.png) | PASS | Full UI re-themed cleanly to Pink ladder (`#FA80CB` / `#382C3C`), within-family Pink pod meters and drawer rim, neutral backgrounds preserved |
| [main-pink-light-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/main-pink-light-100.png) | PASS | Full Light Pink ladder (`#FA29AA` / `#FCECF6`), pure neutral whites/greys, zero blue-pink bleed outside WT logo |
| [main-green-dark-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/main-green-dark-100.png) | PASS | Green theme coexists cleanly with `✓ GREEN`, `✓ diff 0 B`, `✓ Shared & system caches` header icon, and `✕ NO-TOUCH · Service` red pill |
| [settings-purple-dark-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/settings-purple-dark-100.png) | PASS | Settings `Appearance & Theme` card renders `Dark`/`Light`, 7 presets + `Custom`, `Purple` selected (`#9A89FB` / `#2B283B`, `Text/icon contrast 6.3:1`), crisp `✕ LOCKED` chip |
| [settings-custom-light-100.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/settings-custom-light-100.png) | PASS | Custom `#FACC15` Yellow on Light auto-normalises to `#866B04` ink (`5.1:1` contrast), synced `Hex: #FACC15` + R/G/B sliders in active ochre ladder, `! Near status hue` note with zero tofu glyphs |
| [corner-aa-zoom-4x.png](file:///Y:/Repos/y-storage/specs/025-theme-engine/corner-aa-zoom-4x.png) | PASS | 4× nearest-neighbour crop of Drive `C:\` pod + `PrimaryOutline` `Rescan` button confirms smooth `1px` corner AA coverage with crisp H/V edges and zero corner pinholes |

## 4. Deployed Executable Verification

- Deployed `target\release\y-storage.exe` to `Y:\Apps\y-storage\y-storage.exe` with matching SHA-256 hash.
- Ran `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500` cleanly with Exit `0`.
