# Feature Specification: `visual-identity-addon`

**Scope**: Visual-only polish building directly on `visual-identity-1` (`9b267bf`). Zero changes to scan, attribution, dependency, history, icon extraction, or sort/filter logic (`crates/y-core`, `crates/y-platform-windows`, `apps/y-storage/src/list_query.rs` unchanged), and zero computed byte changes.

## Deliverables

1. **ADD 1 — WT Active Gradient Rim (`expanded` App Drawer)**:
   - Port `z-controller`'s diagonal gradient rim (`push_rounded_rect_perimeter`, `build_rounded_rect_polygon`, `paint_active_card_gradient_border`: `8.0px` corner radius, `16` corner segments, `t = 0.65x + 0.35y`, stops `LOGO_BLUE #54B9F2 -> LOGO_LAVENDER #D8A6FF -> LOGO_PINK #F7A8D8`, `5` concentric normal bands `offsets = [-4.5, -2.0, -1.6, -0.4, 0.0]`, `alphas = [0, 52, 255, 255, 0]`) to expanded app drawers in both Dark and Light themes.
   - Update `DESIGN.md` shared core to define the gradient rim as active-container state signalling rather than decoration.

2. **ADD 2 — WT Outline Action Buttons**:
   - Restyle action buttons (`Rescan`, `Settings` / `Back to Apps`, `ThemeToggle`, `Add exclusion`, `Remove`, `Clear`, overlay `All` / `Close`, `used by N apps >`) to `z-controller`'s outline hierarchy:
     - `PrimaryOutline`: `pill_blue_fill` (`#121C28`/`#EBF6FF`), `1px` `accent_line` outline, `accent_line` label.
     - `SecondaryOutline`: `Color32::TRANSPARENT` fill, `1px` `lerp_color(border, accent_line, 0.72)` outline, `accent_line` label/glyph.
     - `SubtleOutline`: `Color32::TRANSPARENT` fill, `1px` `border` outline, `muted` label.
     - Shared Hover (`widget_hover` fill, `accent_line` stroke, `text` fg) & Pressed (`widget_active` fill, `LOGO_PINK` stroke, `text` fg).
   - Ensure `1px` crisp perimeter outlines render unbroken even when fill is `Color32::TRANSPARENT` at `96` DPI.

3. **ADD 3 — Saturated Status Elements vs Quiet Everything Else**:
   - Inspect `z-controller`'s `draw_battery_bar` (`ui.rs:535-566`): uses single chunky `8px` (`Rounding 4.0`) green/red status fills rather than gradient fills. Mirror this principle cleanly in `y-storage`: chunky `7px` rounded drive tracks (`Rounding 3.5` `pal.bg` + `pal.border`, inner `Rounding 2.5`) switching to saturated `STATUS_RED` at `>= 85%` full (`<= 15%` free headroom).
   - Saturate true semantic status elements: `8px` diameter (`r = 4.0`) `STATUS_GREEN` reconciliation dot + green `"diff 0 B"` text; saturated `+`/`-` growth deltas (`#28D66C`/`#0FA84F` and `#F04458`/`#E02444`); saturated `NO-TOUCH`/`LOCKED`/`missing` badges; dotted saturated `● GREEN` / `● RED` zone badges. Non-status accents, `BODY` badges, `proven`/`shared`, `All` filter chips, and `2px` table proportion bars stay at quiet `visual-identity-1` levels.
