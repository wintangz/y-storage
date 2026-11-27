# Verification Report: `store-minecraft-icon-fix`

**Commit**: Updated on `main` from `68633bb587c8208f7c2d07949b557b804a426ee5`  
**Release Binary**: [y-storage.exe](file:///Y:/Repos/y-storage/target/release/y-storage.exe) (`SHA-256: 8A363F55787D497B8D37B2DF5122156E78EC0DEF784FBBD4B7CE05A31CF0F6E5`, `0` compiler warnings)

## Per-Part Verdict

| Part | Summary | Status | Proving Evidence |
|---|---|---|---|
| **Part A** | Store package `_bak` / backup suffix merge | **PASS** | `Microsoft.MinecraftUWP_8wekyb3d8bbwe` (`16,446 B`) + `_bak` (`62 B`) merged into 1 row `store:microsoft.minecraftuwp_8wekyb3d8bbwe`; fixtures `a1`–`a4` passed. |
| **Part B** | Minecraft split from `Xbox Service` | **PASS** | `Store: Microsoft.MinecraftUWP` = `3,280,291,293 B` (`4` locs); `Minecraft Launcher` = `4,815,441,724 B` (`3` locs); `Xbox Service` = `0 B` (`0` Minecraft paths); `C:\XboxGames\GameSave` (`0 B` `pgs`/`wgs` system dir) dropped; fixtures `b1`–`b3` passed. |
| **Part C** | Ollama / cross-drive icon restoration | **PASS** | Step (1) -> (2b) confirmed `icon_rgba: None` in `C:+Z:` report from `excluded_drives` rejection of `Y:\Apps\Ollama\unins000.exe`; general fix restored `198 -> 206` icons (`C:+Z:`) & `223 -> 225` (`All Drives`), `0` lost; fixture `c1` + 125% crops passed. |
| **Part D** | EA shared folders & `Frostbite` unpinned from games | **PASS** | Both entries target `&["ea app"]` only; live `FIFA 23` = `107,199,710,486 B` before -> `107,199,710,486 B` after (`0 B` in `C:\ProgramData\Frostbite\FIFA 23`); fixtures `d1`–`d2` passed. |

---

## Automated Test Suite (`cargo test --workspace`)

- **`y-core`**: **`48` passed, `0` failed**
  - Unit (`lib.rs`): `3` passed (`config`, `dependencies` ×2)
  - `core_tests`: `6` passed
  - `round2_tests`: `6` passed
  - `round3_tests`: `8` passed
  - `round4_tests`: `5` passed
  - `round5_tests`: `11` passed
  - `round6_tests`: `9` passed (`a1`, `a2`, `a3`, `a4`, `b1`, `b2`, `b3`, `d1`, `d2`)
- **`y-platform-windows`**: **`4` passed, `0` failed**
  - Unit (`lib.rs`): `1` passed (`icons::tests::c1_icon_populates_when_display_icon_sits_on_excluded_drive`)
  - `windows_scanner_tests`: `3` passed
- **`y-storage`**: **`41` passed, `0` failed**
- **Workspace Total**: **`93` passed, `0` failed, `0` compiler warnings**.

---

## Live Scan Acceptance Battery (Same Disk State: All Drives `C:, D:, Y:, Z:`)

### 1. Invariant Checks

| Check | Before (`68633bb`) | After (`store-minecraft-icon-fix`) | Delta / Status |
|---|---|---|---|
| `Store: MSTeams` (`store:msteams_8wekyb3d8bbwe`) | `279,428,566 B` (`2` locs) | `279,428,566 B` (`2` locs) | **Byte-identical (`0 B` diff)** |
| `EA SPORTS™ FIFA 23` (`reg:ea-sports-fifa`) | `107,199,710,486 B` (`1` loc: `D:\...\FIFA 23`) | `107,199,710,486 B` (`1` loc: `D:\...\FIFA 23`) | **Part-D live delta = `0 B`** (`C:\ProgramData\Frostbite\FIFA 23` is `0 B` on disk; EA platform folders = `13` locs on `reg:ea-app`, `2,352,460,743 B` unchanged) |
| `C:\` Reconciliation Diff | `0 B` (`248,232,751,104 B`) | `0 B` (`248,232,751,104 B`) | **`diff 0 B`** (`apps 86,555,615,811 + caches 11,715,474,225 + your 3,212,046,102 + unattrib 1,471,038,090 + win_sys 44,415,166,700 - overlap 0 + free 100,863,410,176`) |
| `D:\` Reconciliation Diff | `0 B` (`1,000,168,484,864 B`) | `0 B` (`1,000,168,484,864 B`) | **`diff 0 B`** (`apps 285,703,667,477 + caches 0 + your 257,273 + unattrib 154,519,504,715 + win_sys 23,641,633 - overlap 54,263,263,610 + free 614,184,677,376`) |
| `Y:\` Reconciliation Diff | `0 B` (`256,059,109,376 B`) | `0 B` (`256,059,109,376 B`) | **`diff 0 B`** (`apps 22,339,944,406 + caches 0 + your 0 + unattrib 32,742,544,697 + win_sys 129 - overlap 2,088,330,640 + free 203,064,950,784`) |
| `Z:\` Reconciliation Diff | `0 B` (`262,142,947,328 B`) | `0 B` (`262,142,947,328 B`) | **`diff 0 B`** (`apps 201,325,879,375 + caches 0 + your 0 + unattrib 16,527,521 + win_sys 1,452,453,648 - overlap 0 + free 59,348,086,784`) |

### 2. Minecraft & Xbox Service Breakdown (Parts A + B)

| App Row | Before (`68633bb`) | After (`store-minecraft-icon-fix`) | Locations & Attribution Basis |
|---|---|---|---|
| `Store: Microsoft.MinecraftUWP` (`store:microsoft.minecraftuwp_8wekyb3d8bbwe`) | `16,446 B` (`1` loc) | **`3,280,291,293 B`** (`3.06 GB`, `4` locs) | `C:\XboxGames\Minecraft for Windows` (`2,060,589,255 B`, `Token`, `Body`); `C:\Users\Win\AppData\Roaming\Minecraft Bedrock` (`1,219,685,530 B`, `Token`, `Red`); `...\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe` (`16,446 B`, `PackageFallback`, `Red`); `...\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe_bak` (`62 B`, `PackageFallback`, `Red`) |
| `Store: Microsoft.MinecraftUWP` duplicate `_bak` (`store:microsoft.minecraftuwp_bak`) | `62 B` (`1` loc) | **Merged (`<absent>`)** | Part A merged into `store:microsoft.minecraftuwp_8wekyb3d8bbwe` |
| `Minecraft Launcher` (`xbox:minecraft launcher`) | `<absent>` | **`4,815,441,724 B`** (`4.48 GB`, `3` locs) | `C:\Users\Win\AppData\Roaming\.minecraft` (`4,808,965,584 B`, `Token`, `Red`); `C:\XboxGames\Minecraft Launcher` (`6,476,104 B`, `ExactName`, `Body`); `C:\Users\Win\AppData\Local\MinecraftInstaller` (`36 B`, `Token`, `Red`) |
| `Xbox Service` (`reg:xbox-service`) | `8,095,716,509 B` (`5` locs) | **`0 B` (`<absent>`)** | Zero Minecraft paths remaining (all 5 moved per Part B above; total = `0 B`) |

### 3. App-Count Bridge

- **All-Drives Scan (`298 -> 297`, delta `-1`)**:
  - `-1`: `store:microsoft.minecraftuwp_bak` (`62 B`) merged into `store:microsoft.minecraftuwp_8wekyb3d8bbwe` (Part A)
  - `-1`: `reg:xbox-service` (`8,095,716,509 B`) emptied when its 5 Minecraft folders moved off `Xbox Service` (Part B)
  - `+1`: `xbox:minecraft launcher` (`4,815,441,724 B`) created for `C:\XboxGames\Minecraft Launcher` + `.minecraft` + `MinecraftInstaller` (Part B)
  - Net delta: `-1 - 1 + 1 = -1` (**`298 -> 297` reconciled with zero unexplained rows**).
- **Owner `C:+Z:` Cached Scan (`283 -> 280`, delta `-3`)**:
  - Same Part A/B `-1` (`store:microsoft.minecraftuwp_bak` `-1`, `reg:xbox-service` `-1`, `xbox:minecraft launcher` `+1`)
  - `-2` prior disk removals occurring between `2026-11-27 23:14` (`baseline_68633bb_cz_scope.json`) and `2026-11-28 02:42`: `app:cities-skylines-ii` (`107,400,938 B`) and `app:postman` (`264,102,652 B`).

---

## Part C — Icon Diagnosis, Coverage Count & 125% Visual Proof

- **Diagnosis Path**: Step `(1)` found `reg:ollama` in `%LOCALAPPDATA%\y-storage\last_scan.json` had `icon_rgba: None` (`install_location: None`, `display_icon: Some("Y:\\Apps\\Ollama\\unins000.exe")`) -> entered **Step `(2b)`**, which isolated `excluded_drives` filtering in `icons.rs` (`populate_report_icons` & `extract_app_icon_cached`) rejecting candidates on unchecked drives (`Y:`/`D:`).
- **Icon-Coverage Count Before / After**:
  - **Owner `C:+Z:` scope** (where Ollama lost its icon): **`198 -> 206`** (`0` common apps lost icons; `+7` common apps gained icons: `Ollama version 0.40.9`, `Microsoft Visual Studio Code (User)`, `Rockstar Games Launcher`, `RustDesk`, `Unity 6000.3.27f1`, `Unity Hub 3.22.5`, `Store: Microsoft.MinecraftUWP`, plus `+1` new `Minecraft Launcher`).
  - **All-drives scope (`C:, D:, Y:, Z:`)**: **`223 -> 225`** (`0` common apps lost icons; `+1` `Store: Microsoft.MinecraftUWP` via `Content\AppxManifest.xml` `<Logo>`, plus `+1` `Minecraft Launcher`).
- **125% Scaling (`--dpi 120`) Visual Proof**:
  - Before: [ollama-row-before-125.png](file:///Y:/Repos/y-storage/specs/026-store-minecraft-icon-fix/ollama-row-before-125.png) (generic wireframe placeholder)
  - After: [ollama-row-after-125.png](file:///Y:/Repos/y-storage/specs/026-store-minecraft-icon-fix/ollama-row-after-125.png) (crisp Ollama llama icon restored)
