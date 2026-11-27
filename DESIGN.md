# WT Native Instrument Design System (`DESIGN.md`)

> **Canonical Ecosystem Standard**: This document is the canonical design standard for **all WT applications** (`z-controller`, `y-storage`, `WT Manager`, and future WT native utilities).
> **WT UI Library Governance (`wt-ui-library`)**: **Part I — Shared Core is governed by `WT_UI_LIBRARY.md §0 (v2 — Monochrome Theme Engine)`** (installed globally under the `wt-ui-library` skill). Each screen uses **one hue family** rendered as a full 4-step shade ladder (`deep / ink -> accent -> source -> pastel / soft fill`) from one base colour (default = WT Blue `#7CCBF2`). Backgrounds stay strictly neutral (`R == G == B`), never theme-tinted. Buttons never carry gradients; meter bars and active container rims use a within-family monochrome sweep only (the cross-hue blue→pink gradient belongs strictly to the WT logo mark). Semantic status components (`§4B`) use green/amber/red and always pair **icon + text label** (`✓`, `!`, `✕`) per `§0.8`. Wherever anything in `DESIGN.md` Part I conflicts with `WT_UI_LIBRARY.md`, **`WT_UI_LIBRARY.md` wins**.
> **Reference Implementation**: **`z-controller`** (`Y:\Repos\z-controller`) is the reference implementation for shared controls, rim geometry, and outline hierarchy.
> **Adoption Rule**: Theme colour is an **app-level user setting**, never a per-component hue choice. Application-specific layouts, component forms, and domain widgets belong strictly under **Part II — App-Specific Sections**.

**Last Updated**: 2026-11-28 (`theme-engine` · `WT_UI_LIBRARY §0 v2`)

---

# Part I — Shared Core (WT Family Standard)

All rules, tokens, geometries, and component behaviours in Part I apply identically across every WT application.

## 1. North Star & Anti-Patterns

WT applications are precision instruments, not dashboards: every pixel, colour, border, and type tier must encode real system state or measured quantity, and nothing exists as decoration for its own sake. We design explicitly against the **generic AI UI failure mode**:

1. Repeating identical bordered cards around every list or ledger row (`cards-inside-cards` when expanded).
2. Applying a single uniform corner radius (`6px` on panels, badges, tracks, and buttons alike).
3. Tinting grey surfaces with theme colour or filling large containers with raw saturated accents or status colours.
4. Mixing multiple unrequested accent hues across components instead of deriving one cohesive monochrome shade ladder from the active theme colour.
5. Rendering secondary metadata (publishers, provenance/basis strings, kind labels, supporting counts) at equal size and brightness as primary quantities.
6. Placing badge chips on static labels (`READ-ONLY`, mode banners, descriptors) or using colour-only status indicators without icon + label.

---

## 2. Monochrome Theme Engine & Colour Tokens (`WT_UI_LIBRARY §0`)

### 2.1 Pure Neutral Surface, Border & Typography Tokens (`R == G == B`)

| Token | Dark Mode | Light Mode | Role |
|---|---|---|---|
| `bg` (`S0` Canvas) | `#0A0A0A` | `#F2F2F2` | Window backdrop and top header canvas (strictly untinted neutral) |
| `panel_alt` (`S1` Recessed) | `#121212` | `#F7F7F7` | Sticky table headers, expanded active drawers, pinned footer |
| `panel` (`S2` Primary) | `#161616` | `#FFFFFF` | Ledger wrappers, summary/instrument pods, Settings cards |
| `widget` (`S3` Rest) | `#1C1C1C` | `#F4F4F4` | Filter pill backgrounds, inactive interactive surfaces |
| `widget_hover` (`S3` Hover) | `#262626` | `#EAEAEA` | Hovered buttons, theme toggle, interactive toggles, hovered rows |
| `widget_active` (`S3` Press) | `lerp(#161616, source, 0.24)` | `lerp(#FFFFFF, source, 0.30)` | Pointer-down active buttons and pressed controls |
| `input_bg` (`Well`) | `#0A0A0A` | `#F2F2F2` on `panel` (`#FFFFFF` via `header_input_bg` on `#F2F2F2` canvas) | Text input wells (maintains distinct neutral recess) |
| `border` (`Hairline`) | `#2E2E2E` | `#DADADA` | `1px` crisp structural borders, `SecondaryOutline` rest strokes, hairline ledger dividers |
| `scrollbar_thumb` | `#585858` | `#B0B0B0` | Slim vertical scrollbar thumb at rest |
| `scrollbar_thumb_hover` | `#7C7C7C` | `#888888` | Slim vertical scrollbar thumb on hover (`accent_line` when dragged) |
| `text` | `#EEEEEE` | `#1E1E1E` | Primary headlines, row identities, dominant measurements, `SecondaryOutline` rest labels |
| `muted` | `#9E9E9E` | `#636363` | Secondary metadata, quiet totals, `SubtleOutline` rest labels, disabled labels |
| `subtle_text` | `#848484` | `#787878` | Low-contrast inline descriptors, publishers, truncated basis copy |
| `hint_text` | `#666666` | `#9A9A9A` | Unfocused, empty input placeholder hints only |

