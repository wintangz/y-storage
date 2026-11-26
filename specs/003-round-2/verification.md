# Verification Report: `y-storage` Round 2 (`dependencies`, `attribution-coverage-2`, `ground-truth`)

- **Date**: `2026-11-26 09:47:36` (local time)
- **Workspace Test Suite (`cargo test --workspace`)**: **14 passed / 0 failed / 0 warnings**
  - `y-core` (`core_tests.rs`): 5 passed
  - `y-core` (`round2_tests.rs`): 4 passed
  - `y-platform-windows` (`windows_scanner_tests.rs`): 2 passed
  - `y-storage` (`main.rs` unit tests): 3 passed
- **Release Binary Scan Speed**: `8,682 ms` (`8.68 s`) including `dependencies` binary/manifest/service analysis
- **Logged Assumptions**:
  - `A-004`: `C:\Users\Win\AppData\Roaming\flutter` is a standalone Flutter SDK git checkout (`bin\flutter.bat`, `.git`) with no Uninstall registry owner; promoted to standalone `"flutter"` (`StandaloneFolder`).
  - `A-005`: DirectX core runtime DLLs (`d3d11.dll`, `dxgi.dll`, `D3DCOMPILER_47.dll`) reside inside `C:\Windows\System32` (constitutional `Windows & system` exclusion), so `directx-runtime` reports `present = true` and `own_size_bytes = None`.

---

## 1. Acceptance 1 — `dependencies` (Forward & Reverse)

### Strict Zero Double-Counting Invariant
- Verified across all **252** attributed apps/entries in the live scan: **0** mismatches between `app.total_bytes` and `sum(item.bytes)`.
- `FIFA 23` (`EA SPORTS™ FIFA 23`) total stayed `51,372,195,955 B` (`D:\Games\Steam\steamapps\common\FIFA 23`), never inflated by `EA App` (`3,255,275,223 B`), `EA AntiCheat` (`623,869,408 B`), `.NET` (`1,442,312,076 B`), or `VC++` (`44,006,551 B`).

### `EA SPORTS™ FIFA 23` Forward Dependency List

| Target | Kind | Present | Shared | Own Size (B) | Confidence | Basis |
|---|---|---:|---:|---:|---|---|
| DirectX Runtime | `SharedRuntime` | Yes | Yes | *(OS runtime)* | `Proven` | `PE import: d3dcompiler_47.dll` |
| EA AntiCheat | `AntiCheatServiceDriver` | Yes | Yes | 623,869,408 | `Proven` | `manifest: __Installer/installerdata.xml (EAAntiCheat) + fingerprint: eaanticheat.gameservicelauncher.exe, eaanticheat.gameservicelauncher.dll + catalog: FIFA 23 requires kernel-mode EA AntiCheat + registered service: EAAntiCheatService, driver: EAAntiCheat (DependOnService: FltMgr)` |
| EA App | `PlatformLauncher` | Yes | Yes | 3,255,275,223 | `Proven` | `manifest: appmanifest_1811260.acf (EAappInstaller_installScript.vdf) + fingerprint: eaappinstaller_installscript.vdf, activation64.dll, eastore.ini + catalog: EA Sports title requires EA App client & EABackgroundService + catalog: EA Steam release bridges via EA App + registered service: EABackgroundService` |
| Microsoft .NET Runtime | `SharedRuntime` | Yes | Yes | 1,442,312,076 | `Proven` | `manifest: __Installer/installerdata.xml (.NET)` |
| Microsoft Visual C++ Runtime | `SharedRuntime` | Yes | Yes | 44,006,551 | `Proven` | `PE import: msvcp140.dll, vcruntime140.dll, vcruntime140_1.dll + manifest: __Installer/installerdata.xml (VC++ redist)` |
| Steam | `PlatformLauncher` | Yes | Yes | *(unmounted/none)* | `Proven` | `manifest: appmanifest_1811260.acf + fingerprint: steam_appid.txt + catalog: Steam edition launches through Steam client + registered service: Steam Client Service` |
| Denuvo Anti-Tamper | `AntiCheatServiceDriver` | No | Yes | — | `Inferred` | `catalog: FIFA 23 uses Denuvo Anti-Tamper DRM` |

