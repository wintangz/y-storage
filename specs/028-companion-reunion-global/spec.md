# Spec 028 — `companion-reunion-global`

## 1. Summary
Starting from `main` HEAD `a09ad3290ee7505eaec8838e1a6a97cb807cffa8`, this round introduces two general structural improvements to attribution and categorisation so `y-storage` handles unfamiliar software environments cleanly beyond the development machine:
1. **Part 1 — Component-Folder Reunion Pass**: Reunites companion component folders (such as virtualisation `"box"`/`"vm"` folders or portable runtime roots) that remain unclaimed as folder-only candidates at Step (5) back into their registered parent app row via general PE file evidence (Tier 1) or brand-stem + generic-component-word evidence (Tier 2), without any product-specific hardcoding.
2. **Part 2 — Global Category Coverage**: Adds a generic class-vocabulary keyword rule layer (`[[rules]]`) and a consumer/developer breadth pass (`[[mappings]]`) in `data/app_categories.toml`, including popular Android and console emulators under `Games`, while leaving opaque hashes, internal system stubs, and dot-folders honestly `Uncategorized`.

## 2. Part 1 — Component-Folder Reunion Design
### 2.1 Bounded PE Version Extraction (`crates/y-core/src/pe.rs`, `crates/y-platform-windows/src/scanner.rs`)
- Added `PeVersionStrings { product_name, company_name }` and `parse_pe_version_strings` / `parse_vs_version_strings_bytes` using the existing bounded (`<= 64 KB`, seek-only) `RT_VERSION (16)` -> `VS_VERSION_INFO` -> `StringFileInfo` walker in a single pass with zero extra disk opens.
- Filtered empty strings, `< 2`-char strings, `TODO:` placeholders, control/replacement corruption, and known generic runtime/engine apphost stubs (`Microsoft® Windows® Operating System`, `Microsoft® .NET`, `BootstrapPackagedGame`, `UE4Game`, `UE5Game`, `UnrealGame`, `Electron`) so standalone packaged apps never inherit wrapper vendor strings.
- Added `pub company_name: Option<String>` to `RootExeCandidate` and populated it in `probe_dir_for_main_exe`.

### 2.2 Attribution Pipeline Placement & Model (`crates/y-core/src/attribution.rs`, `crates/y-core/src/model.rs`)
- Added `AttributionBasis::CompanionReunion` (`"companion reunion"`).
- Placed `try_reunite_component_folder` inside `attribute_scan_with_options` at Step (5) right after `infer_step5_app_candidate` identifies an unclaimed folder-only candidate and before creating a standalone `container:*` / `exe:*` bucket:
  - Priority: Steps (1)–(4), curated mappings, and Step (5c) standalone folders still run first, so exact/registered claims for a folder always win.
  - Candidate parents: registered apps (`buckets[0..installed_apps.len()]`, excluding store fallbacks and synthetic rows).
  - **Tier 1 (File Evidence)**: Principal executable's PE `ProductName` whole-token matches a registered app name/alias, or normalized `CompanyName` (`normalize_company_identity` stripping legal entity suffixes `inc`/`corp`/`ltd`/`llc`/`gmbh`/`s.r.o.`/`e.U.`/`foundation`/etc.) equals a registered app's publisher. Uniqueness required (`>= 2` matches refuses reunion).
  - **Tier 2 (Stem Evidence)**: Compacts folder leaf to lowercase ASCII alphanumerics and checks whether it **starts with** a registered app's digit-stripped cumulative word stem (`len >= 3`), with a non-empty remainder consisting **strictly** of ASCII digits and at least one generic component word from the fixed set:
    `("box", "vm", "engine", "runtime", "data", "cache", "bin", "x64", "x86", "common")`.
    Uniqueness (`== 1` registered app) and a **Tier 1 contradiction veto** (refusing Tier 2 if PE metadata points to a different publisher or unrelated product) are enforced.
  - Reunited locations become `ZoneKind::Body` breakdown items of the parent app and automatically inherit the parent app's `AppCategory`.

## 3. Part 2 — Category Coverage (`data/app_categories.toml`)
- **Generic Keyword Rule Layer (`[[rules]]`)**:
  - `emulator`, `virtual machine` (name + publisher `emulator`) -> `Games`
  - `browser` -> `Browsers`
  - `vpn` (name + publisher) -> `Utilities`
  - `antivirus`, `security suite` (name + publisher `antivirus`) -> `System & Runtimes`
  - `torrent`, `download manager` -> `Utilities`
  - `password manager` -> `Utilities`
  - `video editor`, `audio editor`, `photo editor` -> `Media & Creative`
  - `code editor`, `ide`, `sdk` (+ existing `devkit`, `devhome`, `terminal`, `code`) -> `Development`
  - `messenger`, `chat` (+ existing `slack`, `telegram`, `zoom`, `teams`, `yourphone`) -> `Communication`
- **Breadth Mappings (`[[mappings]]`)**:
  - Added widely-installed Windows consumer software across `Games` (Android & console emulators such as LDPlayer, BlueStacks, NoxPlayer, MEmu, MuMu Player, GameLoop, RetroArch, Dolphin, PCSX2, RPCS3, Ryujinx, Cemu, PPSSPP, DuckStation, Xemu, Xenia, Vita3K, etc.), `Browsers`, `Communication`, `Platforms & Launchers`, `Media & Creative`, `Productivity`, `Development`, `Utilities`, and `System & Runtimes`.

## 4. Logged Assumptions
1. Treating OS/engine generic apphost stub `VS_VERSION_INFO` (`BootstrapPackagedGame`, `Microsoft® .NET`, `Microsoft® Windows® Operating System`, `Electron`, `UE4Game`/`UE5Game`/`UnrealGame`) as unpopulated prevents false Tier 1 publisher linking of unrelated packaged games/apps to runtime owners (e.g., Epic Online Services / Microsoft).
2. `normalize_company_identity` treats trailing `"foundation"` as an organizational suffix alongside `"inc"`, `"corp"`, `"ltd"`, etc., allowing `"Node.js"` PE `CompanyName` and `"Node.js Foundation"` registry publisher to align cleanly alongside `ProductName = "Node.js"`.
3. `try_reunite_component_folder` runs only on folders that pass `infer_step5_app_candidate` so arbitrary `AppData`/`ProgramData` folders remain governed by Steps (1)–(4) or stay `Unattributed`.
