# Feature Specification: `feasibility-spike`

**Feature Branch**: `001-feasibility-spike`  
**Feature Slug**: `feasibility-spike`  
**Created**: 2026-11-26  
**Status**: In Progress

## Purpose & Scenarios

Before constructing `storage-ui`, `feasibility-spike` measures and proves on the live dev machine that non-elevated Windows registry enumeration, default-scope directory walking, and deterministic folder-to-app attribution meet performance and coverage expectations.

### User Story 1 - Registry App Discovery Coverage (`feasibility-spike`, `app-discovery`) (Priority: P1)

As the storage analyzer, I want to enumerate `HKLM` and `HKCU` Uninstall keys across both 64-bit and 32-bit registry views so I know how many installed apps exist and how many expose `InstallLocation` directly vs via inference.

**Independent Test**: Run `spike_runner` and record raw non-empty `DisplayName` entries, entries with non-empty `InstallLocation`, canonical deduplicated user-facing apps, canonical apps with direct `InstallLocation`, and canonical apps with inferred install locations.

### User Story 2 - Default-Scope Low-I/O Scan Speed (`feasibility-spike`, `drive-scanner`) (Priority: P1)

As the storage analyzer, I want to walk the default scope (system drive `C:\` minus `C:\Windows`, plus root system files `pagefile.sys`/`hiberfil.sys`/`swapfile.sys`, plus registry `InstallLocation` folders on secondary drives) at low I/O priority without following symlinks/junctions, and measure exact wall-clock seconds.

**Independent Test**: Time a full default-scope walk on the live dev machine with `FindFirstFileExW(FindExInfoBasic, FIND_FIRST_EX_LARGE_FETCH)` and `THREAD_MODE_BACKGROUND_BEGIN`. If runtime exceeds `150` seconds (~2.5 minutes), evaluate an NTFS MFT/USN (`FSCTL_ENUM_USN_DATA`, admin elevation) design in `002-y-storage-v1/plan.md`.

### User Story 3 - Top-50 Folder Attribution Coverage (`feasibility-spike`, `attribution-zones`) (Priority: P1)

As the storage analyzer, I want to rank the 50 largest non-system, non-personal-shell attribution-unit folders found during the default-scope scan and measure how many attribute deterministically to an installed app vs land in `Unattributed`, listing the top 5 `Unattributed` folders.

**Independent Test**: Rank the top 50 attribution-unit folders by byte size across both the default-scope walk (`C:\` + secondary `InstallLocation`s) and `C:\`-only, reporting attributed count/percentage and the top 5 `Unattributed` folder paths and sizes.

## Requirements

- **FR-001 (`app-discovery`)**: Enumerate `HKLM` and `HKCU` Uninstall hives (`KEY_WOW64_64KEY` + `KEY_WOW64_32KEY` + `WOW6432Node`) without elevated privileges.
- **FR-002 (`drive-scanner`)**: Exclude `C:\Windows` and `WinSxS`, skip directory reparse points (`FILE_ATTRIBUTE_REPARSE_POINT`), size root `pagefile.sys` / `hiberfil.sys` / `swapfile.sys`, size secondary-drive `InstallLocation` directories individually, and run worker threads at `THREAD_MODE_BACKGROUND_BEGIN`.
- **FR-003 (`attribution-zones`)**: Attribute folders using `InstallLocation`, `ExactName`, `Token` (incl. `Package Cache` `{GUID}` registry linkage), `Publisher` (excluding generic `"Microsoft Corporation"`), and `PackageFallback`, routing unmatched folders cleanly to `Unattributed`.

## Success Criteria

- **SC-001**: All three spike measurements run cleanly on the live dev machine with zero errors and record exact proving numbers in `specs/001-feasibility-spike/evidence.md`.
- **SC-002**: Default-scope walk completes in `<= 150s` (or triggers an MFT/USN trade-off evaluation if slower).
- **SC-003**: Drive reconciliation invariant (`scanned + windows_system_remainder + free == drive_total`) holds to the exact byte on `C:\`.
