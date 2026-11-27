# Feature Specification: `routing-cleanup-2`

**Slug**: `routing-cleanup-2`  
**Status**: Implemented  
**Baseline Commit**: `06770888e8353525d71b666a75f809f265428da8`

## Summary of Changes

1. **Part E — Split Adobe Shared Containers & Family-Label Residual Pool ([discovery.rs](file:///Y:/Repos/y-storage/crates/y-core/src/discovery.rs), [tree.rs](file:///Y:/Repos/y-storage/crates/y-core/src/tree.rs), [attribution.rs](file:///Y:/Repos/y-storage/crates/y-core/src/attribution.rs))**:
   - Removed the hardcoded `"app:adobe-creative-cloud"` synthetic entry from `known_app_fallbacks`.
   - When any Adobe product app is installed (`is_adobe_product_app`), `ClaimIndex` splits Adobe shared containers (`\Common Files\Adobe`, `\AppData\Roaming\Adobe`, `\AppData\Local\Adobe`, `\AppData\LocalLow\Adobe`) so product-matching subfolders (`match_adobe_product_subfolder`, excluding shared infra folders like `UXP`, `CEP`, `Color`, `OOBE`, `Adobe Desktop Common`, `Adobe PCD`) attribute directly to their owning Adobe product app.
   - Unidentifiable residue (`\Common Files\Adobe`, `\AppData\*\Adobe`, `AppData\Roaming\com.adobe.dunamis`, and loose files inside them) routes to a genuinely registered `"Adobe Creative Cloud"` app if present; otherwise it pools under synthetic family row `app:adobe-shared-data` (`"Adobe (shared data)"`, publisher `"Adobe Inc."`) — never a product name.

2. **Part F — Evidence-Backed NO-TOUCH `"Service"` Classification ([discovery.rs](file:///Y:/Repos/y-storage/crates/y-core/src/discovery.rs), [attribution.rs](file:///Y:/Repos/y-storage/crates/y-core/src/attribution.rs), [scanner.rs](file:///Y:/Repos/y-storage/crates/y-platform-windows/src/scanner.rs))**:
   - Removed name-pattern wildcards (`n.ends_with(" service")` and `(p.contains("asus") && ...)`) from `classify_no_touch`, retaining only the explicit curated service list (`xbox service`, `framework service`, `armoury crate service`, `gamesdk service`, `rog live service`, `aura service`, `aura lighting effect`, `update health tools`).
   - Added `has_windows_service_registration_evidence`, `classify_no_touch_with_services`, and `build_registry_catalog_with_services`, and wired enumerated `HKLM\SYSTEM\CurrentControlSet\Services` entries into `execute_scan_internal` and `AttributionOptions::services`.
   - Service registration grants `NoTouchCategory::Service` only when kernel/driver evidence (`service_type & 0x03`) or Win32 service evidence (`service_type & 0x30`) proves the app itself is a registered service/driver — either its own primary binary stem equals the service binary stem, or the app explicitly identifies as a service matching the registered service name/path — preventing desktop GUI applications that bundle helper updater services (`TeamViewer`, `Chrome`, `Edge`, `Tailscale`, `Logitech G HUB`, etc.) from being misclassified.

3. **Part G — Audit & Documentation of System Package GUID Prefixes and `.NET` MSI Major Encoding ([attribution.rs](file:///Y:/Repos/y-storage/crates/y-core/src/attribution.rs))**:
   - **G(1) System-Package GUID Prefixes**: Verified against `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Appx\AppxAllUserStore\InboxApplications` and `C:\Windows\SystemApps\*\AppxManifest.xml` and documented inline in `attribute_store_package` (no behavior change):
     - `1527c705-` → `Microsoft.Windows.FilePicker_cw5n1h2txyewy`
     - `c5e2524a-` → `Microsoft.Windows.FileExplorer_cw5n1h2txyewy`
     - `e2a4f912-` → `Microsoft.Windows.AppResolverUX_cw5n1h2txyewy`
     - `f46d4000-` → `Microsoft.Windows.AddSuggestedFoldersToLibraryDialog_cw5n1h2txyewy`
   - **G(2) Package Cache `.NET` MSI-Major Mapping**: Documented Microsoft's WiX MSI `ProductVersion` major-byte encoding inline (`SDK Toolset/Templates: Major * 4 + Minor` → `28` = `7.0`, `40` = `10.0`; `Runtime/Host/Targeting/ASP.NET: Major * 8 + Minor` → `48` = `6.0`, `56` = `7.0`, `64` = `8.0`, `80` = `10.0`; `Workload Manifests: Major * 8 + 1` → `81` = `10.0.100`).
     - **Proven Mapping Fix**: Auditing every installed `.NET` entry uncovered 4 `.NET 10.0.100` workload manifest MSIs (`v81.200.61678`, total `2,564,096 B`) whose display names contain `-10.0.100` AND `.net6`/`.net7` target-framework tokens (`Microsoft.NET.Workload.{Mono.Toolchain,Emscripten}.{net6,net7}.Manifest-10.0.100`). Because unversioned `net6`/`net7` substring matching ran before `10.` version and MSI major `81` checks, all 4 mis-routed into `Microsoft .NET SDK 7.0.407`. Reordered `inferred_sdk_ver` to evaluate explicit version tokens (`10.`/`26.`/`36.` vs `6.`/`7.`) and MSI major codes (`28|56` vs `40|80|81`) before unversioned TFM substrings.

## Logged Assumptions

- **`A-001` (Guarded Win32 Service Evidence Scope)**: Requiring either primary-binary-stem equality (`img_stem == app_name`) or explicit service naming (`Service`/`Svc`) on Win32 (`0x10`/`0x20`) services reflects owner intent so ordinary GUI applications (`Chrome`, `Edge`, `TeamViewer`, `Tailscale`, `EA app`, `Epic Games Launcher`) are never turned `NO-TOUCH · Service` by their updater/elevation helper services.
- **`A-002` (Separate Audit Fix in Part G(2))**: Fixing the `Manifest-10.0.100` (`v81.200.61678`) `.NET` workload routing bug obeys Part G's condition (*"NO behavior change unless verification proves a mapping wrong on the owner's machine; any such change must be reported separately with before/after bytes"*).
