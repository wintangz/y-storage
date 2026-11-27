# Verification Report: `drive-scope-coverage-1` (incl. Add-On FIX 7–9)

**Date**: 2026-11-27  
**Deploy Path**: `Y:\Apps\y-storage\y-storage.exe` (`5,443,072` bytes; `--smoke-gui-ms 500` passed with exit `0`, `working_set_bytes = 33,595,392`, `private_bytes = 16,297,984`, `apps_count = 267`, `has_cached_report = true`)  
**User Config Integrity**: `%APPDATA%\y-storage\config.toml` verified against pre-task backup — drive targets remain `C:`-only (only the new FIX 6 `[containers]` section was added).

---

## Summary Verdicts

| Scope | Verdict | Summary |
|---|---|---|
| FIX 1 – FIX 6 (`drive-scope-coverage-1`) | **PASS** | All automated tests, live 4-drive scans (`diff = 0 B`), and inspected screenshots pass. |
| **FIX 7** (Per-Drive Size View + Scope Honesty) | **PASS** | `Size on <D>:` mode works for both `Drive` facet and `Has C: footprint` chip; excluded drives are never walked/sized and out-of-scope apps (`FIFA 23`) are omitted in `C:`-only scans. |
| **FIX 8** (`Locate` + `Web search` on All Location Rows) | **PASS** | Shared `SubtleOutline` `Locate` & `Web search` buttons render cleanly on all app drawer `Locations` rows and `Your files` / `Shared & system caches` / `Unattributed` rows; intentionally omitted on `Windows & system`. |
| **FIX 9** (Accent Derived from the WT Logo) | **PASS** | Sampled `assets/logo-mark.png` blue stop (`#85D4FB` Dark / `#067EB9` lightness-adjusted Light) applied across accent-derived tokens and documented in `DESIGN.md`, verified in before/after Dark & Light pairs. |

---

## 1. Automated Test Suite & Release Build (After All Edits)

- `cargo test --workspace`: **59 passed, 0 failed, 0 skipped, 0 warnings**
  - `y-core` `core_tests.rs`: **6 passed**
  - `y-core` `round2_tests.rs`: **6 passed**
  - `y-core` `round3_tests.rs`: **8 passed**
  - `y-core` `round4_tests.rs`: **5 passed** (includes `excluded_drive_omits_apps_and_ignores_out_of_scope_units_and_install_locations`, `per_drive_rescan_merges_cleanly_and_computes_deltas_only_against_same_drive_history`, `container_rule_claims_unowned_children_and_registry_beats_container_without_double_count`, `steam_vdf_and_appmanifest_discover_unregistered_steam_game`, `executable_inference_pe_product_name_and_os_folder_exclusion_behave_conservatively`)
  - `y-platform-windows` `windows_scanner_tests.rs`: **3 passed** (includes `excluded_drive_apps_are_omitted_and_install_folders_are_never_sized`)
  - `y-storage` unit & UI tests: **31 passed** (includes `list_query::tests::per_drive_size_mode_switches_size_and_sorting_for_drive_facet_and_has_c_chip`, `ui::tests::per_drive_size_mode_relabels_header_and_renders_companion_totals`, `ui::tests::expanded_app_drawer_locations_render_locate_and_web_search_with_missing_folder_guard`, `ui::tests::button_hierarchy_reserves_accent_for_primary_active_and_hover`)
- `cargo build --release --bin y-storage`: **0 warnings**, exit `0`

---

## 2. FIX 7 — Per-Drive Size View & Scope Honesty: **PASS**

### Verification Measurements (`4-Drive Scan` vs `Default C:-Only Scan`)

