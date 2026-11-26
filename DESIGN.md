# WT Native Instrument Design System (`DESIGN.md`)

**Applies to**: All WT native desktop utilities (`y-storage`, `z-controller`, and future `WT Manager`).
**Last Updated**: 2026-11-26 (`visual-identity-addon`)

---

## 1. North Star

WT applications are precision instruments, not dashboards: every pixel, colour, border, and type tier must encode real system state or measured quantity, and nothing exists as decoration for its own sake. We design explicitly against the **generic AI UI failure mode** — wrapping every list row in identical bordered cards of equal visual weight, flattening typography until names, metadata, and measurements all shout at the same volume, applying one corner radius to every box, and scattering decorative pill chips on static labels until no badge carries meaning.

---

## 2. Shared Tokens

### 2.1 Pastel & Semantic Palette
| Token | Dark Mode | Light Mode | Permitted Usage |
|---|---|---|---|
| `accent_line` (`LOGO_BLUE`) | `#54B9F2` | `#2F9BE8` | Active borders, active sort/filter outlines, primary meter fills, chevron cues, gradient stop `0.0` |
| `lavender` (`LOGO_LAVENDER`) | `#D8A6FF` | `#D8A6FF` (`#783EB8` small text) | Secondary/install-only drive meters, `Windows & system` tick, gradient stop `0.5`; `#783EB8` Light chip text |
| `delta_grew` (`LOGO_PINK`) | `#F7A8D8` | `#F7A8D8` | Active pressed control border, `Unattributed` tick, gradient stop `1.0` |
| `status_green` (`STATUS_GREEN`) | `#28D66C` | `#0FA84F` (`#28D66C` dots/ticks) | **Saturated semantic status**: shrank (`-`) deltas, steady-state `diff 0 B` dot (`8px`) & text, `● GREEN` zone mark, `Shared caches` tick |
| `status_red` (`STATUS_RED`) | `#F04458` | `#E02444` (`#F04458` dots/bars) | **Saturated semantic status**: grew (`+`) deltas, `NO-TOUCH` / `LOCKED` / `missing` outline & tint, `● RED` zone mark, critical diff / `<= 15%` free drive bar |

**WT Signature Active Gradient Rim & Pastel Rule**: Pastel accents appear strictly as thin indicators, meter fills, outline borders, and tinted badges (`pill_*_fill` + `1px` outline) — never large solid pastel fills or opaque pastel badges. **Active containers** (`virtual_active` cards in `z-controller`, expanded inspection drawers in `y-storage`) carry the continuous **WT Active Gradient Rim**: `8px` corner radius, diagonal blend from `LOGO_BLUE` (`#54B9F2` at `0.0`) -> `LOGO_LAVENDER` (`#D8A6FF` at `0.5`) -> `LOGO_PINK` (`#F7A8D8` at `1.0`) along `t = 0.65x + 0.35y`, rendered as a `1.2pt` solid perimeter core with a `2.5pt` soft inner glow (`offsets = [-4.5, -2.0, -1.6, -0.4, 0.0]`, `alphas = [0, 52, 255, 255, 0]`). Because the rim signals active selection/expansion state rather than static chrome, it satisfies WT's signal-over-decoration rule (`§1`). Following `z-controller`, interactive buttons use outline language (`PrimaryOutline`: `pill_blue_fill` + `accent_line` outline/text; `SecondaryOutline`: `TRANSPARENT` fill + `72%` blended accent outline + `accent_line` text/glyph; `SubtleOutline`: `TRANSPARENT` fill + `border` outline + `muted` text), and `pill_blue_fill` status badges render text in `pal.text` (`#EDEDF2` / `#1C2733`).

**Saturated Status vs Quiet Everything Else**: Genuine semantic status elements (`diff` dot & label, `+`/`-` growth deltas, `NO-TOUCH`/`LOCKED`/`missing` badges, `● GREEN`/`RED` zone marks, critical `<= 15%` free drive bars) use fully saturated `STATUS_GREEN` / `STATUS_RED` tokens, while non-status labels, metadata pills (`proven`, `shared`), `BODY` zone pills, and `2px` table proportion bars remain quiet.

