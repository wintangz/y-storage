# Feature Specification: `drive-scope-coverage-1`

**Slug**: `drive-scope-coverage-1`  
**Status**: Implementing

## Requirements

1. **FIX 1 — Natural Web Search Query**: Replace the quote-fragment template (`"<folder>" "<parent>" "what is this folder"`) in `apps/y-storage/src/unattributed_assist.rs` with natural phrasing: `what is the {folder name} folder in {parent folder name}`, or `what is the {folder name} folder` when sitting directly at a drive root. Update `DESIGN.md §11` accordingly.
2. **FIX 2 — Scope Editing on the Drive Pods**: Each drive pod gains in-place scope controls synced bidirectionally with `Settings`: (a) include/exclude toggle (`Not scanned` + dimmed card + greyed bar when excluded), and (b) an anchored per-pod exclusions overlay with add/remove. Scope changes mark the current report stale (`Rescan needed` on the header primary action) instead of silently rescanning.
3. **FIX 3 — Per-Drive Rescan**: Each included drive pod exposes its own `Rescan` action that scans ONLY that drive and merges fresh data into the current report without mutating other drives' data, stamps per-pod last-scanned times, disables only the active pod while scanning, records partial scan history snapshots naming refreshed drives, and computes growth deltas per drive against that drive's own prior scan.
4. **FIX 4 — Ledger Covers Filter Row**: Raise the pinned storage ledger's max drag height to `max(156.0, client_height - masthead_height - 4.0)` so dragging it fully covers the drive pods, search box, filter chips, and facet selectors right up to below the masthead row.
5. **FIX 5 — Ledger Filtering Parity**: Add `SecondaryOutline` facet dropdowns (`Drive`, `Size`, `Zone`) with `Reset filters` and live `"N of M folders · <subtotal>"` to expanded `Unattributed`, keeping view-only filtering (group totals invariant) and leaving the three single-purpose groups without redundant facet bars.
6. **FIX 6 — Folder-Only Software & Customizable Container Rulebook**:
   - **(a)** Editable container rulebook in Settings (default names: `Apps`, `Games`, `Game`, `Programs`, `Program Files`, `Software`, `Softwares`, `PortableApps`, plus explicit full paths), claiming immediate child folders as individual apps (`layout rule: <container>`).
   - **(b)** Strict precedence: registry/manifest/curated attribution always wins; container rules claim only otherwise-unattributed folders with zero double-counting.
   - **(c)** Steam libraries discovered on every scanned drive via `steamapps\libraryfolders.vdf` + `appmanifest_*.acf`.
   - **(d)** Conservative drive-root executable inference (`executable scan (inferred)`, reading PE `ProductName`), and explicit OS system folder classification (`$RECYCLE.BIN`, `System Volume Information`, `Recovery`, `Config.Msi`, etc.) routed to `Windows & system`.
   - **(e)** Honest remainder: non-software folders (e.g. `D:\Media`, `Y:\AI Models`, `Y:\Repos`) stay `Unattributed`.

## Logged Assumptions

- **A-001 (Multi-Drive Hardlink Reconciliation)**: On volumes containing NTFS hardlinked files (`D:\Games\Steam\steamapps\common\FIFA 23\FIFAModData`, `Y:\Repos\...\node_modules`), directory tree walks sum logical entry sizes, making `walked_bytes > drive_used_bytes`. Rather than altering `FIFA 23`'s per-directory total, `SystemDriveBreakdown.hardlink_overlap_bytes` records `walked_sum - drive_used_bytes` so reconciliation checks `apps + caches + your_files + unattributed + win_sys - hardlink_overlap + free == drive_total` (`diff = 0 B`).
- **A-002 (`Program Files` OS Inbox Exclusions)**: When applying the default `"Program Files"` container rule on the system drive, Windows inbox skeleton folders (`Common Files`, `WindowsApps`, `Windows Defender`, `Windows Mail`, `Windows Media Player`, `Windows NT`, `Windows Photo Viewer`, `Windows Sidebar`, `WindowsPowerShell`, `Internet Explorer`, `IIS`, `IIS Express`, `ModifiableWindowsApps`, `Uninstall Information`, `Reference Assemblies`, `Microsoft`, `MSBuild`, `RUXIM`, `Application Verifier`, `XboxService`, `Activation-Renewal`) are not promoted as standalone user apps so third-party folders are surfaced cleanly.
