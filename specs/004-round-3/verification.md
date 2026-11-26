# Verification Report: `y-storage` Round 3 (`attribution-coverage-3`, `catalog-embed`)

- **Date**: `2026-11-26 17:56` (local time)
- **Workspace Test Suite (`cargo test --workspace`)**: **18 passed / 0 failed / 0 warnings**
  - `y-core` (`core_tests.rs`): 5 passed
  - `y-core` (`round2_tests.rs`): 4 passed
  - `y-core` (`round3_tests.rs`): 4 passed
  - `y-platform-windows` (`windows_scanner_tests.rs`): 2 passed
  - `y-storage` (`main.rs` unit tests): 3 passed
- **Release Scan Duration**: `8,249 ms` (`8.25 s`)

---

## 1. Acceptance 1 — `attribution-coverage-3`

### Overall `Unattributed` Reduction (`C:\`)
- **`C:\` Drive Stats**: `248,232,751,104 B` capacity (`146,651,889,664 B` used / `101,580,861,440 B` free).
- **Round 2 Baseline `Unattributed`**: `6,423,481,081 B` (`4.32%` of `C:` used).
- **Round 3 `Unattributed`**: **`1,479,761,099 B` (`1.48 GB` = `1.01%` of `C:` used)** across `304` items (`-77.0%` vs Round 2; `-90.1%` vs Round 1).
- **Top-50 Attribution Coverage on `C:\`**:
  - Round 2: `94.0%` (`47/50`).
  - Round 3: **`100.0%` (`50/50`)** non-unattributed (`46` App/Standalone + `4` SharedCache + `0` Unattributed; **`50/50 = 100.0%`** among Top-50 non-cache folders).
- **Audit of Round 2's 15 `>= 100 MB` Unattributed Folders**: **15 / 15 resolved**, **0** remaining `>= 100 MB` items in `Unattributed` (none of the 5 named folders `AndroidStudio2024.2`, `AndroidStudio2023.2`, `TabNine`, `ZaloData`, `ms-playwright` appears in `Unattributed`).

### Resolution Audit & Affected Entry Totals
1. `Android Studio` (`49,864,483 B`): `C:\Users\Win\AppData\Roaming\Google\AndroidStudio2024.2` (`27,865,918 B`) + `AndroidStudio2023.2` (`21,998,565 B`) via general versioned-product-under-publisher rule (`Local|Roaming|LocalLow\<Publisher>\<Product><version>`). *(Per `A-006`, `Local\Google\AndroidStudio2024.2`/`2023.2` were removed externally on disk prior to Round 3; unit test `versioned_product_under_publisher_rule_sums_versions_and_skips_non_versions` verifies exact summation of all three locations = `1,431,193,495 B`.)*
2. `TabNine` (`516,985,662 B`): `C:\Users\Win\AppData\Roaming\TabNine` — dedicated attributed entry (`Tabnine`).
3. `Zalo` (`457,156,942 B`): `C:\Users\Win\AppData\Roaming\ZaloData` — resolved via general `<X>Data` rule (`VNG Corp.`).
4. `Playwright` (`456,676,532 B`): `C:\Users\Win\AppData\Local\ms-playwright` — standalone toolchain entry (`StandaloneFolder`, `ZoneKind::Green`, note `"Standalone toolchain · regenerable dev-tool cache (downloaded browser binaries)"`).
5. `Jellyfin` (`369,386,813 B`): `Local\jellyfin` (`369,283,601 B`) + `Local\JellyfinMediaPlayer` (`103,212 B`).
6. `Store: AppleInc.iTunes` (`787,236,434 B`): attributes `Roaming\Apple Computer` (`272,719,468 B`), `Local\Apple Computer` (`1,478,797 B`), `ProgramData\Apple` (`20,247 B`), `ProgramData\Apple Computer` (`2,005 B`) alongside existing Store package/WindowsApps locations.
7. `Postman` (`264,102,652 B`): `Roaming\Postman` (`263,170,562 B`) + `Roaming\PostmanAgent` (`932,090 B`).
8. `Microsoft Power BI Desktop` (`255,417,288 B`): `C:\Users\Win\Microsoft\Power BI Desktop Store App`.
9. `Local\Microsoft\Windows` (`190,030,802 B` split into children): `usrClass.dat*` (`20,185,088 B`) -> `Windows User Registry (NTUSER.DAT)` (`48,422,189 B` total); shell/browser caches (`WebCache`, `Explorer`, `INetCache`, etc.) -> `Shared & system caches (GREEN)` (`11,826,076,451 B` across `60` folders); small non-cache OS folders (`Fonts`, `Notifications`, etc.) remain openly in `Unattributed`.
10. `GitKraken CLI` (`167,883,588 B`): `Local\GitKrakenCLI` — standalone CLI toolchain (`StandaloneFolder`, `ZoneKind::Body`).
11. `Adobe Creative Cloud` (`384,033,853 B`): `Program Files (x86)\Common Files\Adobe` (`152,212,941 B`) + `Roaming\Adobe` (`148,754,220 B`) + `Roaming\com.adobe.dunamis` (`42,064,409 B`) + `Program Files\Common Files\Adobe` (`29,586,488 B`) + `Local\Adobe` (`11,391,219 B`) + `LocalLow\Adobe` (`24,576 B`).
12. `Cities: Skylines II` (`107,400,938 B`): `LocalLow\Colossal Order\Cities Skylines II`.
13. `Riot Client` (`104,923,767 B`): `Roaming\riot-client-ux`.

### New Top 5 `Unattributed`
1. `C:\Users\Win\onecoremsvsmon` — `96,244,521 B` (`96.24 MB`)
2. `C:\ProgramData\Microsoft\Search` — `93,267,744 B` (`93.27 MB`)
3. `C:\ProgramData\USOShared` — `86,843,392 B` (`86.84 MB`)
4. `C:\Users\Win\AppData\Local\Firaxis Games` — `75,446,222 B` (`75.45 MB`)
5. `C:\Users\Win\AppData\Roaming\UserBenchmark` — `73,907,623 B` (`73.91 MB`)

---

## 2. Acceptance 2 — `catalog-embed`

Tested using isolated copies of `target\release\y-storage.exe` in scratch directories (`--scan-cli ... --ephemeral`) without touching `Y:\Apps\y-storage`:
- **(a) Standalone exe, NO sidecar catalog**:
  - `catalog_source`: `"embedded"`, `warnings`: `0`.
  - `EA SPORTS™ FIFA 23` reported the exact same **7** dependencies, confidences, and bases as Round 2 (`DirectX Runtime`, `EA AntiCheat`, `EA App`, `Microsoft .NET Runtime`, `Microsoft Visual C++ Runtime`, `Steam`, `Denuvo Anti-Tamper`).
- **(b) Valid modified override `dependency_catalog.toml` next to exe**:
  - `catalog_source`: `override: ...\case-b\dependency_catalog.toml`, `warnings`: `0`.
  - `EA SPORTS™ FIFA 23` `Denuvo Anti-Tamper` basis rendered `"catalog: FIFA 23 uses Denuvo Anti-Tamper DRM [ROUND3-OVERRIDE-TEST]"`.
- **(c) Corrupt `dependency_catalog.toml` (`<<<CORRUPT_TOML_SYNTAX [[[`) next to exe**:
  - Scan exited with code `0`, `catalog_source`: `embedded (fallback after invalid override: ...\case-c\dependency_catalog.toml)`.
  - `EA SPORTS™ FIFA 23` retained all **7** embedded dependencies.
  - Warning naming the corrupt `dependency_catalog.toml` path emitted in `stderr`, `report.warnings`, and `%LOCALAPPDATA%\y-storage\y-storage.log`.

---

## 3. Regression & Deployment Verification

- **`C:\` Reconciliation**:
  `Apps 84,078,339,649` + `Caches 11,826,076,451` + `Your files 3,210,721,812` + `Unattributed 1,479,761,099` + `Win/sys 46,056,990,653` + `Free 101,580,861,440` = **`248,232,751,104 B`** vs drive capacity `248,232,751,104 B` (**`diff = 0 B`**).
- **App Total Stability**:
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`** (exact match vs Round 2).
  - `EA App + FIFA 23` combined: **`54,627,471,178 B`** (exact match vs Round 2).
- **Packaging Note**: `y-storage.exe` embeds `data/dependency_catalog.toml` via `include_str!` and runs fully standalone with zero runtime dependency on external catalog files.
