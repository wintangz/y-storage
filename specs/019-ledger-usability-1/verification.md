# Verification Report: `ledger-usability-1`

**Date**: 2026-11-27
**Status**: **PASS** (FIX 1–FIX 6, 44/44 workspace unit tests, live scan invariants, Dark + Light screenshots `(a)`–`(e)`)

---

## Summary of Fixes Verified

| Fix | Description | Status | Key Verification Evidence |
|---|---|---|---|
| **FIX 1** | Full-row hover highlight (`pal.widget_hover`) across expanded ledger rows (`draw_unattributed_item_row` & `draw_simple_item_row`) | **PASS** | `screenshot-unattributed-row-hover-dark.png` shows full-row hover highlight across path, action cells, and size while `Locate`/`Web search` keep `SubtleOutline`. Recorded in `DESIGN.md` Part I (`§6.2`). |
| **FIX 2** | Full-height ledger drag-resize (removes 50% cap) | **PASS** | Max height formula: `max(156.0, client_height - header_band_height - 4.0)` (`483.0 px` at `1020x720`, `96 DPI`, leaving a `4.0px` strip below the `233.0 px` header band). Unit test `ledger_full_height_resize_clamps_and_degrades_cleanly_at_default_and_min_window` passes at both `1020x720` and `760x520`. Verified in `screenshot-ledger-max-height-dark.png`. |
| **FIX 3** | Unattributed search + 4-mode sort + match count/subtotal | **PASS** | Live substring search (`filter_and_sort_unattributed`), `Clear`, 4 sort modes (`Size ↓` default, `Size ↑`, `Name A→Z`, `Name Z→A`), `"N of 304 folders · subtotal"`, and reset on collapse; group header total (`1.37 GB`) and `304 folders` stay unchanged. Verified in `list_query::tests::unattributed_filter_and_sort_preserves_totals_and_matches_substrings` and `screenshot-unattributed-filter-dark.png`. |
| **FIX 4** | Win32 `IDC_IBEAM` cursor over editable text inputs | **PASS** | Disabled `style.interaction.selectable_labels = false` globally so static labels never emit `CursorIcon::Text`; `draw_crisp_text_input` emits `CursorIcon::Text`; `win32_host.rs` handles `WM_SETCURSOR` (`HTCLIENT`) + post-frame `SetCursor` (`Text -> IDC_IBEAM`, `ResizeVertical -> IDC_SIZENS`, default/buttons -> `IDC_ARROW`). Verified in `ui::tests::text_input_sets_ibeam_cursor_while_buttons_do_not`. |
| **FIX 5** | Drive Pod redesign & `DESIGN.md` Part II sync | **PASS** | Plain `"C:\"` (`13.0pt` Strong) with zero icons/glyphs; single `11.0pt Consolas Regular` hierarchy (`"{used} of {total}"` + muted `"{free} free"` companion); `2.0px` flush WT gradient bar inside muted track; `8.0px` radius + `1px pal.border`; multiples-of-4 padding across all 4 pods. Verified in `screenshot-drive-pod-crop-200-dark.png`, `screenshot-drive-pods-dark.png`, `screenshot-drive-pods-light.png`. |
| **FIX 6** | General 7-facet dropdown bar (`Publisher`, `Category`, `Drive`, `Size`, `Deps`, `Zone`, `Growth` when baseline exists) + `"Reset filters"` | **PASS** | `SecondaryOutline` buttons remain neutral at rest and use accent only on hover/open/active; `AND`-combined with filter chips and search; persisted in `AppConfig.facets` with missing-value fallback (`sanitize_facets_for_report`). Verified in `list_query::tests::facet_filters_combine_with_and_and_fallback_cleanly_on_missing_values` and `screenshot-facets-microsoft-1gb-dark.png` (`8 of 269 apps`). |

---

## Automated Tests & Live Scan Invariants

- **`cargo test --workspace`**: **44 passed, 0 failed** (`y-core`: `6 + 6 + 6`, `y-platform-windows`: `2`, `y-storage`: `24`), zero compiler warnings in debug and release builds.
- **Live `--scan-cli` (ephemeral)**:
  - `apps.len()` = **269** (unchanged vs `integrity-pass-1`)
  - `EA SPORTS™ FIFA 23` = **51,372,195,955 B** with **7** dependencies (unchanged)
  - `C:\` reconciliation `diff` = **0 B**
  - `Unattributed` count = **304 folders** (`1,469,010,533 B` on live disk; `+1,356,626 B` from live OS activity during the session vs `1,467,653,907 B` in `integrity-pass-1`, zero attribution-logic change)
- **`--smoke-gui-ms 500`**: Loaded cached `269` apps cleanly (`working_set_bytes = 33,058,816 B`).

---

## Screenshots Inspected

- `(a)` `specs/019-ledger-usability-1/screenshot-unattributed-row-hover-dark.png`
- `(b)` `specs/019-ledger-usability-1/screenshot-ledger-max-height-dark.png`
- `(c)` `specs/019-ledger-usability-1/screenshot-unattributed-filter-dark.png`
- `(d)` `specs/019-ledger-usability-1/screenshot-drive-pod-crop-200-dark.png`, `screenshot-drive-pods-dark.png`, `screenshot-drive-pods-light.png`
- `(e)` `specs/019-ledger-usability-1/screenshot-facets-microsoft-1gb-dark.png`
