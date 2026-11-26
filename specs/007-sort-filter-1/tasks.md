# Tasks: `sort-filter-1`

- [x] Register `sort-filter-1` in `.specify/memory/constitution.md` and author `spec.md`, `plan.md`, `tasks.md`.
- [x] Add `AppSortMode`, `SortDirection`, `AppFilterChip`, and persisted view-state fields to `crates/y-core/src/config.rs`.
- [x] Implement pure sort comparators, filter predicates, and unit tests in `apps/y-storage/src/list_query.rs`.
- [x] Update `apps/y-storage/src/ui.rs`, `win32_host.rs`, and `main.rs` with segmented Sort controls, radio Filter chips, live `"N of M apps"` count, `"new"` no-baseline marker, config persistence, and screenshot CLI flags.
- [x] Run `cargo test --workspace`, `cargo build --release --workspace`, one live scan, capture & inspect both required screenshots, deploy `Y:\Apps\y-storage\y-storage.exe`, write `verification.md`, and commit to `main`.
