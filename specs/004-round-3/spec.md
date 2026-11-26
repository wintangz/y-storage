# Feature Specification: `y-storage` Round 3

**Feature Branch**: `004-round-3`  
**Included Feature Slugs**: `attribution-coverage-3`, `catalog-embed`  
**Created**: 2026-11-26  
**Status**: Approved (Autonomous Spec Kit Run)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Finish `Unattributed` Cleanup Without Force-Matching (`attribution-coverage-3`) (Priority: P1)

As a user inspecting `C:` drive storage, I want identifiable leftover folders in Round 2's `Unattributed` list (`>= 100 MB`) resolved cleanly via principled rules or verified known-app mappings:
- General **versioned-product-under-publisher** rule (`Local|Roaming|LocalLow\<Publisher>\<Product><version>` such as `Google\AndroidStudio2024.2` + `AndroidStudio2023.2` -> summed into ONE `Android Studio` total).
- Dedicated attribution for `Roaming\TabNine` -> `TabNine` (its own entry).
- General **`<X>Data` rule** (`Roaming|Local\<X>Data` -> app `X` when `X` is installed or a known app, e.g. `Roaming\ZaloData` -> `Zalo`).
- Standalone toolchain entry for `Local\ms-playwright` -> `Playwright` (`StandaloneFolder`, `GREEN` cache note for regenerable downloaded browser binaries).
- Audit of every remaining Round 2 `Unattributed` item `>= 100 MB` (`Local\jellyfin`, `Roaming\Apple Computer`, `Roaming\Postman`, `C:\Users\Win\Microsoft\Power BI Desktop Store App`, `Local\Microsoft\Windows`, `Local\GitKrakenCLI`, `Common Files\Adobe` + `Roaming\Adobe`, `LocalLow\Colossal Order\Cities Skylines II`, `Roaming\riot-client-ux`), while keeping genuinely unknown folders openly in `Unattributed`.

**Independent Test**: None of the 5 named folders appears in `Unattributed`, report the new `Unattributed` total + `%` of `C:` used + new top 5 + top-50 coverage `%` + affected entry totals, and verify `0 B` drive reconciliation.

### User Story 2 - Embedded Dependency Catalog With Optional Safe Override (`catalog-embed`) (Priority: P1)

As a user deploying `y-storage.exe` as a standalone single-file binary, I want `dependency_catalog.toml` compiled into the executable so the exe works fully without any sidecar files, while honoring an optional valid `dependency_catalog.toml` override placed next to the exe and safely falling back to the embedded catalog with a logged warning naming the file when an override is invalid or corrupt.

**Independent Test**: Run `y-storage.exe` in isolated scratch directories: (a) with no sidecar catalog present (`FIFA 23` gets all 7 Round 2 dependencies identically), (b) with a valid modified override catalog next to the exe (override rule appears in output), and (c) with a corrupt override file (scan succeeds using embedded catalog and logs a warning naming the file).

## Logged Assumptions

- **A-006 (`AndroidStudio2024.2` / `2023.2` Live Disk Change)**: Between Round 2 (`09:47`) and Round 3 (`17:38`), `C:\Users\Win\AppData\Local\Google\AndroidStudio2024.2` and `AndroidStudio2023.2` were removed externally from disk, while `C:\Users\Win\AppData\Roaming\Google\AndroidStudio2024.2` (`27,865,918 B`) and `AndroidStudio2023.2` (`21,998,565 B`) remain on disk. Both `Local` and `Roaming` `Google` containers are split cleanly in `tree.rs`, so live `Roaming\Google\AndroidStudio*` folders attribute and sum into `Android Studio` on disk and `round3_tests.rs` proves the exact summation of `Local\Google\AndroidStudio2024.2` (`820,748,873 B`) + `AndroidStudio2023.2` (`582,578,704 B`).
- **A-007 (`Local\Microsoft\Windows` Per-User Subfolders)**: Splitting `C:\Users\<User>\AppData\Local\Microsoft\Windows` exposes `usrClass.dat*` hive files (`RegistryUserHive` -> `Windows User Registry (NTUSER.DAT)`), Microsoft shell/browser caches (`WebCache`, `Explorer`, `INetCache`, `Caches`, `ActionCenterCache`, `PRICache`, etc. -> `Shared & system caches (GREEN)`), and remaining unowned OS shell folders (`Fonts`, `Notifications`, `Safety`, `1033`) which stay openly in `Unattributed`.
- **A-008 (Strict Override Schema Validation)**: Applying `#[serde(deny_unknown_fields)]` and non-empty `targets` checks ensures unknown TOML keys or malformed overrides trigger the logged fallback to the embedded catalog instead of silently loading empty rules.
