# Verification 032 — `settings-layout`

**Scope:** Settings view layout-only refactor (`apps/y-storage/src/ui.rs` only — zero changes to `crates/y-core` or `crates/y-platform-windows`; zero colour or theme-token changes; no screenshots captured per round instructions).

---

## 1. Per-Item Verdict

| # | Requirement | Status | Implementation |
|---|---|---|---|
| 1 | Left section list + max-width content pane; narrow collapses to stacked single column in same order | **PASS** | `SettingsSection` (`Appearance & Theme`, `Scan Scope & Exclusions`, `Attribution Zone Rulebook`); `draw_settings_nav_rail` (`SETTINGS_NAV_W = 216.0`) + `SETTINGS_CONTENT_MAX_W = 860.0` at `total_w >= SETTINGS_WIDE_MIN_W` (`760.0`); collapses to single stacked column in identical order below `760.0` |
| 2 | One shared form grid (`input \| selector \| action`); rulebook's 3 add-forms as 3 rows in one `"Add"` block | **PASS** | `compute_settings_form_grid` + `draw_settings_form_grid_row` with `FORM_GRID_ACTION_W = 112.0`, `FORM_GRID_SELECTOR_W = 228.0`; all 3 rulebook add rows (`zone_name_input`, `zone_path_input`, `zone_pin_input`) share the `"Add"` block and align on one vertical axis |
| 3 | Drive pods size to content, equal height per row; `"Include in scope"` beside drive name; exclusion input + Add = 1 grid row; `C:` Windows-lock compact badge on title line | **PASS** | `drive_pod_natural_inner_h` equalizes paired row `min_inner_h`; title line places `"Drive X:\\"`, `"Include in scope"` checkbox, and `"Windows locked"` badge (`StatusKind::Danger` + full-path hover tooltip) inline; exclusion input + `"Add exclusion"` uses `draw_settings_form_grid_row(..., has_selector = false)` |
| 4 | 2 chip groups (`App containers` / `Personal files`) side by side when width allows, stacked when not; chips flow within their column | **PASS** | `draw_rulebook_section` renders 2 equal columns (`SETTINGS_TWO_COL_MIN_W = 560.0`) with per-column chip wrapping (`max_row_w = col_w`), collapsing vertically when narrower |
| 5 | One spacing scale across the page; headings sit close to content | **PASS** | Strictly `{SPACE_4, SPACE_8, SPACE_12, SPACE_16}` (`4, 8, 12, 16`); `SPACE_4` heading-to-subtitle, `SPACE_8` subtitle-to-content, `SPACE_12` card padding/inner blocks, `SPACE_16` section/pane gaps |

---

## 2. Control Inventory (Before vs. After — Removed: None)

| Section | Control / Binding / Action | Preserved | Notes |
|---|---|---|---|
| **Navigation** | Left section selector (`SettingsSection::ALL`) | Added | Switches active pane in wide mode; `settings_focus_containers` selects `Rulebook` |
| **Appearance & Theme** | Mode chips (`Dark`, `Light` -> `UiAction::SetTheme`) | **Yes** | Unchanged |
| | 7 preset swatches + `Custom` swatch (`UiAction::SetTheme`) | **Yes** | Unchanged |
| | Conditional `Custom` hex input (`custom_theme_hex`), `R`/`G`/`B` sliders (`0..=255`), `"Enter #RRGGBB"` warning chip | **Yes** | Unchanged |
| | 4-step shade ladder preview + `Text/icon contrast` readout | **Yes** | Unchanged |
| | Conditional `"Near status hue"` warning chip + semantic note | **Yes** | Unchanged |
| **Scan Scope & Exclusions** | Per-drive `"Include in scope"` checkbox (`&mut target.enabled` -> `SaveConfig`) | **Yes** | Moved onto title line beside `"Drive X:\\"` |
| | Drive stats caption + exact-bytes hover tooltip | **Yes** | Unchanged |
| | `C:\Windows` lock status (`Windows locked` badge + `"Always excluded"` hover) | **Yes** | Compacted onto title line; `target.exclusions` unchanged |
| | Exclusion rows (path + `"Remove"` -> `target.exclusions.remove`) | **Yes** | Unchanged |
| | Exclusion input (`exclusion_input`, `new_exclusion_inputs`) + `"Add exclusion"` | **Yes** | Uses shared form grid (`has_selector = false`) |
| **Attribution Zone Rulebook** | `"Reset defaults"` button (`draft.zones != ZoneRulebook::default()`) | **Yes** | Unchanged |
| | Root zone chips (`App containers` / `Personal files`) + `"Remove"` | **Yes** | Side-by-side 2 columns when wide, stacked when narrow |
| | Explicit folder zones (`App containers:` / `Personal files:`) + `"Remove"` | **Yes** | Unchanged |
| | Manual folder pins (`pin.path`, neutral `pin.kind.label()` chip, `"Remove"`) | **Yes** | Unchanged |
| | Root zone add row (`zone_name_input`, `new_container_name_kind`, `"Add root zone"`) | **Yes** | Row 1 of unified `"Add"` grid |
| | Path zone add row (`zone_path_input`, `new_container_path_kind`, `"Add path zone"`) | **Yes** | Row 2 of unified `"Add"` grid |
| | Manual pin add row (`zone_pin_input`, `new_pin_kind`, `"Pin folder"`) | **Yes** | Row 3 of unified `"Add"` grid |
| **About** | `draw_wt_header_lockup` (`y-storage v...`) | **Yes** | Unchanged |

**Removed controls:** `0` (none).

---

## 3. Verification & Deployment

- **Tripwire check (`git diff --stat`)**: Only `apps/y-storage/src/ui.rs` modified in code (`crates/y-core` and `crates/y-platform-windows` untouched).
- **Touched-crate tests (`cargo test -p y-storage`)**: **`47 / 47` passed**, **`0` failed**, **`0` warnings** (includes 2 new tests: `settings_wide_section_nav_and_narrow_collapse_render_cleanly`, `settings_form_grid_and_drive_pod_row_heights_align`).
- **Release build (`cargo build --release --bin y-storage`)**: **0 warnings**.
- **Deployed binary**: `Y:\Apps\y-storage\y-storage.exe`
  - **SHA-256**: `30493754C889DDAA256B467099A88E45D2306EDF2950901CE134F2353B7E9FBA`
- **GUI smoke test**: `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500` -> `ExitCode = 0`.