### 2.2 Single-Hue Shade Ladder (`ThemeLadder`) & Presets

One base colour generates a complete 4-step monochrome shade ladder (`deep / ink -> accent -> source -> pastel / soft fill`) via HSL luminance/contrast normalisation (`contrast_ratio >= 4.5:1` against neutral surfaces):

| Token | Default WT Blue Dark | Default WT Blue Light | Role |
|---|---|---|---|
| `accent_line` | `#69CBFC` | `#0090D9` | High-contrast ink step for `PrimaryOutline` text/borders, checkboxes, active icons (locked max-vivid `#0090D9` for Blue Light / `#FA29AA` for Pink Light; `>= 4.5:1` auto-normalised on generated presets & Custom) |
| `accent_border` | `#69CBFC` | `#0090D9` | Active/hover/selected `1px` border step |
| `accent_soft_fill` | `#253141` | `#E2F4FC` | Soft tinted pill/button backdrop derived from the active hue |
| `accent_pill_text` | `#AFE3FD` | `#0090D9` | Pill ink over `accent_soft_fill` (`#0090D9` for Blue Light / `#FA29AA` for Pink Light; `>= 4.5:1` normalised on generated presets & Custom) |
| `accent_sweep[0..4]` | `deep -> source -> accent_dark -> pastel` | `deep -> ink -> source_mid -> light_tint` | 4-stop within-family monochrome sweep for meter bars & active drawer rims |

**Curated Presets + Custom Picker**:
- `Blue` (`#7CCBF2`, default — WT Blue), `Pink` (`#F2A9D6`), `Purple` (`#9B8AFB`), `Green` (`#3ECF6E`), `Orange` (`#FB923C`), `Yellow` (`#FACC15`), `Red` (`#F87171`), plus `Custom` (`#RRGGBB` hex input & `R/G/B` channel sliders).
- Any dark/pale `Custom` input (including bright `#FACC15` Yellow on Light `#FFFFFF`/`#F2F2F2`) is automatically normalised by the theme engine so `accent_line` and `accent_pill_text` always clear `4.5:1` contrast without changing hue.
- **No cross-hue UI mixing**: The original blue→pink cross-hue gradient belongs **exclusively** to the WT logo mark (`assets/logo-mark.png`).

### 2.3 Separate Semantic `§4B` Status Family

| Token | Dark Mode | Light Mode | Role (`§0.8` Icon + Label Required) |
|---|---|---|---|
| `status_green` / `pill_green_fill` | `#28D66C` / `#102E20` | `#0A7A3C` / `#E4F9EC` | Positive state (`✓ new`, `✓ GREEN`, `✓ diff 0 B`, `-` shrink deltas, `✓ Shared & system caches`) |
| `status_amber` / `pill_amber_fill` | `#F5B83D` / `#382E25` | `#A66C07` / `#FDF0DA` | Warning state (`! Scope changed — rescan needed`) |
| `status_red` / `pill_red_fill` | `#F04458` / `#2C121E` | `#E02444` / `#FDE8ED` | Danger/error state (`✕ NO-TOUCH`, `✕ LOCKED`, `✕ RED`, `✕ missing`, `✕ ROOT`, `✕ SYSTEM`, `✕ diff N B`, `✕ Read error`, `+` growth deltas) |

---

## 3. Typography Scale & Luminance Hierarchy

WT applications pair **`Segoe UI`** for proportional interface text with **`Consolas`** monospace tabular numerals for measured quantities, coordinates, and timestamps. Minimum readable type floor across all UI copy: **`10.5pt`**. No rendered text may be smaller than `10.5pt`.

- **Numeral Weight Rule (`perf-pass-1`)**: **All numerals (`Total size`, `Own size`, drive `free`/`used` figures, pinned ledger sizes, deltas, and counts) are Regular (`Consolas Regular`) — NEVER Strong/Bold.** Heavy bold numeral slabs add visual noise without aiding legibility.
- **Strong Weight Rule**: **`Strong` weight (`Segoe UI Semibold`) is reserved strictly for the app name in a row and an expanded drawer's title line.** Headlines, metadata, badges, and numerals stay Regular.

| Tier | Size & Weight | Luminance Token | Role |
|---|---|---|---|
| **T1 — Instrument Headline** | `16.0pt` `Segoe UI` Regular (`14.5pt` `Consolas Regular` for primary pod metrics) | `text` | View titles and top-level live readouts (`drive free space`, primary instrument header values) |
| **T2 — Dominant Row Quantity** | `13.0pt`–`13.5pt` `Consolas Regular` (never Strong) | `text` | Single right-aligned primary quantity on every list row or card |
| **T3 — Primary Row Identity & Drawer Title** | `13.0pt`–`14.0pt` `Segoe UI Semibold` (`Strong`) on row app names & expanded drawer titles; `12.0pt`–`12.5pt` Regular for pinned ledger titles, button labels, input values | `text` (controls follow button state table `§6`) | Row app names, expanded drawer titles, pinned section titles, button captions, active filter/input copy |
| **T4 — Secondary Metadata & Quiet Numbers** | `10.5pt`–`11.5pt` Regular (`11.5pt` `Consolas Regular` paths/supporting sizes; `11.0pt` column headers & publishers; `10.5pt` section eyebrows, badges, word-capped `"…"` provenance) | `muted` / `subtle_text` | Sticky column headers, secondary row attributes, sub-table quantities, full-text hover tooltips when capped |

