# Feature Specification: `store-minecraft-icon-fix`

**Slug**: `store-minecraft-icon-fix`  
**Status**: Implemented  
**Baseline Commit**: `68633bb587c8208f7c2d07949b557b804a426ee5`

## Summary of Fixes

1. **Part A — Store Package Backup Suffix Merging ([attribution.rs](file:///Y:/Repos/y-storage/crates/y-core/src/attribution.rs))**:
   - In `attribute_store_package`, extract `(stem, pub_id)` via `resolve_store_package_stem_and_pub_id` so trailing backup suffixes on `AppData\Local\Packages\<Stem>_<PubId>_<Suffix>` (explicitly covering `_bak`, `_old`, `_backup`, `_copy` with optional numeric suffix, plus the general rule that any trailing non-publisher-ID segment following a publisher-ID-shaped segment resolves back to that publisher ID) merge into the primary `store:<stem>_<pub_id>` synthetic app as separate location entries with real paths and byte sizes.
   - `Program Files\WindowsApps` 5-segment names (`<Stem>_<Version>_<Arch>_<ResourceId>_<PubId>`) remain untouched via `is_windowsapps_five_segment_form`.

2. **Part B — Per-Product Minecraft Routing ([attribution.rs](file:///Y:/Repos/y-storage/crates/y-core/src/attribution.rs))**:
   - Replaced the single spike-era `"Minecraft / Xbox"` entry routing all 6 Minecraft/Xbox patterns to `xbox service` with per-product routing:
     - **Bedrock (`Store: Microsoft.MinecraftUWP`)**: `C:\XboxGames\Minecraft for Windows` (`ZoneKind::Body`) and `AppData\Roaming\Minecraft Bedrock` (`ZoneKind::Red`) resolve directly to the `Microsoft.MinecraftUWP_8wekyb3d8bbwe` Store synthetic app via `attribute_store_package`, consolidating all 4 Bedrock locations cleanly regardless of unit scan order.
     - **Java / Launcher (`Minecraft Launcher`)**: `C:\XboxGames\Minecraft Launcher` (`ZoneKind::Body`), `AppData\Roaming\.minecraft` (`ZoneKind::Red`), and `AppData\Local\MinecraftInstaller` (`ZoneKind::Red`) route to `"Minecraft Launcher"` when present; if no `"Minecraft Launcher"` app exists in a report, they fall through to normal claim resolution (`Unattributed`), never to `"Xbox Service"`.
     - **`C:\XboxGames\GameSave`**: Dropped from the routing table (see **Logged Assumptions** `A-001`).

3. **Part C — Cross-Drive & MSIXVC App Icon Extraction ([icons.rs](file:///Y:/Repos/y-storage/crates/y-platform-windows/src/icons.rs))**:
   - Ordered diagnosis result:
     - **Step (1)**: In the owner's cached report (`%LOCALAPPDATA%\y-storage\last_scan.json`, `C:` + `Z:` enabled, `Y:` + `D:` unchecked), `reg:ollama` (`120,008,870 B` on `C:`) carried `icon_rgba: None` (`install_location: None`, `display_icon: Some("Y:\\Apps\\Ollama\\unins000.exe")`), whereas on an all-drives scan `reg:ollama` carried `icon_rgba: Some(18×18)` and painted cleanly at 125% DPI — proving **Step (2b)** extraction loss rather than **(2a)** painter premultiplication.
     - **Step (2b) Root Cause**: `populate_report_icons` and `extract_app_icon_cached` rejected icon candidates (`display_icon` and `reg_icon_map` lookups) whenever their drive letter belonged to `excluded_drives`, and `app.install_location` was `None` when the install root lived on an unchecked drive — stripping real icons from 9 multi-drive apps (including Ollama, VS Code, Unity, Unity Hub, RustDesk, Rockstar Games Launcher).
     - **General Fix**: Drive-scope exclusions govern disk walking and byte attribution, not UI icon lookup for apps already present in `report.apps`. Removed the `excluded_drives` gate from `populate_report_icons` and `extract_app_icon_cached`, supplied non-mutating registry `install_location` fallback search roots to `collect_icon_candidates`, and extended `find_appx_logo_pngs` to inspect `Content\AppxManifest.xml` (resolving `<Logo>` relative to the manifest's parent folder) for MSIXVC XboxGames titles. Zero per-app or `"Ollama"` special cases.

4. **Part D — Unpin EA Shared Folders from FIFA 23 ([attribution.rs](file:///Y:/Repos/y-storage/crates/y-core/src/attribution.rs))**:
   - Updated both the EA platform companion entry and `ProgramData\Frostbite` in `find_by_token` so their sole named target is `&["ea app"]`, without altering path lists or any other routing rule. When `EA App` is absent, both fall through rather than pinning to `"ea sports"` or `"fifa 23"`.

## Logged Assumptions

- **`A-001` (`C:\XboxGames\GameSave` dropped from routing)**: Direct inspection of `C:\XboxGames\GameSave` on the owner's machine showed only two empty Xbox Gaming Services cloud-save sync directories (`pgs`, `wgs`; `0` files, `0 B` total), while Minecraft Bedrock's `1,219,685,530 B` saves reside under `C:\Users\Win\AppData\Roaming\Minecraft Bedrock`. Per Part B(3), `c:\xboxgames\gamesave` is therefore dropped from the routing table.
- **`A-002` (Windows 11 MSIXVC `Minecraft Launcher` pre-seeding)**: Because `C:\XboxGames\Minecraft Launcher` (`Content\appxmanifest.xml`, `<DisplayName>Minecraft Launcher</DisplayName>`, `<PublisherDisplayName>Microsoft Studios</PublisherDisplayName>`) has no Win32 Uninstall Registry entry and NTFS enumerates `C:\Users\...` before `C:\XboxGames\...`, `attribute_scan_with_options` pre-seeds `"Minecraft Launcher"` (`id = "xbox:minecraft launcher"`) ONLY when a non-zero `c:\xboxgames\minecraft launcher` unit is present in the scanned non-excluded units. Reports lacking both a registered `"Minecraft Launcher"` and `C:\XboxGames\Minecraft Launcher` create no synthetic bucket and leave `.minecraft` / `MinecraftInstaller` unowned by `Xbox Service`.
- **`A-003` (`FIFA 23` Part-D Live-Disk `0 B` Delta)**: On the owner's live system, `EA app` is installed (`reg:ea-app`) and `C:\ProgramData\Frostbite\FIFA 23` contains `0` files (`0 B`), while `EA app` already claimed the non-empty EA/Origin shared folders ahead of `fifa 23`. Therefore live-disk `FIFA 23` remains `107,199,710,486 B` before and after (`0 B` moved on disk); non-zero `ProgramData\Frostbite` (`50,000,000 B`) and `ProgramData\Origin` (`12,148 B`) behavior with and without `EA app` is proven directly in integration fixtures `d1` and `d2`.
