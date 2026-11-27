# Verification Report: `attribution-engine-2`

**Date**: 2026-11-27  
**Workspace**: `Y:\Repos\y-storage`  
**Test Suite (`cargo test --workspace`)**: **70 passed, 0 failed, 0 warnings** (`core_tests`: 6, `round2_tests`: 6, `round3_tests`: 8, `round4_tests`: 5, `round5_tests`: 11, `windows_scanner_tests`: 3, `y-storage`: 31).  
**Release Build (`cargo build --release --bin y-storage`)**: **exit 0, 0 warnings**.  
**Deployed Executable**: `Y:\Apps\y-storage\y-storage.exe` — SHA-256 `559A8335CE721FE32CF046159389ED4BB6C60DEC8731E5B6B14AEAF9CEBF2A6E` (matches release build).  
**GUI Smoke Check (`--smoke-gui-ms 500`)**: exit `0`, `working_set_bytes = 33,820,672`, `private_bytes = 14,639,104`, `apps_count = 269`, `has_cached_report = true`.

---

## Per-Requirement Verification Matrix

### Part A — Unified Zone Rulebook: **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `personal_zone_routes_media_folder_to_your_files_without_app_hunting`: Explicit `PersonalFiles` zone `D:\Media` containing a `.exe` subfolder (`D:\Media\Jellyfin\jellyfin.exe`) and `Movies` routes all `152,471,339,008 B` directly to `Your files` with `apps = []`, `unattributed = []`, and `diff = 0 B`.
  - `registered_app_inside_personal_zone_keeps_its_folder`: Registered `VLC media player` (`D:\Media\Tools\VLC`, `5,000,000,000 B`) is carved out of explicit `PersonalFiles` zone `D:\Media`, leaving `95,000,000,000 B` in `Your files` and `5,000,000,000 B` on `VLC media player`, `diff = 0 B`.
  - `ui::tests::pod_scope_controls_and_settings_container_rulebook_render_and_mark_stale` ([`ui.rs`](file:///Y:/Repos/y-storage/apps/y-storage/src/ui.rs)): Adding/editing `ZoneRulebook` entries renders both `App container` and `Personal files` badges and marks stale scope (`ScanReport::is_scope_stale`).
  - `AppConfig::load_or_default` ([`config.rs`](file:///Y:/Repos/y-storage/crates/y-core/src/config.rs)): Automatically migrates legacy `[containers]` TOML configs into `[zones]` (`8 AppContainer` + `6 PersonalFiles` defaults preserved).
- **`LIVE`** (ephemeral 4-drive scan on `C:\`, `D:\`, `Y:\`, `Z:\`):
  - With `[[zones.explicit_zones]] path = 'D:\Media' kind = "PersonalFiles"` in `%APPDATA%\y-storage\config.toml`, **`D:\Media`** moved from `Unattributed` (`153,011,056,894 B`) to `Your files` (**`153,011,056,894 B`** in `your_files`, **`0 B`** in `unattributed`), exact byte match.
  - **`D:\Documents`** (`257,273 B`) likewise moved cleanly to `Your files` under the visible `Documents` `PersonalFiles` default rule.
  - Inspected dark Settings screenshot: [a-settings-zone-rulebook-dark.png](file:///Y:/Repos/y-storage/specs/022-attribution-engine-2/a-settings-zone-rulebook-dark.png) shows both `App container` and `Personal files` badges on drive-root rules, explicit path `D:\Media`, and manual pin controls.
  - Inspected live ledger screenshot: [c-live-your-files-d-media-dark.png](file:///Y:/Repos/y-storage/specs/022-attribution-engine-2/c-live-your-files-d-media-dark.png) shows `D:\Media` (`142.50 GB`) under `Your files` and `diff 0 B` on all four drives.

---

### Part B (1) — Registered Roots First + Nested Carve-Out at Any Count: **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `nested_carve_out_count_1_splits_launcher_and_records_carved_child`: Registered `HoYoPlay` (`C:\Program Files\HoYoPlay`, `1 GB` direct + `2 GB` `LauncherData` + `90 GB` nested registered `Genshin Impact`) splits even with nesting count = **1**. `HoYoPlay` keeps `3,000,000,000 B`, `Genshin Impact` receives `90,000,000,000 B`, and `HoYoPlay.carved_out_apps == ["Genshin Impact"]` (`"contains Genshin Impact — counted under Genshin Impact"`).
  - `nested_carve_out_count_2_and_remainder_keep_real_paths_and_zero_files_suffix`: Nesting count = **2** (`MyLauncher` -> `GameOne` + `GameTwo`) cleanly carves both children (`40 GB` & `50 GB`), keeps `3 GB` on `MyLauncher`, and records `carved_out_apps == ["GameOne", "GameTwo"]`.
  - Inspected dark UI screenshot: [b-group-node-children-fixture-dark.png](file:///Y:/Repos/y-storage/specs/022-attribution-engine-2/b-group-node-children-fixture-dark.png) displays `Genshin Impact` (`88.94 GB`) and expanded `HoYoPlay` (`1.58 GB`) with `"contains Genshin Impact — counted under Genshin Impact"`.
- **`LIVE`** (4-drive scan):
  - Nested carve-out executed cleanly for registered parent/child sets already present on live disk (`Steam` -> `Slay the Spire 2`, `Spacewar`, `Steamworks Common Redistributables`; `Armoury Crate Service` -> `AURA lighting effect add-on x64`, `ROG Live Service`; `Unity Hub 3.22.5` -> `Unity 6000.3.27f1`; `AutoHotkey` -> `AutoHotkey` stayed intact at `19,855,277 B` without splitting internal `v2`/`UX` subdirs).
  - *Note*: The `HoYoPlay ⊃ Genshin Impact` nested layout is `FIXTURE`-only here because on this machine `HoYoPlay` (`C:\Program Files\HoYoPlay`) and `Genshin Impact` game data (`Z:\Games\Genshin Impact`) reside on separate drives.

---

### Part B (2) — Manifest Containers Next (Steam + Epic; Siblings Excluded): **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `manifest_container_includes_manifest_members_and_excludes_non_member_siblings`: `SteamLibrary` promotes `Hades` (`15 GB`) and `Hollow Knight` (`8 GB`) from `appmanifest_*.acf`, routes `shadercache` (`500 MB`) to `Shared & system caches`, and leaves unmanifested siblings (`workshop`, `downloading`, `OrphanMod`) in `Unattributed` (`3.3 GB`) — never promoted as apps even when `OrphanMod` contains a `.exe`.
  - `containment_is_weakest_claim_and_epic_manifest_excludes_dlc`: `parse_epic_manifest_item` parses base game `Sid Meier's Civilization VI` and skips incomplete installs / DLC entries (`MainGameAppName != ""`).
- **`LIVE`** (4-drive scan):
  - `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\` contains base `.item` (`Sid Meier's Civilization VI`) and DLC `.item` (`Civilization VI : Aztec DLC`) both pointing at `D:\Games\Epic\SidMeiersCivilizationVI`. Previously, the folder appeared only as a generic `ContainerRule` row named `Epic` (`19,670,833,651 B`). Now `Epic` is gone and **`Sid Meier's Civilization VI`** (`19,670,833,651 B`) is promoted directly via manifest discovery without creating a separate DLC row.
  - Steam manifest siblings (`common`, `workshop`, `downloading`) remain properly excluded from app promotion.

---

### Part B (3) — Remainder Units Keep Real Paths (Zero `[files]` / `[root files]`): **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `nested_carve_out_count_2_and_remainder_keep_real_paths_and_zero_files_suffix`: `ScannedUnitKind::ContainerLooseFiles` emits real directory paths (`D:\Launcher` -> `LocationQualifier::InstallLooseFiles` labeled `"loose files in the install folder"`; `D:\Softwares` & `D:\` -> `Unattributed` with `is_loose_files = true`), with `0` occurrences of `[files]` or `[root files]` across `units`, `apps`, `shared_caches`, and `unattributed`.
- **`LIVE`** (`after_ae2_4drive.json` & `after_ae2_c_only.json`):
  - Before baseline contained **`23`** occurrences of literal `[files]` / `[root files]` (e.g., `C:\[root files]`, `D:\Softwares\[files]`, `C:\Program Files (x86)\Common Files\Steam\[files]`).
  - After `attribution-engine-2`: **`0`** occurrences of `[files]` or `[root files]` anywhere in either the 4-drive report or the `C:`-only report. Remainders carry real paths (`C:\`, `D:\Softwares`, `C:\Program Files (x86)\Common Files\Steam`).

---

### Part B (4) — Two-Segment `Publisher\Product` Composition: **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `two_segment_publisher_product_matches_google_drive_and_rejects_ties`: `Google\Drive` claims `Google Drive` (`300 MB`) and `Google\Chrome` claims `Google Chrome` (`700 MB`) with `AttributionBasis::PublisherProduct` (`"publisher+folder match"`), even when `Google Drive` precedes `Google Chrome` in the bucket slice, while `Acme\Sync` with two tied `Acme Sync` apps (`150 MB`) is rejected to `Unattributed`.
- **`LIVE`** (4-drive scan):
  - Two-segment `Publisher\Product` matched live folders cleanly on disk: **`ArtCraft Launcher`** (`Y:\Apps\ArtCraft\Launcher`, `26,504,225 B` -> `PublisherProduct`), **`AMD Chipset Software`**, **`Rockstar Games Launcher`**, **`Rockstar Games SDK`**, **`Microsoft Edge`**, **`Google Chrome`**, **`Epic Games Launcher`**.
  - Control-app stability verified: **`EA SPORTS™ FIFA 23`** (`107,199,710,486 B`) and **`Store: MSTeams`** (`279,428,566 B`) remained byte-for-byte identical before vs after.

---

### Part B (5) — Three-Way Inference (Own-Exe, Group Node, Single-App Wrapper): **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `group_node_promotes_three_children_with_group_publisher_and_no_parent_app`: Unregistered publisher folder `D:\Apps\Adobe` with no own root `.exe` and 3 children with main `.exe`s promotes `Adobe Photoshop 2025` (`4 GB`), `Adobe Illustrator 2025` (`3 GB`), and `Adobe Premiere Pro 2025` (`5 GB`) with publisher `Adobe` and no `Adobe` app row.
  - `single_child_wrapper_promotes_outer_folder_not_child`: `D:\PortableTool` wrapping a single child `v2.1\PortableTool.exe` promotes `PortableTool` (`55,010,000 B`) at `D:\PortableTool`, never `v2.1`.
  - `app_container_children_and_installed_roots_do_not_fracture_or_misname`: Confirms `AppContainer` immediate children without shallow `.exe` (`Y:\Apps\Rust`) stay `ContainerRule` apps, registered apps (`AutoHotkey`, `Microsoft GameInput`) never fracture on internal `v2`/`UX`/`x86`/`x64` subdirs, and `AppContainer` children preserve clean folder names instead of wrapper exe `ProductName`s.
  - Inspected dark UI screenshot: [b-group-node-children-fixture-dark.png](file:///Y:/Repos/y-storage/specs/022-attribution-engine-2/b-group-node-children-fixture-dark.png) renders the 3 `Adobe` children with publisher `Adobe` and zero `Adobe` app row.
- **`LIVE`** (4-drive scan):
  - `AppContainer` children (`Rust` `2,472,937,548 B`, `DBSZ v3022.021.003.012.015` `36,916,383,905 B`, `DS Awakening v1.1.1` `21,359,544,654 B`, `Lossless Scaling v3.2.2` `191,670,171 B`, `Aseprite v1.3`, `Aseprite v1.3.5-dev`) stay accurately named and intact; Drive Root Rule + own-exe promoted unregistered standalone tools cleanly (`LightingService` `14,227,636 B`, `NVIDIA Container` `2,960,096 B`, `GHelper` `6,123,624 B`, `z-controller`, `y-storage`, `AutoHotkey` intact). No multi-app unregistered group node exists on this machine (covered by **`FIXTURE`**).

---

### Part B (6) — Containment Is Weakest Claim: **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `containment_is_weakest_claim_and_epic_manifest_excludes_dlc`: Inside `D:\Launcher` (`InstallLocation` of `MyLauncher`), manifest child `D:\Launcher\ManifestGame` (`20 GB`) goes to `ManifestGame`, pinned child `D:\Launcher\PinnedModTool` (`5 GB`) goes to `PinnedModTool`, and only unclaimed residue `D:\Launcher\Engine` (`2 GB`) + loose root files (`1 GB`) stay with `MyLauncher` (`3 GB`).
- **`LIVE`** (4-drive scan):
  - Registered enclosing `InstallLocation`s claim only unclaimed internal subdirs after Steps (1)–(5) carve-outs (`diff = 0 B`).

---

### Part B (7) — Manual Pins (`App` / `Group` / `Personal` / `Ignore`): **PASS**

- **`FIXTURE`** ([`round5_tests.rs`](file:///Y:/Repos/y-storage/crates/y-core/tests/round5_tests.rs)):
  - `manual_pins_override_wrong_inferences_and_persist_in_config`: Exercises all four `PinKind` variants (`Ignore`, `App`, `Group`, `Personal`) overriding automatic inference (`D:\Apps\NotAnApp` -> `Unattributed`, `D:\Apps\SuiteAsOne` -> single app with `AttributionBasis::ManualPin`, `D:\WrapPinGroup` -> `Group` split promoting `Split Child App`, `D:\Games\HomeClip` -> `Your files`) and verifies TOML atomic save + reload round-trip.
  - [a-settings-zone-rulebook-dark.png](file:///Y:/Repos/y-storage/specs/022-attribution-engine-2/a-settings-zone-rulebook-dark.png) shows `D:\Games\OrphanMod (Pin: Ignore)` and pin controls (`App`, `Group`, `Personal`, `Ignore`, `Pin folder`).
- **`LIVE`**:
  - `%APPDATA%\y-storage\config.toml` preserves empty `pins = []` cleanly alongside `[zones]` defaults and `D:\Media`; pin persistence and inference override behavior on arbitrary paths are verified in **`FIXTURE`** tests above.

---

## Live Scan Before/After Summary (`LIVE`)

| Metric | Before (`before_ae2_4drive.json`) | After (`after_ae2_4drive.json`) | Status |
|---|---|---|---|
| Attributed apps (4-drive) | `298` | `299` | PASS |
| Unattributed items (4-drive) | `317` | `313` | PASS |
| `D:\Media` in `Unattributed` | `153,011,056,894 B` | `0 B` | PASS |
| `D:\Media` in `Your files` | `0 B` | **`153,011,056,894 B`** | PASS |
| `EA SPORTS™ FIFA 23` | `107,199,710,486 B` | **`107,199,710,486 B`** | PASS |
| `Store: MSTeams` | `279,428,566 B` | **`279,428,566 B`** | PASS |
| `D:\Games\Epic\SidMeiersCivilizationVI` | `Epic` (`19,670,833,651 B`) | **`Sid Meier's Civilization VI`** (`19,670,833,651 B`) | PASS |
| `Y:\Apps\ArtCraft` (`PublisherProduct`) | `ArtCraft` (`545,911,230 B`) | `PhotoCraft` (`295.4 MB`) + `FilmCraft` (`224.0 MB`) + `ArtCraft Launcher` (`26.5 MB`) | PASS |
| Literal `[files]` / `[root files]` tags | `23` | **`0`** (`4drive`) / **`0`** (`c_only`) | PASS |
| Reconciliation `diff` (`C:`, `D:`, `Y:`, `Z:`) | `0 B` each | **`0 B` each** (`C:` `c_only` also `0 B`) | PASS |


---

## App-Count Bridge (`270 → 269` Default `C:`-Only Smoke & `270 → 272 → 298 → 299` 4-Drive)

### Why `--smoke-gui-ms 500` Reports `269` Instead of Including Secondary-Drive Promotions

- `--smoke-gui-ms 500` loads `%LOCALAPPDATA%\y-storage\last_scan.json`, which reflects the user's persisted **`C:`-only default scope** (`C:` `enabled = true`; `D:`, `Y:`, `Z:` `enabled = false`).
- Under `drive-scope-coverage-1` FIX 7(b) (*Scope Honesty*), excluded drives (`D:`, `Y:`, `Z:`) are never walked or sized, so secondary-drive-only apps (**`DBSZ v3022.021.003.012.015`** on `D:\`, **`DS Awakening v1.1.1`** on `Z:\`, **`Lossless Scaling v3.2.2`** on `Z:\`, **`Sid Meier's Civilization VI`** on `D:\`) appear in the **4-drive scan (`299` apps)** and are intentionally omitted from the **`C:`-only scan (`269` apps)**.

### Exact Row-by-Row Bridge on Default `C:`-Only Scope (`270 → 267 → 269`)

| Stage / Transition | Delta | Exact Apps & Reason |
|---|---:|---|
| Pre-round `C:` baseline (`ad61871`, `substring-matcher-fix`) | `270` | Registered `InstallLocation` folders on `D:` / `Z:` were still sized and listed before FIX 7(b). |
| Disappeared in `d348e97` (`drive-scope-coverage-1` FIX 7(b) Scope Honesty) | **`-3`** | **`CPM 2`** (`D:\Games\Steam\steamapps\common\CPM2`, `5,456,917,328 B` — `0 B` on `C:`) · **`EA SPORTS™ FIFA 23`** (`D:\Games\Steam\steamapps\common\FIFA 23`, `51,372,195,955 B` pre-hardlink / `107,199,710,486 B` walked — `0 B` on `C:`) · **`Slay the Spire 2`** (`Z:\Games\Steam\steamapps\common\Slay the Spire 2`, `3,168,269,485 B` — `0 B` on `C:`). Omitted because `D:` and `Z:` are excluded. |
| Post-`drive-scope-coverage-1` `C:`-only (`d348e97`) | `267` | Verified in `smoke_fix789.json` (`apps_count = 267`). |
| Appeared in `859ee90` (`attribution-engine-2` Part B(5) executable inference) | **`+2`** | **`LightingService`** (`C:\Program Files (x86)\LightingService`, `14,227,636 B`, `ExecutableScan`) · **`NVIDIA Container`** (`C:\Program Files\NVIDIA Corporation\NvContainer`, `2,960,096 B`, `ExecutableScan` under group node `C:\Program Files\NVIDIA Corporation`). Both promoted from `C:\` `Unattributed`. |
| Merged / Renamed on `C:` | `0` | None. |
| **Post-`attribution-engine-2` `C:`-only / `--smoke-gui-ms 500` (`859ee90`)** | **`269`** | **`270 − 3 + 2 = 269` apps** (**0 unexplained delta**). |

### Exact Row-by-Row Bridge on Full 4-Drive Scope (`270 → 272 → 298 → 299`)

1. **`270` (`ad61871` `C:`-only) → `272` (pre-FIX-6 4-drive baseline, `+2` rows walked on `D:\`)**:
   - `+` **`Grand Theft Auto V Enhanced`** (`D:\Games\Grand Theft Auto V Enhanced`) — matched when `D:\` was walked.
   - `+` **`My dream setup`** (`D:\Games\Steam\steamapps\common\My dream setup`) — matched when `D:\` was walked.

2. **`272` → `298` (`d348e97`, `drive-scope-coverage-1` FIX 6, `+26` rows promoted on `D:\` / `Y:\` / `Z:\`, `0` removed)**:
   - **Appeared via `ContainerRule` (`+24`)**: `AdeptiScanner` (`Z:\Softwares\AdeptiScanner`), `ArtCraft` (`Y:\Apps\ArtCraft`), `Aseprite v1.3` (`D:\Softwares\Aseprite.v1.3`), `Aseprite v1.3.5-dev` (`Z:\Softwares\Aseprite.v1.3.5-dev`), `BCUninstaller 6.3.0 net8.0-windows10.0.18362.0` (`Z:\Softwares\BCUninstaller_6.3.0_net8.0-windows10.0.18362.0`), `BetterJoy v7.1` (`Z:\Softwares\BetterJoy_v7.1`), `Blockbench` (`D:\Softwares\Blockbench`), **`DBSZ v3022.021.003.012.015`** (`D:\Games\DBSZ.v3022.021.003.012.015_LinkNeverDie.Com`), **`DS Awakening v1.1.1`** (`Z:\Games\DS.Awakening.v1.1.1_LinkNeverDie.Com`), **`Epic`** (`D:\Games\Epic`), `FIFA Mod Manager v2.2.2` (`D:\Games\FIFA Mod Manager v2.2.2`), `FurMark win64` (`Z:\Softwares\FurMark_win64`), `GHelper` (`Z:\Softwares\GHelper`), `Genshin Tools` (`Z:\Games\Genshin Tools`), **`Lossless Scaling v3.2.2`** (`Z:\Games\Lossless.Scaling.v3.2.2_LinkNeverDie.Com`), `Minecraft` (`D:\Games\Minecraft`), `PortableGit` (`Y:\Apps\PortableGit`), `Rust` (`Y:\Apps\Rust`), `TranslucentTB-portable-x64` (`Y:\Apps\TranslucentTB-portable-x64`), `aida64extreme840` (`Z:\Softwares\aida64extreme840`), `cpu-z 3.01-en` (`Z:\Softwares\cpu-z_3.01-en`), `nodejs` (`Y:\Apps\nodejs`), `y-storage` (`Y:\Apps\y-storage`), `z-controller` (`Y:\Apps\z-controller`).
   - **Appeared via Steam `appmanifest_*.acf` (`+2`)**: `Spacewar` (`Z:\Games\Steam\steamapps\common\Spacewar`), `Steamworks Common Redistributables` (`Z:\Games\Steam\steamapps\common\Steamworks Shared`).
   - **Subtotal**: **`272 + 24 + 2 = 298` apps**.

3. **`298` (`before_ae2_4drive.json`, `d348e97`) → `299` (`after_ae2_4drive.json`, `859ee90`, net `+1` row)**:
   - **Replaced 1-for-1 (`-1` / `+1`, net `0`)**:
     - `-` **`Epic`** (`D:\Games\Epic`, `19,670,833,651 B`, `ContainerRule`) → `+` **`Sid Meier's Civilization VI`** (`D:\Games\Epic\SidMeiersCivilizationVI`, `19,670,833,651 B`, Epic `.item` manifest; `Civilization VI : Aztec DLC` excluded).
   - **Merged into existing rows (`-1` row)**:
     - `-` **`ArtCraft`** (`Y:\Apps\ArtCraft`, `545,911,230 B`, `ContainerRule`) — publisher folder split and **merged into 3 already-present app rows** that previously held only `C:\Users\Win\AppData\...` folders: **`PhotoCraft`** (`334,175 B` → `295,710,886 B`), **`FilmCraft`** (`3,995 B` → `224,034,289 B`), and **`ArtCraft Launcher`** (`1,964,985 B` → `28,469,210 B` via `PublisherProduct`).
   - **Appeared (`+2` rows)**:
     - `+` **`LightingService`** (`C:\Program Files (x86)\LightingService`, `14,227,636 B`, `ExecutableScan`) — promoted from `Unattributed`.
     - `+` **`NVIDIA Container`** (`C:\Program Files\NVIDIA Corporation\NvContainer`, `2,960,096 B`, `ExecutableScan`) — promoted from `Unattributed`.
   - **Reconciliation**: **`298 − 1 + 1 − 1 + 2 = 299` apps** (**0 unexplained delta**).