---

## 4. Spacing Scale & Corner Radii

### 4.1 Spacing Scale (Multiples of 4 Only)
`SPACE_4 = 4.0`, `SPACE_8 = 8.0`, `SPACE_12 = 12.0`, `SPACE_16 = 16.0`. All outer margins, panel padding, header control gaps, table cell gaps, and vertical section rhythms derive strictly from `{4, 8, 12, 16}`. Rows hug their content cleanly without trailing dead whitespace; empty/sparse states align top-left rather than floating centred in dead vertical space.

### 4.2 Corner-Radius Scale (Differentiated by Role)
- **`0.5 * badge_height` (~`9.0px` `RADIUS_BADGE_PILL`)**: Non-interactive status/zone/confidence/qualifier **pill badges** (`✕ NO-TOUCH`, `✕ LOCKED`, `proven`, `inferred`, `shared`, `✕ missing`, `✓ GREEN`, `✕ RED`, `BODY`, `✓ new`, qualifiers).
- **`8.0px` (`RADIUS_PANEL` / `CARD_ROUNDING`)**: Elevated containers — summary pods, ledger wrappers, active/expanded containers & monochrome rims, Settings panels, overlays.
- **`6.0px` (`RADIUS_CONTROL`)**: Shared interactive controls synced with `z-controller` — `ThemeToggle`, outline action buttons (`Rescan`, `Settings`, `Clear`, `Add exclusion`, `Remove`, overlay `Close`/`All`), segmented controls, text input wells.
- **`4.0px` (`RADIUS_PILL`)**: Compact interactive single-select filter buttons, preset swatch chips, inline `used by N apps >` triggers, and `16x16` toggle checkboxes.
- **`3.5px`–`4.0px` / `1.0px` (Track Fill)**: Capacity and proportion meters (`2px` slim bars use `1.0px` rounding).
- **`0.0px`**: Hairline-divided ledger rows and horizontal dividers.

---

## 5. Monochrome Sweep-Rim State Rule (WT Signature)

The signature continuous perimeter rim is a **state signal**, never decorative chrome:

- **Where permitted**: Strictly **active or expanded containers** — an active virtual/controller card (`z-controller`) or an expanded inspection drawer (`y-storage`). Collapsed rows, idle cards, and static containers MUST NOT carry highlight rims (`1px border` hairlines only).
- **Geometry & Within-Family Monochrome Sweep (`WT_UI_LIBRARY §0.6`)**:
  - **Outer rounding**: `8.0px` (`CARD_ROUNDING`), **`16`** corner arc segments (`17` points/corner), **`BANDS = 5`**.
  - **Edge profile**: `offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.5*px, 0.5*px]`, `alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8]`, paired with an inner content fill polygon `rect.shrink(1px)` at **`7.0px`** rounding (`panel_alt` when expanded over a ledger).
  - **Stops**: `pal.sample_sweep(t)` (`t = clamp(((x - left)/w)*0.65 + ((y - top)/h)*0.35, 0.0, 1.0)`) interpolating strictly within the active theme's 4-step monochrome ladder (zero blue→pink mixing).

---

## 6. Family Button & Interactive Control Standard

All shared interactive controls mirror **`z-controller`** (`Y:\Repos\z-controller\apps\z-controller\src\ui.rs`) so headers, forms, dialogs, and scrollbars feel built from one toolkit.

> **WT UI Library Rule (`WT_UI_LIBRARY.md §0`)**: Buttons **never** carry gradient borders or gradient fills; all active controls use the app's single monochrome theme ladder (`accent_soft_fill`, `accent_border`, `accent_line`). Unaccented actions remain **neutral** (`SecondaryOutline`) or **quiet** (`SubtleOutline`) at rest.

### 6.1 Crisp 9-Slice Outline Geometry & Padding
- **Perimeter ring rendering**: Controls use pixel-snapped geometry via a concentric physical-pixel-feathered rounded-box mesh (`16` segments per corner quadrant, `1px` edge AA feathering on curves with exact pixel-snapped straight H/V edges and `±0.42px` diagonal stroke compensation) paired with a 3-quad + 4-corner-fan 9-slice interior so straight panel regions blit at memory-fill speed while retaining smooth corner AA across `100%`, `125%`, and `150%` display scaling. When `border_color == Color32::TRANSPARENT` (`SubtleOutline`), no border is drawn.
- **Horizontal padding**: `SPACE_12` (`12px`) for standard controls (`min_size.y > 22.0`); `SPACE_8` (`8px`) for compact buttons (`min_size.y <= 24.0`, `6px` for compact `PrimaryOutline` pod `Rescan`); `3px` for `SubtleOutline` row actions (`<= 20px`).

