# Feature Specification: `button-discipline-1`

**Feature Branch**: `main` (building on `de9cf50`)
**Created**: 2026-11-26
**Status**: Active
**Input**: Owner review finding that every button in `y-storage` (`Rescan`, `Settings`, `ThemeToggle`, `All`, `Add exclusion`, and repeated row-level `Locate`, `Web search`, and `used by N apps >` actions) renders with the same accent-blue outline and accent text, leaving no visual primary action hierarchy.

## Requirements

### Accent Rationing Rule (Family Standard — Part I `DESIGN.md §6`)
**"Accent is rationed: primary + active states only."** Across all WT applications (`y-storage`, `z-controller`, future WT utilities), `pal.accent_line` (`#54B9F2` Dark / `#2F9BE8` Light) is reserved strictly for:
1. **Single primary action of a view** (`Rescan` in the main header).
2. **Selected / active states** (active filter chip, active sort arrow, checked checkbox, focused input border, active scrollbar drag thumb, open reverse-dependency trigger/panel, expanded drawer rim & chevron).
3. **Hover feedback on lower-tier buttons**.

Everything else renders neutral (`SecondaryOutline`) or quiet (`SubtleOutline`) at rest:
- **`PrimaryOutline`** (`Rescan` only in main view): `fill = pill_blue_fill`, `border = accent_line`, `text = accent_line`; hover uses `widget_hover` + `accent_line` border + `accent_line` text; press uses `widget_active` + `LOGO_PINK` border + `text`.
- **`SecondaryOutline`** (`Settings` / `Back to Apps`, `ThemeToggle`, overlay `Close` / `All`, Settings `Add exclusion`): NEUTRAL at rest — `fill = TRANSPARENT`, `border = pal.border`, `text/glyph = pal.text`; on hover: `fill = pal.widget_hover`, `border = pal.accent_line`, `text/glyph = pal.accent_line`.
- **`SubtleOutline` / Row-Level Actions** (`Locate`, `Web search`, `used by N apps >`, Settings `Remove`, filter `Clear`): QUIET at rest — no visible border (`TRANSPARENT`), `fill = TRANSPARENT`, `text = pal.muted`; on hover: `fill = pal.widget_hover`, no border (`TRANSPARENT`), `text = pal.accent_line`. Active-open reverse-dependency triggers (`active_highlight == true`) render active accent (`pill_blue_fill` + `accent_line` + `accent_line`).

### Scope & Constraints
- Visuals + `DESIGN.md §6` documentation only: zero changes to scan, attribution, dependency, history, sort, filter, or OS launcher behavior, and zero computed byte changes.