Measured on the live system with `target\release\y-storage.exe --scan-cli`:
- **4-Drive Ephemeral Scan** (`C:`, `D:`, `Y:`, `Z:` enabled): `298` apps, `317` unattributed items (`267` apps with a `C:` footprint totaling `83,380,338,473` B = `77.65` GB on `C:`), all 4 drives reconciled at **`diff = 0 B`**:
  - `C:`: `apps = 83,380,338,473` B, `shared = 11,713,553,441` B, `your_files = 3,211,739,251` B, `unattributed = 1,488,078,782` B (`307`), `win_sys = 48,695,071,877` B, `hardlink_overlap = 0` B, **`diff = 0` B**
  - `D:`: `apps = 284,767,374,596` B, `unattributed = 155,456,054,869` B (`5`), `win_sys = 2,553,552,763` B, `hardlink_overlap = 56,492,610,260` B, **`diff = 0` B**
  - `Y:`: `apps = 22,338,811,862` B, `unattributed = 32,289,290,441` B (`2`), `win_sys = 129` B, `hardlink_overlap = 2,090,037,536` B, **`diff = 0` B**
  - `Z:`: `apps = 201,325,647,317` B, `unattributed = 16,527,521` B (`3`), `win_sys = 1,453,238,666` B, `hardlink_overlap = 0` B, **`diff = 0` B**
- **Default `C:`-Only Live Scan** (`C:` enabled; `D:`, `Y:`, `Z:` excluded): `267` apps, `307` unattributed items, `C:` `apps = 83,381,417,551` B, `shared = 11,713,553,441` B, `your_files = 3,211,739,251` B, `unattributed = 1,488,078,778` B (`307`), `win_sys = 48,695,487,843` B, **`diff = 0` B** on `C:` (excluded drives `D:`, `Y:`, `Z:` unwalked with `0` bytes walked/sized).

### Worked Examples (`Before/Global 4-Drive` vs `Per-Drive C:` / `C:-Only Scan`)

| App | Global (`4-Drive` Total) | `4-Drive` By Drive (`C:` / Other) | `C:`-Only Live Scan Result |
|---|---:|---|---|
| **`EA SPORTS™ FIFA 23`** | `107,199,710,486` B (`99.84 GB`, `D:\Games\Steam\steamapps\common\FIFA 23`) | `D: 107,199,710,486` B, `C: 0` B | **ABSENT (omitted completely)** — excluded drive `D:` is neither walked nor sized, and apps solely on excluded drives are omitted |
| `Genshin Impact` | `175,442,801,779` B (`163.39 GB`) | `C: 2,451,149,093` B (`2.28 GB`), `Z: 172,991,652,686` B | `2,451,149,093` B (`C:` only) |
| `Microsoft Visual Studio Code (User)` | `3,097,101,688` B (`2.88 GB`, `install: Y:\Apps\Microsoft VS Code`) | `C: 1,943,522,044` B, `Y: 1,153,579,644` B | `1,943,522,044` B (`C:` only; out-of-scope `Y:` `install_location` cleared) |
| `Unity 6000.3.27f1` | `9,258,545,090` B (`8.62 GB`, `install: Y:\Apps\Unity`) | `C: 446,628,164` B, `Y: 8,811,916,926` B | `446,628,164` B (`C:` only; out-of-scope `Y:` `install_location` cleared) |
| `Microsoft Edge` | `8,277,248,159` B (`7.71 GB`) | `C: 8,277,248,159` B (`7.71 GB`) | `8,277,295,320` B (`C:` only) |

### Visual Verification (`fix7-*` Dark Screenshots)

