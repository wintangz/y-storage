# Feature Specification: `y-storage` V1

**Feature Branch**: `002-y-storage-v1`  
**Included Feature Slugs**: `feasibility-spike`, `app-discovery`, `drive-scanner`, `attribution-zones`, `scan-history`, `storage-ui`  
**Created**: 2026-11-26  
**Status**: Approved (Autonomous Spec Kit Run)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - True Per-App Footprint & Breakdown (`app-discovery`, `drive-scanner`, `attribution-zones`) (Priority: P1)

As a Windows user trying to see where my disk space went, I want `y-storage` to enumerate installed apps from the Windows Registry (`HKLM` + `HKCU`, 32/64-bit views) and Store `Packages`, scan enabled drives + secondary `InstallLocation`s at low I/O priority without following junctions/symlinks or walking `Windows`/`WinSxS`, and show ONE combined total per app expandable into its per-location breakdown with zone tags (`BODY`, `RED`, `RED · Roaming config`, `GREEN`, `ProgramData · Machine-shared`), attribution basis, and `NO-TOUCH` badges.

**Independent Test**: Run `y-storage.exe --scan-cli` and verify totals for (a) EA App + FIFA 23, (b) Visual Studio, and (c) Discord match independent Explorer/Win32 file sizes within 5%, and every byte on `C:\` reconciles (`apps + your_files + unattributed + windows_system + free == drive_total`).

### User Story 2 - On-Demand Scan Settings, Exclusions & Pinned Groups (`drive-scanner`, `storage-ui`) (Priority: P1)

As a user, I want default scans to walk only the system drive (`C:\` excluding `C:\Windows`) plus secondary-drive registry `InstallLocation`s, let me toggle full-walk drives and edit per-drive exclusions in Settings, route personal shell folders (`Documents`, `Downloads`, `Pictures`, `Music`, `Videos`, `Desktop`, `OneDrive`, `Saved Games`) strictly to `Your files` (unattributed, un-zoned, non-actionable), and pin `Windows & system`, `Your files`, and `Unattributed` at the bottom of the main view.

**Independent Test**: Verify unit tests enforce `Windows`/`WinSxS` exclusion, symlink/junction skip, personal shell isolation, and exact Settings persistence in `%APPDATA%\y-storage\config.toml`.

### User Story 3 - Cached Startup, Scan History & Growth Delta (`scan-history`, `storage-ui`) (Priority: P2)

As a user tracking storage bloat, I want folder sizes and the last scan report cached locally (`%LOCALAPPDATA%\y-storage\`) so the window opens immediately without auto-scanning, and I want per-app sizes stored per scan timestamp in `scan_history.json` so every app row displays its growth delta (`+X MB`, `-X MB`, `NEW`, `0 B`) since the previous scan.

**Independent Test**: Run two consecutive scans and verify `scan_history.json` appends snapshots and renders accurate deltas (`NoBaseline` -> `0 B` / `+delta` / `-delta`).

## Requirements

- **FR-001 (`app-discovery`)**: Read `HKLM` and `HKCU` Uninstall keys (64-bit & 32-bit views), infer missing install locations cleanly when safe, merge duplicate `InstallLocation` entries, and retain MSI `{GUID}` -> app mappings for `ProgramData\Package Cache`.
- **FR-002 (`drive-scanner`)**: Never walk `Windows` or `WinSxS`; never follow `FILE_ATTRIBUTE_REPARSE_POINT` directories; size root `hiberfil.sys` / `pagefile.sys` / `swapfile.sys` directly into `Windows & system`; run worker threads at `THREAD_MODE_BACKGROUND_BEGIN`; cache folder sizes locally; scan only when user clicks `Rescan`.
- **FR-003 (`attribution-zones`)**: Attribute with deterministic priority (`InstallLocation` -> `ExactName` -> `Token` -> `Publisher` -> `PackageFallback`), label `BODY` / `RED` / `GREEN` zones + `Roaming · Config` & `ProgramData · Machine-shared` qualifiers, tag permanent `NO-TOUCH` categories (`Driver`, `Service`, `AntiCheat`, `SystemRuntime`, `Registry`), and surface unmatched folders transparently in `Unattributed`.
- **FR-004 (`scan-history`)**: Persist per-app scan history and compute growth deltas against the previous scan.
- **FR-005 (`storage-ui`)**: Render a single standalone Win32 dark/light UI matching `z-controller`'s `SoftwarePainter` + pastel palette (`#54B9F2`, `#D8A6FF`, `#F7A8D8`), strictly read-only in v1 with zero modal popups.

## Logged Assumptions

- **A-001 (Visual Studio SKU on Dev Machine)**: The dev machine has `Visual Studio Community 2022` (`C:\Program Files\Microsoft Visual Studio\2022\Community`) and `Microsoft Visual Studio Installer` (`C:\Program Files (x86)\Microsoft Visual Studio\Installer`) installed rather than standalone `Visual Studio Build Tools`. Acceptance check (b) therefore verifies `Visual Studio Community 2022` + `Microsoft Visual Studio Installer` against independent Explorer/Win32 sizing across all VS folders (`Program Files`, `Program Files (x86)`, `ProgramData`, `AppData\Local`, `AppData\Roaming`).
- **A-002 (Per-User App in `AppData\Local`)**: On this dev machine, `C:\Users\Win\AppData\Local\Programs` contains only an empty `Common` folder, while `Discord` is installed per-user in `C:\Users\Win\AppData\Local\Discord` (`BODY`) with Roaming data in `C:\Users\Win\AppData\Roaming\discord` (`RED · Roaming config`). Acceptance check (c) therefore verifies `Discord` (plus cross-checks `Microsoft Visual Studio Code (User)`).
- **A-003 (Windows & System Estimate)**: `C:\Windows` and `WinSxS` are never walked due to hardlink inflation; `Windows & system` combines directly sized root files (`pagefile.sys`, `swapfile.sys`, `hiberfil.sys`, `DumpStack.log.tmp`) with the remainder (`drive_used - everything_scanned`, marked as an estimate) so drive totals reconcile exactly.
