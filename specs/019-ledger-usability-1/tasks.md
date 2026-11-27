# Tasks: `ledger-usability-1`

- [x] Persist `FacetFilters` in `y_core::config` and extend `list_query.rs` with Unattributed search/sort + facet filtering
- [x] Implement FIX 1 (ledger full-row hover `pal.widget_hover`) and update `DESIGN.md` Part I
- [x] Implement FIX 2 (full-height ledger resize, persistence clamp, and graceful 0-row main table)
- [x] Implement FIX 3 (Unattributed live search + 4-mode sort + match count/subtotal + collapse reset)
- [x] Implement FIX 4 (Win32 `IDC_IBEAM` via `WM_SETCURSOR` / `SetCursor` for text inputs only)
- [x] Implement FIX 5 (drive pod redesign to `DESIGN.md` tokens/type/2px bar) and update `DESIGN.md` Part II
- [x] Implement FIX 6 (7 `SecondaryOutline` facet dropdowns, AND combination, persistence/fallback, `"Reset filters"`)
- [x] Run `cargo test --workspace`, verify live scan zero-change invariants, inspect all screenshots `(a)`–`(e)`, write `verification.md`, copy release `.exe` to `Y:\Apps\y-storage\`, and commit
