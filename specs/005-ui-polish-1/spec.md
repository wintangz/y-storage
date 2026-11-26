# Feature Specification: `y-storage` (`ui-polish-1`)

**Feature Branch**: `005-ui-polish-1`  
**Included Feature Slug**: `ui-polish-1`  
**Created**: 2026-11-26  
**Status**: Approved (Autonomous UI Round)

## Scope
UI-only refinement with zero changes to scan, attribution, dependency, or history calculation logic.

## Polish Requirements (Items 1–10)
1. **Unified spacing scale (multiples of 4)**: `4 / 8 / 12 / 16` spacing tokens across header padding, card/row padding, and gaps; rows strictly hug their content without empty vertical slack.
2. **Scannable numbers**: All sizes aligned in one right-aligned column using one consistent 2-decimal human format (`50.88 GB`, `782.73 MB`), exact byte count on hover, and semantic +/- growth deltas (grew = pink/red, shrank = green, unchanged = muted grey).
3. **Consistent chips & quiet basis**: `GREEN` cache = green, `RED` data = red/pink, `BODY` = pastel blue, `Roaming config` & `ProgramData · shared` = lavender, `NO-TOUCH` = muted red outline, dependency `Proven` = solid chip, `Inferred` = outline chip; basis text rendered small and muted underneath.
4. **Expanded `"Dependencies"` sub-section & non-crowding reverse view**: Dedicated `"Dependencies"` sub-section inside expanded app cards showing dependency name, kind chip, confidence chip, `"shared"` mark, own size if any, and clickable `"used by N apps"` opening a floating reverse-view overlay without shifting or crowding location rows.
5. **Universal truncation tooltips**: Any truncated app name, path, dependency name, consumer name, or basis text reveals full text on hover via one shared truncation helper.
6. **Header & Dark/Light pastel palette**: Theme icon button placed immediately LEFT of `Rescan`, zero divider line beneath header, `#54B9F2` / `#D8A6FF` / `#F7A8D8` accents/chips only, and semantic status colors across Dark and Light modes.
7. **Header drive strip**: Per drive, a slim proportional used/free bar with `used` and `free` figures beside it, with `free` space as the most prominent number.
8. **Pinned bottom groups & reconciliation bar**: `Windows & system`, `Your files`, `Shared & system caches`, `Unattributed`, and the reconciliation line stay permanently visible and unclipped at `1020 x 720`, while only the app list scrolls between header and footer.
9. **Full-bleed embedded icon**: Multi-size `.ico` (`~97%` tile width) embedded in `y-storage.exe` and applied to Explorer, taskbar, and title bar (`WM_SETICON`).
10. **Window & theme persistence**: Remembers window position, size, maximized state, and Dark/Light theme mode in `config.toml` across launches.

## Logged Assumptions
- **A-009 (Binary `1024`-Based 2-Decimal Format)**: UI size display standardizes on `1024`-based units with 2 decimals (`format_size_2dec`, e.g. `47.84 GB`, `782.73 MB`) matching prior rounds' human readout while exposing the exact comma-separated byte integer in every size hover tooltip.
