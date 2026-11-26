# Tasks: `table-layout-1`

- [x] T001 Update `.specify/memory/constitution.md` with `table-layout-1`
- [x] T002 Extend `DESIGN.md` shared hierarchy/table rules, `y-storage` section, and `C1`–`C12` checklist
- [x] T003 Create Spec Kit docs (`spec.md`, `plan.md`, `tasks.md`) under `specs/009-table-layout-1/`
- [x] T004 Implement `MainTableColumns`, `LocationsTableColumns`, `DependenciesTableColumns`, and header helpers (`draw_sortable_col_header`, `draw_static_col_header`) in `apps/y-storage/src/ui.rs`
- [x] T005 Remove standalone `Sort:` segmented bar from Header Row 3 in `apps/y-storage/src/ui.rs`
- [x] T006 Convert `draw_main_app_list` in `apps/y-storage/src/ui.rs` to fixed-column table with sticky sortable header, hover highlight, and aligned `Locations` + `Dependencies` sub-tables
- [x] T007 Run `cargo test --workspace`, `cargo build --release --workspace`, and one live CLI scan verification
- [x] T008 Render and visually inspect all 4 `1020x720` screenshots (`main-dark`, `main-light`, `main-growth-sort`, `expanded-fifa23-dark`)
- [x] T009 Write `specs/009-table-layout-1/verification.md`, copy release exe to `Y:\Apps\y-storage\y-storage.exe`, and commit cleanly on `main`