- [fix7-per-drive-c-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix7-per-drive-c-dark.png): With `Drive: C:\` selected, the table header switches to **`Size on C: ↓`**, the facet summary displays **`267 apps · 77.65 GB on C:`**, rows sort by their `C:` footprint (`Microsoft Edge 7.71 GB`, `Xbox Service 7.54 GB`, ..., `Genshin Impact` showing muted **`of 163.39 GB`** alongside **`2.28 GB`**), and `EA SPORTS™ FIFA 23` is excluded.
- [fix7-unfiltered-global-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix7-unfiltered-global-dark.png): Unfiltered list cleanly reverts to **`Total size ↓`**, **`298 of 298 apps`**, and global totals (`Genshin Impact 163.39 GB`, `EA SPORTS™ FIFA 23 99.84 GB`).
- [fix9-accent-after-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix9-accent-after-dark.png): Confirms **`Has C: footprint`** chip alone also triggers **`Size on C: ↓`** mode and **`267 apps · 77.65 GB on C:`**.
- [b-pod-exclusions-overlay-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/b-pod-exclusions-overlay-dark.png) & [f-settings-container-rulebook-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/f-settings-container-rulebook-dark.png): Both surfaces display the plain one-sentence scope honesty explanation cleanly without clipping.

---

## 3. FIX 8 — Locate + Web Search on All Location Rows: **PASS**

### Row Coverage & Omission Rationale

- **App drawer `Locations`** (`draw_app_drawer` -> `LocationsTableColumns` with `actions_w = 144.0` px) and ledger folder groups (**`Your files`**, **`Shared & system caches`**, **`Unattributed`**) share `draw_ledger_folder_item_row`, rendering identical `SubtleOutline` **`Locate`** and **`Web search`** buttons, full-row hover fill, missing-path disabled guard (`"no longer on disk"`), and FIX 1's natural-question search preview (`"what is the {folder} folder in {parent}"`).
- **Intentionally omitted group — `Windows & system`**: Entries in `Windows & system` mix non-folder/synthetic accounting items (`Hardlink extent overlap`, `Scanned non-system bytes`, root page/hibernation/swap/system files) and protected OS system roots where opening Explorer `/select` or launching web-triage queries (`"what is the ... folder"`) is either non-existent on disk or unsafe/misleading for user cleanup triage.

### Worked Example (`Microsoft Edge` Expanded Drawer)

`Microsoft Edge` (`8,277,248,159` B / `7.71 GB`) in [fix8-edge-drawer-locations-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix8-edge-drawer-locations-dark.png) shows all 5 `Locations` rows with crisp, non-overlapping `Locate` and `Web search` buttons:
1. `RED` — `C:\Users\Win\AppData\Local\Microsoft\Edge` (`4,639,824,683` B / `4.32 GB`)
2. `BODY` — `C:\Program Files (x86)\Microsoft\EdgeCore` (`2,050,554,248` B / `1.91 GB`)
3. `BODY` — `C:\Program Files (x86)\Microsoft\Edge\Application` (`1,233,185,300` B / `1.15 GB`)
4. `BODY` — `C:\Program Files (x86)\Microsoft\EdgeUpdate` (`337,725,844` B / `322.08 MB`)
5. `RED` (`ProgramData · shared`) — `C:\ProgramData\Microsoft\EdgeUpdate` (`15,958,084` B / `15.22 MB`)

Also confirmed on folder-only apps (`DBSZ v3022.021.003.012.015` `BODY` row) in [g-folder-only-apps-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/g-folder-only-apps-dark.png).

---

## 4. FIX 9 — Accent Derived from the WT Logo: **PASS**

### Sampled `assets/logo-mark.png` Stops vs Shipped `ThemePalette` Tokens

Measured directly from [assets/logo-mark.png](file:///y:/Repos/y-storage/assets/logo-mark.png) (`1200x1200` RGBA):

| Token / Role | Sampled Logo / Previous Shipped | Shipped Value | Contrast / Notes |
|---|---|---|---|
| Blue Accent Stop (`W` upper stroke `(885, 335)`, `A=230`) | **Sampled**: `#85D4FB` `RGB(133, 212, 251)` (`H=199.8°, S=93.7%, L=75.3%`; median `#7DD0F9`) | **Dark `accent_line`**: **`#85D4FB`** (unchanged from sampled stop; replaced previous `#54B9F2`) | **`12.04:1`** on Dark `bg` `#0A0A10`; **`10.30:1`** on Dark `pill_blue_fill` `#12222C` |
| Light Accent (`accent_line` + active chip/button/ring) | Previous Light accent: `#2F9BE8`; Unadjusted `#85D4FB` gives **`1.48:1`** on `#EEF4FA` (`1.64:1` on `#FFFFFF`) | **Light `accent_line`**: **`#067EB9`** `RGB(6, 126, 185)` | Lightness lowered (`L=75.3% -> 37.5%`) while preserving exact hue/saturation (`H=199.8°, S=93.7%`) for **`4.01:1`** on `#EEF4FA` and **`4.46:1`** on `#FFFFFF` |
| Accent-Derived Blue Companions | Derived from Dark `#85D4FB` / Light `#067EB9` | Dark `pill_blue_fill` `#12222C`, `widget_active` `#142832`, `proven_text` `#9DDFFC`; Light `pill_blue_fill` `#E6F6FE`, `widget_hover` `#E8F4FB`, `widget_active` `#D8EEF9`, `proven_text` `#056B9E` | Preserves button hierarchy & crisp contrast |
| Pink / Lavender Gradient Stops (`T` `(489, 424)` / `(649, 924)`) | **Sampled**: `#F7ACD9` `RGB(247, 172, 217)` (`H=324.0°, S=82.4%, L=82.2%`), `#D7A5FF` `RGB(215, 165, 255)` | **`LOGO_PINK`** `#F7ACD9`, **`LOGO_LAVENDER`** `#D7A5FF` | Scoped strictly to active/expanded rims, logo mark, and button press states |
| Semantic `STATUS_GREEN` / `STATUS_RED` | Unchanged (`#34D399`/`#059669`, `#F87171`/`#DC2626`) | Untouched | Reserved strictly for health/growth/diff and `NO-TOUCH`/`RED` |