### 6.2 Outline Button Variants & Interactive States

| Variant / State | Background Fill | `1px` Outline Stroke | Text / Glyph Foreground | Typical Use |
|---|---|---|---|---|
| **`PrimaryOutline`** (rest) | `accent_soft_fill` | `accent_border` | `accent_line` | Global and pod `Rescan` primary actions |
| **`SecondaryOutline`** (rest) | `TRANSPARENT` | `border` | `text` | Neutral view/utility/overlay/card buttons (`Settings`, `Back to Apps`, `ThemeToggle`, `All`, `Close`, `Add exclusion`) |
| **`SubtleOutline` / Row-Level** (rest) | `TRANSPARENT` | `TRANSPARENT` | `muted` | Quiet repeated row actions & inline resets (`Locate`, `Web search`, `used by N apps >`, `Remove`, `Clear`, `Exclusions`) |
| **Hovered** (`PrimaryOutline` / `SecondaryOutline`) | `widget_hover` | `accent_border` | `accent_line` | Hover promotes border & label/glyph to active theme accent |
| **Hovered** (`SubtleOutline` / Row-Level) | `widget_hover` | `TRANSPARENT` | `accent_line` | Muted surface fill + accent label with zero border box |
| **Pressed** (`is_pointer_button_down_on`) | `widget_active` | `accent_border` (`TRANSPARENT` on `SubtleOutline`) | `text` | Active pointer press feedback in the active theme ladder |
| **Active-Highlight** (open/selected trigger) | `accent_soft_fill` | `accent_border` | `accent_line` | Open/latched cross-reference trigger (`Exclusions`, `used by N apps >`) |
| **Disabled** (`!enabled`) | `TRANSPARENT` | `border` (`TRANSPARENT` on `SubtleOutline`) | `muted` (`lerp(bg, muted, 0.55)` on `SubtleOutline`) | Unavailable action (`Scanning...`, missing-on-disk `Locate`) |

### 6.3 Shared Header & Form Controls
- **`ThemeToggle` Icon Button**: Square `28x28 px` (`HEADER_CONTROL_H`) `6.0px` `SecondaryOutline` container sharing the neutral-rest / accent-hover / accent-press states above (8-ray sun glyph in Dark mode, crescent moon in Light mode).
- **Text Inputs**: Recessed `RADIUS_CONTROL = 6.0px` frame filled with `input_bg` (`header_input_bg` when sitting on `bg` canvas), `1px` `border` (`accent_line` while focused), `12.0pt` `text`, `12.0pt` `hint_text` when unfocused and empty.
- **Checkboxes**: `16x16 px` (`14x14 px` compact pod) `RADIUS_PILL = 4.0px` box (`accent_soft_fill` + `accent_border` + `accent_line` checkmark when checked).
- **Scrollbars**: Non-floating `6.0px` vertical thumb (`scrollbar_thumb` -> `scrollbar_thumb_hover` -> `accent_line`), `handle_min_length = 24.0`, `SPACE_8` gutter.

### 6.4 WT Header Lockup & Application Icon Standard
- **Original Brand Mark Only**: Every WT application embeds the original unmodified WT mark (`assets/logo-mark.png`, `24 x 24 px` logical in-app header lockup, transparent backdrop — the sole place in the UI allowed to carry the cross-hue blue→pink gradient).
- **Header Lockup Geometry**: Left-aligned on the top header row (`HEADER_CONTROL_H = 28.0px`): `[24x24 WT mark] + SPACE_8 + [16.0pt app name in text] + SPACE_8 + [11.5pt caption]`.

---

## 7. Chip Discipline

1. **Pill Badges Exist Only for Decisions or State**: Full pill shape (`0.5 * height` ~ `9.0px`, `10.5pt` Regular) for live state, safety, provenance, or qualifiers. Both Dark and Light modes use `1px` outline + `accent_soft_fill` + high-contrast `accent_pill_text` (for qualifiers and `proven`) or `§4B` status pairs (for status badges).
2. **Forbidden Static Chips**: Never wrap persistent modes (`READ-ONLY`), hardware/OS labels, or prose category names in bordered badge chips.
3. **Interactive Filter Buttons**: Single-select filter chips (`RADIUS_PILL = 4.0px`) use `accent_soft_fill` + `accent_border` + `text` when active; `widget` + `border` + `muted` when inactive.

---

## 8. Semantic Status Rules (`WT_UI_LIBRARY §0.8 & §4B`)

Because the user can set the app theme to `Green`, `Yellow`, `Orange`, or `Red`, **semantic status (`§4B`) must never rely on colour alone**:

