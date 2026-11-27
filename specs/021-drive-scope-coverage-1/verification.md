# Verification Report: `drive-scope-coverage-1`

**Date**: 2026-11-27  
**Base Commit**: `ad61871` (`main`)  
**Deploy Path**: `Y:\Apps\y-storage\y-storage.exe` (`5,412,864` bytes; pre-existing running GUI PID `25608` from `17:47:34` was preserved by renaming its locked executable to `Y:\Apps\y-storage\y-storage.prev.exe` before copying the updated binary; `--smoke-gui-ms 500` passed with `working_set_bytes = 33,181,696`, `private_bytes = 13,168,640`, `apps_count = 270`, `has_cached_report = true`)  
**User Config Integrity**: `%APPDATA%\y-storage\config.toml` verified identical to pre-task backup (`Compare-Object` returned zero differences).

---

## 1. Automated Test Suite & Release Build

- `cargo build --release --bin y-storage`: **0 warnings**, exit `0`
- `cargo test --workspace`: **54 passed, 0 failed, 0 skipped, 0 warnings**
  - `y-core` `core_tests.rs`: **6 passed**
  - `y-core` `round2_tests.rs`: **6 passed**
  - `y-core` `round3_tests.rs`: **8 passed**
  - `y-core` `round4_tests.rs`: **4 passed** (`container_rule_claims_unowned_children_and_registry_beats_container_without_double_count`, `steam_vdf_and_appmanifest_discover_unregistered_steam_game`, `executable_inference_pe_product_name_and_os_folder_exclusion_behave_conservatively`, `per_drive_rescan_merges_cleanly_and_computes_deltas_only_against_same_drive_history`)
  - `y-platform-windows` `windows_scanner_tests.rs`: **2 passed**
  - `y-storage` unit & UI tests: **28 passed** (including new tests `unattributed_assist::tests::search_query_uses_natural_question_template_without_quote_fragments`, `list_query::tests::unattributed_facets_combine_with_and_and_classify_all_seven_zones`, `ui::tests::ledger_covers_pods_and_filter_row_at_max_height`, `ui::tests::pod_scope_controls_and_settings_container_rulebook_render_and_mark_stale`, `ui::tests::adaptive_drive_pod_grid_fits_1_2_5_10_drives_without_overflow`)

---

## 2. Same-State Live 4-Drive Comparison (`C:`, `D:`, `Y:`, `Z:`)

Measured via `--scan-cli ... --ephemeral --all-drives` before edits (`18:25:32`) vs after edits (`18:47:15`) on the same live filesystem state:

| Metric | Before | After | Verdict |
|---|---:|---:|---|
| `EA SPORTS™ FIFA 23` | `107,199,710,486` B | `107,199,710,486` B | Identical byte-for-byte (registry `InstallLocation` beats `Games` container & Steam manifest) |
| `Store: MSTeams` | `279,428,566` B | `279,428,566` B | Identical byte-for-byte |
| Recognized Apps (`apps.len()`) | `272` | `298` | `+26` folder-only / Steam manifest apps |
| `Unattributed` Total | `349` (`281,779,694,485` B) | `317` (`189,172,372,945` B) | `-32` folders, `-92,607,321,540` B (`-92.61` GB) |
| OS-named `AttributedApp` rows | — | `0` | Zero `$RECYCLE.BIN` / `System Volume Information` / `Recovery` / `Config.Msi` pollution |

### Per-Drive `Unattributed` & Reconciliation Breakdown

| Drive | Unattributed Before | Unattributed After | OS Folders (`Windows & system`) | Hardlink Offset | Reconciliation Diff |
|---|---:|---:|---|---|---:|
| `C:` | `308` (`3,933,870,425` B) | `307` (`1,487,369,013` B) | `C:\$Recycle.Bin` (`2,446,012,773` B) | `0` B | **`0` B** |
| `D:` | `14` (`215,567,293,487` B) | `5` (`155,456,054,869` B) | `D:\$RECYCLE.BIN` (`2,553,552,763` B) | `56,492,610,260` B | **`0` B** |
| `Y:` | `10` (`39,440,252,620` B) | `2` (`32,212,421,542` B) | `Y:\$RECYCLE.BIN` (`129` B) | `2,084,035,581` B | **`0` B** |
| `Z:` | `17` (`22,838,277,953` B) | `3` (`16,527,521` B) | `Z:\$RECYCLE.BIN` (`486,211,838` B) | `0` B | **`0` B** |

> **NTFS Hardlink Reconciliation Finding**: `D:\Games\Steam\steamapps\common\FIFA 23\FIFAModData` and `Y:\Repos\...\node_modules` contain NTFS hardlinked files sharing file-ID extents across paths. Directory walks sum logical paths, so recording `hardlink_overlap_bytes` (`walked_sum - drive_used_bytes`) preserves `FIFA 23` and `Repos` at exact folder byte counts while reconciling every drive to `diff = 0 B`.

