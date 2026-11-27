# Verification 030 — `evidence-pipeline-1`

**Scope:** Graded evidence step in the claim pipeline (`crates/y-core` + `crates/y-platform-windows` + `apps/y-storage` literal updates), positioned after registered install roots + manifest containers and **before** the curated routing table, backed by a `path+size+mtime` executable evidence cache (`%LOCALAPPDATA%\y-storage\evidence_cache.json`).

---

## 1. Automated Test Suite (`cargo test --workspace`)

Ran `cargo test --workspace` after the final code edit with **0 compiler warnings** (`EXIT=0`):

| Crate / Fixture Group | Passed | Failed | Ignored | Notes |
|---|---:|---:|---:|---|
| `y-core` (`src/lib.rs` unit) | 3 | 0 | 0 | `config`, `dependencies` ×2 |
| `y-core` (`core_tests`) | 6 | 0 | 0 | Core attribution, reconciliation, cache |
| `y-core` (`round2_tests`) | 6 | 0 | 0 | PE imports, manifest parsers, categories |
| `y-core` (`round3_tests`) | 8 | 0 | 0 | Token rules, xdata, MSTeams/Steam guard |
| `y-core` (`round4_tests`) | 5 | 0 | 0 | Steam VDF, container rules, rescan merge |
| `y-core` (`round5_tests`) | 11 | 0 | 0 | Personal zones, pins, carve-outs |
| `y-core` (`round6_tests`) | 9 | 0 | 0 | Store `_bak`, Minecraft split, EA unpin |
| `y-core` (`round7_tests`) | 7 | 0 | 0 | Adobe shared pool, service no-touch, .NET |
| `y-core` (`round8_tests`) | 8 | 0 | 0 | Companion reunion F1–F7 + Tier 1 veto |
| `y-core` (`round9_evidence_tests`) | 11 | 0 | 0 | **E1–E8** + bonus **E9–E11** |
| `y-platform-windows` (`src/lib.rs` unit) | 1 | 0 | 0 | Cross-drive icon extraction |
| `y-platform-windows` (`spike_runner`) | 0 | 0 | 0 | Binary smoke target |
| `y-platform-windows` (`evidence_platform_tests`) | 5 | 0 | 0 | Authenticode, PE version, App Paths, `.lnk`, services |
| `y-platform-windows` (`windows_scanner_tests`) | 3 | 0 | 0 | Scanner exclusions & junctions |
| `y-storage` (`src/main.rs` unit) | 41 | 0 | 0 | UI, theme, rasterizer, queries, host |
| **Workspace Total** | **124** | **0** | **0** | **0 compiler warnings** |

### Fixtures E1–E8 (+ Bonus E9–E11) (`crates/y-core/tests/round9_evidence_tests.rs`)
- **E1** `e1_signer_and_product_claims_unrelated_name_folder`: **PASS** — Matching Authenticode signer org + token-matching `ProductName` claims `D:\z9_bin_drop` for `Quux Studio` with `AttributionBasis::Signature`.
- **E2** `e2_signer_only_with_different_product_does_not_claim`: **PASS** — Matching signer org with non-matching `ProductName` does NOT claim.
- **E3** `e3_unsigned_exe_falls_through_unchanged`: **PASS** — Unsigned executable falls through unchanged to Step 5 (`ExecutableScan`).
- **E4** `e4_shortcut_target_claims_unrelated_folder`: **PASS** — Start Menu `.lnk` target matching folder exe claims `D:\Apps\mystery_dir_a` with `AttributionBasis::Shortcut`.
- **E5** `e5_app_paths_target_claims_unrelated_folder`: **PASS** — `App Paths` entry matching folder exe claims `D:\Apps\mystery_dir_b` with `AttributionBasis::AppPaths`.
- **E6** `e6_equal_grade_conflict_between_two_apps_makes_no_claim`: **PASS** — Equal-grade STRONG shortcuts for `Alpha Tool` and `Beta Tool` produce no claim (`shared_drop` remains standalone `ContainerRule`).
- **E7** `e7_msteams_steam_and_steamvr_regression_guards`: **PASS** — `msteams` and `SteamVR` remain separate from `Steam`.
- **E8** `e8_medium_alone_does_not_claim_but_medium_plus_independent_weak_does`: **PASS** — 1 MEDIUM (`VersionResource`) alone does NOT claim `D:\Apps\unrelated_box`; MEDIUM (`VersionResource`) + independent WEAK (`Stem`) DOES claim `D:\Apps\acmeplayer9box` with `AttributionBasis::VersionResource`.
- **E9** *(bonus)* `e9_evidence_beats_curated_table_on_conflict_and_records_conflict_in_report`: **PASS** — Shortcut evidence beats curated `\appdata\local\discord` rule and records the conflict in `report.evidence_curated_conflicts`.
- **E10** *(bonus)* `e10_service_image_path_strong_and_medium_rules`: **PASS** — Service `ImagePath` STRONG (exe-path equality) claims, MEDIUM alone does not, MEDIUM + WEAK claims (`AttributionBasis::ServiceRegistration`).
- **E11** *(bonus)* `e11_exe_evidence_cache_hits_and_invalidates_on_size_or_mtime_change`: **PASS** — `ExeEvidenceCache` hits on exact `path+size+mtime` and invalidates on size or mtime change.

