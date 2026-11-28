# Verification 031 — `ledger-paging-perf`

**Scope:** UI and software-rasterizer performance round (`apps/y-storage` only — zero changes to `crates/y-core` or `crates/y-platform-windows`):
1. Baseline `--bench-perf` bottleneck measurement (`1275x900`, `120 DPI` / `1.25x`, `296` apps).
2. 50-row ledger pagination (`Previous` / `Page X of Y` / `Next` footer, full filtered counts, page-1 reset on filter/sort/search/scope/rescan, drawers scoped to their page) plus painter/layout bottleneck fix.
3. Exact-cell column header hover highlight (`3px` small table radius, full header height, sort-arrow zone included) verified at `100%` (`96 DPI`) and `125%` (`120 DPI`).

---

## 1. Bottleneck Diagnosis & `--bench-perf` Before / After (`1275x900`, `120 DPI`, `296` apps)

**Step 1 Bottleneck (one sentence):** CPU software rasterization (`7.76–9.19 ms`, ~88% of frame time) was dominated by ~18,100–19,900 triangles per frame from dense `CORNER_ARC_PTS = 17` rounded boxes, non-9-sliced button/chip interiors, full-ring straight borders, and per-pixel generic barycentric/division work in `sw_painter.rs`.

| Scenario | Before (idle, `5c3969e`) | After Paging Only | Before (re-measured same load) | Final (Paging + Painter) | Delta vs Idle Before | Delta vs Same-Load Before |
|---|---:|---:|---:|---:|---:|---:|
| `idle_5s` CPU / `wm_paint` | `0.0%` / `0` | `0.0%` / `0` | `0.0%` / `0` | `0.0%` / `0` | `0` | `0` |
| `scroll_collapsed_5s` avg (`p50`) | `8.73 ms` (`8.40`) | `8.59 ms` (`8.54`) | `10.88 ms` (`10.79`) | **`6.93 ms`** (`6.71`) | **`-20.6%`** | **`-36.3%`** |
| `scroll_expanded_5s` avg (`p50`) | `9.71 ms` (`9.65`) | `10.15 ms` (`9.97`) | `12.18 ms` (`11.54`) | **`8.02 ms`** (`7.59`) | **`-17.4%`** | **`-34.2%`** |
| `resize_drag_5s` avg (`p50`) | `10.43 ms` (`10.26`) | `10.39 ms` (`10.26`) | `11.96 ms` (`11.41`) | **`8.95 ms`** (`8.61`) | **`-14.2%`** | **`-25.2%`** |
| Avg Triangles (`coll` / `exp` / `resize`) | `18,132` / `19,855` / `18,508` | `18,974` / `20,645` / `19,278` | `18,132` / `19,855` / `18,485` | **`9,772`** / **`10,511`** / **`9,992`** | **`-46.1%`** / **`-47.1%`** / **`-46.0%`** | **`-46.1%`** / **`-47.1%`** / **`-46.0%`** |

- **Honest pagination-only result:** Pagination alone (`8.59 / 10.15 / 10.39 ms`) did not measurably reduce frame cost because `draw_main_app_list` already used O(log N) viewport virtualization.
- **Painter/layout fixes implemented in Step 2:**
  - `apps/y-storage/src/ui.rs`: reduced `CORNER_ARC_PTS` `17 -> 7`; enabled 9-slice interior quads on small controls (`buttons`, `chips`, `facet triggers`); split `build_feathered_rounded_box_mesh` straight 1px edges into axis-aligned `add_solid_rect_quad` bars (`stitch_corner_arcs_quads` on corners only); simplified 2px proportion bar endcaps and switched 2px tracks to `Rounding::ZERO`.
  - `apps/y-storage/src/sw_painter.rs`: added a linear-stepping untextured gradient triangle rasterizer path, precomputed per-triangle `inv_neg_ey` edge slopes once per triangle, replaced per-pixel `/ 255` integer divisions with exact bit-shift `div255`, and gated quad detection on `WHITE_UV`.

---

## 2. Automated Workspace Test Suite (`cargo test --workspace`)

Ran `cargo test --workspace` after the final code edit with **0 compiler warnings** (`EXIT=0`):

| Crate / Target | Passed | Failed | Ignored | Notes |
|---|---:|---:|---:|---|
| `y-core` (`lib` + 9 integration suites) | 74 | 0 | 0 | Unchanged attribution engine |
| `y-platform-windows` (`lib` + `evidence_platform` + `windows_scanner`) | 9 | 0 | 0 | Unchanged Windows platform layer |
| `y-storage` (`src/main.rs` unit tests) | 45 | 0 | 0 | Includes 4 new pagination + header-hover tests |
| **Workspace Total** | **128** | **0** | **0** | **0 compiler warnings** |

---

## 3. Screenshots & Visual Verification (`100%` + `125%` DPI)

All screenshots inspected directly via `read_file`:
- `specs/031-ledger-paging-perf/screenshots/footer_mid_list_125_dark.png` (`120 DPI`, Dark) — unclipped `Showing 51–100 of 296`, `Previous`, `Page 2 of 6`, `Next`, full bottom ledger border, crisp `CORNER_ARC_PTS = 7` corner AA, neutral `SecondaryOutline` footer buttons (no button gradients).
- `specs/031-ledger-paging-perf/screenshots/footer_mid_list_125_light.png` (`120 DPI`, Light) — identical unclipped mid-list footer layout and contrast in Light mode.
- `specs/031-ledger-paging-perf/screenshots/header_hover_125_dark.png` (`120 DPI`, Dark, `Total size ↓` hovered in sort-arrow zone) — hover highlight matches exact column cell bounds (`min_x..max_x`, full `26px` height, `3px` small table radius, sort arrow included).
- `specs/031-ledger-paging-perf/screenshots/header_hover_100_dark.png` (`96 DPI`, Dark, `Total size ↓` hovered) — exact cell bounds verified at `100%` DPI.
- `specs/031-ledger-paging-perf/screenshots/header_hover_name_125_dark.png` (`120 DPI`, Dark, `App name` hovered) — exact cell bounds verified on left-aligned header.

---

## 4. Release Build & Deployment

- Release build (`cargo build --release --bin y-storage`): **0 warnings**.
- Deployed `target\release\y-storage.exe` -> `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `DB0706894F8BF2DD08AFB09722F9231D9E1909550477D7412180F2AF20017EDA`).
- GUI smoke test (`Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500`): **`ExitCode = 0`**.

---

## 5. Step Verdict

- **Step 1 (`--bench-perf` baseline + bottleneck diagnosis):** **PASS**
- **Step 2 (50-row pagination + painter/layout bottleneck fix + re-bench):** **PASS**
- **Step 3 (Exact-cell header hover at 100% + 125% DPI):** **PASS**
- **Acceptance (tests, bench table, screenshots, deploy, `STATUS.md`, commit & push):** **PASS**
