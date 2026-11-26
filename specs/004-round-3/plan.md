# Implementation Plan: `y-storage` Round 3

**Branch**: `004-round-3` | **Slugs**: `attribution-coverage-3`, `catalog-embed`

## Technical Approach
1. **`catalog-embed` (`crates/y-core/src/{model,dependencies}.rs`, `crates/y-platform-windows/src/dep_scan.rs`, `apps/y-storage/src/ui.rs`)**:
   - Add `#[serde(deny_unknown_fields)]` to catalog structs and add `CatalogSourceKind` + `CatalogLoadResult` in `dependencies.rs`.
   - Load external overrides ONLY relative to `current_exe().parent()` (`dependency_catalog.toml`, then legacy `data/dependency_catalog.toml`), falling back to `EMBEDDED_DEPENDENCY_CATALOG` (`include_str!("../../../data/dependency_catalog.toml")`) when missing or invalid, returning a descriptive warning naming the override file when invalid.
   - Surface `catalog_source` and `warnings` on `ScanReport`, emit warnings to `stderr` + `%LOCALAPPDATA%\y-storage\y-storage.log`, and render any active warnings in `ui.rs` without visual restyling.
2. **`attribution-coverage-3` (`crates/y-platform-windows/src/scanner.rs`, `crates/y-core/src/{tree,attribution}.rs`)**:
   - Retain depth-6 children of `AppData\Local\Microsoft\Windows` in `scanner.rs` and add container splits in `tree.rs` (`\appdata\roaming\google`, `\appdata\local|roaming\jetbrains`, `\appdata\locallow\colossal order`, `\appdata\local\microsoft\windows`, `Users\<User>\Microsoft`).
   - Implement general `<Publisher>\<Product><version>` rule (`match_versioned_publisher_product`), general `<X>Data` rule (`match_xdata_folder`), `Playwright` & `GitKraken CLI` standalone entries, `TabNine`, `Jellyfin`, `Store: AppleInc.iTunes` (`Apple Computer`), `Postman`, `Microsoft Power BI Desktop`, `Adobe Creative Cloud`, `Cities: Skylines II`, `Riot Client`, and `Local\Microsoft\Windows` cache/registry classification.
   - Preserve strict `0 B` drive reconciliation and zero dependency size inflation.

## Verification Strategy
- `cargo test --workspace` with new `crates/y-core/tests/round3_tests.rs`.
- Isolated scratch-folder CLI verification of `y-storage.exe` for `catalog-embed` (no sidecar / valid modified override / corrupt override + warning).
- Live release `--scan-cli` audit of `attribution-coverage-3`, `C:\` `0 B` reconciliation, `FIFA 23` / `EA App + FIFA 23` totals, and scan speed (`~9 s`).
