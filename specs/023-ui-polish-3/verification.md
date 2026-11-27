# Verification Report — `ui-polish-3` (`specs/023-ui-polish-3`)

## Summary Result — **PASS (15 / 15 Fixes Verified)**

| Item | Description | Status | Verification Evidence |
|---|---|---|---|
| **FIX 1** | Pod `Rescan` promoted to `PodPrimaryOutline` (`24px` height, `accent_line` outline + text, bounded exception in `DESIGN.md §6`) | **PASS** | `before-pods-{dark,light}.png` vs `after-pods-{dark,light}.png`, `ui::tests::growth_noise_floor_blanks_under_one_mb_and_uses_new_pill` |
| **FIX 2** | Ledger list viewport clipped strictly inside closed `1px` frame above reserved reconciliation block | **PASS** | `before-ledger-max-dark.png` vs `after-ledger-unattrib-bottom-dark.png` & `after-ledger-your-files-dark.png`, `ui::tests::ledger_clips_above_reconciliation_across_min_and_default_windows` |
| **FIX 3** | Settings sections stack naturally inside `settings_scroll` with 2-column drive grid, zero occlusion | **PASS** | `before-settings-dark.png` vs `after-settings-full-dark.png` |
| **FIX 4** | Pod `Exclusions` overlay explains folder-level exclusion; pod `Include` tooltip explains drive exclusion | **PASS** | `before-pod-exclusions-dark.png` vs `after-pod-exclusions-dark.png` |
| **FIX 5** | Split `DependencyKind` into `AntiCheat` (`"anti-cheat"`) vs `ServiceOrDriver` (`"service or driver"`) | **PASS** | `before-edge-drawer-dark.png` vs `after-edge-drawer-dark.png`, live JSON `Microsoft Edge` & `FIFA 23`, `dependencies::tests::*` |
| **FIX 6** | Suppress numeric growth deltas `< 1 MB` (`< 1,048,576 B`) visually while keeping exact byte deltas on hover | **PASS** | `before-growth-new-deltas-dark.png` vs `after-growth-new-deltas-dark.png`, `before-per-drive-c-dark.png` vs `after-per-drive-c-dark.png`, `ui::tests::growth_noise_floor_*` |
| **FIX 7** | Move per-drive companion `"of <global>"` inside Size column after per-drive size (`"2.28 GB of 163.39 GB"`) | **PASS** | `before-per-drive-c-dark.png` vs `after-per-drive-c-dark.png` (`Genshin Impact` row) |
| **FIX 8** | Zone Rulebook grouped under `"App container"` / `"Personal files"` subheads (`name + Remove` only); neutral muted pins | **PASS** | `before-settings-dark.png` vs `after-settings-full-dark.png` |
| **FIX 9** | Stale masthead caption in muted amber `status_amber` (`#F5B83D` / `#A66C07` after FIX 15) with `Last scan` timestamp moved to hover | **PASS** | `before-pods-dark.png` vs `after-pods-stale-dark.png` & `after-settings-full-dark.png` |
| **FIX 10** | `Your files` subtitle corrected to `"Shell folders & Personal-files zones · read-only"` | **PASS** | `after-ledger-your-files-dark.png` |
| **FIX 11** | Regenerate `1/2/5/10`-drive sim screenshots with shipped `"Include"`, `"Exclusions"`, `"Scanned HH:MM"`, `"Rescan"` labels | **PASS** | `sim-1-drive-dark.png`, `sim-2-drives-dark.png`, `sim-5-drives-dark.png`, `sim-10-drives-dark.png` |
| **FIX 12** | Word/segment-boundary drawer truncation (`draw_word_capped_text_cached`) + `font_id.family` hash + deduped `(service)` suffix | **PASS** | `before-edge-drawer-dark.png` vs `after-edge-drawer-dark.png`, `ui::tests::word_boundary_truncation_*` & dep truncation check |
| **FIX 13** | Legitimate icons extracted from exe/service binaries & MSIX `<Logo>` PNGs without fabricating missing ones | **PASS** | Live 4-drive scan icon coverage rose `99 -> 223` (`+124` gained, `0` lost); `Xbox Service` stays neutral fallback |
| **FIX 14** | `"new"` growth state rendered as `status_green` pill over `pill_green_fill` in both global and per-drive modes | **PASS** | `before-growth-new-deltas-dark.png` vs `after-growth-new-deltas-dark.png` & `after-per-drive-c-dark.png` |
| **FIX 15** | Palette re-anchored to flat logo (`#7CCBF2` / `#F2A9D6` -> `#69CBFC`/`#FA80CB` Dark, `#0090D9`/`#FA29AA` Light, 6-stop rim/meter gradient, `#28D66C`/`#0A7A3C` green, `#F5B83D`/`#A66C07` warning, `#F04458`/`#E02444` error); named violet token removed everywhere | **PASS** | `fix15-{main,drawer}-{dark,light}.png`, `after-*`, `ui::tests::palette_reanchor_fix15_matches_owner_flat_logo_and_status_tokens` |

