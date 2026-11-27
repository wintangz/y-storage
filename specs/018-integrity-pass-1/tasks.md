# Tasks: `integrity-pass-1`

- [x] Confirm STEP 0 diagnosis in `discovery.rs` and `attribution.rs`
- [x] Add `display_version`, `uninstall_guids`, and `installed_registry_names` across `model.rs` / `registry.rs`
- [x] Split versioned product families cleanly in `discovery.rs` and route `Package Cache` by GUID / version / arch in `attribution.rs`
- [x] Author version-specific VC++ / .NET / shared runtime rules in `data/dependency_catalog.toml` and expand `dep_scan.rs` / `dependencies.rs`
- [x] Surface prominent `"Used by N apps"` line in expanded shared-runtime drawers (`ui.rs`) + screenshot views (`main.rs`)
- [x] Add required FIX A unit tests, pass `cargo test --workspace`, verify live scan & dark screenshots, write `verification.md`, copy release `.exe` to `Y:\Apps\y-storage\`, and commit