| Category | Element Examples | Required Treatment |
|---|---|---|
| **§4B Semantic Status (Icon + Label Required)** | • `✓ new` (`NewPill`), `✓ GREEN` / `✕ RED` zone pills, `✕ NO-TOUCH`, `✕ LOCKED`, `✕ missing`, `✕ ROOT`, `✕ SYSTEM`<br>• `✓ diff 0 B` (vector checkmark glyph + text) / `✕ diff N B` (alert pill)<br>• `Read-only · ! Scope changed — rescan needed` (`status_amber`)<br>• `[✕ Warning] <msg>` (`status_red`), `✕ Read error` on `0`-byte drive reads<br>• Signed deltas (`>= 1 MB`: `+` in `status_red`, `-` in `status_green`) and `✓` checkmark icon on `Shared & system caches` | Saturated green (`#28D66C` Dark / `#0A7A3C` Light), amber (`#F5B83D` / `#A66C07`), or red (`#F04458` / `#E02444`) ALWAYS accompanied by an explicit icon (`✓` / `!` / `✕` or `+`/`-` sign) + text label |
| **Theme-Engine Monochrome Accent / Neutral** | • Included drive allocation bars & row proportion bars (`pal.sample_sweep(t)`)<br>• Expanded drawer rim (`pal.sample_sweep(t)`)<br>• `proven`, `HARDLINK`, `ESTIMATE`, `ProgramData · shared`, `loose files`, `used by N apps`, `N binary importers` pills<br>• `inferred`, `shared`, `BODY`, kind chips, manual pin badges (`Pin: App`/`Group`/`Personal`/`Ignore`) | Active theme's `accent_soft_fill` / `accent_border` / `accent_pill_text` or neutral `border` / `muted` |

---

## 9. Shared Layout & Family Critique Checklist (`FC1`–`FC10`)

Before accepting screenshots of any WT application, verify **`FC1`–`FC10`**:
- **FC1 (No Card Soup)**: List rows sit inside hairline-divided ledger panels rather than stacked individual cards.
- **FC2 (Restrained Numeral Weight & Clear Row Hierarchy)**: Right-aligned `T2` totals lead by scale/luminance (`13.0pt` Consolas Regular, never Bold) alongside `T3` `Segoe UI Semibold` app names and `T4` metadata.
- **FC3 (State-Signal Monochrome Rim Only)**: Within-family sweep rims appear strictly on active/expanded containers; collapsed containers use `1px border`.
- **FC4 (Multi-Tier Corner Radii)**: Distinct radii across `8px` panels/rims, `6px` buttons/inputs, `4px` filter/swatch pills/checkboxes, `1px` bar ends, `9px` full-pill badges, and `0px` ledger rows.
- **FC5 (Monochrome Discipline & §0.8 Icon+Label Status)**: One active hue ladder per screen; neutral surfaces (`R == G == B`); zero cross-hue UI mixing outside the WT mark; every semantic status element pairs `✓` / `!` / `✕` (or `+`/`-`) with a label.
- **FC6 (Outline Button Hierarchy)**: `PrimaryOutline` (`Rescan`), neutral `SecondaryOutline` (`Settings`, `ThemeToggle`, facet triggers), and quiet `SubtleOutline` (`Locate`, `Web search`, `Exclusions`, `used by N apps >`); buttons never carry gradients.
- **FC7 (`>= 10.5pt` Floor)**: All visible labels, badges, captions, and tooltips meet or exceed `10.5pt`.
- **FC8 (Dual-Mode Surface Contrast)**: Dark and Light neutral modes both maintain clear surface separation and `>= 4.5:1` contrast for all theme ink/accent tokens (including normalised Custom colours).
- **FC9 (Content-Hugging Top-Left)**: Empty or sparse panels align top-left cleanly without centred dead space.
- **FC10 (Unmodified WT Brand Lockup)**: Header and Settings retain the original unmodified `24px` WT mark + `SPACE_8` + `16.0pt` app name.

---

# Part II — App-Specific Sections

Each section below documents only what is unique to an individual WT application beyond Part I.

## 10. App-Specific: `z-controller`

- **Active Controller Cards**: The active virtual controller card renders the `8.0px` Active Monochrome Sweep Rim (`§5`); standby cards use `panel` surfaces with `1px border`.
- **Battery Instrument Meter**: `124.0 x 8.0 px` chunky track (`Rounding 4.0`) using `status_green` above `20%` and `status_red` with `!` / low indicator at or below `20%`.

---

## 11. App-Specific: `y-storage`

### 11.1 Theme Setting & Component Forms (`y-storage`, `WT_UI_LIBRARY §0`)

> **App-level Theme Setting**: Theme colour (`Blue`, `Pink`, `Purple`, `Green`, `Orange`, `Yellow`, `Red`, or `Custom` `#RRGGBB`) and `Dark`/`Light` mode are chosen by the user once at the app level in **Settings -> Appearance & Theme** and persisted in `AppConfig` (`theme_mode`, `theme_preset`, `theme_custom_rgb`). Individual components never pick different brand hues; the table below records each component's **structural form** and **§0.6 / §4B exceptions**.

