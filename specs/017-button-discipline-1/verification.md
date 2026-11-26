# Verification Report: `button-discipline-1`

**Date**: 2026-11-26
**Branch / Base Commit**: `main` (building on `de9cf50`)
**Status**: **PASS**

---

## 1. Per-Button Hierarchy Audit & Tier Assignment

Rule enforced across `apps/y-storage/src/ui.rs` and codified in `DESIGN.md` (`Part I §6`):
**"Accent is rationed: primary + active states only."** Accent colour (`#54B9F2` Dark / `#2F9BE8` Light) appears strictly on **(1)** the single primary action of a view, **(2)** selected/active states, and **(3)** hover feedback on lower-tier buttons.

| Button / Control | Surface / Location | Tier (`ButtonVariant`) | Rest Visual (`fill / border / fg`) | Hover / Active Visual | Why |
|---|---|---|---|---|---|
| **`Rescan`** (or **`Scanning...`**) | Global Header (`Main`, `Settings`, `Dependencies`) | **`PrimaryOutline`** (Primary) | `pill_blue_fill` / `accent_line` / `accent_line` (disabled while scanning: `TRANSPARENT` / `border` / `muted`) | Hover: `widget_hover` / `accent_line` / `accent_line` | Sole primary action of the main view and application header; unchanged CTA emphasis. |
| **`Settings`** / **`Back to Apps`** | Global Header (all views) | **`SecondaryOutline`** (Secondary) | `TRANSPARENT` / `border` / `text` | Hover: `widget_hover` / `accent_line` / `accent_line` | Top-level view navigation toggle; neutral at rest so `Rescan` leads unmistakably. |
| **`ThemeToggle`** (Sun / Moon icon) | Global Header (all views) | **`SecondaryOutline`** (Secondary) | `TRANSPARENT` / `border` / `text` glyph | Hover: `widget_hover` / `accent_line` / `accent_line` glyph | Utility mode switch paired beside `Rescan`; neutral border and `text` glyph at rest. |
| **Filter radio chips** (`All`, `Grew`, `NO-TOUCH`, `Standalone`, `Has C: footprint`) | Main Filter Strip | Selected state vs Neutral option (`draw_filter_radio_chip`) | Selected: `pill_blue_fill` / `accent_line` / `text` · Unselected: `widget` / `border` / `muted` | Hover: `widget_hover` / `accent_line` / `accent_line` (or `text` when selected) | Active filter state retains accent per Rule (2); inactive choices remain neutral per Rule (1). |
| **`Clear`** (search input reset, shown when query non-empty) | Main Search Bar (`Filter:` row) | **`SubtleOutline`** *(Assumption logged)* | `TRANSPARENT` / `TRANSPARENT` / `muted` | Hover: `widget_hover` / `TRANSPARENT` / `accent_line` | Sits inline next to the framed search input well; assigned to `SubtleOutline` so it acts as a quiet inline reset rather than competing with the search field box. |
| **`Locate`** (enabled on existing Unattributed folder; disabled `"no longer on disk"` when missing) | Expanded `Unattributed` ledger rows (repeated per row) | **`SubtleOutline`** (Row-level) | Enabled: `TRANSPARENT` / `TRANSPARENT` / `muted` · Disabled: `TRANSPARENT` / `TRANSPARENT` / `lerp(bg, muted, 0.55)` | Hover: `widget_hover` / `TRANSPARENT` / `accent_line` | Repeated per row in the ledger; borderless quiet text prevents stacked blue rectangles. |
| **`Web search`** | Expanded `Unattributed` ledger rows (repeated per row) | **`SubtleOutline`** (Row-level) | `TRANSPARENT` / `TRANSPARENT` / `muted` | Hover: `widget_hover` / `TRANSPARENT` / `accent_line` | Repeated per row in the ledger; reads as one quiet text column at rest. |
| **`used by N apps >`** (outbound dependency row & drawer companion line) | Expanded App Drawer (`Dependencies` table + `Category` header row) | **`SubtleOutline`** (Row-level) | Closed: `TRANSPARENT` / `TRANSPARENT` / `muted` · Open (`active_highlight`): `pill_blue_fill` / `accent_line` / `accent_line` | Hover: `widget_hover` / `TRANSPARENT` / `accent_line` | Repeated per dependency row; borderless at rest, accented when actively inspecting its reverse-dependency list per Rule (2). |
| **`Close`** | Reverse-Dependency Overlay header | **`SecondaryOutline`** (Secondary) | `TRANSPARENT` / `border` / `text` | Hover: `widget_hover` / `accent_line` / `accent_line` | Overlay dismiss action; neutral outline button matching family standard. |
| **`All`** (open full `Dependencies` view) | Reverse-Dependency Overlay header | **`SecondaryOutline`** (Secondary) | `TRANSPARENT` / `border` / `text` | Hover: `widget_hover` / `accent_line` / `accent_line` | Navigation shortcut inside floating overlay (`Rescan` in header remains the primary CTA); demoted from `PrimaryOutline` to `SecondaryOutline`. |
| **`Add exclusion`** | `Settings` view (repeated once per drive card) | **`SecondaryOutline`** (Secondary) | `TRANSPARENT` / `border` / `text` | Hover: `widget_hover` / `accent_line` / `accent_line` | Repeated once per drive card in `Settings`; demoted from `PrimaryOutline` to neutral `SecondaryOutline` so multiple drive cards do not compete with the primary CTA. |
| **`Remove`** (user-added exclusion entry) | `Settings` view (repeated per exclusion row) | **`SubtleOutline`** (Row-level) | `TRANSPARENT` / `TRANSPARENT` / `muted` | Hover: `widget_hover` / `TRANSPARENT` / `accent_line` | Repeated per exclusion row; borderless `muted` at rest, `widget_hover` + accent text on hover. |

