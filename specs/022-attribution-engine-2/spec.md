# Feature Specification: `attribution-engine-2`

**Slug**: `attribution-engine-2`  
**Status**: Implemented

## Part A — Unified Zone Rulebook

Generalize the FIX 6 container list into ONE `ZoneRulebook` configured in a single Settings panel (`Attribution Zone Rulebook`). Each rule matches either a top-level folder name at a drive root or an explicit full folder path:
- **App container (`AppContainer`)**: immediate subfolders are app candidates (`Apps`, `Games`, `Game`, `Programs`, `Program Files`, `Software`, `Softwares`, `PortableApps` by default).
- **Personal files (`PersonalFiles`)**: the folder and its uncarved subtree route to `Your files` (never app-hunted, never `Unattributed`). Defaults: `Documents`, `Downloads`, `Pictures`, `Videos`, `Music`, `Desktop`. Explicit personal paths (such as `D:\Media`) are added/edited in the same rulebook.
- **Drive Root Rule**: every top-level folder directly at a scanned drive root (`<D>:\<Folder>`) is an app candidate evaluated after registered/manifest claims via Part B three-way tests (covering root-installed software such as `C:\Riot Games` or `D:\FIFA Mod Manager`), in addition to FIX 6(d).
- **Precedence**: Registered install locations inside a `PersonalFiles` zone keep their app ownership and are carved out of the personal zone's byte total; zones govern only folders without a stronger claim. Zone changes persist and mark the cached report stale (`Rescan needed`).

## Part B — Bottom-Up Claim Resolution

Resolve claims from deepest/most specific to least, never outermost-first:
1. **Registered Roots First + Nested Carve-Out**: Registered install roots (`Registry` / `Store`) claim their exact root at any depth; nested/contained child install roots are carved out of outer install roots at **any** nesting count (`>= 1` even when the outer directory is itself an install root), subtracting carved bytes with zero double-counting and recording contained child app names on the parent (`"contains <Child> — counted under <Child>"`).
2. **Manifest Containers Next**: Steam (`appmanifest_*.acf`) and Epic (`*.item`) manifest files define game membership; non-member sibling folders (`workshop`, `downloading`, caches, unmanifested siblings) are not promoted as manifest games.
3. **Remainder Units Keep Real Paths**: Loose direct files inside any split directory emit `ScannedUnitKind::ContainerLooseFiles` with the real directory path (never fabricated `"\[files]"` or `"\[root files]"` paths). When matching an app's install location, the item is labeled `loose files in the install folder`; otherwise non-app remainders keep their real path cleanly for `Locate` / triage.
4. **Two-Segment `Publisher\Product` Composition**: Before leaf-only rules, compose the last two path segments in order with whole-token matching against installed apps' full names (`"publisher+folder match"`), requiring the parent segment to whole-token match the app's registered publisher OR a unique full-name match among installed apps (any tie leaves the folder unclaimed).
5. **Three-Way Inference Last**: On still-unclaimed candidate folders inside an app container or at a drive root:
   - **(a)** Own-level app evidence (flat main `.exe` with optional PE `ProductName`, or step 1–2 claim) -> promote as app and stop descending for inference (never blocks Step 1 nested carve-outs).
   - **(b)** No own-level evidence AND `>= 2` immediate children pass (a) -> **Group node** (publisher folder): descend into children and record the group folder name as publisher context.
   - **(c)** No own-level evidence AND `== 1` child passes (a) -> **Single-app wrapper**: keep the outer folder as the app.
   - **(d)** Neither -> honest fallback (`Shared & system caches`, `Windows & system`, or `Unattributed`).
6. **Containment Is Weakest**: Enclosing `InstallLocation` claims only still-unclaimed residue left after steps (1)–(5).
7. **Manual Pins (Final Word)**: Per-folder pins (`App`, `Group`, `Personal`, `Ignore`) in `ZoneRulebook` override inference and persist across scans.

## Logged Assumptions

- **A-001 (Backwards-Compatible Config Migration)**: Existing `config.toml` files carrying legacy `[containers]` without `[zones]` migrate automatically on load into `ZoneRulebook` (`root_folder_names` + `explicit_paths` as `AppContainer` entries alongside the 6 default `PersonalFiles` entries).
- **A-002 (Legacy Shell Folder Compatibility)**: Unit tests and callers using `decompose_scanned_tree(root, apps, shell_folders, root_sys)` continue honoring explicit `shell_folders` parameters alongside `ZoneRulebook` rules, while Windows scans combine OS shell folders with active `PersonalFiles` zone entries so removing a default shell zone from Settings takes effect cleanly.
- **A-003 (Live `D:\Media` Rule Configuration)**: Because `D:\Media` is the user's personal media directory rather than a hardcoded built-in constant, adding `D:\Media` as an explicit `PersonalFiles` zone entry in `%APPDATA%\y-storage\config.toml` after implementation (without altering drive enablement or window state) demonstrates the Settings-driven workflow cleanly on live disk while keeping Rust defaults free of machine-specific paths.