| # | Screen / place | Component instance | Colour system | Form | Note |
|---|---|---|---|---|---|
| 1 | Settings (`Appearance & Theme`) | Mode toggles (`Dark` / `Light`), 7 preset swatches + `Custom`, `#RRGGBB` + RGB sliders, 4-step ladder preview | **Active theme ladder** | Swatch chips + `1px` bordered frame + live normalised preview | Persists in `AppConfig` (`config.toml`); normalises custom RGB to `>= 4.5:1` |
| 2 | Masthead & Drive pods | `Rescan` / `Rescan needed` (header) & per-pod `Rescan` | **Active theme ladder** | `PrimaryOutline` (`accent_soft_fill` + `accent_border` + `accent_line`) | Solid accent outline — never gradient |
| 3 | Masthead, Filter bar, Pod strip, Overlay, Drawer | `Settings`, `ThemeToggle`, `Locate`, `Web search`, `Exclusions`, `used by N apps >`, `Include`, active filter chips, sort arrows, qualifiers (`HARDLINK`, `ESTIMATE`, `ProgramData · shared`, `loose files`), `proven` | **Neutral (`SecondaryOutline` / `SubtleOutline`) + Active theme ladder** | Outline / Subtle / Checkbox / Pill | Uses `pal.accent_*` only when active/hovered/selected or qualifier/proven |
| 4 | Drive pods & App table (`Total size` / `Size on X:`) | Included drive usage bars & row proportion bars | **Within-family monochrome sweep (`§0.6`)** | 4-stop within-family sweep (`pal.sample_sweep(t)`) | Never mixes blue→pink outside the WT logo mark |
| 5 | Expanded app drawer | Active/expanded inspection drawer perimeter | **Within-family monochrome sweep (`§5`)** | 5-band smooth sweep rim (`pal.sample_sweep(t)`) | Cached origin-relative mesh translated per frame |
| 6 | Masthead logo & OS chrome | WT logo mark (`assets/logo-mark.png`) | **Original brand mark** | Full-colour blue→pink gradient raster (`24px` header) | Sole permitted location for the cross-hue brand gradient (`§0.6`) |
| 7 | App table, drawers, ledger, masthead, Settings | `✓ new`, `✓ GREEN`, `✕ RED`, `✕ NO-TOUCH`, `✕ LOCKED`, `✕ missing`, `✕ ROOT`, `✕ SYSTEM`, `✓ diff 0 B`, `✕ diff N B`, `! Scope changed — rescan needed`, `✕ Read error`, `[✕ Warning]`, signed `+/-` deltas | **§4B Semantic Status (`§0.8`)** | Icon (`✓` / `!` / `✕` or `+/-`) + text label + status palette | Never rendered without both icon/sign and text label |

---

- **Masthead & Settings Lockup**: Top masthead band (`Main`, `Settings`, `Dependencies`) renders `[24x24 WT mark] + 8px + "y-storage" (16.0pt text) + 8px + status caption` (`11.5pt muted` for `"Read-only · Last scan: <timestamp> · <duration>s"`, `accent_line` during active scans, or `status_amber` `"Read-only · ! Scope changed — rescan needed"` on stale scope with last-scan timestamp preserved on hover), paired with right-aligned `[ThemeToggle] [Rescan / Rescan needed] [Settings / Back to Apps]`. `Settings` houses naturally stacked, non-occluding sections (**`Appearance & Theme`** with `Dark`/`Light`, 7 presets + `Custom` `#RRGGBB` / RGB picker, live 4-step shade ladder, and `Near status hue` note, **`Scan Scope & Exclusions`** per-drive scope cards, unified **Attribution Zone Rulebook** grouping chips under `"App container"` / `"Personal files"` subheads with neutral muted `Manual pins`, and the `8px` `panel` About card repeating the `24px` WT lockup).
- **Adaptive Drive Instrument Header & In-Place Scope**: Physical fixed drives sit inside `CentralPanel` above the search/filter bar in a responsive `8px` `panel` pod grid (`1..=4` drives in 1 row, `5..=8` in 2 balanced rows, `9+` in a 5-column wrap; `1px border`, `Margin::symmetric(SPACE_12, SPACE_8)`, `SPACE_4`/`SPACE_8` multiples-of-4 gaps). Excluded drives are never walked or sized at all (including registered `InstallLocation` folders), and apps whose folders all live on excluded drives are omitted from the report; the Settings scope section and pod `Include` tooltip explain drive exclusion, while the `Exclusions` popover explains folder-level exclusion. Each pod uses consistent production copy across all drive counts (`1..10`):
  1. `"C:\"` (`13.0pt` `Segoe UI Semibold` `text`) + right-aligned `"{used} of {total}"` (`11.0pt` / `10.5pt` compact `Consolas Regular`);
  2. `"Scanned HH:MM"` (or `"Not scanned"` when excluded / `"✕ Read error"` when `0`-byte capacity / `"Scanning…"` during active per-drive rescan) + right-aligned `"{free} free"`;
  3. Flush `2.0px` proportional bar using the active theme's within-family monochrome sweep (`pal.sample_sweep(t)`) when included (`lerp(pod_fill, muted, 0.35)` greyed bar + dimmed surface/border when excluded; `status_red` with `"✕ Read error"` text when drive read fails);
  4. Compact in-place control strip (`24px`, single `"Hx"` optical baseline across `"Include"` and `"Exclusions"` sharing one `11.0pt` / compact `10.5pt` font size): `"Include"` checkbox (`accent_soft_fill`/`accent_border`/`accent_line`) + quiet `"Exclusions"` / `"Exclusions (N)"` (`SubtleOutline`) overlay trigger + right-aligned per-drive `"Rescan"` (`PrimaryOutline`: `accent_soft_fill` + `accent_border` + `accent_line`, `24px` min height). Scope or zone-rulebook edits mark the cached report stale (`! Scope changed — rescan needed`) cleanly without auto-triggering a surprise walk.
