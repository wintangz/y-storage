# Verification Report: `substring-matcher-fix`

**Date**: 2026-11-27  
**Status**: **PASS** (46/46 workspace tests, live scan invariants, full collision sweep, MSTeams dark drawer screenshot)

---

## Root Causes & Fixes Implemented

1. **Token-Boundary App Predicate Matcher (`crates/y-core/src/discovery.rs`)**:
   - Exported `split_alnum_words`, `AppTokenMatchKind` (`ExactFullToken` > `WholeWordSlice`), `match_app_predicate`, and `contains_whole_token_sequence`.
   - Predicates now match ONLY when equal to the app's normalized identity token (`normalize_app_token`, extended with `arm64`/`amd64` and Unity patch tokens `27f1`) or a contiguous whole-word token slice — never a substring inside a longer word (`"steam"` never matches `"msteams"`, `"code"` never matches `"avcencoder"`, `"unity"` never matches `"community"`).
   - `infer_install_root_from_exe` now returns `None` for protocol launcher redirects (`://`, e.g. `"Z:\Games\Steam\steam.exe" steam://uninstall/376210`) and `\steam\games\*.ico` icon caches so `The Isle` no longer steals `Z:\Games\Steam` or shadows `Steam` (`Z:\Games\Steam\uninstall.exe`).
2. **Disambiguated Attribution & Publisher Lookup (`crates/y-core/src/attribution.rs`)**:
   - Rewrote `find_app_matching` (`match_installed_app_index`): prefers `ExactFullToken` over `WholeWordSlice` and refuses to force-match when multiple unrelated apps tie at the winning tier (falling through cleanly to `Unattributed`), allowing multi-row ties only across split versioned rows of the same canonical family.
   - Updated Package Cache token checks and `find_by_publisher` (blocking generic organizational directory words like `"studio"` / `"microsoft"` and using token-boundary matching with single-owner disambiguation).
3. **Categories & Dependency Engine (`crates/y-core/src/categories.rs`, `data/app_categories.toml`, `crates/y-core/src/dependencies.rs`)**:
   - Switched `classify_app` (`[[mappings]]` exact-first + `[[rules]]` keyword rules), `contains_token_boundary`, `is_self_dependency`, `folder_kinship` (path-segment / whole-token exact instead of mid-word compressed `contains`), and `catalog.app_rules` to token-boundary matching.
   - Added explicit mappings/tokens for `Store: Microsoft.Edge.GameAssist` (`Utilities`), `Microsoft Edge WebView2 Runtime` (`System & Runtimes`), `Microsoft.MinecraftUWP`, `Microsoft.VisualStudioCode`, and Store media/utility package suffixes in `data/app_categories.toml`.

---

## Live Scan Collision Sweep (Pre-Fix `ledger_usability_1` vs Post-Fix `substring-matcher-fix`)

### 1. Installed App Name Substring Pairs Enumerated (9 Distinct Non-Generic Pairs Across 251 Unique Normalized Tokens)

| # | Pair Kind | Shorter Normalized App (`X`) | Longer Normalized App (`Y`) | Pre-Fix vs Post-Fix Outcome |
|---|---|---|---|---|
| 1 | `MID_WORD_SUBSTRING` | `steam` (`Steam`) | `store msteams` (`Store: MSTeams`) | **FIXED**: Pre-fix matched `Store: MSTeams` for `"steam"` rules (`htmlcache`, `cefdata`, `Common Files\Steam\drivers`), gave it Steam's category (`Platforms & Launchers`), and inferred `MSTeams -> steam`. Post-fix routes all 3 Steam folders to `Steam`, reclassifies `Store: MSTeams` to `Communication`, and drops `steam` dep. |
| 2 | `MID_WORD_SUBSTRING` | `unity` (`Unity 6000.3.27f1`) | `visual studio community` (`Visual Studio Community 2022`) | **SAFE**: `"unity"` no longer substring-matches inside `"community"`. |
| 3 | `WHOLE_TOKEN_SLICE` | `unity` (`Unity 6000.3.27f1`) | `unity hub` (`Unity Hub 3.22.5`) | **REFINED**: `C:\Users\Win\.unity` (`117,089,200 B`) attributed cleanly to exact full token `Unity 6000.3.27f1` via `ExactName`. |
| 4 | `WHOLE_TOKEN_SLICE` | `python` (`Python 3.15.1 (64-bit)`) | `python launcher` (`Python Launcher`) | **REFINED**: Exact full-token `"python"` (`Python 3.15.1 (64-bit)`) preferred over partial slice `Python Launcher` for `pip` & `uv` (`274,122,957 B`). |
| 5 | `WHOLE_TOKEN_SLICE` | `microsoft edge` (`Microsoft Edge`) | `microsoft edge webview2 runtime` | **FIXED Category**: Explicit `WebView2` mapping classifies runtime as `System & Runtimes` (was `Browsers`). |
| 6 | `WHOLE_TOKEN_SLICE` | `microsoft edge` (`Microsoft Edge`) | `store microsoft edge gameassist` | **FIXED Category**: Explicit `microsoft.edge.gameassist` mapping keeps `Utilities` (not `Browsers`). |
| 7 | `WHOLE_TOKEN_SLICE` | `ai noise cancelation audio software` | `... sdk` | No false cross-attribution (distinct `InstallLocation`s). |
| 8 | `WHOLE_TOKEN_SLICE` | `store microsoft vclibs` (`140.00`) | `store microsoft vclibs uwpdesktop` | Distinct UWP Store packages (`PackageFallback`). |
| 9 | `WHOLE_TOKEN_SLICE` | `winrar` (`WinRAR 6.24 (64-bit)`) | `store winrar shellextension` | `ExactFullToken` `"winrar"` cleanly selects desktop `WinRAR`. |