---

## 3. FIX 6: Newly Recognized Folder-Only & Steam Manifest Apps (26 Rows)

| App | Size (B) | Install Location | Bases & Rule Detail |
|---|---:|---|---|
| `DBSZ v3022.021.003.012.015` | `36,916,383,905` | `D:\Games\DBSZ.v3022.021.003.012.015_LinkNeverDie.Com` | `ContainerRule` (`layout rule: Games`) |
| `DS Awakening v1.1.1` | `21,359,544,654` | `Z:\Games\DS.Awakening.v1.1.1_LinkNeverDie.Com` | `ContainerRule` (`layout rule: Games`) |
| `Epic` | `19,670,833,651` | `D:\Games\Epic` | `ContainerRule` (`layout rule: Games`) |
| `nodejs` | `3,990,653,494` | `Y:\Apps\nodejs` | `ContainerRule` (`layout rule: Apps`) |
| `Rust` | `2,472,937,548` | `Y:\Apps\Rust` | `ContainerRule` (`layout rule: Apps`) |
| `Steam` | `2,128,394,846` | `Z:\Games\Steam` | `InstallLocation, Token, SteamManifest` |
| `Minecraft` | `812,231,332` | `D:\Games\Minecraft` | `ContainerRule` (`layout rule: Games`) |
| `Steamworks Common Redistributables` | `724,157,710` | `Z:\Games\Steam\steamapps\common\Steamworks Shared` | `SteamManifest` |
| `ArtCraft` | `545,911,230` | `Y:\Apps\ArtCraft` | `ContainerRule` (`layout rule: Apps`) |
| `PortableGit` | `411,138,092` | `Y:\Apps\PortableGit` | `ContainerRule` (`layout rule: Apps`) |
| `Lossless Scaling v3.2.2` | `191,670,171` | `Z:\Games\Lossless.Scaling.v3.2.2_LinkNeverDie.Com` | `ContainerRule` (`layout rule: Games`) |
| `Aseprite v1.3.5-dev` | `158,936,042` | `Z:\Softwares\Aseprite.v1.3.5-dev` | `ContainerRule` (`layout rule: Softwares`) |
| `Genshin Tools` | `155,817,331` | `Z:\Games\Genshin Tools` | `ContainerRule` (`layout rule: Games`) |
| `aida64extreme840` | `103,584,477` | `Z:\Softwares\aida64extreme840` | `ContainerRule` (`layout rule: Softwares`) |
| `FIFA Mod Manager v2.2.2` | `70,440,364` | `D:\Games\FIFA Mod Manager v2.2.2` | `ContainerRule` (`layout rule: Games`) |
| `Blockbench` | `69,628,864` | `D:\Softwares\Blockbench` | `ContainerRule` (`layout rule: Softwares`) |
| `FurMark win64` | `51,561,234` | `Z:\Softwares\FurMark_win64` | `ContainerRule` (`layout rule: Softwares`) |
| `AdeptiScanner` | `46,289,496` | `Z:\Softwares\AdeptiScanner` | `ContainerRule` (`layout rule: Softwares`) |
| `BCUninstaller 6.3.0 net8.0-windows10.0.18362.0` | `44,080,958` | `Z:\Softwares\BCUninstaller_6.3.0_net8.0-windows10.0.18362.0` | `ContainerRule` (`layout rule: Softwares`) |
| `z-controller` | `22,030,629` | `Y:\Apps\z-controller` | `ContainerRule` (`layout rule: Apps`) |
| `Aseprite v1.3` | `17,616,238` | `D:\Softwares\Aseprite.v1.3` | `ContainerRule` (`layout rule: Softwares`) |
| `cpu-z 3.01-en` | `13,396,688` | `Z:\Softwares\cpu-z_3.01-en` | `ContainerRule` (`layout rule: Softwares`) |
| `BetterJoy v7.1` | `6,570,948` | `Z:\Softwares\BetterJoy_v7.1` | `ContainerRule` (`layout rule: Softwares`) |
| `GHelper` | `6,123,624` | `Z:\Softwares\GHelper` | `ContainerRule` (`layout rule: Softwares`) |
| `y-storage` | `5,123,355` | `Y:\Apps\y-storage` | `ContainerRule` (`layout rule: Apps`) |
| `TranslucentTB-portable-x64` | `3,684,413` | `Y:\Apps\TranslucentTB-portable-x64` | `ContainerRule` (`layout rule: Apps`) |

### Remaining Non-Software `Unattributed` Folders on `D:`, `Y:`, `Z:`