- **Unified Attribution Zone Rulebook & Bottom-Up Claim Resolution**:
  - **Zone Rulebook Types (`AppConfig.zones`)**:
    - **`AppContainer`**: immediate subfolders become candidate apps (defaults: `Apps`, `Games`, `Game`, `Programs`, `Program Files`, `Software`, `Softwares`, `PortableApps`).
    - **`PersonalFiles`**: folder and uncarved subtree route atomically to `Your files` (never app-hunted, never `Unattributed`; defaults: editable Windows shell names `Documents`, `Downloads`, `Pictures`, `Videos`, `Music`, `Desktop`, plus any user-added root names or explicit full paths such as `D:\Media`). Registered install roots inside a `PersonalFiles` zone still keep their own app ownership and are carved cleanly out of the personal zone's total.
    - **`Drive Root Rule`**: every top-level folder sitting directly at a scanned drive root (`<D>:\<Folder>`) is also treated as an app candidate after registered/manifest claims via the three-way inference test below.
    - **`Manual Pins`**: per-folder overrides (`Pin: App`, `Pin: Group`, `Pin: Personal`, `Pin: Ignore`) take final precedence over automated inference and persist across scans.
  - **Bottom-Up Claim Resolution Order** (deepest/most specific first, never outermost-first):
    1. **Registered Install Roots + Nested Carve-Outs**: Registry/Store install roots claim their exact path at any depth; contained child install roots are carved out at **any** nesting count (`>= 1` even when the outer root itself is an install location), subtracting carved bytes cleanly and noting `"contains <Child> — counted under <Child>"` in the parent drawer.
    2. **Manifest Containers**: Steam (`appmanifest_*.acf`) and Epic (`*.item`) manifests claim manifest-backed game directories (`Steam manifest` / `launcher manifest`); unmanifested sibling folders (`workshop`, `downloading`, caches, unmanifested folders) are never promoted as apps.
    3. **Real-Path Remainder Units**: Direct loose files inside any split directory emit `ScannedUnitKind::ContainerLooseFiles` carrying the real directory path (never fabricated `"[files]"` / `"[root files]"` paths). When matching an app's install folder they are qualified `"loose files in the install folder"`; otherwise unowned loose remainders keep their real path cleanly for `Locate` / triage with a `"loose files"` qualifier.
    4. **Two-Segment `Publisher\Product` Matching**: Before leaf-only rules, the last two path segments are composed in order with whole-token matching against installed apps' full names (`"publisher+folder match"`), gated by publisher token match or uniqueness across installed apps (any tie refuses attribution and leaves the folder `Unattributed`).
    5. **Leaf / Curated Token / Publisher / Store Package / Dot-Folder Rules**: Whole-token exact, curated, publisher, Microsoft Store, and standalone toolchain rules.
    6. **Three-Way Inference on Candidate Folders** (`AppContainer` children & drive-root children, never crossing manifest-only siblings):
       - **(5a)** Own-level app evidence (flat main `.exe` with PE `ProductName` when available) -> promote as app and stop inference descent (never blocks Step 1 carve-outs);
       - **(5b)** No own-level evidence and `>= 2` immediate children pass (a) -> **Group node**: descend and record the group folder name as publisher context on each child app;
       - **(5c)** No own-level evidence and `== 1` child passes (a) -> **Single-app wrapper**: keep the outer folder as the app;
       - **(5d)** Neither -> honest fallback (`Shared & system caches`, `Windows & system`, or `Unattributed`).
    7. **Containment Residue Weakest**: Enclosing `InstallLocation` claims only unclaimed residual subtrees left after steps (1)–(6) and manual pins.