*(Plus `Microsoft.AVCEncoderVideoExtension` rule collision where `"code"` matched mid-word inside `"avcencoder"` -> fixed from `Development` to `Media & Creative`, and `C:\ProgramData\Studio` where generic `"studio"` matched publisher `"Campfire Studio"` under `My dream setup` -> moved honestly to `Unattributed`.)*

### 2. Reassigned Folders & Before / After App Totals

| Path | Size (B) | Pre-Fix Owner (Basis) | Post-Fix Owner (Basis) |
|---|---:|---|---|
| `C:\Users\Win\AppData\Local\Steam\htmlcache` | 777,226,881 | `Store: MSTeams` (`Token`) | **`Steam`** (`Token`) |
| `C:\Users\Win\AppData\Local\Steam\cefdata` | 19,136,578 | `Store: MSTeams` (`Token`) | **`Steam`** (`Token`) |
| `C:\Program Files (x86)\Common Files\Steam\drivers` | 8,966,361 | `Store: MSTeams` (`Token`) | **`Steam`** (`Token`) |
| `Z:\Games\Steam` | 2,055,302,872 | `The Isle` (`InstallLocation`) | **`Steam`** (`InstallLocation`) |
| `C:\Users\Win\AppData\Roaming\uv` | 123,056,742 | `Python Launcher` (`Token`) | **`Python 3.15.1 (64-bit)`** (`Token`) |
| `C:\Users\Win\AppData\Local\uv` | 113,537,014 | `Python Launcher` (`Token`) | **`Python 3.15.1 (64-bit)`** (`Token`) |
| `C:\Users\Win\AppData\Local\pip` | 37,529,201 | `Python Launcher` (`Token`) | **`Python 3.15.1 (64-bit)`** (`Token`) |
| `C:\Users\Win\.unity` | 117,089,200 | `Unity Hub 3.22.5` (`Token`) | **`Unity 6000.3.27f1`** (`ExactName`) |
| `C:\ProgramData\Studio` | 172,413 | `My dream setup` (`Publisher`) | **`<Unattributed>`** (`Unattributed`) |

| Affected App | Pre-Fix Total (B) | Post-Fix Total (B) | Delta (B) | Category (`Pre -> Post`) | Dependency Delta |
|---|---:|---:|---:|---|---|
| **`Store: MSTeams`** | 1,084,758,386 | **279,428,566** (`266.48 MB`) | **-805,329,820** (`-768.02 MB`) | `Platforms & Launchers -> Communication` | Removed `steam` |
| **`Steam`** | 0 *(aliased to The Isle)* | **2,860,632,692** (`2.66 GB`) | **+2,860,632,692** (`Z:\Games\Steam` + `768.02 MB`) | `Platforms & Launchers` | Added `dotnet-9` (from `Z:\Games\Steam`) |
| **`The Isle`** | 2,056,137,026 | **834,154** | -2,055,302,872 | `Games -> Games` | Removed `dotnet-9`, `steam` |
| **`Python 3.15.1 (64-bit)`** | 0 *(zero-byte)* | **274,122,957** | +274,122,957 | `Uncategorized` | Added `vcpp-v14` |
| **`Python Launcher`** | 274,684,109 | **561,152** | -274,122,957 | `Development -> Development` | Removed `python-runtime`, `vcpp-v14` |
| **`Unity 6000.3.27f1`** | 9,141,455,890 | **9,258,545,090** | +117,089,200 | `Development -> Development` | Unchanged |
| **`Unity Hub 3.22.5`** | 1,173,070,255 | **1,055,981,055** | -117,089,200 | `Development -> Development` | Unchanged |
| **`My dream setup`** | 172,413 | **0** | -172,413 | `Games` *(0-byte omitted)* | Unchanged |
| **`Microsoft Edge WebView2 Runtime`** | 1,207,499,322 | **1,207,499,322** | 0 | `Browsers -> System & Runtimes` | Unchanged |
| **`Store: Microsoft.AVCEncoderVideoExtension`** | 4,667,138 | **4,667,138** | 0 | `Development -> Media & Creative` | Unchanged |

---

## Acceptance & Invariants

- **`cargo test --workspace`**: **46 passed, 0 failed** (`y-core`: `6 + 6 + 8`, `y-platform-windows`: `2`, `y-storage`: `24`), zero warnings.
- **Live Scan Invariants (`substring_matcher_fix_scan_final.json`)**:
  - `apps.len()` = **270** (`+1`: `Steam` separated cleanly from `The Isle`)
  - `EA SPORTS™ FIFA 23` = **51,372,195,955 B** with **7** dependencies
  - `C:\` reconciliation `diff` = **0 B**
  - `Store: MSTeams` = **279,428,566 B (`266.48 MB`)**, package folders only, `Communication`, no Steam dependency; `Steam` = `Z:\Games\Steam` (`2,055,302,872 B`) + reclaimed **805,329,820 B (`768.02 MB`)** = **2,860,632,692 B**.
- **Screenshot Verified**: `specs/020-substring-matcher-fix/screenshot-msteams-drawer-dark.png`.
