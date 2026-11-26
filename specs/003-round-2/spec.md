# Feature Specification: `y-storage` Round 2

**Feature Branch**: `003-round-2`  
**Included Feature Slugs**: `dependencies`, `attribution-coverage-2`, `ground-truth`  
**Created**: 2026-11-26  
**Status**: Approved (Autonomous Spec Kit Run)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Forward & Reverse Dependency Reporting (`dependencies`) (Priority: P1)

As a user inspecting installed apps and games, I want each app to show its forward dependencies (kind, present/missing, own size, shared flag, basis, confidence `proven`/`inferred`, or `"no known dependencies"` when none are found) without inflating the app's own `total_bytes`, and a reverse dependency view (`"What else uses this?"`) listing every app that depends on each launcher, anti-cheat, service/driver, or shared runtime along with dependent and binary-importer counts.

**Independent Test**: Verify `EA SPORTS™ FIFA 23` reports `Steam`, `EA App`, `EA AntiCheat`, `Microsoft Visual C++ Runtime`, and `DirectX Runtime` with concrete bases (`PE import`, `manifest: appmanifest_1811260.acf`, `service: EAAntiCheatService + driver: EAAntiCheat`, `fingerprint`, `catalog`), reverse view lists all dependent apps for `EA App` and `Microsoft Visual C++ Runtime`, and zero app totals change due to dependency sizes.

### User Story 2 - Shrinking `Unattributed` Without Force-Matching (`attribution-coverage-2`) (Priority: P1)

As a user reconciling disk usage, I want shared `GREEN` caches (`Local\Temp`, `D3DSCache`, `CrashDumps`, `NVIDIA\DXCache`, `AMD\DxCache`, etc.) routed to a dedicated labelled `Shared & system caches (GREEN)` group instead of `Unattributed`, top-level user-profile dot-folders (`.gemini`, `.local`, `.continue`, etc.) and standalone `Roaming\flutter` surfaced under their own names instead of buried in `Unattributed`, while genuinely unknown leftover folders stay honestly in `Unattributed`.

**Independent Test**: Verify `Unattributed` drops substantially below Round 1 (`14,884,258,917 B` / `~10.0%`), report new `Unattributed` bytes + `%` of `C:` used + new top 5 + top-50 attribution coverage `%`, and prove `C:\` reconciliation closes at `0 B`.

### User Story 3 - Non-Circular Independent Ground Truth (`ground-truth`) (Priority: P1)

As a reviewer validating accuracy, I want `EA App + FIFA 23` and `Discord` totals compared against an independent measurement from Windows Explorer Folder Properties (`explorer.exe` General tab Size) summed over pre-fixed human folder lists fixed prior to running the Round 2 scan, reporting the honest source and delta (`<= ~5%`).

**Independent Test**: Capture Explorer Folder Properties `Size` and `Size on disk` for each pre-fixed folder via UIAutomation, sum per target, and compare against `y-storage` Round 2 scan totals.

## Logged Assumptions

- **A-004 (`Roaming\flutter` Standalone SDK)**: Live inspection of `C:\Users\Win\AppData\Roaming\flutter` confirms it is a standalone Flutter SDK `.git` checkout (`bin\flutter.bat`, `packages\`, `dev\`) with no Windows Registry Uninstall entry; per rule 2(c), it is surfaced as a standalone `"flutter"` entry (`AttributionBasis::StandaloneFolder`) rather than force-matched to an unrelated app.
- **A-005 (`DirectX` OS Runtime Footprint)**: Core DirectX DLLs (`d3d11.dll`, `d3d12.dll`, `dxgi.dll`, `D3DCOMPILER_47.dll`) live in `C:\Windows\System32` (constitutionally unwalked), so `DirectX Runtime` reports `present = true` (verified on disk) with `own_size_bytes = None` unless a standalone `DirectXRedist` entry exists.