---

## 1. Test & Build Verification (0 Warnings)
- **`cargo test --workspace`**: **75 passed, 0 failed, 0 warnings**
  - `y-core` unit tests (`dependencies::tests`): **2 passed**
  - `y-core` `core_tests`: **6 passed**
  - `y-core` `round2_tests`: **6 passed**
  - `y-core` `round3_tests`: **8 passed**
  - `y-core` `round4_tests`: **5 passed**
  - `y-core` `round5_tests`: **11 passed**
  - `y-platform-windows` `windows_scanner_tests`: **3 passed**
  - `y-storage` unit tests: **34 passed**
- **`cargo build --release --bin y-storage`**: compiled cleanly with **0 warnings**.
- **Deployment & GUI Smoke**:
  - Release & deployed `Y:\Apps\y-storage\y-storage.exe` SHA-256: `426D4F717712794E42989876B5F8CB2EC38E4616F8753AEDC71F1E515F1ACAAD`.
  - `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500` exited `0` (`working_set_bytes: 33,910,784`, `private_bytes: 15,925,248`, `apps_count: 283`, `has_cached_report: true`).

---

## 2. Live Attribution & Reconciliation Invariance
- **`4-drive` live ephemeral scan (`--scan-cli --ephemeral --all-drives`)**:
  - `EA SPORTS™ FIFA 23`: `107,199,710,486 B` before (`ae2`) vs **`107,199,710,486 B`** after (`0 B` delta)
  - `Store: MSTeams`: `279,428,566 B` before (`ae2`) vs **`279,428,566 B`** after (`0 B` delta)
  - Reconciliation diff on every drive (`C:`, `D:`, `Y:`, `Z:`): **`diff_used = 0 B`, `diff_total = 0 B`**
- **`C:`-only live ephemeral scan (`--scan-cli --ephemeral`)**:
  - `C:` & system drive breakdown: **`diff_used = 0 B`, `diff_total = 0 B`**
- Note: The `ae2` reference JSON (`299` apps) had `D:\Media` configured as an explicit `PersonalFiles` zone, whereas the default ephemeral config on disk did not, so `after_uip3_4drive.json` reflects `300` apps (`+1` `SubtitleEdit` inside `D:\Media`) and normal runtime browser/IDE cache growth with identical attribution logic.

---

## 3. FIX 2 Pre-Fix Reproduction Analysis
Before FIX 2, `draw_crisp_frame` in `draw_pinned_bottom_groups` sat **inside** `ScrollArea` rather than enclosing the scroll viewport, leaving the bottom of the scrolling list open/unbordered:
- At **`1020x720` (`4` drives enabled, `sys_reconcile.len() == 4` consuming `~100px`)**:
  - Whenever `ledger_height_px` was small-to-medium (`<= 280px`, e.g. `280px` in `specs/022-attribution-engine-2/c-live-your-files-d-media-dark.png`) OR maximized (`668px` in `before-ledger-max-dark.png`), expanded list rows rendered directly to the open bottom of `ScrollArea` and sliced vertically into `C:\ reconciliation` (`before-ledger-max-dark.png` visibly sliced `C:\Users\Win\AppData\LocalLow\NVIDIA`).
- At **minimum window `760x520`**: the fixed `LEDGER_MIN_H = 156.0` and `.max(88.0)` scroll height exceeded `TopBottomPanel` budget when 4 reconciliation rows took `~100px`.
- At the owner's maximized desktop window (`~1293x947` with `C:`-only `1` reconciliation row), the list either fit comfortably or was not mid-scrolled at the open edge, masking the bug.
- After FIX 2, `pinned_groups_panel` reserves `reconcile_reserve_h` first and `draw_pinned_bottom_groups` renders a closed `1px`-bordered `pal.panel` container around `ScrollArea` strictly above the reconciliation lines at every size (`760x520`, `1020x720`, `>= 1293x947`).

---

