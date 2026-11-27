# Verification Report: `integrity-pass-1`

**Date**: 2026-11-27
**Status**: **PASS** (STEP 0, FIX A, FIX B, 40/40 workspace tests, live scan reconciliation, dark screenshots)

---

## STEP 0 — Confirmed Diagnosis

1. **Confirmed (`crates/y-core/src/discovery.rs`)**: `normalize_app_token` stripped version digits, `v<N>`, `x64`, `x86`, and `64` tokens, collapsing all `Microsoft Visual C++ <year> ... Redistributable` records to `"microsoft visual c redistributable"` (`Microsoft Windows Desktop Runtime` to `"microsoft windows desktop runtime"`, `Microsoft .NET SDK` to `"microsoft net sdk"`) and merging them into the first enumerated entry in `build_registry_catalog`.
2. **Confirmed (`crates/y-core/src/attribution.rs`)**: `find_by_token` resolved `{GUID}` via `guid_to_app_name` and routed `ProgramData\Package Cache` folders by substring match (`"visual c++"` / `".net"` / `"windows software development kit"`) to `find_app_matching`, dumping all payloads into the single merged bucket (and misrouting `.NET Runtime 8.0.21` Package Cache folders into `.NET SDK 7.0.407`) without checking the folder's own version or architecture.

---

## FIX A — Audit Table (All 15 Pre-Round Multi-Record Groups)

| # | Group Lead (Pre-Round) | Raw Records | Action | One-Line Reason |
|---|---|---|---|---|
| 1 | `Microsoft Visual C++ ... Redistributable` | 15 | **SPLIT** | Independently installable/uninstallable side-by-side CRT versions/arches (`2005`–`v14`); split into 15 canonical records (`8` non-zero Package-Cache-backed rows + `7` zero-byte WinSxS-backed records). |
| 2 | `Microsoft Windows Desktop Runtime` | 2 | **SPLIT** | Independently installable side-by-side `.NET` desktop runtimes (`10.0.14 (x64)` and `8.0.33 (x64)`). |
| 3 | `Microsoft .NET SDK` (+ `.NET Runtime 8.0.21` cache routing) | 2 (+1) | **SPLIT** | Independently installable side-by-side SDKs (`10.0.403 (x64)` and `7.0.407 (x64)`), plus `.NET Runtime - 8.0.21 (x64)` unshadowed by GUID/version Package Cache routing. |
| 4 | `EA SPORTS™ FIFA 23` / `FIFA 23` | 2 | **KEPT** | Steam + EA duplicate Uninstall entries pointing to the identical `D:\Games\Steam\steamapps\common\FIFA 23` install folder. |
| 5 | `Genshin Impact` / `HoYoPlay` | 2 | **KEPT** | Single shared `C:\Program Files\HoYoPlay` install location and shared HKLM launcher/game lifecycle. |
| 6 | `The Isle` / `Spacewar` / `Steam` | 3 | **KEPT** | Duplicate Steam root `Z:\Games\Steam` path registrations. |
| 7 | `AURA lighting effect add-on x64` / `AURA lighting effect add-on` | 2 | **KEPT** | Driver-suite companion `x64`/`x86` MSI pair at identical `v0.0.61` sharing one folder (scope guard). |
| 8 | `Epic Online Services` | 2 | **KEPT** | Bootstrapper (`2.0.44.0`) + self-updated service (`5.8.1`) sharing one `C:\Program Files (x86)\Epic Games\Epic Online Services` folder. |
| 9–15 | `Discord`, `Ink/Stitch`, `Rustup`, `Spotify`, `Python 3.15.1 (64-bit)`, `Ollama`, `Microsoft VS Code (User)` | 2 each | **KEPT** | Exact `HKCU-64` / `HKCU-32` registry-reflection mirrors of identical subkeys (deduplicated cleanly). |

---

## FIX A — Family Byte & App-Count Reconciliation (Live Scan)

- **App Count Delta**: `259` (pre-round) -> **`269`** (`+10` rows):
  - `+7` rows from `Visual C++ Redistributable` (`1` -> `8` non-zero rows)
  - `+1` row from `Microsoft Windows Desktop Runtime` (`1` -> `2` rows)
  - `+2` rows from `.NET SDK / .NET Runtime` (`1` -> `3` rows: `10.0.403`, `7.0.407`, `8.0.21`)
  - Total arithmetic check: `259 + 7 + 1 + 2 = 269` (`0` unexplained rows; single apps such as `EA SPORTS™ FIFA 23 = 51,372,195,955 B` and `Windows Software Development Kit = 2,312,067,601 B` remain byte-for-byte identical).

