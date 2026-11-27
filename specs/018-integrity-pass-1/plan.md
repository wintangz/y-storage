# Implementation Plan: `integrity-pass-1`

1. **`y-core` (`model.rs`, `registry.rs`, `discovery.rs`, `attribution.rs`)**:
   - Carry `display_version` on `RawRegistryApp` and `InstalledApp`, `uninstall_guids` on `InstalledApp`, and `installed_registry_names` on `ScanReport`.
   - Implement `versioned_family_identity_key` and HKCU reflection deduplication in `discovery.rs`.
   - Route `Package Cache` bundles and versioned MSI components by exact GUID first, then semantic version + arch match in `attribution.rs`.
2. **Dependency Engine & Catalog (`data/dependency_catalog.toml`, `dependencies.rs`, `dep_scan.rs`)**:
   - Author version-specific VC++ (`vcpp-2008`, `vcpp-2010`, `vcpp-2012`, `vcpp-2013`, `vcpp-v14`) including UWP `*_app.dll` imports, version-specific `.NET` (`dotnet-6`..`dotnet-10` + `dotnet-runtime` fallback), and documented shared runtimes (`openal`, `physx-runtime`, `python-runtime`, `winui-xaml`, `dotnet-native`, etc.).
   - Expand `dep_scan.rs` traversal & fallback directory scanning, `.deps.json`/`.runtimeconfig.json` TFM parsing, and manifest coalescing.
3. **UI (`apps/y-storage/src/{ui.rs, main.rs}`)**:
   - Map `CatalogTarget.registry_matches` onto `CachedReportData` so split VC++ / .NET / WebView2 / OpenAL / PhysX / Xbox / Launcher rows render a prominent `"Used by N apps"` header line in their expanded drawer.
   - Add `--view vcpp-split` and `--view vcpp-drawer` screenshot views.
4. **Tests & Verification**:
   - Update `round2_tests.rs` for `vcpp-v14` and add `vc_package_cache_lands_on_its_own_versions_row` + `split_family_row_sums_reconcile_with_former_merged_total`.
   - Run `cargo test --workspace`, build release, run live scan & verify byte conservation (`diff = 0 B`, `FIFA 23 = 51,372,195,955 B` with `7` deps), capture dark screenshots, copy release binary to `Y:\Apps\y-storage\`, and commit.