---

## 2. Live Machine Before / After Rescan (`--ephemeral --all-drives`)

### 2.1 Performance Comparison & Gate Verdict
| Run | Cache State | `duration_ms` | vs Warm-OS BEFORE |
|---|---|---:|---:|
| BEFORE (initial cold OS cache) | N/A | `82,019 ms` | — |
| BEFORE (warm OS cache) | N/A | `20,338 ms` | baseline |
| AFTER (cold `evidence_cache.json`) | empty -> populated | `24,339 ms` | `+19.7%` (`-70.3%` vs cold BEFORE) |
| AFTER (warm `evidence_cache.json`) | warm hit | `21,656 ms` | **`+6.5%`** (**PASS — within +10% gate**) |

Warm-cache scan duration (`21,656 ms`) is **+6.5%** vs warm-OS BEFORE (`20,338 ms`), well within the `+10%` threshold (`22,372 ms`), so Authenticode signer extraction remains enabled for all folder-only candidates and registered install roots.

### 2.2 Baseline Invariants
- **`EA SPORTS™ FIFA 23`** (`reg:ea-sports-fifa`, `specs/026` baseline = `107,199,710,486 B`):
  - Before: **`107,199,710,486 B`**
  - After (cold & warm): **`107,199,710,486 B`** (`0 B` delta — byte-identical)
- **`Store: MSTeams`** (`store:msteams_8wekyb3d8bbwe`):
  - Before: **`279,428,566 B`**
  - After (cold & warm): **`279,428,566 B`** (`0 B` delta — byte-identical)

### 2.3 Drive Reconciliation (After Warm Scan, All Drives)
Formula: `apps + shared_caches + your_files + unattributed + windows_system.total_bytes - windows_system.hardlink_overlap_bytes == drive.used_bytes`:

| Drive | Used (B) | Apps (B) | Shared Caches (B) | Your Files (B) | Unattributed (B) | System (B) | Hardlink Overlap (B) | Diff |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `C:` | 146,111,094,784 | 83,128,441,180 | 11,718,282,114 | 3,212,439,286 | 1,467,469,283 | 46,584,462,921 | 0 | **0 B** |
| `D:` | 385,983,807,488 | 285,703,667,477 | 0 | 257,273 | 154,519,504,715 | 23,641,633 | 54,263,263,610 | **0 B** |
| `Y:` | 53,756,481,536 | 22,339,992,022 | 0 | 0 | 33,509,276,730 | 129 | 2,092,787,345 | **0 B** |
| `Z:` | 202,794,860,544 | 201,325,879,375 | 0 | 0 | 16,527,521 | 1,452,453,648 | 0 | **0 B** |

### 2.4 Itemised App-Count Bridge (`296 -> 296`, net `0`)
- **Added apps**: `0`
- **Removed apps**: `0`
- **Live background disk-delta rows (owner/basis unchanged)**:
  - `.gemini` (`dot:.gemini`): `1,809,505,116 B` -> `1,818,203,354 B` (`+8,698,238 B`, active session logs)
  - `Google Chrome` (`reg:google-chrome`): `3,787,574,676 B` -> `3,787,605,427 B` (`+30,751 B`)
  - `Microsoft Edge` (`reg:microsoft-edge`): `8,060,356,743 B` -> `8,063,095,175 B` (`+2,738,432 B`)
  - `Microsoft Visual Studio Code (User)` (`reg:microsoft-visual-studio-code`): `3,155,388,233 B` -> `3,157,143,899 B` (`+1,755,666 B`)

---

## 3. EVIDENCE-CHANGE Report

### 3.1 Owner / Basis Changes (`13` paths upgraded, `0` owner changes / `0` misattributions)
All 13 changes upgraded heuristic/name/curated matches to explicit Windows evidence bases on the **same** app owner:

