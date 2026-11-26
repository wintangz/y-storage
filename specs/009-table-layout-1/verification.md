# Verification Report: `table-layout-1`

**Date**: 2026-11-26  
**Scope**: Structural layout conversion ONLY (`ui.rs`, `DESIGN.md`, Spec Kit docs); zero changes to `crates/y-core`, `crates/y-platform-windows`, or `apps/y-storage/src/list_query.rs`.

## 1. Automated Tests & Build

- `cargo test --workspace`: **23 passed, 0 failed** (5 `core_tests`, 4 `round2_tests`, 4 `round3_tests`, 2 `windows_scanner_tests`, 8 `y-storage` unit tests), **0 warnings**.
- `cargo build --release --workspace`: **Succeeded (exit code 0), 0 warnings**.

## 2. Live CLI Regression Scan (`--scan-cli ... --ephemeral`)

- Installed apps: `262`
- `EA SPORTS™ FIFA 23`: `51,372,195,955 B` (unchanged vs pre-round baseline)
- `C:\` reconciliation:
  - Apps (`C:`): `84,027,961,747 B`
  - Shared & system caches (`C:`): `11,826,117,517 B`
  - Your files (`C:`): `3,210,724,335 B`
  - Unattributed (`C:`): `1,480,827,892 B`
  - Windows & system remainder (`C:`): `46,112,070,397 B`
  - `C:\` used: `146,657,701,888 B`
  - **`C:\` diff**: **`0 B`**

## 3. Visual Inspection (`1020x720`, `96` DPI)

Rendered and visually inspected against `DESIGN.md` `C1`–`C12`:
- `screenshot-main-dark.png`: Sticky `26px` `panel_alt` header (`App name`, `Publisher`, `Growth delta`, `Total size ↓`), fixed columns across all `9` visible rows, inline `NO-TOUCH · Service` beside `Xbox Service`, right edge of `Total size` aligned with `Pinned storage ledger` below, slim `6px` scrollbar in outer gutter. **PASS**.
- `screenshot-main-light.png`: Crisp `1px` borders, distinct `panel_alt` sticky header surface contrast, semantic `-3.05 MB` / `+90.93 KB` delta legibility, clean `input_bg` search well, intact light theme palette. **PASS**.
- `screenshot-main-growth-sort.png`: Active sort indicator arrow (`↓`) moved cleanly to `Growth delta` header column; sorted highest growers first (`.gemini +18.36 MB`, `StartExperiencesApp +396.41 KB`, `VS Code +159.43 KB`); no standalone `Sort:` bar in Row 3. **PASS**.
- `screenshot-expanded-fifa23-dark.png`: `EA SPORTS™ FIFA 23` elevated with `2px` left `accent_line` bar on `panel_alt` containing both aligned sub-tables: `Locations (1)` (`Zone · Path · Basis · Size`) and `Dependencies` (`Dependency name · Kind · Confidence · Used by · Own size`) with `dep.basis` cleanly placed on a quieter full-width second line beneath each dependency row. **PASS**.

## 4. Deliverable Status (Items 1–4)

- **Item 1 (Main list as table)**: **PASS** — 5 fixed columns (`chevron · App name · Publisher · Growth delta · Total size`), sticky `panel_alt` header, hairline dividers, subtle `widget_hover` row highlight, aligned with bottom ledger.
- **Item 2 (Headers are the sort control)**: **PASS** — clickable `App name`, `Growth delta`, `Total size` column headers flip/switch `AppSortMode` + `SortDirection`; standalone `Sort:` segmented bar removed; filter chips + search + count intact.
- **Item 3 (Expanded view as aligned sub-tables)**: **PASS** — `Locations` (`Zone · Path · Basis · Size`) and `Dependencies` (`Dependency name · Kind · Confidence · Used by · Own size` + quiet second-line `dep.basis`) aligned cleanly with right `Size` column.
- **Item 4 (Numbers stay put + `DESIGN.md`)**: **PASS** — zero computed number changes, `Publisher` comfortably fits at `1020x720`, all type `>= 10.5pt`, `DESIGN.md` extended cleanly.