| Family | Pre-Round Merged Total | Post-Round Split Sum | Split Row Breakdown | Delta |
|---|---|---|---|---|
| **Visual C++ Redistributable** | **44,006,551 B** (`1` row) | **44,006,551 B** (`8` rows) | `v14 x64 14.51.36247` (`7,771,661 B`), `2013 x64 12.0.40664` (`7,376,477 B`), `2012 x64 11.0.61030` (`7,369,449 B`), `v14 x86 14.51.36247` (`7,145,960 B`), `2012 x86 11.0.61030` (`6,735,097 B`), `2013 x86 12.0.40664` (`6,683,595 B`), `2013 x64 12.0.30501` (`462,172 B`), `2013 x86 12.0.30501` (`462,140 B`) | **0 B** |
| **Windows Desktop Runtime** | **61,994,890 B** (`1` row) | **61,994,890 B** (`2` rows) | `10.0.14 (x64)` (`31,061,202 B`), `8.0.33 (x64)` (`30,933,688 B`) | **0 B** |
| **.NET SDK + .NET Runtime** | **1,380,317,186 B** (`1` row) | **1,380,317,186 B** (`3` rows) | `SDK 7.0.407 (x64)` (`1,131,674,312 B`), `SDK 10.0.403 (x64)` (`219,699,704 B`), `Runtime 8.0.21 (x64)` (`28,943,170 B`) | **0 B** |

### Fallback Rules Used When Evidence Cannot Distinguish Versions
1. **VC++ Installer Manifests (`-req:vc` in `__Installer/installerdata.xml`)**: Attached to any versioned `vcpp-*` target already proven by PE imports on the same app (e.g. `vcruntime140.dll` -> `vcpp-v14` on `EA SPORTS™ FIFA 23`), falling back to `vcpp-v14` when no version-encoded CRT DLL is imported.
2. **.NET Generic Imports (`mscoree.dll`, bare `hostfxr.dll` without `.runtimeconfig.json`/`.deps.json`, `-req:dotnet`, `WindowsAppRuntime`)**: Routed to `dotnet-runtime` (`Microsoft .NET Runtime`) when metadata does not declare a major `.NET` version (`6`/`7`/`8`/`9`/`10`).
3. **Shared `ProgramData\Package Cache` & Shared `.NET` SDK Folders (`C:\Program Files\dotnet`, etc.)**: Exact bundle `{GUID}` -> exact `uninstall_guids` row; versioned MSI components route by `{GUID}v<ver>` / registry component name (`v56.*`/`net7`/`net6` -> `.NET SDK 7.0.407`, `v80.*`/`v81.*`/`net10` -> `.NET SDK 10.0.403`); shared host folders fall back deterministically within the `.NET SDK` family.

---

## FIX B — Dependency Coverage Sweep