### Reverse Dependency View Highlights
- **`EA App` (`ea-app`)**: **1 dependent app** (`binary_importer_count = 0`, `own_size = 3,255,275,223 B`):
  - `EA SPORTS™ FIFA 23` (`51,372,195,955 B`) `[Proven]`
- **`Microsoft Visual C++ Runtime` (`vcpp-runtime`)**: **83 dependent apps** total, **36 direct PE binary importers** (`own_size = 44,006,551 B`), including:
  - `EA SPORTS™ FIFA 23`, `Xbox Service`, `Microsoft Edge`, `NVIDIA App 11.0.9.585`, `Logitech G HUB`, `Microsoft Office LTSC Professional Plus 2021 - en-us`, `Genshin Impact`, `AMD Software`, `Epic Online Services`, `Store: MSTeams`, `Store: Microsoft.Windows.Photos`, `Store: B9ECED6F.ArmouryCrate`, `Rockstar Games SDK`, `RustDesk`, `RivaTuner Statistics Server 7.3.4`, `HidHide`, plus WinUI / UWP packages via `AppxManifest: Microsoft.VCLibs`.
- **`DirectX Runtime` (`directx-runtime`)**: **42 dependent apps**, **41 binary PE importers**.
- **`Steam` (`steam`)**: **3 dependent apps** (`EA SPORTS™ FIFA 23`, `CPM 2`, `Slay the Spire 2`).
- **`EA AntiCheat` (`ea-anticheat`)**: **2 dependent apps** (`EA SPORTS™ FIFA 23`, `EA app`).

---

## 2. Acceptance 2 — `attribution-coverage-2`

