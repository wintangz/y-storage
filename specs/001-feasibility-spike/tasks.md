# Tasks: `feasibility-spike`

**Input**: `specs/001-feasibility-spike/spec.md`, `specs/001-feasibility-spike/plan.md`

- [x] T001 Initialize `y-storage` git repo, Spec Kit templates, `.specify/memory/constitution.md`, and workspace `Cargo.toml`
- [x] T002 Implement `crates/y-core` modules: `model.rs`, `discovery.rs`, `tree.rs`, `zones.rs`, `attribution.rs`, `config.rs`, `history.rs`, `cache.rs`
- [x] T003 Implement `crates/y-platform-windows` modules: `registry.rs`, `scanner.rs`, `paths.rs`, `lib.rs`
- [x] T004 Implement `crates/y-platform-windows/src/bin/spike_runner.rs` for spike items (1) registry enumeration, (2) default-scope scan speed, and (3) top-50 folder attribution coverage
- [x] T005 Run `cargo run --release --bin spike_runner` on the live dev machine, record proving numbers in `specs/001-feasibility-spike/evidence.md`, and commit