- **Before (`button_discipline_1`)**: `130 / 259` apps with `>= 1` dependency (`129` with `0` dependencies).
- **After (`integrity-pass-1`)**: **`152 / 269`** apps with `>= 1` dependency (`117` with `0` dependencies; `24` previously-empty apps now linked via expanded PE traversal, non-Body fallback scan, `512 MiB` header-seek PE limit, UWP `*_app.dll` VC++ rules, `msvcr90.dll` VC++ 2008 rules, and `python3*.dll` rules).
- **Invariants & Acceptance**:
  - `cargo test --workspace`: **40 passed, 0 failed** (including `vc_package_cache_lands_on_its_own_versions_row` and `split_family_row_sums_reconcile_with_former_merged_total`).
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`** with exact **`7` dependencies** (`directx-runtime`, `ea-anticheat`, `ea-app`, `dotnet-runtime`, `vcpp-v14`, `steam`, `denuvo`).
  - `C:\` reconciliation: **`diff = 0 B`**.
  - Screenshots captured & verified: `screenshot-vcpp-split-dark.png`, `screenshot-vcpp-drawer-dark.png`.

### Complete Classification of All 129 Pre-Round Zero-Dependency Apps

- `MISSED LINK (Fixed)`: **24**
- `SHARED RUNTIME / SYSTEM COMPONENT` (now displaying first-class `"Used by N apps"` line in expanded drawer): **8**
- `TRULY STANDALONE`: **97**

| # | App Name (Pre-Round) | Size (B) | Classification | Basis / Resolution |
|---|---|---|---|---|
| 1 | Unity 6000.3.27f1 | 9,141,455,890 | MISSED LINK (Fixed) | directx-runtime, vcpp-2010, vcpp-v14 |
| 2 | Ollama version 0.40.9 | 3,408,398,738 | MISSED LINK (Fixed) | vcpp-v14 |
| 3 | NVIDIA Graphics Driver 621.01 | 3,015,266,135 | TRULY STANDALONE | Self-contained kernel/driver/service binary or anti-cheat runtime |
| 4 | Windows Software Development Kit - Windows 10.0.26100.9457 | 2,312,067,601 | MISSED LINK (Fixed) | dotnet-runtime |
| 5 | .gemini | 1,887,101,972 | TRULY STANDALONE | User profile config/dot-folder or OS registry hive data only (no executable binaries) |
| 6 | Microsoft Edge WebView2 Runtime | 1,207,499,322 | MISSED LINK (Fixed) | directx-runtime, vcpp-v14 |
| 7 | Store: Microsoft.4297127D64EC6 | 911,260,591 | MISSED LINK (Fixed) | directx-runtime, vcpp-v14 |
| 8 | Spotify | 728,990,191 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 9 | Inkscape | 667,077,251 | MISSED LINK (Fixed) | python-runtime |
| 10 | GitHub Copilot CLI | 471,101,768 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 11 | Zalo | 457,156,942 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 12 | Playwright | 456,676,532 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 13 | NVIDIA PhysX System Software 9.26.0904 | 421,178,493 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 14 | Adobe Creative Cloud | 384,033,853 | MISSED LINK (Fixed) | vcpp-v14 |
| 15 | Jellyfin | 369,386,813 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 16 | Store: MicrosoftWindows.Client.CBS | 289,814,348 | MISSED LINK (Fixed) | vcpp-v14 |
| 17 | Python Launcher | 274,684,109 | MISSED LINK (Fixed) | vcpp-v14, python-runtime |
| 18 | Postman | 264,102,652 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 19 | Microsoft Power BI Desktop | 255,417,288 | MISSED LINK (Fixed) | vcpp-v14 |
| 20 | Store: MicrosoftWindows.Speech.en-US.1 | 193,601,931 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 21 | Ink/Stitch version v3.3.0 | 185,390,289 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 22 | Internet Download Manager | 171,746,815 | MISSED LINK (Fixed) | vcpp-2008 |
| 23 | AI Noise Cancelation Audio Software | 161,559,616 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 24 | Java 8 Update 513 | 150,897,812 | MISSED LINK (Fixed) | vcpp-v14 |
| 25 | Node.js | 130,025,934 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 26 | AMD Chipset Software | 120,003,528 | MISSED LINK (Fixed) | directx-runtime, vcpp-v14 |
| 27 | Cities: Skylines II | 107,400,938 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 28 | Riot Client | 104,923,767 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 29 | TreeSize Free V4.8.1 | 101,059,682 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 30 | BattlEye | 89,602,694 | TRULY STANDALONE | Self-contained kernel/driver/service binary or anti-cheat runtime |
| 31 | Store: Microsoft.LanguageExperiencePackvi-VN | 85,813,349 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 32 | Store: Microsoft.UI.Xaml.2.8 | 83,783,419 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 33 | MSI Afterburner 4.6.5 | 74,453,080 | MISSED LINK (Fixed) | vcpp-2008, vcpp-v14 |
| 34 | AURA Service | 72,345,702 | TRULY STANDALONE | Self-contained kernel/driver/service binary or anti-cheat runtime |
| 35 | Microsoft Windows Desktop Runtime 10.0.14 (x64) | 61,994,890 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 36 | Windows User Registry (NTUSER.DAT) | 53,330,221 | TRULY STANDALONE | User profile config/dot-folder or OS registry hive data only (no executable binaries) |
| 37 | Store: Microsoft.UI.Xaml.2.7 | 49,440,798 | MISSED LINK (Fixed) | vcpp-v14 |
| 38 | Microsoft Visual C++ 2010  x64 Redistributable - 10.0.40219 | 44,006,551 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 39 | CrystalDiskMark 9.0.1 Aoi Edition | 33,885,099 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 40 | Store: Microsoft.UI.Xaml.2.6 | 32,015,088 | MISSED LINK (Fixed) | vcpp-v14 |
| 41 | Store: NVIDIACorp.NVIDIAControlPanel | 31,137,903 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 42 | Store: Microsoft.NET.Native.Framework.2.2 | 24,803,926 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 43 | Store: Microsoft.NET.Native.Framework.2.1 | 23,680,423 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 44 | Store: Microsoft.UI.Xaml.2.4 | 22,865,275 | MISSED LINK (Fixed) | vcpp-v14 |
| 45 | Store: Microsoft.UI.Xaml.2.3 | 20,626,818 | MISSED LINK (Fixed) | vcpp-v14 |
| 46 | AutoHotkey | 19,855,277 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 47 | .continue | 18,482,096 | TRULY STANDALONE | User profile config/dot-folder or OS registry hive data only (no executable binaries) |
| 48 | Store: Microsoft.Ink.Handwriting.en-US.1.0 | 16,045,544 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 49 | Store: Microsoft.Advertising.Xaml | 15,941,483 | MISSED LINK (Fixed) | vcpp-v14 |
| 50 | Store: Microsoft.HEIFImageExtension | 15,871,212 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 51 | HWiNFOÂ® 64 | 15,672,571 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 52 | Store: Microsoft.NET.Native.Framework.1.7 | 13,622,768 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 53 | TP-Link Archer T2U Nano Driver | 13,251,968 | TRULY STANDALONE | Self-contained kernel/driver/service binary or anti-cheat runtime |
| 54 | Store: Microsoft.UI.Xaml.2.0 | 9,762,550 | MISSED LINK (Fixed) | vcpp-v14 |
| 55 | Store: Microsoft.Winget.Source | 9,326,438 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 56 | WinRAR 6.24 (64-bit) | 8,322,024 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 57 | Store: Microsoft.WebMediaExtensions | 5,208,243 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 58 | Store: Microsoft.Windows.ShellExperienceHost | 3,911,760 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 59 | Store: Microsoft.RawImageExtension | 3,386,072 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 60 | Store: Microsoft.ApplicationCompatibilityEnhancements | 2,944,601 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 61 | NVIDIA HD Audio Driver 1.4.6.6 | 2,554,931 | TRULY STANDALONE | Self-contained kernel/driver/service binary or anti-cheat runtime |
| 62 | Store: windows.immersivecontrolpanel | 2,214,144 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 63 | Store: Microsoft.Windows.CloudExperienceHost | 2,108,624 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 64 | Free Auto Clicker 8.8.3.0 | 2,039,960 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 65 | ArtCraft Launcher | 1,964,985 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 66 | Microsoft Update Health Tools | 1,957,096 | TRULY STANDALONE | Self-contained kernel/driver/service binary or anti-cheat runtime |
| 67 | Store: Microsoft.MPEG2VideoExtension | 1,923,130 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 68 | Store: Microsoft.Windows.StartMenuExperienceHost | 1,915,091 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 69 | Store: Microsoft.AAD.BrokerPlugin | 1,795,635 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 70 | Store: DeletedAllUserPackages | 1,617,891 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 71 | Store: Microsoft.Services.Store.Engagement | 1,596,071 | MISSED LINK (Fixed) | vcpp-v14 |
| 72 | Store: Microsoft.NET.Native.Runtime.1.7 | 1,259,762 | MISSED LINK (Fixed) | vcpp-v14 |
| 73 | Store: Microsoft.Windows.ContentDeliveryManager | 1,244,429 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 74 | Store: Microsoft.WebpImageExtension | 980,195 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 75 | Store: Microsoft.NET.Native.Runtime.2.2 | 905,704 | MISSED LINK (Fixed) | vcpp-v14 |
| 76 | Store: cr.sb.odmf5200eafd3ad904629cbb0f87a78a3c7211081fe | 887,344 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 77 | Store: Microsoft.NET.Native.Runtime.2.1 | 883,858 | MISSED LINK (Fixed) | vcpp-v14 |
| 78 | OpenAL | 809,496 | SHARED RUNTIME / SYSTEM COMPONENT | Shared runtime/framework or MSI payload (no independent forward runtime deps) |
| 79 | Store: cr.sb.odm3E4D1A088C1F6D498C84F3C86DE73CE49F82A104 | 474,234 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 80 | PhotoCraft | 334,175 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 81 | Store: AdvancedMicroDevicesInc-RSXCM | 247,738 | TRULY STANDALONE | UWP Store package with no external CRT/runtime PE imports |
| 82 | My dream setup | 172,413 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 83 | Store: cr.sb.odm8c3a99bdfb73ce9abf2ba64480175027ae542cad | 146,706 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 84 | Store: cr.sb.odmeecc7f22c796e6e2ec0afe58069ea1e322a97c81 | 146,706 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 85 | Store: Microsoft.LockApp | 142,632 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 86 | Store: Resources | 132,327 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 87 | Store: cr.sb.cdm3e4d1a088c1f6d498c84f3c86de73ce49f82a104 | 131,940 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 88 | Store: cr.sb.cdmf5200eafd3ad904629cbb0f87a78a3c7211081fe | 131,940 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 89 | Store: Microsoft.Ink.Handwriting.Main.en-US.1.0.1 | 113,367 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 90 | Store: cr.sb.odmDD263C8BF30B78A1DCF989083D8CA0AA7035944F | 95,818 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 91 | Store: Microsoft.AccountsControl | 81,920 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 92 | Store: Microsoft.MicrosoftEdge.Stable | 78,384 | TRULY STANDALONE | UWP Store package with no external CRT/runtime PE imports |
| 93 | .local | 47,104 | TRULY STANDALONE | User profile config/dot-folder or OS registry hive data only (no executable binaries) |
| 94 | Store: WinRAR.ShellExtension | 36,500 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 95 | Store: JAMSoftware.TreeSizeFreeContextMenu | 35,529 | TRULY STANDALONE | UWP Store package with no external CRT/runtime PE imports |
| 96 | Store: MicrosoftWindows.Client.OOBE | 34,372 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 97 | Store: Microsoft.VisualStudioCode | 25,048 | TRULY STANDALONE | UWP Store package with no external CRT/runtime PE imports |
| 98 | Store: ASUSAmbientHAL64 | 23,576 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 99 | Store: MdOdrMcpFilterPackage | 19,850 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 100 | Store: Microsoft.MinecraftUWP | 16,446 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 101 | Store: 1527c705-839a-4832-9118-54d4Bd6a0c89 | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 102 | Store: E2A4F912-2574-4A75-9BB0-0D023378592B | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 103 | Store: F46D4000-FD22-4DB4-AC8E-4E1DDDE828FE | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 104 | Store: Microsoft.AsyncTextService | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 105 | Store: Microsoft.BioEnrollment | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 106 | Store: Microsoft.CredDialogHost | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 107 | Store: Microsoft.ECApp | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 108 | Store: Microsoft.MicrosoftEdgeDevToolsClient | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 109 | Store: Microsoft.Win32WebViewHost | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 110 | Store: Microsoft.Windows.Apprep.ChxApp | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 111 | Store: Microsoft.Windows.CapturePicker | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 112 | Store: Microsoft.Windows.NarratorQuickStart | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 113 | Store: Microsoft.Windows.ParentalControls | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 114 | Store: Microsoft.Windows.PeopleExperienceHost | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 115 | Store: Microsoft.Windows.PinningConfirmationDialog | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 116 | Store: Microsoft.Windows.XGpuEjectDialog | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 117 | Store: MicrosoftWindows.Client.Core | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 118 | Store: MicrosoftWindows.Client.CoreAI | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 119 | Store: Windows.CBSPreview | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 120 | Store: Windows.PrintDialog | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 121 | Store: c5e2524a-ea46-4f67-841f-6a9465d9d515 | 16,384 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 122 | Store: Microsoft.Windows.OOBENetworkCaptivePortal | 8,192 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 123 | Store: Microsoft.Windows.OOBENetworkConnectionFlow | 8,192 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 124 | Store: Microsoft.Windows.PrintQueueActionCenter | 8,192 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 125 | Store: MicrosoftWindows.UndockedDevKit | 8,192 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
| 126 | FilmCraft | 3,995 | TRULY STANDALONE | Static/MSVCRT/Node/Electron or data-only folder with no matching external runtime PE imports |
| 127 | .agents | 977 | TRULY STANDALONE | User profile config/dot-folder or OS registry hive data only (no executable binaries) |
| 128 | .impeccable | 219 | TRULY STANDALONE | User profile config/dot-folder or OS registry hive data only (no executable binaries) |
| 129 | Store: Microsoft.MinecraftUWP | 62 | TRULY STANDALONE | UWP OS shell host / codec extension / data package with no non-OS CRT/runtime imports |
