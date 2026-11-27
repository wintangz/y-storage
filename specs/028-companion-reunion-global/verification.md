# Verification 028 — `companion-reunion-global`

## 1. Per-Part Result Summary
- **Part 1 — Component-Folder Reunion Pass**: **PASS**
- **Part 2 — Global Category Coverage (`data/app_categories.toml`)**: **PASS**

## 2. Test Suite (`cargo test --workspace`)
Ran `cargo test --workspace` after the final code/catalog edit with **0 compiler warnings**:
- `y-core` (`lib.rs` unit tests): **3 / 3 passed**
- `y-core` (`core_tests`): **6 / 6 passed**
- `y-core` (`round2_tests`): **6 / 6 passed**
- `y-core` (`round3_tests`): **8 / 8 passed**
- `y-core` (`round4_tests`): **5 / 5 passed**
- `y-core` (`round5_tests`): **11 / 11 passed**
- `y-core` (`round6_tests`): **9 / 9 passed**
- `y-core` (`round7_tests`): **7 / 7 passed**
- `y-core` (`round8_tests`, fixtures F1–F7 + Tier 1 contradiction veto): **8 / 8 passed**
- `y-platform-windows` (`lib.rs` + `windows_scanner_tests`): **4 / 4 passed**
- `y-storage` (`main.rs` unit tests): **41 / 41 passed**
- **Total**: **108 / 108 passed**, **0 failed**, **0 warnings**.

### Fixtures `(F1)–(F7)` (`crates/y-core/tests/round8_tests.rs`)
- `(F1)` `f1_registered_acme_player_reunites_acmeplayer9box_via_tier2`: **PASS** — `Acme Player 14` + `acmeplayer9box` reunited into 1 row (`2,250,000,000 B`) via Tier 2 stem+component.
- `(F2)` `f2_acmeplayer9box_without_registered_app_stays_standalone`: **PASS** — stays standalone (`acmeplayer9box`, `ContainerRule`).
- `(F3)` `f3_msteams_never_reunites_with_registered_steam`: **PASS** — `msteams` stays standalone; never absorbed into `Steam`.
- `(F4)` `f4_registered_steamvr_not_stolen_by_steam`: **PASS** — registered `SteamVR` keeps `900,000,000 B`; orphan `SteamVR` also rejected by `Steam` Tier 2 (`"vr"` not in generic component set).
- `(F5)` `f5_ambiguous_shared_stem_foo_player_and_pro_refuses_reunion`: **PASS** — `Foo Player` + `Foo Player Pro` + `foobox` produces 3 separate rows (`foobox` refused reunion).
- `(F6)` `f6_unknown_super_emulator_deluxe_categorises_to_games_via_keyword_rule`: **PASS** — classified as `Games` via generic `[[rules]]` (`rule: Game emulator or virtual machine class keyword`).
- `(F7)` `f7_tier1_company_name_reunites_zxvm_with_quux_player_name_independently`: **PASS** — synthetic PE64 `VS_VERSION_INFO` parsed cleanly; `zxvm` reunited into `Quux Player` (`1,550,000,000 B`) via Tier 1 PE `CompanyName` with zero shared stem.
- Bonus guard `tier1_contradiction_vetoes_tier2_stem_match`: **PASS** — contradicting `CompanyName` / `ProductName` vetoes Tier 2 stem match.

## 3. Live Machine Before/After Rescan (`--ephemeral --all-drives`)
Both rescans (`BEFORE` using previous `a09ad32` binary, `AFTER` using new release binary) ran sequentially against the live filesystem:

### 3.1 Baseline Invariants
- `EA SPORTS™ FIFA 23` (`store-minecraft-icon-fix` after-total baseline: `107,199,710,486 B`):
  - Before (`a09ad32`): **`107,199,710,486 B`**
  - After (`companion-reunion-global`): **`107,199,710,486 B`** (0 B delta — byte-identical)
- `Store: MSTeams`:
  - Before: **`279,428,566 B`**
  - After: **`279,428,566 B`** (0 B delta — byte-identical)

### 3.2 Drive Reconciliation (After All-Drives Scan)
Formula per drive: `apps + shared_caches + your_files + unattributed + windows_system.total_bytes - windows_system.hardlink_overlap_bytes == drive.used_bytes` (`diff == 0 B`):

| Drive | Used (B) | Apps (B) | Shared Caches (B) | Your Files (B) | Unattributed (B) | System (B) | Hardlink Overlap (B) | Diff |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `C:` | 147,024,306,176 | 86,513,293,689 | 11,715,479,249 | 3,212,439,286 | 1,473,078,341 | 44,110,015,611 | 0 | **0 B** |
| `D:` | 385,983,807,488 | 285,703,667,477 | 0 | 257,273 | 154,519,504,715 | 23,641,633 | 54,263,263,610 | **0 B** |
| `Y:` | 53,079,957,504 | 22,339,962,838 | 0 | 0 | 32,834,943,915 | 129 | 2,094,949,378 | **0 B** |
| `Z:` | 202,794,860,544 | 201,325,879,375 | 0 | 0 | 16,527,521 | 1,452,453,648 | 0 | **0 B** |

### 3.3 App-Count Bridge (`297 -> 296`, delta `-1`)
- **Removed row (`-1`)**: `container:y:\apps\nodejs` (`nodejs`, `3,990,653,494 B`, `Y:\Apps\nodejs`)
- **Reunited onto**: `reg:node-js` (`Node.js`, `Development`):
  - Before: `130,025,934 B` (`npm` + `pnpm`)
  - After: **`4,120,679,428 B`** (`npm` + `pnpm` + `Y:\Apps\nodejs`)
  - Basis / detail: `CompanionReunion` — `"companion reunion (Tier 1: PE ProductName) -> Node.js"`