### 2.2 Surface Hierarchy
| Surface | Dark Mode | Light Mode | Role |
|---|---|---|---|
| `bg` (`S0` Canvas) | `#0A0A10` | `#EEF4FA` | Window backdrop and header canvas |
| `panel_alt` (`S1` Recessed) | `#10101A` | `#F4F8FC` | Sticky table header, expanded drawer, pinned footer (`z-controller` `faint_bg`) |
| `panel` (`S2` Primary) | `#14141F` | `#FFFFFF` | Ledger containers, drive pods, Settings cards |
| `widget` / `hover` / `active` (`S3`) | `#181826` / `#1F1F31` / `#182230` | `#F5F8FC` / `#E9F2FA` / `#DCEBF8` | Interactive buttons, sort toggles, filter pills, hovered table rows |
| `input_bg` (`Well`) | `#0A0A10` | `#EEF4FA` on `panel` (`#FFFFFF` on `bg` canvas) | Text input fields (matches `z-controller` inside white cards while preserving crisp contrast on `#EEF4FA` header canvas) |
| `border` (`Hairline`) | `#26263A` | `#D9E4F0` | `1px` crisp structural borders and ledger row dividers |

### 2.3 Typography & Luminance Hierarchy (`Segoe UI` + `Consolas` Instrument Numerals)
Minimum readable type floor across all UI copy: **`10.5pt`**. No text may render smaller than `10.5pt`.

