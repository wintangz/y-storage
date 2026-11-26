# Implementation Plan: `table-layout-1`

**Spec**: [spec.md](./spec.md)  
**Design Doc**: [../../DESIGN.md](../../DESIGN.md)

## Architecture & Geometry

### Outer Frame & Sticky Header Split (`ui.rs`)
- In `draw_main_app_list(ui, ui_state, report, pal, emit)`:
  - `table_outer_w = (avail_w - SCROLL_GUTTER_W).max(320.0)` so the right border aligns identically with `draw_pinned_bottom_groups`'s `aligned_w`.
  - Paint one `RADIUS_PANEL` (`8px`) ledger frame via `painter.add(Shape::Noop)` slots (`outer_border_slot`, `inner_fill_slot`, `header_bg_slot`), snapped via `snap_rect_to_pixels`.
  - Render a sticky `TABLE_HEADER_H = 26.0` row at the top of the panel **outside** `draw_synced_scroll_area`, shaded with `pal.panel_alt` + a `1px` `pal.border` bottom divider.
  - Render `draw_synced_scroll_area` directly below (`max_height = (avail_h - (TABLE_HEADER_H + 2.0)).max(80.0)`), keeping `content_rect.x` and `content_rect.width` constant for every collapsed row so column positions never drift between header and body.

### Fixed Column Structs (`ui.rs`)
- `MainTableColumns::compute(origin_x, row_w)`:
  - `chevron_x`: `[origin_x, +16.0]`
  - `name_x`: `[chevron_x.max + 8.0, ...]` (`flex >= 180.0`)
  - `publisher_x`: fixed `208.0` (`24.0` right gap before delta on wide widths) -> right edge aligned cleanly
  - `delta_x`: fixed `96.0`
  - `size_x`: fixed `96.0` (`size_x.max == origin_x + row_w`)
- `LocationsTableColumns::compute(origin_x, sub_w)`:
  - `zone_x`: `64.0`
  - `path_x`: flex (`>= 140.0`)
  - `basis_x`: `170.0`
  - `size_x`: `96.0` (`size_x.max == origin_x + sub_w`, sharing `MainTableColumns.size_x.max`)
- `DependenciesTableColumns::compute(origin_x, sub_w)`:
  - `name_x`: flex (`>= 160.0`)
  - `kind_x`: `140.0`
  - `confidence_x`: `130.0`
  - `used_by_x`: `84.0`
  - `own_size_x`: `96.0` (`own_size_x.max == origin_x + sub_w`, sharing `MainTableColumns.size_x.max`)

### Cell Clipping Safety
- Each cell uses `ui.scope_builder(UiBuilder::new().max_rect(cell_rect).layout(...))` with `.set_clip_rect(cell_rect.expand2(Vec2::new(0.0, 1.0)).intersect(parent_clip))` so text/widgets stay inside their column horizontally while `1px` pill/button strokes never clip on top/bottom edges.

## File Changes
- `.specify/memory/constitution.md`: append `table-layout-1`
- `DESIGN.md`: document sticky sortable table pattern and sub-table schemas
- `apps/y-storage/src/ui.rs`: implement column geometry helpers, sticky header sorting, collapsed/expanded table rendering, remove standalone sort bar
- `specs/009-table-layout-1/{spec,plan,tasks,verification}.md` + 4 screenshots
