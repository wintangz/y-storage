# Implementation Plan: `polish-fix-2`

1. Author `data/dependency_catalog.toml` (FIX 1) & `data/app_categories.toml` (FIX 8) + `.specify/memory/constitution.md` + `specs/014-polish-fix-2/`.
2. Update `y-core` (`model.rs`, `config.rs`, `categories.rs`, `dependencies.rs`, `lib.rs`) & `y-platform-windows` (`dep_scan.rs`, `scanner.rs`).
3. Update `apps/y-storage` (`list_query.rs`, `ui.rs`, `win32_host.rs`, `main.rs`) and [DESIGN.md](file:///Y:/Repos/y-storage/DESIGN.md).
4. Add unit tests for FIX 1, FIX 6, FIX 7, and FIX 8; run `cargo test --workspace` & `cargo build --release --workspace`; execute live scan & extract FIX 1 sanity sweep + FIX 8 coverage report; render & visually inspect all required screenshots including `200%` centering crop; deploy `Y:\Apps\y-storage\y-storage.exe`; write `verification.md` & commit on `main`.