---

## 2. Logged Design Assumptions

1. **`Clear` (search input reset)**: Assigned to `SubtleOutline` rather than `SecondaryOutline` because it only appears transiently adjacent to the bordered search input box; making it borderless keeps the search strip visually clean without an adjacent rectangle.
2. **Global Header `Rescan` across views**: In `Settings` and `Dependencies`, the global header remains visible with `Rescan` still serving as the single `PrimaryOutline` CTA, while view-local actions (`Add exclusion` repeated across 4 drive cards, overlay `All`/`Close`) stay `SecondaryOutline` (no secondary view needs a competing `PrimaryOutline` CTA).
3. **`active_highlight` (open `used by N apps >`)**: Updated from `pill_red_fill` + `LOGO_PINK` to `pill_blue_fill` + `accent_line` + `accent_line` to obey Rule (2) (active/selected states take accent).

---

## 3. Automated Checks, Live Scan & Screenshots

- **`cargo test --workspace`**: **`38 passed, 0 failed, 0 ignored`** (`0` warnings):
  - `y-core`: `16` passed (`core_tests`: 6, `round2_tests`: 6, `round3_tests`: 4)
  - `y-platform-windows`: `2` passed
  - `y-storage`: `20` passed (including new `ui::tests::button_hierarchy_reserves_accent_for_primary_active_and_hover`)
- **`cargo build --release --workspace`**: Succeeded (`22.29s`, `0` warnings); deployed to `Y:\Apps\y-storage\y-storage.exe`.
- **Live Ephemeral Scan (`--scan-cli ... --ephemeral`)**:
  - `apps`: **`259`**, `unattributed`: **`303`**
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`** (`7` dependencies)
  - `C:\` reconciliation: **`diff = 0 B`**
- **Screenshots Rendered (`1020x720` logical, 96 DPI) & Visually Inspected**:
  - `screenshot-main-dark.png`: `Rescan` is the only accent-blue CTA in the header; `ThemeToggle` (sun) and `Settings` use neutral `#26263A` borders and `#EDEDF2` foreground.
  - `screenshot-main-light.png`: `Rescan` is the only `#2F9BE8` CTA in the header; `ThemeToggle` (crescent moon) and `Settings` use neutral `#D9E4F0` borders and `#1C2733` foreground.
  - `screenshot-unattributed-expanded-hover-dark.png`: Expanded `Unattributed` group shows borderless `muted` `Locate` / `Web search` columns at rest, and row 0's hovered `Web search` renders `widget_hover` fill (`#1F1F31`) + `accent_line` (`#54B9F2`) text with zero border.
  - `screenshot-expanded-fifa23-dark.png`: Expanded `EA SPORTS™ FIFA 23` drawer retains the diagonal active gradient rim while `used by 42 apps >` / `2 apps >` / `1 app >` / `46 apps >` render cleanly as one quiet borderless `muted` column.
