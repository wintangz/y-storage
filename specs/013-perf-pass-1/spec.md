# Feature Specification: `perf-pass-1`

**Feature Branch**: `main` (building on `c964790`)
**Created**: 2026-11-26
**Status**: Active
**Input**: Owner hands-on feedback: (A) strong numerals render too heavy; (B) UI interaction feels laggy. Keep all scan, attribution, dependency, history, sort/filter semantics, and computed byte totals unchanged.

## Requirements

1. **FIX A — Numeral Weight**:
   - Drop every numeral using Strong weight (`Total size`, `Own size`, drive `free` / `used`, pinned ledger sizes, overlay/dependency sizes, counts, and delta/diff counters) down one step to Medium (or Regular `Consolas` where Windows provides no Medium face — `consola.ttf` / `consolab.ttf` only) in both Dark and Light modes.
   - Reserve Strong (`Segoe UI Semibold` via `ProportionalStrong`) exclusively for row app names and expanded drawer titles.
   - Maintain tabular monospace numerals and update [DESIGN.md](file:///Y:/Repos/y-storage/DESIGN.md) `§3` (plus `§9` / `§11`) in the same commit.
2. **FIX B — Measure-First Performance Overhaul**:
   - Measure 4 baseline metrics at default window size (`1020x720` logical) with the real `262`-app report loaded before altering rendering/UI code:
     1. Idle CPU % over 5 s untouched
     2. Average frame time during 5 s continuous mouse-wheel scrolling (collapsed list)
     3. Average frame time during 5 s continuous mouse-wheel scrolling (with `EA SPORTS™ FIFA 23` drawer expanded)
     4. Resize drag smoothness (5 s window-edge drag frame time)
   - Implement optimizations targeting measured bottlenecks in priority order:
     - **(a) Row virtualization**: render only the visible viewport slice (`ScrollArea::show_viewport`) while keeping exact scroll height, alignment, hover, and expansion behaviour.
     - **(b) Strict repaint-on-demand**: zero continuous repaints when idle; stationary mouse/unfocused idle yields `~0%` CPU.
     - **(c) Cache per-frame work**: `Arc<ScanReport>`, precomputed report view-model/filter cache, cached corner trig normals & active gradient-rim mesh/poly, memoized text measurements/truncation.
     - **(d) Measured raster/host bottlenecks**: pooled DIB capacity across resize drag and scanline-clipped span rasterization in `SoftwarePainter`.
   - Re-measure all 4 metrics after fixes and report before/after side by side.
3. **Verification (Light)**: `cargo test --workspace`, `cargo build --release --workspace`, live CLI scan (`FIFA 23 = 51,372,195,955 B`, `C:\ diff = 0 B`), Dark/Light screenshots showing lighter numerals, deploy `Y:\Apps\y-storage\y-storage.exe`, commit on `main`.
