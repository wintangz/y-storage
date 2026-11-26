# Verification Report — `perf-pass-1`

- **Feature**: `013-perf-pass-1` (building on `c964790`)
- **Invariants**: `crates/y-core`, `crates/y-platform-windows`, and `apps/y-storage/src/list_query.rs` untouched; zero changes to attribution, sorting, filtering, or computed byte totals.
- **Overall Result**: **PASS**

---

## 1. FIX A — Numeral Weight & `DESIGN.md §3` Sync

- **Logged Assumption**: Windows `Consolas` ships `consola.ttf` (Regular) and `consolab.ttf` (Bold) with no Medium TTF; per prompt instruction ("Medium, or Regular where Medium is unavailable"), all monospace numerals (`Total size`, `Own size`, drive `free` / `used`, pinned ledger sizes, overlay/dependency sizes, counts, `"diff 0 B"`, and deltas) render in **Regular `Consolas`** (`Monospace`), removing `MonospaceBold` (`consolab.ttf`) completely.
- **Strong Weight Reservation**: `C:\Windows\Fonts\seguisb.ttf` (`Segoe UI Semibold`, registered as `ProportionalStrong`) is reserved strictly for the **app name in a row** and **expanded/overlay drawer title lines**. Section headers, drive letters, badges, and all numerals use Regular weight.
- **Screenshots (`96` DPI, `1020x720`)**:
  - [screenshot-main-dark.png](file:///Y:/Repos/y-storage/specs/013-perf-pass-1/screenshot-main-dark.png) & [screenshot-main-light.png](file:///Y:/Repos/y-storage/specs/013-perf-pass-1/screenshot-main-light.png)
  - [screenshot-expanded-fifa23-dark.png](file:///Y:/Repos/y-storage/specs/013-perf-pass-1/screenshot-expanded-fifa23-dark.png) & [screenshot-expanded-fifa23-light.png](file:///Y:/Repos/y-storage/specs/013-perf-pass-1/screenshot-expanded-fifa23-light.png)
- **Standard Updated**: [DESIGN.md](file:///Y:/Repos/y-storage/DESIGN.md) `§3`, `§9` (`FC2`), and `§11` (`YS2`) updated in the same commit.

---

## 2. FIX B — Performance Measurements (Before vs After Side-by-Side)

Measured on the real Win32 window (`y-storage.exe --bench-perf`) at default logical size `1020x720` (`120` DPI / `1.25` scale = `1275x900` physical px) with the real `262`-app `last_scan.json` report loaded across 16 logical CPUs:

| Metric | Baseline (Before) | Optimized (After) | Speedup / Delta | Result |
|---|---:|---:|---:|---|
| **1. Idle CPU % (5 s untouched)** | `0.00%` (`0.0 ms`, `0` paints) | `0.00%` (`0.0 ms`, `0` paints) | `0.00%` maintained | **PASS** |
| **2. Collapsed scroll (5 s mouse-wheel)** | **`54.92 ms`** (`91` frames · `p95 56.38 ms`) | **`9.62 ms`** (`515` frames · `p95 10.22 ms`) | **5.71x faster (`-82.5%`)** | **PASS** |
| **3. Expanded (`FIFA 23`) scroll (5 s)** | **`55.35 ms`** (`90` frames · `p95 58.45 ms`) | **`11.79 ms`** (`421` frames · `p95 12.82 ms`) | **4.69x faster (`-78.7%`)** | **PASS** |
| **4. Window-edge resize drag (5 s)** | **`69.42 ms`** (`71` frames · `p95 78.76 ms`) | **`12.21 ms`** (`374` frames · `p95 14.53 ms`) | **5.69x faster (`-82.4%`)** | **PASS** |

### Sub-Phase Breakdown (Average ms per Frame)

| Phase Breakdown | `scroll_collapsed` (Before -> After) | `scroll_expanded` (Before -> After) | `resize_drag` (Before -> After) |
|---|---:|---:|---:|
| **`lock + report clone`** | `0.546 ms` -> **`0.002 ms`** | `0.472 ms` -> **`0.002 ms`** | `0.562 ms` -> **`0.003 ms`** |
| **`egui_run` (UI layout)** | `4.545 ms` -> **`0.405 ms`** | `4.531 ms` -> **`0.494 ms`** | `5.969 ms` -> **`0.715 ms`** |
| **`tessellate`** | `0.617 ms` -> **`0.232 ms`** | `0.557 ms` -> **`0.249 ms`** | `0.657 ms` -> **`0.273 ms`** |
| **`raster_blit` (`SoftwarePainter`)** | `49.190 ms` -> **`8.967 ms`** | `49.770 ms` -> **`11.025 ms`** | `62.214 ms` -> **`11.197 ms`** |

### Implemented Optimizations (`(a)`–`(d)`)
- **(a) Row Virtualization** ([ui.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/ui.rs)): `draw_synced_scroll_area_viewport` (`ScrollArea::show_viewport`) renders only rows inside `[viewport.min.y - 76, viewport.max.y + 76]` using exact collapsed (`38.0 px`) + deterministic/measured expanded row offsets, cutting `egui_run` work by ~10x.
- **(b) Strict Repaint-on-Demand** ([win32_host.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/win32_host.rs)): Stationary `WM_MOUSEMOVE` pixel deduplication, `TrackMouseEvent(TME_LEAVE)` -> `WM_MOUSELEAVE` (`Event::PointerGone`), `WM_SETFOCUS`/`WM_KILLFOCUS` propagation, and `TIMER_REPAINT_ID` timer handling so idle `GetMessageW` sleeps at `0.00%` CPU (`0` `WM_PAINT`s).
- **(c) Per-Frame Caching** ([win32_host.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/win32_host.rs), [ui.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/ui.rs)): `Arc<ScanReport>` (`lock_clone` ~`0.002 ms`), cached `CachedReportData` & `CachedFilteredList`, static `OnceLock` unit corner arcs (`0` runtime trig calls), origin-relative `CachedRimMesh` translation for expanded drawers, and memoized word-boundary truncation + text widths.
- **(d) Measured Rasterizer & GDI DIB Bottleneck** ([sw_painter.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/sw_painter.rs)): Baseline profiling showed `raster_blit_ms` accounted for `~90%` of frame time (`49–62 ms`). Added scanline half-plane span clipping (`scanline_x_span`) with contiguous `.fill()` spans for solid triangles and `256px`-chunked pooled `DibBackBuffer` capacity so window-edge resize dragging avoids per-frame `CreateDIBSection` allocations.

---

## 3. Build, Tests & Live Scan Invariants

| Check | Result | Details |
|---|---|---|
| `cargo test --workspace` | **PASS** | **`27 passed, 0 failed`** (`y-core`: `6 + 4 + 4 = 14`; `y-platform-windows`: `2`; `y-storage`: `11`, `0` warnings) |
| `cargo build --release --workspace` | **PASS** | `0` warnings; deployed `4,516,864 B` to [y-storage.exe](file:///Y:/Apps/y-storage/y-storage.exe) |
| Live scan (`--scan-cli --ephemeral`) | **PASS** | `APPS = 262`, `EA SPORTS™ FIFA 23 = 51,372,195,955 B`, `C:\` reconciliation `diff = 0 B` |
| Scope guard (`git diff --stat`) | **PASS** | `crates/*` and `apps/y-storage/src/list_query.rs` untouched |