| Path | Bytes | Owner (`id` / `display_name`) | Before Basis | After Basis & `basis_detail` |
|---|---:|---|---|---|
| `C:\Program Files (x86)\ASUS\GameSDK Service` | 7,856,044 | `reg:gamesdk-service` (`GameSDK Service`) | `ExactName` | `ServiceRegistration` — `service registration: GameSDK Service (GameSDK Service) + stem match (GameSDK Service)` |
| `C:\Program Files (x86)\Epic Games\Epic Online Services` | 1,218,573,308 | `reg:epic-online-services` (`Epic Online Services`) | `ExactName` | `ServiceRegistration` — `service registration: Epic Online Services (EpicOnlineServices) + stem match (Epic Online Services)` |
| `C:\Program Files (x86)\TP-Link\TP-Link Archer T2U Nano Driver` | 1,085,296 | `reg:tp-link-archer-t2u-nano-driver` (`TP-Link Archer T2U Nano Driver`) | `ExactName` | `Shortcut` — `shortcut: Uninstall TP-Link Archer T2U Nano Driver -> C:\Program Files (x86)\TP-Link\TP-Link Archer T2U Nano Driver\uninstall.exe` |
| `C:\Program Files\AMD\InstallUEP` | 3,262,216 | `reg:amd-software` (`AMD Software`) | `Token` | `Signature` — `signature: signer=Advanced Micro Devices + product=AMD Software (amdinstalluep.exe)` |
| `C:\Program Files\Common Files\Discord` | 2,698,318 | `reg:discord` (`Discord`) | `ExactName` | `ServiceRegistration` — `service registration: Discord System Helper (DiscordSystemHelper) + stem match (Discord)` |
| `C:\Program Files\Common Files\Microsoft Shared` | 124,854,259 | `reg:microsoft-office-ltsc-professional-plus-en-us` (`Microsoft Office LTSC Professional Plus 2021 - en-us`) | `Token` | `ServiceRegistration` — `service registration: ClickToRunSvc -> C:\Program Files\Common Files\Microsoft Shared\ClickToRun\OfficeClickToRun.exe` |
| `Y:\Apps\ArtCraft\FilmCraft` | 224,030,294 | `reg:filmcraft` (`FilmCraft`) | `ExactName` | `AppPaths` — `App Paths: filmcraft.exe -> Y:\Apps\ArtCraft\FilmCraft\filmcraft.exe` |
| `Y:\Apps\ArtCraft\Launcher` | 26,504,225 | `reg:artcraft-launcher` (`ArtCraft Launcher`) | `PublisherProduct` | `AppPaths` — `App Paths: artcraft-launcher.exe -> Y:\Apps\ArtCraft\Launcher\artcraft-launcher.exe` |
| `Y:\Apps\ArtCraft\PhotoCraft` | 295,376,711 | `reg:photocraft` (`PhotoCraft`) | `ExactName` | `AppPaths` — `App Paths: photocraft.exe -> Y:\Apps\ArtCraft\PhotoCraft\photocraft.exe` |
| `Y:\Apps\nodejs` | 3,990,653,494 | `reg:node-js` (`Node.js`) | `CompanionReunion` | `Shortcut` — `shortcut: Node.js -> Y:\Apps\nodejs\node.exe` |
| `Y:\Apps\Python 3.15` | 465,083,174 | `reg:python-x64-3.15.1` (`Python 3.15.1 (64-bit)`) | `ExactName` | `VersionResource` — `version resource: company=Python Software Foundation + product=Python (python.exe) + stem match (Python 3.15)` |
| `Y:\Apps\Tailscale` | 74,066,952 | `reg:tailscale` (`Tailscale`) | `ExactName` | `ServiceRegistration` — `service registration: Tailscale (Tailscale) + version resource: company=Tailscale Inc. + product=Tailscale (tailscale.exe)` |
| `Z:\Games\Genshin Impact` | 172,991,652,686 | `reg:genshin-impact` (`Genshin Impact`) | `ExactName` | `Signature` — `signature: signer=COGNOSPHERE PTE. LTD. + product=Genshin Impact (launcher.exe)` |

### 3.2 Evidence-vs-Curated Conflicts (`report.evidence_curated_conflicts`: `1` entry)
1. **`C:\Program Files (x86)\Epic Games\Epic Online Services`** (`1,218,573,308 B`):
   - **Evidence winner**: `reg:epic-online-services` (`Epic Online Services`), basis `ServiceRegistration` (`"service registration: Epic Online Services (EpicOnlineServices) + stem match (Epic Online Services)"`)
   - **Curated table candidate overridden**: `reg:epic-games-launcher` (`Epic Games Launcher`) via `\program files (x86)\epic games`

---

## 4. Release Build & Deployment

- Release build (`cargo build --release --bin y-storage`): **0 warnings**.
- Copied `target\release\y-storage.exe` -> `Y:\Apps\y-storage\y-storage.exe`, SHA-256 verified matching (`CBF8F7E615C6164679253E0194647EB56A78D74D48C0DEEDD50CEA853F3373C0`).
- GUI smoke test (`Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500`): **`ExitCode = 0`**.