### Before / After Accent Comparison Screenshots (Dark & Light)

- **Dark Before -> After** (header `Rescan`, active `Has C: footprint` chip, open `Category` facet dropdown):  
  [fix9-accent-before-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix9-accent-before-dark.png) -> [fix9-accent-after-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix9-accent-after-dark.png)
- **Light Before -> After** (header `Rescan`, active `Has C: footprint` chip, open `Category` facet dropdown):  
  [fix9-accent-before-light.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix9-accent-before-light.png) -> [fix9-accent-after-light.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/fix9-accent-after-light.png)

---

## 5. FIX 6 Live Attribution Breakdown & Remaining Inspected Screenshots

All remaining round artifacts were re-rendered with shipped FIX 7–9 UI/palette tokens and inspected:
- [a-pods-scope-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/a-pods-scope-dark.png), [b-pod-exclusions-overlay-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/b-pod-exclusions-overlay-dark.png), [c-drive-excluded-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/c-drive-excluded-dark.png), [c-drive-excluded-light.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/c-drive-excluded-light.png), [d-ledger-covers-filters-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/d-ledger-covers-filters-dark.png), [e-unattributed-facets-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/e-unattributed-facets-dark.png), [f-settings-container-rulebook-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/f-settings-container-rulebook-dark.png), [g-folder-only-apps-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/g-folder-only-apps-dark.png)
- Responsive grid simulations: [sim-1-drive-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-1-drive-dark.png), [sim-2-drives-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-2-drives-dark.png), [sim-5-drives-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-5-drives-dark.png), [sim-10-drives-dark.png](file:///y:/Repos/y-storage/specs/021-drive-scope-coverage-1/sim-10-drives-dark.png)

> **NTFS Hardlink Reconciliation (A-001)**: Volumes containing NTFS hardlinked files (`D:\Games\Steam\steamapps\common\FIFA 23\FIFAModData`, `Y:\Repos\...\node_modules`) record `hardlink_overlap_bytes` (`walked_sum - drive_used_bytes`) under `SystemDriveBreakdown`, preserving exact folder sizes while keeping every scanned drive at `diff = 0 B`.
