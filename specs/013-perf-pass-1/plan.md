# Implementation Plan: `perf-pass-1`

- **Slug**: `perf-pass-1`
- **Preserved Invariants**: `crates/y-core`, `crates/y-platform-windows`, `apps/y-storage/src/list_query.rs` untouched; zero change to computed totals (`FIFA 23 = 51,372,195,955 B`, `C:\ diff = 0 B`).

## Phases
1. Add `--bench-perf <out.json>` driver in [main.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/main.rs) & [win32_host.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/win32_host.rs) against untouched rendering/UI/painter code; run & save `perf_baseline.json`.
2. Implement FIX A (Regular `Consolas` numerals, `ProportionalStrong` `Segoe UI Semibold` reserved for row app name + expanded drawer title) in [ui.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/ui.rs) and update [DESIGN.md](file:///Y:/Repos/y-storage/DESIGN.md).
3. Implement FIX B in [ui.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/ui.rs), [win32_host.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/win32_host.rs), and [sw_painter.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/sw_painter.rs): row virtualization, strict repaint-on-demand, cached report/filter/geometry/text layout, pooled DIB resize buffer, scanline-clipped rasterization.
4. Run unit tests + release build, capture `perf_after.json`, run live CLI scan regression check, render & inspect screenshots, write `verification.md`, deploy [y-storage.exe](file:///Y:/Apps/y-storage/y-storage.exe), commit on `main`.