- **`C:\` Drive Totals**: `248,232,751,104 B` total (`148,592,439,296 B` used / `99,640,311,808 B` free).
- **Round 1 `Unattributed`**: `14,884,258,917 B` (~`10.0%` of `C:\` used).
- **Round 2 `Unattributed`**: **`6,423,481,081 B` (`6.42 GB` = `4.32%` of `C:\` used)** across `309` items (`-56.8%` reduction).
- **New `Shared & system caches (GREEN)` group**: **`11,660,796,101 B` (`11.66 GB`)** across `54` folders (`NVIDIA\DXCache`, `Local\Temp`, `D3DSCache`, `CrashDumps`, `NVIDIA\GLCache`, `Package Cache` unmatched items, etc.).
- **Promoted Standalone Profile Dot-Folders & SDK (`StandaloneFolder`)**:
  - `flutter`: `2,031,983,212 B` (`C:\Users\Win\AppData\Roaming\flutter`)
  - `.gemini`: `1,797,906,319 B` (`C:\Users\Win\.gemini`)
  - `.continue`: `18,482,096 B` (`C:\Users\Win\.continue`)
  - `.local`, `.agents`, `.impeccable`
- **New Top 5 `Unattributed`**:
  1. `C:\Users\Win\AppData\Local\Google\AndroidStudio2024.2` — `820,748,873 B` (`782.73 MiB`)
  2. `C:\Users\Win\AppData\Local\Google\AndroidStudio2023.2` — `582,578,704 B` (`555.59 MiB`)
  3. `C:\Users\Win\AppData\Roaming\TabNine` — `516,985,662 B` (`493.04 MiB`)
  4. `C:\Users\Win\AppData\Roaming\ZaloData` — `457,156,942 B` (`435.98 MiB`)
  5. `C:\Users\Win\AppData\Local\ms-playwright` — `456,676,532 B` (`435.52 MiB`)
- **Top-50 Coverage on `C:\`**:
  - Round 1 baseline: `42/50` (`84.0%`).
  - Round 2: **`47/50` (`94.0%`)** non-unattributed (`43` App/Standalone + `4` SharedCache vs `3` Unattributed); **`46/50` (`92.0%`)** among Top-50 non-cache folders.

---

## 3. Acceptance 3 — `ground-truth` (Windows Explorer Folder Properties)

Measured independently via `ShellExecuteExW(lpVerb = "properties")` + UIAutomation (`explorer.exe`) over pre-fixed canonical folder lists BEFORE executing the Round 2 CLI scan.

### Target 1: `EA App + FIFA 23` (Pre-fixed 7 folders)
| Folder | Explorer Size (B) | Explorer Size on Disk (B) |
|---|---:|---:|
| `D:\Games\Steam\steamapps\common\FIFA 23` | 51,372,195,955 | 51,372,789,760 |
| `C:\Program Files\Electronic Arts` | 1,612,387,660 | 1,615,536,128 |
| `C:\Program Files\EA` | 623,869,408 | 623,882,240 |
| `C:\ProgramData\EA Desktop` | 1,148,728 | 1,163,264 |
| `C:\Users\Win\AppData\Local\Electronic Arts` | 93,801,607 | 95,268,864 |
| `C:\Users\Win\AppData\Local\EADesktop` | 1,325,824 | 1,527,808 |
| `C:\Users\Win\AppData\Roaming\EA` | 649,142,768 | 649,150,464 |
| **Pre-fixed Ground-Truth Sum** | **54,353,871,950** | **54,359,318,528** |

- **`y-storage` Combined (`EA SPORTS™ FIFA 23` + `EA app`)**: **`54,627,471,178 B`**
- **Honest Delta**: **`+273,599,228 B` (`+0.503%`)** (well within `~5%`)
- **Cause of Non-Zero Delta**: `y-storage` also attributes 7 additional installer/cache/helper locations outside the pre-fixed 7 roots: `Package Cache\{C2622085...}v12.0.0.0` (`268,800,000 B`), `Package Cache\{c8007f09...}` (`2,365,766 B`), `Common Files\EAInstaller` (`2,141,208 B`), `Local\EALaunchHelper` (`274,112 B`), `ProgramData\Origin` (`9,398 B`), `ProgramData\Electronic Arts` (`6,370 B`), and `Local\Origin` (`2,374 B`), totaling exactly `273,599,228 B`.

### Target 2: `Discord` (Pre-fixed 2 folders)
| Folder | Explorer Size (B) | Explorer Size on Disk (B) |
|---|---:|---:|
| `C:\Users\Win\AppData\Local\Discord` | 641,872,950 | 643,788,800 |
| `C:\Users\Win\AppData\Roaming\discord` | 429,863,156 | 431,730,688 |
| **Pre-fixed Ground-Truth Sum** | **1,071,736,106** | **1,075,519,488** |

- **`y-storage` (`Discord`)**: **`1,074,429,604 B`**
- **Honest Delta**: **`+2,693,498 B` (`+0.251%`)** (well within `~5%`)
- **Cause of Non-Zero Delta**: `y-storage` additionally includes `C:\Program Files\Common Files\Discord` (`2,693,498 B`).

---

## 4. Regression & Packaging Verification

- **`C:\` Byte Reconciliation**:
  - Attributed Apps (`C:`): `81,338,298,455 B`
  - Shared & system caches (`C:`): `11,660,796,101 B`
  - Your files (`C:`): `3,210,721,812 B`
  - Unattributed (`C:`): `6,423,481,081 B`
  - Windows & system (`C:` remainder + root files): `45,959,141,847 B`
  - Free (`C:`): `99,640,311,808 B`
  - **Total Sum**: `248,232,751,104 B` vs `C:\` capacity `248,232,751,104 B` -> **Diff = `0 B`**
- **Deployment**: Copied release `y-storage.exe`, `THIRD-PARTY-LICENSES.md`, and `data\dependency_catalog.toml` to `Y:\Apps\y-storage\`.
