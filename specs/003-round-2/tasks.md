# Tasks: `y-storage` Round 2

**Input**: `specs/003-round-2/{spec.md,plan.md,data-model.md}`

- [x] T001 Update `.specify/memory/constitution.md` and author `specs/003-round-2/{spec.md,plan.md,data-model.md,tasks.md}`
- [x] T002 Author `data/dependency_catalog.toml` covering VC++/DirectX/.NET/WebView2 DLLs, major launchers, anti-cheats, fingerprints, and FIFA 23/EA App/Steam rules
- [x] T003 Implement `attribution-coverage-2` in `crates/y-core` (`shared_caches`, standalone dot-folders, `flutter`, verified token fixes, exact `0 B` reconciliation)
- [x] T004 Implement `dependencies` in `crates/y-core` (`pe.rs`, `dependencies.rs`, `model.rs`) and `crates/y-platform-windows` (`services.rs`, `dep_scan.rs`)
- [x] T005 Add minimal `storage-ui` views for forward dependencies, reverse dependencies (`ActiveView::Dependencies`), and `Shared & system caches (GREEN)` without restyling
- [x] T006 Add unit/integration tests in `crates/y-core/tests/round2_tests.rs` and run `cargo test --workspace`
- [x] T007 Execute independent `ground-truth` measurement via Windows Explorer Folder Properties on pre-fixed folder lists and verify Acceptance 1, 2, 3 + `C:\` reconciliation + scan speed
- [x] T008 Build release binary, deploy to `Y:\Apps\y-storage\`, write `specs/003-round-2/verification.md`, and commit to `main`