- **Tabular App Footprint Ledger, Per-Drive Size Mode & Inspection Drawer**:
  - Sticky `26px` sortable column header (`Chevron · App name · Publisher · Category · Growth delta · Total size` — relabeling `Total size` to `Size on X:` when exactly one drive constraint is active via either the `Drive` facet or the `Has X: footprint` chip alone). Clicking any column sorts cleanly; `Size` and `Growth` sorting follow the displayed per-drive figure when in per-drive mode.
  - Fixed `18x18 px` extracted Windows app icon (from registry `DisplayIcon`, Steam/Epic manifests, Windows service executables, `Body`/install `.exe`s, or MSIX `AppxManifest.xml` `<Logo>` PNGs — never fabricated where none exist) or consistent neutral folder-tile glyph keeping column x-positions fixed across rows.
  - `Total size` / `Size on X:` embeds a quiet `<= 2px` (`PROP_BAR_W = 76.0`, `PROP_BAR_H = 2.0`) proportional bar in the active theme's within-family monochrome sweep relative to `max_bar_bytes`, renders a smaller muted `10.5pt` `"of <global>"` companion **after** the per-drive total inside the size cell whenever an app spans multiple drives (never in the Growth delta column), and updates the filter summary line to `"N apps · <sum> on X:"`. Missing or first-seen drive baselines display the `"✓ new"` green status pill (`status_green` border + text + checkmark over `pill_green_fill`); numeric deltas `< 1 MB` (`1,048,576 B`) render blank with exact signed bytes on hover; signed `>= 1 MB` deltas render as `+` red / `−` green monospace values.
  - Expanding an app row converts it into an `8px` `panel_alt` inspection drawer wrapped in the continuous **WT Active Monochrome Sweep Rim** (`§5`), suppressing the row hairline above it, showing its `Category` + rule basis (plus a quiet neutral `inferred` pill when all bases are inferred, a quiet `SubtleOutline` `used by N apps >` trigger when referenced as a dependency, and word-capped `"contains <Child> — counted under <Child>"` carve-out note lines), and aligning two sub-tables cleanly to the main size column's right edge with word/segment-boundary `"…"` truncation and full-text hover throughout:
    - `Locations (N)`: `Zone · Path · Basis · Locate / Web search · Size` (`✓ GREEN` / `✕ RED` full-pill status zone badges, `BODY` quiet structural pill, `ProgramData · shared` / `loose files in the install folder` / `loose files` theme-accent qualifier pills using `accent_soft_fill`/`accent_border`/`accent_pill_text`, word/segment-boundary `"…"` paths & basis with full hover tooltips, full-row `widget_hover`, plus `SubtleOutline` `Locate` & `Web search` actions with `"no longer on disk"` disabled guard and natural-question hover preview).
    - `Dependencies`: `Dependency name · Kind · Confidence · Used by · Own size` (`service or driver` vs `anti-cheat` quiet neutral kind chips, `proven` / `inferred` / `shared` / `✕ missing` pills, quiet borderless `SubtleOutline` `used by N apps >` overlay triggers, word/segment-boundary `"…"` name, kind, & basis with hover tooltips).
- **Collapsible & Full-Height Resizable Reconciliation Ledger Footer**: Pinned storage groups (`Windows & system`, `Your files` with `"Shell folders & Personal-files zones · read-only"` subtitle, `Shared & system caches`, `Unattributed`) sit in one `8px` closed `1px`-bordered `panel` scroll container whose height strictly reserves space below it for the per-drive reconciliation block at every window size (`760x520` minimum up) and any ledger resize height, with top `6px` drag-resize handle (`max(min_ledger_h, client_height - masthead_height - 4px)`), a `"Pinned storage ledger"` collapse chevron folding to a single inline summary line (`Win/sys · Your files · Shared caches · Unattributed` + `✓ diff 0 B`), and semantic `2px` colour ticks (`muted`, `muted`, `status_green` + `✓`, muted neutral). Beneath the closed frame sits the live per-drive reconciliation equation (accounting for NTFS `hardlink deduplication offset` where applicable) ending with green `diff 0 B` text + `✓` vector checkmark glyph (or `✕ diff N B` alert status pill if `diff != 0`), while the main app list paints a soft `22px` bottom gradient fade above the footer when scrollable content overflows. Expanded folder rows in **`Your files`**, **`Shared & system caches`** (`✓ GREEN`), and **`Unattributed`** share `Locate` + `Web search` (`SubtleOutline`, natural question hover query + `"no longer on disk"` guard) and paint full-row `widget_hover` across the row including action cells (`Windows & system` uses `✕ ROOT`, `✕ SYSTEM`, and accent `HARDLINK`/`ESTIMATE` badges and omits folder actions). Expanding **Unattributed** additionally provides a compact view-only toolbar with text search, `Clear`, 4 sort chips (`Size ↓ / ↑`, `Name A–Z`, `Path A–Z`), three `SecondaryOutline` facet dropdowns (`Drive`, `Size`, `Zone`), `Reset filters`, and a live `"N of M folders · <subtotal>"` counter.
- **`y-storage` Domain Critique Checks (`YS1`–`YS3`)**:
  - **YS1 (Shared Size Column Alignment)**: Sticky header `Total size`, main row totals, expanded `Locations`/`Dependencies` size columns, and pinned summary totals share one exact right vertical edge (accounting for the `14px` `6px + SPACE_8` scrollbar gutter).
  - **YS2 (Calm Drive Pod Hierarchy)**: Each drive pod gives `"C:\"` (`13.0pt` Strong) and `"{used} of {total}"` (`11.0pt Consolas Regular`) quiet structural primacy over timestamps, `"{free} free"`, flush `2.0px` bar, and the quiet `Include` / `Exclusions` / `Rescan` control strip.
  - **YS3 (Subordinate Shared Dependencies)**: Inside expanded drawers, `Dependencies` own sizes (`11.5pt` `muted`) visually recede behind the app's own total (`13.0pt` `text`) so shared runtimes never read as additive footprint.
