# Verification Report: `routing-cleanup-2`

**Baseline Commit**: `06770888e8353525d71b666a75f809f265428da8` (`store-minecraft-icon-fix`)

## Per-Part Verdict

| Part | Summary | Status | Proving Evidence |
|---|---|---|---|
| **Part E** | Adobe shared containers split + `"Adobe (shared data)"` pool | **PASS** | `app:adobe-creative-cloud` (`384,033,853 B`) removed; all `6` live Adobe shared/dunamis folders (`384,033,853 B`) moved onto `app:adobe-shared-data` (`"Adobe (shared data)"`, `"Adobe Inc."`); fixtures `e1`–`e3` passed. |
| **Part F** | Evidence-backed NO-TOUCH `"Service"` | **PASS** | Wildcard `ends_with(" service")` / ASUS wildcard removed; only **`LightingService`** (`exe:c:\program files (x86)\lightingservice`) changed (`None -> Service`, backed by `LightingService.exe` service registration); all `7` prior service apps kept `Service`; zero user apps flipped; fixtures `f1`–`f3` passed. |
| **Part G(1)** | System package GUID prefix audit & docs | **PASS** | All `4` prefixes identified from live `InboxApplications` + `C:\Windows\SystemApps\*\AppxManifest.xml` and documented in `attribution.rs`; zero behavior change. |
| **Part G(2)** | `.NET` MSI-major -> SDK audit, docs & proven fix | **PASS** | Encoding basis documented; audit caught `4` `v81.200.61678` `Manifest-10.0.100` MSIs (`2,564,096 B`) mis-routed to `.NET SDK 7.0.407`, now fixed onto `.NET SDK 10.0.403` (`1,131,674,312 -> 1,129,110,216 B` and `219,699,704 -> 222,263,800 B`); fixture `g2` passed. |

---

## Automated Test Suite (`cargo test --workspace`)

- **`y-core`**: **`55` passed, `0` failed**
  - Unit (`lib.rs`): `3` passed (`config`, `dependencies` ×2)
  - `core_tests`: `6` passed
  - `round2_tests`: `6` passed
  - `round3_tests`: `8` passed
  - `round4_tests`: `5` passed
  - `round5_tests`: `11` passed
  - `round6_tests`: `9` passed
  - `round7_tests`: `7` passed (`e1`, `e2`, `e3`, `f1`, `f2`, `f3`, `g2`)
- **`y-platform-windows`**: **`4` passed, `0` failed** (`1` unit + `3` integration)
- **`y-storage`**: **`41` passed, `0` failed**
- **Workspace Total**: **`100` passed, `0` failed, `0` compiler warnings**.

---

## Live All-Drives Rescan & Acceptance Battery (`C:, D:, Y:, Z:`)

### 1. Invariants & Drive Reconciliation