## 4. FIX 5 & FIX 13 Live Verification Details
- **FIX 5 — `Microsoft Edge` & `FIFA 23` Live Dependency Taxonomy**:
  - `Microsoft Edge Elevation Service (MicrosoftEdgeElevationService)` -> `ServiceOrDriver` (`"service or driver"`)
  - `Microsoft Edge Update Service (edgeupdate)` -> `ServiceOrDriver` (`"service or driver"`, trailing duplicate `(edgeupdate)` removed)
  - `Microsoft Edge Update Service (edgeupdatem)` -> `ServiceOrDriver` (`"service or driver"`, trailing duplicate `(edgeupdatem)` removed)
  - `EA AntiCheat` & `Denuvo Anti-Tamper` (`EA SPORTS™ FIFA 23`) -> `AntiCheat` (`"anti-cheat"`)
- **FIX 13 — Icon Coverage Delta (`99 -> 223`, `+124` gained, `0` lost)**:
  - Newly populated rows include desktop/launcher apps & runtimes (`Adobe Creative Cloud`, `EasyAntiCheat`, `Epic Games Launcher`, `Microsoft Edge WebView2 Runtime`, `Sid Meier's Civilization VI`, `Steamworks Common Redistributables`, `SubtitleEdit`, `Windows Defender`) plus **116 Microsoft Store / MSIX packages** via `AppxManifest.xml` `<Logo>` PNGs (`Store: Microsoft.Windows.Photos`, `Store: Microsoft.Paint`, `Store: Microsoft.ScreenSketch`, `Store: AppleInc.iTunes`, `Store: MSTeams`, `Store: B9ECED6F.ArmouryCrate`, `Store: Microsoft.WindowsTerminal`, etc.).
  - `Xbox Service` (`C:\Program Files\XboxService\x64\XboxService.exe`, `0` embedded Win32 icons) and headless tools (`Rust`, `.gemini`, `Playwright`, `GitHub Copilot CLI`, `Zalo`, `Node.js`, `Postman`) honestly keep the neutral fallback tile.

---

## 5. Before / After & Simulation Screenshot Index (`specs/023-ui-polish-3/`)
- **FIX 15 Dual-Mode Palette Audit**: `fix15-main-dark.png`, `fix15-main-light.png`, `fix15-drawer-dark.png`, `fix15-drawer-light.png`
- Pods & Header Stale State: `before-pods-dark.png`, `before-pods-light.png` -> `after-pods-dark.png`, `after-pods-light.png`, `after-pods-stale-dark.png`
- Pod Exclusions Overlay: `before-pod-exclusions-dark.png` -> `after-pod-exclusions-dark.png`
- Ledger Bounded Viewport & `Your files` Subtitle: `before-ledger-max-dark.png` -> `after-ledger-unattrib-bottom-dark.png`, `after-ledger-your-files-dark.png`
- Settings Non-Occluding Full View & Zone Rulebook Badge Diet: `before-settings-dark.png` -> `after-settings-full-dark.png`
- Drawer Word-Boundary Truncation & Service Kinds: `before-edge-drawer-dark.png` -> `after-edge-drawer-dark.png`
- Per-Drive `C:` Companion Placement & Noise Floor: `before-per-drive-c-dark.png` -> `after-per-drive-c-dark.png`
- `"new"` Green Status Pill & `< 1 MB` Delta Floor: `before-growth-new-deltas-dark.png` -> `after-growth-new-deltas-dark.png`
- Regenerated Drive-Count Sims: `sim-1-drive-dark.png`, `sim-2-drives-dark.png`, `sim-5-drives-dark.png`, `sim-10-drives-dark.png`

## 6. FIX 15 Removed-Token Reassignment Ledger
- Active press borders (`apply_theme_visuals`, `resolve_button_colors`, `draw_theme_toggle_button`, `draw_filter_radio_chip`) -> `pal.accent_2` (`#FA80CB` Dark / `#FA29AA` Light).
- Location `qualifier_chip_style` & qualifier text width measurement (`draw_expanded_drawer`) -> `pal.pill_pink_fill` + `pal.accent_2` stroke + `pal.pill_pink_text` (`#382C3C` / `#FA80CB` / `#FBBBE3` Dark; `#FCECF6` / `#FA29AA` / `#FA29AA` Light).
- Dependency kind chips (`draw_reverse_dependency_overlay`, `draw_dependencies_view`) -> Quiet Neutral (`Color32::TRANSPARENT` fill, `pal.border` stroke, `pal.muted` text).
- Active container gradient rims (`build_cached_rim_geometry`, `paint_active_card_gradient_border`) & drive meters (`wt_gradient_color_at`) -> unified 6-stop `WT_RIM_GRADIENT_STOPS` helper (`#54B9F2 → #7CD0FA → #A5E6FF → #D8A6FF → #F7A8D8 → #FFC7E3`).