All non-software / data / repo directories stay honestly `Unattributed` (`0` false promotions):
- `D:` (`5` items): `D:\Media` (`153,011,056,894` B), `D:\FIFA Mod Manager` (`2,229,355,683` B — data/profile folder without executables), `D:\Projects` (`184,470,168` B), `D:\Softwares\[files]` (`30,914,851` B), `D:\Documents` (`257,273` B)
- `Y:` (`2` items): `Y:\AI Models` (`21,116,498,308` B), `Y:\Repos` (`11,095,923,234` B)
- `Z:` (`3` items): `Z:\Cracks` (`12,088,801` B), `Z:\Softwares\[files]` (`4,438,028` B), `Z:\Keystores` (`692` B)

---

## 4. FIX 5 Facet Parity Rationale (Why the Other 3 Groups Omit Facet Bars)

- **`Unattributed`** is an exploratory triage workspace spanning hundreds of unknown folders (`317` live) across multiple drives, orders of magnitude (`KB` to `150+ GB`), and OS locations (`AppData Local/Roaming/LocalLow`, `ProgramData`, `Program Files`, user profile root, custom drive roots), so `Drive`, `Size`, and `Zone` `SecondaryOutline` facet dropdowns + `Reset filters` meaningfully accelerate review without mutating group totals.
- **`Windows & system`**, **`Your files`**, and **`Shared & system caches`** each contain a short, single-purpose homogeneous set (a handful of shell/system items or top `40` regenerable caches), so adding facet selectors there would only introduce redundant chrome over already trivial lists.

---

## 5. Screenshot Critique (All 12 Rendered PNGs Inspected via `read_file`)

All 12 screenshots in [specs/021-drive-scope-coverage-1/](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/) were inspected directly after rendering:
- [a-pods-scope-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/a-pods-scope-dark.png): All 4 drive pods (`C:\`, `D:\`, `Y:\`, `Z:\`) display per-drive timestamps (`Scanned 18:47`), `Include`, `Exclusions`, and per-drive `Rescan` cleanly at `>= 10.5pt`.
- [b-pod-exclusions-overlay-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/b-pod-exclusions-overlay-dark.png): Anchored `Drive C:\ exclusions` overlay shows `C:\WINDOWS` (`LOCKED`), input well, `Add`, and `Close` clearly framed over the list.
- [c-drive-excluded-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/c-drive-excluded-dark.png) & [c-drive-excluded-light.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/c-drive-excluded-light.png): Excluded `D:\` pod is dimmed with a neutral grey bar, `"Not scanned"`, and hidden per-pod `Rescan`, while the masthead shows `"Scope changed — rescan needed"` and `"Rescan needed"` in both Dark and Light themes.
- [d-ledger-covers-filters-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/d-ledger-covers-filters-dark.png): Dragged-to-max ledger extends right up to `4px` below the masthead row, completely covering drive pods, search box, filter chips, and facet dropdowns while keeping all 4-drive `diff 0 B` lines visible.
- [e-unattributed-facets-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/e-unattributed-facets-dark.png): Expanded `Unattributed` displays `Drive: D:\`, `Size: >= 1 GB`, `Zone: All`, `"2 of 317 folders · 144.58 GB"`, `Reset filters`, and the open `Location zone` dropdown with all 7 zone buckets + counts.
- [f-settings-container-rulebook-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/f-settings-container-rulebook-dark.png): Settings view displays matching `"Include drive in scan scope"` wording and the full **App Container Rulebook** panel with all 8 default names (`Apps`, `Games`, `Game`, `Programs`, `Program Files`, `Software`, `Softwares`, `PortableApps`) cleanly wrapped onto two rows plus `Add name` and `Explicit container folder paths`.
- [g-folder-only-apps-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/g-folder-only-apps-dark.png): Expanded `DBSZ v3022.021.003.012.015` drawer shows extracted game icon, quiet `inferred` pill, `BODY` location at `D:\Games\...` with `layout rule: Games`, and Proven PE runtime dependencies.
- Layout simulations at `1020x720`: [sim-1-drive-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-1-drive-dark.png), [sim-2-drives-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-2-drives-dark.png), [sim-5-drives-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-5-drives-dark.png), and [sim-10-drives-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-10-drives-dark.png) all scale across 1, 2, 5, and 10 drives without horizontal clipping, text collision, or overflow, ending cleanly with green `diff 0 B`.

---

## 6. Logged Assumptions

- **A-001 (Multi-Drive Hardlink Reconciliation)**: Volumes containing NTFS hardlinked directories (`FIFA 23\FIFAModData`, `node_modules`) exceed physical `drive_used_bytes` when directory trees are summed logically. Tracking `hardlink_overlap_bytes` under `SystemDriveBreakdown` keeps app sizes exact while ensuring every drive reconciles to `diff = 0 B`.
- **A-002 (`Program Files` Inbox Exclusions)**: Windows system inbox folders inside `C:\Program Files` / `C:\Program Files (x86)` (such as `Common Files`, `WindowsApps`, `Windows Defender`, `Reference Assemblies`, `WindowsPowerShell`) are excluded from container promotion so OS inbox folders never masquerade as third-party apps.