- **Added rows**: `0` (zero false splits or unintended reunions across all 296 apps).

### 3.4 Part 2 Category Coverage Delta
- **Before**: **`56 / 297`** `Uncategorized`
- **After**: **`30 / 296`** `Uncategorized` (`-26` uncategorized rows: 1 reunited into categorized `Node.js`, 25 newly categorized)

#### Newly Categorized / Reunited From Before List (26 rows):
- `nodejs` (`container:y:\apps\nodejs`) -> reunited into `Node.js` (`Development`)
- `Rust` (`container:y:\apps\rust`) -> `Development`
- `Python 3.15.1 (64-bit)` (`reg:python-x64-3.15.1`) -> `Development`
- `PortableGit` (`container:y:\apps\portablegit`) -> `Development`
- `SubtitleEdit` (`exe:d:\media\subtitleedit`) -> `Media & Creative`
- `Adobe (shared data)` (`app:adobe-shared-data`) -> `Media & Creative`
- `Aseprite v1.3.5-dev` (`container:z:\softwares\aseprite.v1.3.5-dev`) -> `Media & Creative`
- `Aseprite v1.3` (`container:d:\softwares\aseprite.v1.3`) -> `Media & Creative`
- `Blockbench` (`container:d:\softwares\blockbench`) -> `Media & Creative`
- `aida64extreme840` (`container:z:\softwares\aida64extreme840`) -> `Utilities`
- `FurMark win64` (`container:z:\softwares\furmark_win64`) -> `Utilities`
- `AdeptiScanner` (`container:z:\softwares\adeptiscanner`) -> `Utilities`
- `BCUninstaller 6.3.0 net8.0-windows10.0.18362.0` (`container:z:\softwares\bcuninstaller_6.3.0_net8.0-windows10.0.18362.0`) -> `Utilities`
- `z-controller` (`container:y:\apps\z-controller`) -> `Utilities`
- `y-storage` (`container:y:\apps\y-storage`) -> `Utilities`
- `cpu-z 3.01-en` (`container:z:\softwares\cpu-z_3.01-en`) -> `Utilities`
- `BetterJoy v7.1` (`container:z:\softwares\betterjoy_v7.1`) -> `Utilities`
- `GHelper` (`container:z:\softwares\ghelper`) -> `Utilities`
- `TranslucentTB-portable-x64` (`container:y:\apps\translucenttb-portable-x64`) -> `Utilities`
- `NVIDIA Container` (`exe:c:\program files\nvidia corporation\nvcontainer`) -> `System & Runtimes`
- `Store: Microsoft.People` -> `Communication`
- `Store: Microsoft.Getstarted` -> `Productivity`
- `Store: Microsoft.Wallet` -> `Productivity`
- `Store: MicrosoftCorporationII.MicrosoftFamily` -> `Productivity`
- `Store: Microsoft.MicrosoftEdge.Stable` -> `Browsers`
- `Store: Microsoft.MicrosoftEdgeDevToolsClient` -> `Development`

#### Remaining `Uncategorized` After (`30 / 296` — opaque Store hashes/IDs, internal Store plumbing, dot-folders):
`Store: Microsoft.4297127D64EC6`, `Store: Microsoft.LanguageExperiencePackvi-VN`, `Store: Microsoft.StorePurchaseApp`, `Store: Microsoft.XboxIdentityProvider`, `Store: Microsoft.Ink.Handwriting.en-US.1.0`, `Store: Microsoft.StartExperiencesApp`, `Store: Microsoft.Xbox.TCUI`, `Store: Microsoft.549981C3F5F10`, `Store: DeletedAllUserPackages`, `Store: Microsoft.Services.Store.Engagement`, `Store: Microsoft.XboxSpeechToTextOverlay`, `Store: cr.sb.odmf5200eafd3ad904629cbb0f87a78a3c7211081fe`, `Store: cr.sb.odm3E4D1A088C1F6D498C84F3C86DE73CE49F82A104`, `Store: AdvancedMicroDevicesInc-RSXCM`, `Store: cr.sb.odm8c3a99bdfb73ce9abf2ba64480175027ae542cad`, `Store: cr.sb.odmeecc7f22c796e6e2ec0afe58069ea1e322a97c81`, `Store: Resources`, `Store: cr.sb.cdm3e4d1a088c1f6d498c84f3c86de73ce49f82a104`, `Store: cr.sb.cdmf5200eafd3ad904629cbb0f87a78a3c7211081fe`, `Store: Microsoft.Ink.Handwriting.Main.en-US.1.0.1`, `Store: cr.sb.odmDD263C8BF30B78A1DCF989083D8CA0AA7035944F`, `.local`, `Store: ASUSAmbientHAL64`, `Store: MdOdrMcpFilterPackage`, `Store: Microsoft.AsyncTextService`, `Store: Microsoft.ECApp`, `Store: Microsoft.Win32WebViewHost`, `Store: Microsoft.XboxGameCallableUI`, `.agents`, `.impeccable`.

## 4. Release Build & Deployment
- Release build (`cargo build --release --bin y-storage`): **0 warnings**.
- Installed to `Y:\Apps\y-storage\y-storage.exe`, SHA-256 verified (`D31C3783D3B07789BF3869473E6D6A75A09C9C3DB8577733518DD29792D755A4`).
- `%APPDATA%\y-storage\last_scan.json` refreshed cleanly (`exit 0`).
- `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500`: **`ExitCode = 0`**.
