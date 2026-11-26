# Verification Report: `polish-fix-2`

**Commit Parent**: `6e3c9ad` (`main`)
**Live Scan Date**: `2026-11-26` (`259` attributed apps, `19,251 ms` duration, `15,867,904 B` CLI peak WS)

## 1. Build, Tests & Core Invariants

- **`cargo test --workspace`**: **`32 passed, 0 failed, 0 ignored`** (`0` compiler warnings)
  - `y-core` `core_tests`: `6 passed`
  - `y-core` `round2_tests`: `6 passed` (includes `epic_launcher_links_epic_online_services_in_both_directions_without_inflating_totals` and `app_category_classifier_precedence_mapping_beats_signal_beats_keyword`)
  - `y-core` `round3_tests`: `4 passed`
  - `y-platform-windows` `windows_scanner_tests`: `2 passed`
  - `y-storage` unit tests: `14 passed` (includes `publisher_sort_orders_tie_breaks_by_name_and_groups_empty_publishers`, `category_sort_and_search_group_by_category_then_size_desc_then_name`, `segment_boundary_truncation_splits_dotted_filenames_and_paths`, and UI rendering/virtualization checks)
- **`cargo build --release --workspace`**: Succeeded cleanly (`0` warnings), deployed to `Y:\Apps\y-storage\y-storage.exe`.
- **Invariants**:
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`**, **`7` dependencies** (unchanged).
  - `C:\` drive reconciliation: `81,675,494,887` (apps) + `11,827,479,136` (shared caches) + `3,211,262,640` (your files) + `1,476,638,729` (unattributed) + `46,100,784,384` (Win/sys estimate) + `103,941,091,328` (free) = **`248,232,751,104 B`** (**`diff = 0 B`**).
  - Dependency size isolation: `0` mismatches between `app.total_bytes` and `sum(item.bytes)` across all `259` apps (`TOTAL_VS_ITEMS_MISMATCHES=0`).

---

## 2. Deliverable Verification (`FIX 1` – `FIX 8`)

| Fix | Scope | Status | Evidence |
|---|---|---|---|
| **FIX 1** | Epic / EOS bidirectional dependency link & general detection rules | PASS | `Epic Games Launcher` (`1,960,387,113 B`) now lists `Epic Online Services` (`Proven`, `1,236,736,012 B`) and `Easy Anti-Cheat` (`Proven`, `75,771,346 B`); `reverse_dependencies` maps `Epic Online Services` -> `["Epic Games Launcher"]` (`Proven`). `Tailscale` (`40,242,127 B`) also gains `service: Tailscale` via Source 2c split-install rule. |
| **FIX 2** | Optical vertical text centering (`100%` & `125%` DPI) | PASS | Pixel-snapped `optical_galley_pos` centering across header buttons, filter chips, pill badges, column headers, table cells, pinned ledger, and drive pods; verified in `screenshot-centering-zoom-200.png`. |
| **FIX 3** | Full semicircular pill badges (`radius = 0.5 * height`) | PASS | `BADGE_PILL_H = 18.0`, `RADIUS_BADGE_PILL = 9.0` (`0.5 * snapped.height()`) on `● GREEN`, `● RED`, `BODY`, `NO-TOUCH`, `proven`, `inferred`, `shared`, `missing`, category, and qualifier badges; buttons keep `6.0px` / `4.0px`. |
| **FIX 4** | Colour discipline (`drive bars`, `scrollbar`, `Light proven`, `ledger ticks`) | PASS | (a) 3-stop WT horizontal gradient (`LOGO_BLUE -> LOGO_LAVENDER -> LOGO_PINK`) across `C:\`, `D:\`, `Y:\`, `Z:\`; (b) crisp Dark `#52567A` / Light `#98B0CA` scrollbar thumbs; (c) Light `proven` uses outline `#2F9BE8` + soft tint `#E3F2FD` + `#1565B8` ink; (d) semantic ticks (`muted`, `muted`, `STATUS_GREEN`, muted neutral). |
| **FIX 5** | Collapsible & resizable pinned ledger + `22px` bottom fade | PASS | Top `6px` grip (`156px..50%`), chevron fold to single inline summary line + `● diff 0 B` (`screenshot-ledger-collapsed-dark.png`), resized height (`screenshot-ledger-resized-light.png`), persisted in `config.toml`, `22px` scroll fade (`paint_bottom_fade_overlay`). |
| **FIX 6** | Segment-boundary basis truncation | PASS | Breaks cleanly at ` `, `\`, `/`, `.`, `,`, `;`, `)`, `|` (`eaanticheat…` in `screenshot-expanded-fifa23-dark.png`), full string preserved on hover. |
| **FIX 7** | Clickable `Publisher` column sort | PASS | `Publisher` header toggles `A→Z` / `Z→A` with `display_name` tie-break (`screenshot-main-sort-publisher-dark.png`). |
| **FIX 8** | Data-driven 10-category taxonomy & clickable `Category` sort | PASS | `data/app_categories.toml` (`mapping` > `signal` > `rule` > `Uncategorized`), clickable `Category` column (`screenshot-main-sort-category-dark.png`), search matching, expanded basis line, and `DESIGN.md §11`. |

---

## 3. `FIX 1` Root Cause & Empty-Dependency Sanity Sweep

### Root Cause
1. `data/dependency_catalog.toml` conflated `EpicOnlineServices` / `eossdk-*.dll` inside the `[apps]` entry `epic-games` (`Epic Games Launcher`), so the self-dependency guard skipped EOS on Epic Games Launcher.
2. `dep_scan.rs` stopped scanning subtrees at `depth <= 1`, missing `Z:\Games\Epic Games\Launcher\Portal\Binaries\Win64\EOSSDK-Win64-Shipping.dll` and `Portal\Extras\EOS\*`.
3. `dependencies.rs` Source (2) only linked Windows services whose `ImagePath` lived inside the app's own install folder, missing companion services installed in separate sibling directories (`C:\Program Files (x86)\Epic Games\Epic Online Services\service\EpicOnlineServicesHost.exe`).

### Sanity Sweep (Non-Store Services/Drivers & Shared-Publisher Apps + Split-Install Services)
- **Newly Linked**:
  - `Epic Games Launcher` (`Epic Games, Inc.`) -> `Epic Online Services` (`Proven`, `service: EpicOnlineServices + companion folder kinship + same-publisher companion service + catalog: ...`) & `Easy Anti-Cheat` (`Proven`, `service: EasyAntiCheat_EOS + same-publisher companion service`).
  - `Tailscale` (`Tailscale Inc.`) -> `Tailscale (Tailscale)` (`Proven`, `service: Tailscale` via Source 2c split-install rule).
- **Reviewed — Intentionally Not Linked (Not a Missing Runtime Dependency)**:
  - `NVIDIA Graphics Driver 621.01`, `NVIDIA PhysX System Software 9.26.0904`, `NVIDIA HD Audio Driver 1.4.6.6` (`NVIDIA Corporation`): standalone hardware driver packages; no runtime dependency coupling between audio/PhysX/display installer packages (`NVIDIA App` itself is independent management software).
  - `AMD Chipset Software` (`Advanced Micro Devices, Inc.`), `TP-Link Archer T2U Nano Driver` (`TP-Link`): standalone driver packages.
  - `Microsoft Corporation` suite (`Windows Software Development Kit`, `Microsoft Edge WebView2 Runtime`, `Microsoft Power BI Desktop`, `Microsoft Windows Desktop Runtime 10.0.14 (x64)`, `Windows User Registry (NTUSER.DAT)`, `Microsoft Visual C++ 2010 x64 Redistributable`, `Microsoft Update Health Tools`): root redistributables / OS tooling or standalone MSI apps with distinct runtimes; none of these empty-dependency entries act as a consumer depending on an unlinked sibling runtime.
  - `AI Noise Cancelation Audio Software` & `AURA Service` (`ASUSTeK`): `AURA Service` is an RGB/lighting service already marked `NO-TOUCH · Service`; `AI Noise Cancelation` is a standalone audio utility unrelated to RGB lighting (`deps = []` is accurate).
  - `ArtCraft Launcher`, `PhotoCraft`, `FilmCraft` (`Learning Machines LLC`): standalone portable/local apps sharing a studio publisher with zero shared registered services or companion runtimes.
  - `CrystalDiskMark 9.0.1 Aoi Edition`, user profile dot-folders (`.gemini`, `.continue`, `.local`, `.agents`, `.impeccable`): standalone tools / config stores.
  - `80` `Microsoft Store` inbox/UWP packages: system UI/protocol handlers/extension packs without external runtime dependencies.

---

## 4. `FIX 8` App Category Coverage Report

- **Categorized Apps by Count**: **`223 / 259`** (**`86.10%`**)
- **Categorized Apps by Byte Share**: **`157,777,722,663 / 158,957,643,278 B`** (**`99.26%`**)

| Category | Apps | Total Bytes |
|---|---:|---:|
| System & Runtimes | 94 | 22,686,284,418 |
| Utilities | 37 | 15,729,425,261 |
| Uncategorized | 36 | 1,179,920,615 |
| Media & Creative | 29 | 5,662,945,381 |
| Development | 20 | 23,817,674,066 |
| Productivity | 13 | 3,522,881,677 |
| Games | 11 | 65,928,618,867 |
| Platforms & Launchers | 9 | 7,578,661,125 |
| Communication | 7 | 2,343,239,756 |
| Browsers | 3 | 10,507,992,112 |

### `Uncategorized` Apps `>= 100 MB`
- **`Store: Microsoft.4297127D64EC6`** — `911,260,591 B` (`publisher: Microsoft Store`, `install_location: None`, `basis: none: insufficient evidence for classification`). Opaque hex Store package ID with no descriptive name or registry metadata on disk; left `Uncategorized` per evidence-only rule rather than guessing.

### Spot-Check Examples (6 Categories & Precedence Tiers)
1. **Games (Mapping)**: `EA SPORTS™ FIFA 23` -> `Games` (`mapping: Electronic Arts PC sports game title`)
2. **Platforms & Launchers (Mapping)**: `Epic Games Launcher` -> `Platforms & Launchers` (`mapping: Epic Games storefront and game launcher`)
3. **Development (Mapping)**: `Unity 6000.3.27f1` -> `Development` (`mapping: Unity game engine editor & project hub`)
4. **Browsers (Mapping)**: `Microsoft Edge` -> `Browsers` (`mapping: Microsoft Chromium web browser`)
5. **System & Runtimes (Signal)**: `Xbox Service` -> `System & Runtimes` (`signal: NO-TOUCH · Service component`)
6. **Communication (Keyword Rule)**: `Store: Microsoft.YourPhone` -> `Communication` (`rule: Communication / phone-link name keyword`)

---

## 5. Rendered Screenshots (`specs/014-polish-fix-2/`)
- [screenshot-main-dark.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-main-dark.png)
- [screenshot-main-light.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-main-light.png)
- [screenshot-main-sort-publisher-dark.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-main-sort-publisher-dark.png)
- [screenshot-main-sort-category-dark.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-main-sort-category-dark.png)
- [screenshot-expanded-epic-dark.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-expanded-epic-dark.png)
- [screenshot-expanded-fifa23-dark.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-expanded-fifa23-dark.png)
- [screenshot-expanded-fifa23-light.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-expanded-fifa23-light.png)
- [screenshot-ledger-collapsed-dark.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-ledger-collapsed-dark.png)
- [screenshot-ledger-resized-light.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-ledger-resized-light.png)
- [screenshot-centering-zoom-200.png](file:///Y:/Repos/y-storage/specs/014-polish-fix-2/screenshot-centering-zoom-200.png)

## 6. Logged Assumptions
- **Pure-lowercase optical centering (`FIX 2`)**: Pure-lowercase badges (`proven`, `inferred`, `shared`, `missing`, `free`) center the `"x"` height band vertically while uppercase/mixed labels (`Rescan`, `Settings`, `BODY`, `GREEN`, `RED`) center the `"H"` cap-height band so both sets sit visually balanced at `100%` and `125%` DPI.
- **Unlinked `Microsoft.4297127D64EC6` (`FIX 8`)**: Intentionally kept in `Uncategorized` rather than hardcoding an unverified opaque Store GUID heuristic.