| Tier | Size & Weight | Luminance Token | Role |
|---|---|---|---|
| **T1 — Instrument Headline** | `16.0pt` Strong (`14.5pt` `Consolas Bold` for drive free space) | `text` (`#EDEDF2` / `#1C2733`) | View titles (`y-storage`, Settings title) & **drive free-space headline figures** |
| **T2 — Dominant Row Quantity** | `13.0pt`–`13.5pt` `Consolas Bold` | `text` | Primary row totals (each app's attributed byte total) |
| **T3 — Primary Row Identity** | `13.0pt` `Segoe UI` | `text` | App display names; `12.0pt`–`12.5pt` for pinned ledger titles and button labels |
| **T4 — Secondary Metadata & Quiet Numbers** | `10.5pt`–`11.5pt` | `muted` (`#9A9AB0` / `#5F7186`) / `subtle_text` (`#82829A` / `#748499`) | Publishers (`11.0pt subtle_text`), breakdown paths (`11.5pt` `Consolas`), breakdown & shared dependency sizes (`11.5pt` `Consolas`), provenance/basis (`10.5pt subtle_text`, word-capped `…`), section eyebrows (`10.5pt`) |

### 2.4 Spacing Scale (Multiples of 4 Only)
`SPACE_4 = 4.0`, `SPACE_8 = 8.0`, `SPACE_12 = 12.0`, `SPACE_16 = 16.0`. All margins, row padding, and control gaps come strictly from `{4, 8, 12, 16}`.

### 2.5 Corner-Radius Scale (Differentiated by Role)
Not everything gets the same corner radius:
- **`8.0px` (`RADIUS_PANEL`)**: Elevated containers — drive pods, ledger wrapper panels, elevated expanded inspection panel, Settings drive panels, overlays (matches `z-controller` `CARD_ROUNDING = 8.0`).
- **`6.0px` (`RADIUS_CONTROL`)**: Shared interactive controls synced with `z-controller` — `ThemeToggle`, buttons (`Rescan`, `Settings`, `Clear`, `Add exclusion`, `Remove`, `Close`, `All`), segmented `Sort` controls, text inputs.
- **`4.0px` (`RADIUS_PILL`)**: Compact single-select filter pills and checkboxes.
- **`2.0px` (`RADIUS_BADGE`)**: Non-interactive semantic state badges (`GREEN`, `RED`, `BODY`, `NO-TOUCH`, `LOCKED`, `proven`, `inferred`, `shared`) and drive meter tracks.
- **`0.0px`**: Hairline-divided ledger rows and horizontal rules.

---

## 3. Hierarchy & Chip Discipline Rules

1. **One Dominant Element Per Row**: The right-aligned measured quantity (`13.5pt` Strong) is the single strongest signal on every row. Identity (`13.0pt` Regular) leads cleanly on the left; inline metadata steps down in both size and contrast (`10.5pt`–`11.0pt subtle_text`).
2. **Shared Tabular Alignment & Sticky Sortable Headers**: Every primary size number aligns cleanly into one continuous right-aligned column across both main ledger rows and pinned summary rows. Multi-column list views use fixed column x-positions (`Chevron`, `Name`, `Publisher`, `Growth delta`, `Total size` in ledger views) anchored beneath a sticky column-header row (`panel_alt` surface, quiet `T4` `11.0pt` meta tier). Column headers act directly as interactive sort triggers showing the single active sort arrow rather than duplicating sort state into separate segmented control bars.
3. **Strict Chip Discipline**: A badge exists **only** to encode a user decision or state (`GREEN` / `RED` / `BODY` zone, `NO-TOUCH` / `LOCKED` safety, `proven` / `inferred` / `shared` / `missing` dependency state). **Forbidden decorative chips**: app-wide static modes (`READ-ONLY`), static drive flags (`SYSTEM DRIVE`), descriptive dependency categories (`shared runtime`, `platform/launcher client`), internal spec jargon (`Constitutional`), and normal `diff 0 B` steady-state ledger output.
4. **Cross-App Control & Signature Parity**: Shared controls (`ThemeToggle`, outline primary/secondary/dismiss buttons, text inputs, checkboxes, slim `6px` `SPACE_8`-gutter scrollbar) and the diagonal `8px` Active Gradient Rim MUST match `z-controller` (`ui-polish-2`, `visual-identity-addon`).

---

## 4. Anti-Patterns ("The AI Look" Checklist)
Reject any screen exhibiting:
1. Repeating identical bordered cards around every list/ledger row (`cards-inside-cards` when expanded).
2. A single uniform corner radius (`6px` on panels, badges, tracks, and buttons alike).
3. Raw pastel accents (`#54B9F2`, `#D8A6FF`, `#F7A8D8`) used as solid opaque badge/surface fills.
4. Centred empty-state labels floating in dead vertical space.
5. Publishers, dependency kinds, basis strings, or shared dependency sizes rendered at equal weight/brightness with primary totals.
6. Chips placed on static descriptors rather than state/decisions.

---

## 5. App-Specific: `y-storage`

- **Signature Element 1 — Drive Instrument Header**: Each physical drive sits in an `8px` pod whose dominant readout is **free space** (`T1`, `14.5pt` `Consolas Bold`) paired with a full-width `7px` chunky rounded capacity meter (`used_bytes / total_bytes`, matching `z-controller`'s chunky rounded bar geometry: `pal.accent_line` when healthy/full-walk, `LOGO_LAVENDER` when install-only, and saturated `STATUS_RED` when `<= 15%` headroom remains). Free space must visually outweigh app-row figures.
- **Signature Element 2 — Tabular App Footprint Ledger & Sub-Tables**: The main app list renders inside one `8px` ledger panel headed by a sticky `26px` sortable column-header row (`Chevron · App name · Publisher · Growth delta · Total size`). Clicking `App name`, `Growth delta`, or `Total size` sorts (`Size` desc default; active column shows directional sort arrow; clicking again flips asc/desc). Each `App name` cell renders a fixed `18x18 px` Windows app icon extracted once per scan (or one consistent neutral muted folder-tile glyph for standalone/service entries) without shifting column x-positions. All numeric columns (`Total size`, `Growth delta`, `Own size`, drive counters) use `Consolas` monospace numerals; `Total size` includes a quiet `<= 2px` muted proportion bar relative to the largest unfiltered app and exposes exact bytes + `%` share on hover. Expanding a row elevates it into an `8px` `panel_alt` inspection drawer wrapped in the continuous **WT Active Gradient Rim** (`LOGO_BLUE -> LOGO_LAVENDER -> LOGO_PINK`), rendering two aligned sub-tables whose right size columns share the exact right edge of the main `Total size` column:
  - `Locations (N)` table: `Zone · Path · Basis · Size` (`● GREEN` / `● RED` saturated status marks vs quiet `BODY`, `Basis` word-boundary capped `"…"` with full hover tooltip).
  - `Dependencies` table: `Dependency name · Kind · Confidence · Used by · Own size`, with `dep.basis` capped cleanly to one word-boundary line ending in `"…"` underneath the dependency row (full text on hover).
- **Signature Element 3 — Reconciliation Ledger Footer**: Pinned storage groups (`Windows & system`, `Your files`, `Shared & system caches`, `Unattributed`) sit in one compact `8px` ledger block with `2px` colour ticks and sizes aligned to the exact same right column as the app list; beneath them sits the exact reconciliation equation ending with a chunky `8px` saturated `STATUS_GREEN` dot + `diff 0 B` (or saturated `STATUS_RED` alert badge if `diff != 0`).

---

## 6. Screenshot Critique Checklist (`C1`–`C12`, Pass/Fail From Screenshots Alone)

- **C1 (No Uniform Card Soup)**: Collapsed app rows and pinned storage groups sit inside clean hairline-divided ledger surfaces rather than individual stacked bordered cards; expanded rows do not contain nested sub-cards.
- **C2 (One Dominant Row Number)**: On every app row, the right-aligned total (`13.5pt` Strong) is unmistakably heavier than the app name (`13.0pt` Regular), which is clearly brighter than the inline publisher (`11.0pt` subtle).
- **C3 (Shared Size Column Alignment)**: Sticky table header `Total size`, main row totals, expanded `Locations`/`Dependencies` size columns, and pinned storage group totals share one exact vertical right edge (accounting for scrollbar gutter).
- **C4 (Headline Drive Strip)**: Drive free space (`16.0pt` Strong) is visually larger than app row text and immediately anchors each `8px` drive pod above its `2px` proportional capacity bar.
- **C5 (Quiet Shared Dependency Sizes)**: Inside an expanded app, shared dependency sizes (`11.5pt` muted) are visibly subordinate to the app's own total (`13.5pt` Strong) and never read like additive components.
- **C6 (Chip Discipline)**: Zero chips appear on static labels (`READ-ONLY`, `SYSTEM DRIVE`, `diff 0 B`, `shared runtime` / `platform/launcher client`, `Constitutional`). Chips exist only for zones, `NO-TOUCH`/`LOCKED`, and dependency verification state (`proven`/`inferred`/`shared`).
- **C7 (State Signal Gradient Rim, Zero Pastel Card Soup)**: Active gradient rims (`#54B9F2 -> #D8A6FF -> #F7A8D8`) appear ONLY on active/expanded containers; zero non-status badges or collapsed containers use raw solid `#54B9F2`/`#D8A6FF`/`#F7A8D8` fills.
- **C8 (Multi-Tier Corner Radii)**: Distinct visual hierarchy across `8px` panels/rims, `6px` controls, `4px` filter pills, `2px` state badges, `3.5px` meter tracks, and `0px` hairline ledger rows.
- **C9 (`>= 10.5pt` Typography Floor & Saturated Status)**: All copy stays `>= 10.5pt`; semantic status (`+`/`-` growth, `diff 0 B` `8px` dot, `NO-TOUCH`, `● GREEN`/`RED` zone marks) renders in saturated `#28D66C` / `#F04458` (`#0FA84F` / `#E02444` Light) while metadata stays quiet.
- **C10 (Dark & Light Input/Surface Contrast)**: In both Dark and Light screenshots, text inputs sit inside distinct `input_bg` wells and `bg`/`panel_alt`/`panel` separation reads cleanly.
- **C11 (Content-Hugging Settings & Top-Left Alignment)**: Settings cards with zero exclusions show no orphaned empty headers; non-populated states align top-left cleanly without floating centred whitespace.
- **C12 (`z-controller` Control Sync Intact)**: Header theme toggle, outline `Rescan`, `Settings`/`Back to Apps`, `Add exclusion`, overlay `All`/`Close`, `used by` buttons, inputs, checkboxes, and slim `6px` `SPACE_8`-gutter scrollbar mirror `z-controller` outline geometry, fill hierarchy, hover/pressed borders, and continuous `1px` outlines.