| Metric | `0677088` End Baseline | Before Scan (`rc2_before`) | After Scan (`rc2_after`) | Verdict |
|---|---|---|---|---|
| `EA SPORTS™ FIFA 23` (`reg:ea-sports-fifa`) | `107,199,710,486 B` | `107,199,710,486 B` | **`107,199,710,486 B`** | **Byte-identical (`0 B` delta)** |
| `Store: MSTeams` (`store:msteams_8wekyb3d8bbwe`) | `279,428,566 B` | `279,428,566 B` | **`279,428,566 B`** | **Byte-identical (`0 B` delta)** |
| `C:\` Reconciliation | `diff 0 B` | `diff 0 B` | **`diff 0 B`** (`248,232,751,104 B`) | **PASS** |
| `D:\` Reconciliation | `diff 0 B` | `diff 0 B` | **`diff 0 B`** (`1,000,168,484,864 B`) | **PASS** |
| `Y:\` Reconciliation | `diff 0 B` | `diff 0 B` | **`diff 0 B`** (`256,059,109,376 B`) | **PASS** |
| `Z:\` Reconciliation | `diff 0 B` | `diff 0 B` | **`diff 0 B`** (`262,142,947,328 B`) | **PASS** |

Exact per-drive reconciliation breakdown (`rc2_after_all_drives`):
- `C:\`: `apps 86,513,878,299 + caches 11,715,481,264 + your 3,212,375,779 + unattrib 1,471,335,593 + win_sys 44,479,203,497 - overlap 0 + free 100,840,476,672 = 248,232,751,104 B (diff 0 B)`
- `D:\`: `apps 285,703,667,477 + caches 0 + your 257,273 + unattrib 154,519,504,715 + win_sys 23,641,633 - overlap 54,263,263,610 + free 614,184,677,376 = 1,000,168,484,864 B (diff 0 B)`
- `Y:\`: `apps 22,339,949,014 + caches 0 + your 0 + unattrib 32,776,238,227 + win_sys 129 - overlap 2,087,368,426 + free 203,030,290,432 = 256,059,109,376 B (diff 0 B)`
- `Z:\`: `apps 201,325,879,375 + caches 0 + your 0 + unattrib 16,527,521 + win_sys 1,452,453,648 - overlap 0 + free 59,348,086,784 = 262,142,947,328 B (diff 0 B)`

### 2. Part E — Adobe Pool Bridge on Owner's Machine

Synthetic `"Adobe Creative Cloud"` (`app:adobe-creative-cloud`) existed in the baseline report (`384,033,853 B` across `6` folders, with `0` installed Adobe product apps and `0` registered `"Adobe Creative Cloud"` apps). After Part E, every byte moved `1`-for-`1` to `"Adobe (shared data)"` (`app:adobe-shared-data`, publisher `Adobe Inc.`):

| Location | Bytes | Before Owner | After Owner |
|---|---|---|---|
| `C:\Program Files (x86)\Common Files\Adobe` | `152,212,941 B` | `Adobe Creative Cloud` (`app:adobe-creative-cloud`) | `Adobe (shared data)` (`app:adobe-shared-data`) |
| `C:\Users\Win\AppData\Roaming\Adobe` | `148,754,220 B` | `Adobe Creative Cloud` (`app:adobe-creative-cloud`) | `Adobe (shared data)` (`app:adobe-shared-data`) |
| `C:\Users\Win\AppData\Roaming\com.adobe.dunamis` | `42,064,409 B` | `Adobe Creative Cloud` (`app:adobe-creative-cloud`) | `Adobe (shared data)` (`app:adobe-shared-data`) |
| `C:\Program Files\Common Files\Adobe` | `29,586,488 B` | `Adobe Creative Cloud` (`app:adobe-creative-cloud`) | `Adobe (shared data)` (`app:adobe-shared-data`) |
| `C:\Users\Win\AppData\Local\Adobe` | `11,391,219 B` | `Adobe Creative Cloud` (`app:adobe-creative-cloud`) | `Adobe (shared data)` (`app:adobe-shared-data`) |
| `C:\Users\Win\AppData\LocalLow\Adobe` | `24,576 B` | `Adobe Creative Cloud` (`app:adobe-creative-cloud`) | `Adobe (shared data)` (`app:adobe-shared-data`) |
| **Total** | **`384,033,853 B`** | `Adobe Creative Cloud` | **`Adobe (shared data)`** |

### 3. Part F — NO-TOUCH Status Changes

- **Changed Apps (`1`)**:
  - `LightingService` (`exe:c:\program files (x86)\lightingservice`): **`None -> Service`** (`NoTouchCategory::Service`), backed by `HKLM\SYSTEM\CurrentControlSet\Services\LightingService` (`"C:\Program Files (x86)\LightingService\LightingService.exe"`, `type = 16`).
- **Unchanged Existing Non-Store Service Apps (`7`)**: `ASUS Framework Service`, `AURA Service`, `AURA lighting effect add-on x64`, `Armoury Crate Service`, `GameSDK Service`, `Microsoft Update Health Tools`, `Windows Defender`.
- **Unchanged Desktop GUI Apps**: `TeamViewer`, `Tailscale`, `Google Chrome`, `Microsoft Edge`, `EA app`, `Epic Games Launcher`, `Rockstar Games Launcher`, `Logitech G HUB` remain `no_touch = None`.

### 4. Part G — Audit Results

#### G(1) System-Package GUID Prefixes

| Prefix | Package Family Found (`AppxAllUserStore\InboxApplications` / `SystemApps`) | Manifest `<Identity Name>` | Behavior Change |
|---|---|---|---|
| `1527c705-` | `Microsoft.Windows.FilePicker_cw5n1h2txyewy` | `1527c705-839a-4832-9118-54d4Bd6a0c89` | None |
| `c5e2524a-` | `Microsoft.Windows.FileExplorer_cw5n1h2txyewy` | `c5e2524a-ea46-4f67-841f-6a9465d9d515` | None |
| `e2a4f912-` | `Microsoft.Windows.AppResolverUX_cw5n1h2txyewy` | `E2A4F912-2574-4A75-9BB0-0D023378592B` | None |
| `f46d4000-` | `Microsoft.Windows.AddSuggestedFoldersToLibraryDialog_cw5n1h2txyewy` | `F46D4000-FD22-4DB4-AC8E-4E1DDDE828FE` | None |

#### G(2) Installed `.NET` Entries & Package Cache Audit (Separate Fix Delta Reported)

| Installed `.NET` Entry | Before Bytes | After Bytes | Delta & Audit Finding |
|---|---|---|---|
| `Microsoft .NET SDK 7.0.407 (x64)` (`reg:dotnet-sdk-x64-7.0.407`) | `1,131,674,312 B` | **`1,129,110,216 B`** | **`-2,564,096 B`**: removed `4` mis-routed `v81.200.61678` `.NET 10.0.100` workload manifests (`{5EE43FB0-...}` `638,976 B`, `{99B1B215-...}` `643,072 B`, `{D411A281-...}` `638,976 B`, `{DF1E9467-...}` `643,072 B`); retains `v28.*` (`7.0.407` Toolset/Templates), `v56.*` (`7.0.17` Runtime/Host/Workloads), and `v7/12/15/16/33` `Manifest-7.0.100` + `v48.104.7000` (`.NET Host 6.0.26`, fallback when no `.NET 6 SDK` exists). |
| `Microsoft .NET SDK 10.0.403 (x64)` (`reg:dotnet-sdk-x64-10.0.403`) | `219,699,704 B` | **`222,263,800 B`** | **`+2,564,096 B`**: gained the `4` `v81.200.61678` `Manifest-10.0.100` entries above (`net6`/`net7` workload packs targeting `.NET SDK 10.0.100`); retains `v40.*` (`10.0.403` Toolset/Templates), `v80.*` (`10.0.14` Runtime/ASP.NET/Host/Targeting), `v81.200.*`, and `Manifest-10.0.100`. |
| `Microsoft Windows Desktop Runtime 10.0.14 (x64)` | `31,061,202 B` | `31,061,202 B` | `0 B` — Verified (`v80.56.61678` + bundle `{29A18B06-...}`). |
| `Microsoft Windows Desktop Runtime - 8.0.33 (x64)` | `30,933,688 B` | `30,933,688 B` | `0 B` — Verified (`v64.132.61678` + bundle `{4d0027d4-...}`). |
| `Microsoft .NET Runtime - 8.0.21 (x64)` | `28,943,170 B` | `28,943,170 B` | `0 B` — Verified (`v64.84.40925` + `{B382D796-...}` + bundle `{cf83f56a-...}`). |
| Store `.NET.Native` Frameworks/Runtimes (`1.7`, `2.1`, `2.2`) | `64,156,441 B` | `64,156,441 B` | `0 B` across all `6` Store `.NET.Native` rows. |

### 5. App-Count Bridge (`297 -> 297`, Net `0`)

- `-1`: `app:adobe-creative-cloud` (`"Adobe Creative Cloud"`, `384,033,853 B`) removed (Part E)
- `+1`: `app:adobe-shared-data` (`"Adobe (shared data)"`, `384,033,853 B`) added (Part E)
- Net delta: **`0`** (**`297 -> 297`**, `0` unexplained rows; icon coverage `225 -> 225`).
