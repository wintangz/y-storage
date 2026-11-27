# Plan: `attribution-engine-2`

1. Capture pre-change 4-drive live baseline (`before_ae2_4drive.json`) and back up `config.toml`.
2. Extend `y-core::model` & `y-core::config` with `ZoneRulebook` (`AppContainer`, `PersonalFiles`, `FolderPin`), `ManifestAppEntry`, `ContainerLooseFiles`, new `LocationQualifier` / `AttributionBasis` variants, and `carved_out_apps` / `is_loose_files` / `applied_zones`.
3. Update `y-core::dependencies` (`parse_epic_manifest_item`), `y-core::tree` (arbitrary-count nested carve-out, `PersonalFiles` carve-out splitting, real-path `ContainerLooseFiles`, two-segment split assist, three-way group/wrapper/pin splitting), and `y-core::attribution` (bottom-up 7-step claim resolution, `carved_out_apps`, zero `[files]` paths).
4. Update `y-platform-windows::scanner` (combined personal shell + `ZoneRulebook`, Epic + Steam `ManifestAppEntry`, generalized candidate `.exe` probing across drive roots / app containers / 2 levels deep, retention for nested carve-outs & explicit rules/pins).
5. Update `apps/y-storage` Settings (`Attribution Zone Rulebook` with both zone types + manual pins), app drawer (`"contains <Child> — counted under <Child>"` + loose-files qualifiers), and `--view settings-zones` / `--view group-node-fixture` screenshot modes + `DESIGN.md` Part II `§11`.
6. Add comprehensive synthetic `FolderNode` end-to-end fixture suite in `crates/y-core/tests/round5_tests.rs`.
7. Verify `cargo test --workspace` (0 warnings), `cargo build --release --bin y-storage` (0 warnings), live 4-drive & `C:`-only scans, inspect dark verification PNGs, deploy to `Y:\Apps\y-storage\y-storage.exe`, write `verification.md`, and commit on `main`.
