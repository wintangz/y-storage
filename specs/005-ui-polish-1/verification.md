# Verification Report: `y-storage` (`ui-polish-1`)

- **Date**: `2026-11-26 18:16` (local time)
- **Feature Slug**: `ui-polish-1` (UI-only pass; zero changes to scan, attribution, dependency, or history calculation logic)
- **Workspace Tests (`cargo test --workspace`)**: **18 passed / 0 failed / 0 warnings**
- **Live Scan Regression**:
  - Duration: `8.93 s` (`8,931 ms`)
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`** (`47.84 GB`, exact match vs Round 3)
  - `EA App + FIFA 23` combined: **`54,627,471,178 B`** (exact match vs Round 3)
  - `Unattributed`: **`1,493,710,126 B`** (`304` items; `+13.95 MB` vs Round 3's `1,479,761,099 B` from live OS/disk writes during build & runtime; top unattributed items unchanged, zero items `>= 100 MB`)
  - `C:\` Reconciliation: **`diff = 0 B`** (`Apps 78.22 GB + Caches 11.01 GB + Your files 2.99 GB + Unattributed 1.39 GB + Win/sys 42.91 GB + Free 94.66 GB = 231.18 GB` exact)
- **GUI Smoke Test (`--smoke-gui-ms 600`)**: `262` cached apps loaded, Working Set `26,861,568 B` (~25.6 MB), Private Bytes `7,397,376 B` (~7.05 MB).
- **Icon Verification**: `256x256` ICO entry artwork spans `hx = 124.0` (`248 / 256 ≈ 96.9%` of tile width; `ExtractAssociatedIcon` returned valid embedded `32x32` icon on `y-storage.exe`).

---

## Polish Items Checklist (1–10)

| # | Requirement | Status | Visual / Code Evidence |
|---|---|---|---|
| 1 | Multiples-of-4 spacing scale (`4/8/12/16`) & content-hugging rows | **PASS** | Uniform `SPACE_4/8/12/16`, fixed-height `draw_split_row` (`24/28/20/16px`) eliminating vertical slack in [screenshot-main-dark.png](screenshot-main-dark.png) & [screenshot-expanded-fifa23.png](screenshot-expanded-fifa23.png) |
| 2 | One right-aligned 2-decimal size column + exact bytes hover + semantic deltas | **PASS** | `SIZE_COL_W = 96.0` right-aligned across apps, locations, dependencies, and pinned groups; exact byte tooltip on hover; `+1.15 MB` pink / `0 B` muted grey (`Microsoft Edge`) |
| 3 | Consistent chips & quiet muted basis underneath | **PASS** | Unified `paint_chip` (`BODY` blue, `GREEN` green, `RED` red/pink, lavender qualifiers, `NO-TOUCH` muted red outline, `Proven` solid, `Inferred` outline); basis text muted on second line |
| 4 | Expanded `"Dependencies"` sub-section + non-crowding reverse view (`used by N apps >`) | **PASS** | [screenshot-expanded-fifa23.png](screenshot-expanded-fifa23.png) & [screenshot-reverse-ea-app.png](screenshot-reverse-ea-app.png): overlay opens without shifting location breakdown |
| 5 | Universal truncation tooltips | **PASS** | `draw_truncated_text` measures rendered width and attaches `.on_hover_text(text)` wherever truncation occurs |
| 6 | Theme icon immediately LEFT of `Rescan`, no header divider, Dark/Light pastel palette | **PASS** | [screenshot-main-dark.png](screenshot-main-dark.png) & [screenshot-main-light.png](screenshot-main-light.png) |
| 7 | Slim proportional drive bars with `used` and prominent `free` | **PASS** | Drive tiles show bold `94.66 GB free` on top and slim `4px` bar + `136.52 GB used` below |
| 8 | Pinned bottom groups & reconciliation always visible/unclipped | **PASS** | `Windows & system`, `Your files`, `Shared & system caches`, `Unattributed`, and `diff 0 B` reconciliation visible across all main/expanded views |
| 9 | Embedded `.ico` (`~97%` tile fill) in exe, Explorer, taskbar & title bar | **PASS** | `build.rs` full-bleed multi-size ICO + `WM_SETICON` (`ICON_BIG`/`ICON_SMALL`) in `win32_host.rs` |
| 10 | Persist window size/position & Dark/Light theme across launches | **PASS** | `WindowPlacementConfig` saved/restored via `GetWindowPlacement` on `WM_EXITSIZEMOVE`/`WM_CLOSE` in `config.toml` |

## Captured Screenshots (`1020 x 720`, 96 DPI)
- [screenshot-main-dark.png](screenshot-main-dark.png)
- [screenshot-main-light.png](screenshot-main-light.png)
- [screenshot-expanded-fifa23.png](screenshot-expanded-fifa23.png)
- [screenshot-reverse-ea-app.png](screenshot-reverse-ea-app.png)
- [screenshot-settings.png](screenshot-settings.png)
