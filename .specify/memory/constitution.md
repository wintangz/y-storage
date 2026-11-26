# y-storage Constitution

## Core Principles

### I. Slug-Only Feature Identity
Every feature in specifications, plans, tasks, code documentation, commits, and status reports MUST be referenced strictly by its canonical slug:
- `feasibility-spike`
- `app-discovery`
- `drive-scanner`
- `attribution-zones`
- `scan-history`
- `storage-ui`
- `dependencies`
- `attribution-coverage-2`
- `ground-truth`
- `attribution-coverage-3`
- `catalog-embed`

### II. Every GB Has an Owner & Exact Drive Reconciliation
After a scan, every scanned folder on an enabled drive MUST belong to exactly one owner: an attributed installed app (`InstalledApp`), a standalone user-profile dot-folder or toolchain (`StandaloneFolder`), `Shared & system caches (GREEN)`, `Your files`, `Windows & system`, or an openly displayed `Unattributed` group. Nothing is silently dropped, double-counted, or force-matched; every app attribution records its explicit basis (`InstallLocation`, `ExactName`, `Publisher`, `Token`, `PackageFallback`, `StandaloneFolder`). Shared dependencies NEVER inflate any dependent app's `total_bytes`. On every enabled drive, `apps_on_drive + shared_caches_on_drive + your_files_on_drive + unattributed_on_drive + windows_and_system_on_drive + free_space == drive_total`, with the `Windows & system` remainder line (`drive_used - everything_scanned`, marked as an estimate) absorbing unwalked system files and filesystem overhead.

### III. Permanent No-Touch & Zone Integrity
Drivers, services, anti-cheat software, system runtimes (VC++, DirectX, .NET, Windows SDK), and registry hives (`NTUSER.DAT`) are permanently display-only (`NO-TOUCH`) across all versions. Every app breakdown item carries a clear zone tag:
- `BODY`: program binaries and install directories (including `Local\Programs` per-user installs).
- `GREEN`: safely regenerable caches (`Temp`, `D3DSCache`, `NVIDIA\DXCache`, `AMD\DxCache`, `CrashDumps`, `Package Cache`, shader caches).
- `RED`: real user/app data (browser profiles, `Local\Packages` including Store game saves, app data folders, toolchain homes); `AppData\Roaming` defaults to `RED · Roaming config`.
- `ProgramData` entries additionally carry the `ProgramData (Machine-shared)` qualifier.
Personal shell folders (`Documents`, `Downloads`, `Pictures`, `Music`, `Videos`, `Desktop`, `OneDrive`, `Saved Games`) are grouped under `Your files` — never attributed to apps, never zone-labelled, and never actionable.

### IV. Read-Only On-Demand Low-Impact Execution
`y-storage` v1 is strictly read-only: scan and report only, with zero delete, move, or junction actions anywhere in the UI or engine, and zero writes outside `%APPDATA%\y-storage\` and `%LOCALAPPDATA%\y-storage\`. Never walk a `Windows` folder or `WinSxS`; size root `hiberfil.sys` / `pagefile.sys` / `swapfile.sys` directly into `Windows & system`; never follow symlinks or directory junctions (`FILE_ATTRIBUTE_REPARSE_POINT`). Scans run only on demand (`Rescan`) at low I/O priority (`THREAD_MODE_BACKGROUND_BEGIN`), with folder sizes cached between scans.

### V. Lightweight Single-Binary Delivery & Spec-Driven Verification
The deliverable is a single standalone Windows executable (`y-storage.exe`) built with pure-CPU `egui` + GDI DIB `SoftwarePainter` (zero WebView2/GPU driver heaps), zero modal popups (`NIF_INFO` and `MessageBoxW` forbidden), compile-time embedded runtime data (`dependency_catalog.toml` via `include_str!`) with optional validated sidecar override next to the executable (logging a warning and falling back to the embedded copy on invalid overrides), and full `specify -> plan -> tasks -> implement -> test` verification before reporting completion.

**Version**: 1.0.0 | **Ratified**: 2026-11-26 | **Last Amended**: 2026-11-26
