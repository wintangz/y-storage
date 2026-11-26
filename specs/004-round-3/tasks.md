# Tasks: `y-storage` Round 3

**Input**: `specs/004-round-3/{spec.md,plan.md,data-model.md}`

- [x] T001 Update `.specify/memory/constitution.md` and author `specs/004-round-3/{spec.md,plan.md,data-model.md,tasks.md}`
- [x] T002 Implement `catalog-embed` (`dependencies.rs`, `model.rs`, `dep_scan.rs`, minimal warning banner in `ui.rs`) with strict override validation and fallback logging
- [x] T003 Implement `attribution-coverage-3` (`scanner.rs`, `tree.rs`, `attribution.rs`) covering versioned-product rule, `<X>Data` rule, `TabNine`, `Playwright` standalone, and remaining `>= 100 MB` unattributed folders
- [x] T004 Add unit/integration tests in `crates/y-core/tests/round3_tests.rs` and verify `cargo test --workspace` + release build
- [x] T005 Prove `catalog-embed` (no sidecar / modified override / corrupt override) in an isolated scratch folder and run full live scan verifying `attribution-coverage-3` + `0 B` reconciliation
- [x] T006 Write `specs/004-round-3/verification.md`, deploy release exe to `Y:\Apps\y-storage\`, and commit to `main`
