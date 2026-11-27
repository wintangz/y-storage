# WT Native Instrument Design System (`DESIGN.md`)

> **Canonical Ecosystem Standard**: This document is the canonical design standard for **all WT applications** (`z-controller`, `y-storage`, `WT Manager`, and future WT native utilities).
> **Reference Implementation**: **`z-controller`** (`Y:\Repos\z-controller`) is the reference implementation for all shared controls, theme tokens, rim shaders, and outline button geometry.
> **Adoption Rule**: Any WT application adopting this standard MUST copy `DESIGN.md` into its repository root and implement **Part I — Shared Core** unchanged. Application-specific layouts, domain widgets, and intentional deviations belong strictly inside that application's own section under **Part II — App-Specific Sections**.

**Last Updated**: 2026-11-27 (`ui-polish-4`)

---

# Part I — Shared Core (WT Family Standard)

All rules, tokens, geometries, and component behaviours in Part I apply identically across every WT application.

## 1. North Star & Anti-Patterns

WT applications are precision instruments, not dashboards: every pixel, colour, border, and type tier must encode real system state or measured quantity, and nothing exists as decoration for its own sake. We design explicitly against the **generic AI UI failure mode**:

1. Repeating identical bordered cards around every list or ledger row (`cards-inside-cards` when expanded).
2. Applying a single uniform corner radius (`6px` on panels, badges, tracks, and buttons alike).
3. Using raw pastel accents (`#69CBFC`, `#FA80CB`) or saturated status tones (`#28D66C`, `#F04458`) as large opaque surface fills.
4. Centring empty-state labels or orphaned section headers in dead vertical space.
5. Rendering secondary metadata (publishers, provenance/basis strings, kind labels, supporting counts) at equal size and brightness as primary quantities.
6. Placing badge chips on static labels (`READ-ONLY`, mode banners, descriptors) rather than live states or user decisions.

---

## 2. Colour Tokens (Dark & Light Modes)

All WT applications share one dual-mode palette calibrated against `z-controller` (`Y:\Repos\z-controller\apps\z-controller\src\ui.rs`).

### 2.1 Surface, Border & Typography Luminance Tokens

| Token | Dark Mode | Light Mode | Role |
|---|---|---|---|
| `bg` (`S0` Canvas) | `#0A0A10` | `#EEF4FA` | Window backdrop and top header canvas |
| `panel_alt` (`S1` Recessed) | `#10101A` | `#F4F8FC` | Sticky table headers, expanded active drawers, pinned footer (`z-controller` `faint_bg`) |
| `panel` (`S2` Primary) | `#14141F` | `#FFFFFF` | Ledger wrappers, summary/instrument pods, Settings cards |
| `widget` (`S3` Rest) | `#181826` | `#F5F8FC` | Filter pill backgrounds, inactive interactive surfaces |
| `widget_hover` (`S3` Hover) | `#1F1F31` | `#E8F4FB` | Hovered buttons, theme toggle, interactive toggles, hovered rows |
| `widget_active` (`S3` Press) | `#142832` | `#D8EEF9` | Pointer-down active buttons and pressed controls |
| `input_bg` (`Well`) | `#0A0A10` | `#EEF4FA` on `panel` (`#FFFFFF` directly on `bg` canvas) | Text input wells (maintains distinct recess vs `#FFFFFF` cards and `#EEF4FA` canvas) |
| `border` (`Hairline`) | `#26263A` | `#D9E4F0` | `1px` crisp structural borders, `SecondaryOutline` rest strokes, hairline ledger dividers |
| `scrollbar_thumb` | `#52567A` | `#98B0CA` | Slim vertical scrollbar thumb at rest (minimum visible contrast against track) |
| `scrollbar_thumb_hover` | `#7A86B8` | `#648CB8` | Slim vertical scrollbar thumb on hover (brightens to `accent_line` when dragged) |
| `text` | `#EDEDF2` | `#1C2733` | Primary headlines, row identities, dominant measurements, `SecondaryOutline` rest labels/glyphs |
| `muted` | `#9A9AB0` | `#5F7186` | Secondary metadata, quiet totals, `SubtleOutline` row-level rest labels, disabled button labels |
| `subtle_text` | `#82829A` | `#748499` | Low-contrast inline descriptors, publishers, truncated basis copy |
| `hint_text` | `#64647A` | `#98A6B8` | Unfocused, empty input placeholder hints only (hidden on focus or non-empty value) |

### 2.2 Accent & Semantic Status Tokens

> **Provenance (`ui-polish-3` / FIX 15)**: Accent family re-anchored to the owner's two-colour original flat mark (`logo-mark-flat`: blue `#7CCBF2` and pink `#F2A9D6`). Dark `accent_1` (`accent_line`: `#69CBFC`) and `accent_2` (`#FA80CB`) boost saturation and contrast from those two flat stops for Dark surfaces (`z-controller` inherits this updated Part I family on its next adoption); Light `accent_ink_1` (`#0090D9`) and `accent_ink_2` (`#FA29AA`) are the **maximum-vivid** steps chosen explicitly by the owner over darker muted variants. Note that the converted vector/raster asset (`assets/logo-mark.png`, `1200x1200` RGBA) blends across a 6-stop sweep (`#54B9F2 → #7CD0FA → #A5E6FF → #D8A6FF → #F7A8D8 → #FFC7E3`) whose unnamed violet midpoint `#D8A6FF` exists **strictly inside the gradient sweep** — the conversion gradient is not a palette source, and violet is removed as a named colour token everywhere.

| Token | Dark Mode | Light Mode | Role |
|---|---|---|---|
| `accent_1` (`accent_line`) | `#69CBFC` | `#0090D9` | **Rationed** (`§6`): `PrimaryOutline` CTA, `PodPrimaryOutline` Rescan, selected/active states, lower-tier button hover, chevrons |
| `accent_2` | `#FA80CB` | `#FA29AA` | Repeated local pod `PodPrimaryOutline` border & label, pressed control outlines, location qualifier badge stroke |
| `pill_blue_fill` | `#253141` (text `#AFE3FD`) | `#E2F4FC` (text `#0090D9`) | Subtle fill for `PrimaryOutline` global CTA, active filter pills, checkboxes, provenance badges |
| `pill_pink_fill` | `#382C3C` (text `#FBBBE3`) | `#FCECF6` (text `#FA29AA`) | Fill for `PodPrimaryOutline` local pod `Rescan` buttons and soft backdrop for location qualifier badges (`stroke: accent_2`) |
| `WT_RIM_GRADIENT_STOPS` | `#54B9F2 → #7CD0FA → #A5E6FF → #D8A6FF → #F7A8D8 → #FFC7E3` | `#54B9F2 → #7CD0FA → #A5E6FF → #D8A6FF → #F7A8D8 → #FFC7E3` | Unnamed 6-stop logo-file sweep for active/expanded container rims (`§5`) and included drive capacity meters |
| `STATUS_GREEN` (`status_green`) | `#28D66C` | `#0A7A3C` (`#28D66C` dots/marks) | Saturated positive/healthy semantic status (`§8`) |
| `pill_green_fill` | `#102E20` | `#E4F9EC` | Soft backdrop for saturated green semantic status pills |
| `STATUS_RED` (`status_red`) | `#F04458` | `#E02444` (`#F04458` dots/bars) | Saturated alert/growth/locked semantic status (`§8`) |
| `pill_red_fill` | `#2C121E` | `#FDE8ED` | Soft backdrop for saturated red alerts |
| `status_amber` | `#F5B83D` | `#A66C07` | Muted warning status (`§8`, e.g., stale scan scope — never `STATUS_RED`) |
| `pill_amber_fill` | `#382E25` | `#FDF0DA` | Soft backdrop for warning status pills (`§8`) |

---

## 3. Typography Scale & Luminance Hierarchy

WT applications pair **`Segoe UI`** for proportional interface text with **`Consolas`** monospace tabular numerals for measured quantities, coordinates, and timestamps. Minimum readable type floor across all UI copy: **`10.5pt`**. No rendered text may be smaller than `10.5pt`.

- **Numeral Weight Rule (`perf-pass-1`)**: **All numerals (`Total size`, `Own size`, drive `free`/`used` figures, pinned ledger sizes, deltas, and counts) are Medium — or Regular (`Consolas Regular`) where Medium is unavailable in the bundled font — NEVER Strong/Bold.** Heavy bold numeral slabs add visual noise without aiding legibility.
- **Strong Weight Rule**: **`Strong` weight (`Segoe UI Semibold`) is reserved strictly for the app name in a row and an expanded drawer's title line.** Headlines, metadata, badges, and numerals stay Regular or Medium.

| Tier | Size & Weight | Luminance Token | Role |
|---|---|---|---|
| **T1 — Instrument Headline** | `16.0pt` `Segoe UI` Regular (`14.5pt` `Consolas Regular` for primary pod metrics) | `text` | View titles and top-level live readouts (`drive free space`, primary instrument header values) |
| **T2 — Dominant Row Quantity** | `13.0pt`–`13.5pt` `Consolas Regular` (Medium where available; never Strong) | `text` | Single right-aligned primary quantity on every list row or card (dominant by size & brightness, never bold weight) |
| **T3 — Primary Row Identity & Drawer Title** | `13.0pt`–`14.0pt` `Segoe UI Semibold` (`Strong`) on row app names & expanded drawer titles; `12.0pt`–`12.5pt` Regular for pinned ledger titles, button labels, input values | `text` (controls follow button state table `§6`) | Row app names, expanded drawer titles, pinned section titles, button captions, active filter/input copy |
| **T4 — Secondary Metadata & Quiet Numbers** | `10.5pt`–`11.5pt` Regular (`11.5pt` `Consolas Regular` paths/supporting sizes; `11.0pt` column headers & publishers; `10.5pt` section eyebrows, badges, word-capped `"…"` provenance) | `muted` / `subtle_text` | Sticky column headers, secondary row attributes, sub-table quantities, full-text hover tooltips when capped |

---

## 4. Spacing Scale & Corner Radii

### 4.1 Spacing Scale (Multiples of 4 Only)
`SPACE_4 = 4.0`, `SPACE_8 = 8.0`, `SPACE_12 = 12.0`, `SPACE_16 = 16.0`. All outer margins, panel padding, header control gaps, table cell gaps, and vertical section rhythms derive strictly from `{4, 8, 12, 16}`. Rows hug their content cleanly without trailing dead whitespace; empty/sparse states align top-left rather than floating centred in dead vertical space.

### 4.2 Corner-Radius Scale (Differentiated by Role)
Different architectural tiers carry distinct corner radii:
- **`0.5 * badge_height` (~`9.0px` `RADIUS_BADGE_PILL`)**: Non-interactive status/zone/confidence/qualifier **pill badges** (`NO-TOUCH`, `LOCKED`, `proven`, `inferred`, `shared`, `missing`, `● GREEN`, `● RED`, `BODY`, `Roaming config`, `ProgramData · shared`, kind chips).
- **`8.0px` (`RADIUS_PANEL` / `CARD_ROUNDING`)**: Elevated containers — summary pods, ledger wrappers, active/expanded containers & gradient rims, Settings panels, overlays.
- **`6.0px` (`RADIUS_CONTROL`)**: Shared interactive controls synced with `z-controller` — `ThemeToggle`, outline action buttons (`Rescan`, `Settings`, `Clear`, `Add exclusion`, `Remove`, overlay `Close`/`All`), segmented sort controls, text input wells.
- **`4.0px` (`RADIUS_PILL`)**: Compact interactive single-select filter buttons, inline `used by N apps >` triggers, and `16x16` toggle checkboxes.
- **`3.5px`–`4.0px` (Track Outer) / `2.5px`–`3.0px` (Track Fill)**: Chunky `7px`–`8px` rounded capacity and battery meters.
- **`0.0px`**: Hairline-divided ledger rows, inline `<= 2px` proportion rules, and horizontal dividers.

---

## 5. Gradient-Rim State Rule (WT Signature)

The signature continuous gradient rim is a **state signal**, never decorative chrome:

- **Where permitted**: Strictly **active or expanded containers** — an active virtual/controller card (`z-controller`) or an expanded inspection drawer (`y-storage`). Collapsed rows, idle cards, meters, and static containers MUST NOT carry gradient rims (`1px border` hairlines only).
- **Exact geometry (`z-controller` `paint_active_card_gradient_border`)**:
  - **Outer rounding**: `8.0px` (`CARD_ROUNDING`), **`SEGS = 16`** corner arc segments, **`BANDS = 5`**.
  - **Edge profile**: `offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.4_f32, 0.0_f32]`, `alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8]` (`1.2pt` solid perimeter core + `2.5pt` soft inner glow), paired with an inner content fill polygon `rect.shrink(1.0)` at **`7.0px`** rounding (`panel_alt` when expanded over a ledger).
  - **Diagonal gradient stops**: parameter `t = clamp(((x - rect.left()) / w) * 0.65 + ((y - rect.top()) / h) * 0.35, 0.0, 1.0)` interpolating uniformly across the logo file's 6 stops: **`#54B9F2`** -> **`#7CD0FA`** -> **`#A5E6FF`** -> **`#D8A6FF`** -> **`#F7A8D8`** -> **`#FFC7E3`** (`0.0, 0.2, 0.4, 0.6, 0.8, 1.0`).

---

## 6. Family Button & Interactive Control Standard

All shared interactive controls mirror **`z-controller`** (`Y:\Repos\z-controller\apps\z-controller\src\ui.rs`) so headers, forms, dialogs, and scrollbars feel built from one binary toolkit.

> **Accent Rationing Family Rule (`button-discipline-1` — scope split updated in `ui-polish-4`)**: **PrimaryOutline splits by scope: the view's single global primary is `accent_1` blue; repeated local primaries (one per repeated card/pod) are `accent_2` pink. Buttons never carry gradient borders; gradients live on container rims and meters only.** `accent_line` (`#69CBFC` Dark / `#0090D9` Light) is reserved strictly for **(1)** the single global primary CTA of a view (`PrimaryOutline`, e.g., header `Rescan` / `Rescan needed`), **(2)** selected/active states (active filter chip, active sort arrow, checked checkbox, focused input border, active scrollbar drag, expanded chevron, open reverse-dependency trigger/overlay), and **(3)** hover feedback on neutral/quiet lower-tier buttons. Repeated local primary CTAs (`PodPrimaryOutline`, one per drive pod) take `accent_2` pink (`#FA80CB` on `#382C3C` Dark; `#FA29AA` on `#FCECF6` Light). Everything else is **neutral** (`SecondaryOutline`) or **quiet** (`SubtleOutline`) at rest.

### 6.1 Crisp Outline Geometry & Padding
- **Perimeter ring rendering**: Controls use pixel-snapped `RADIUS_CONTROL = 6.0px` / `RADIUS_PILL = 4.0px` / `RADIUS_PANEL = 8.0px` geometry via a single concentric physical-pixel-feathered rounded-box mesh (`32` segments per quadrant, `1px` edge AA feathering on curves with exact pixel-snapped straight H/V edges) and `egui` `bezier_tolerance = 0.015`, guaranteeing smooth corner anti-aliasing with zero binary stair-steps or corner gaps at `100%`, `125%`, and `150%` display scaling. When `border_color == Color32::TRANSPARENT` (`SubtleOutline` / row-level actions), no perimeter border is drawn at rest or on hover.
- **Horizontal padding**: `SPACE_12` (`12px`) for standard controls (`min_size.y > 22.0`); `SPACE_8` (`8px`) for compact table/inline action buttons (`min_size.y <= 22.0`, e.g., `Locate`, `Web search`, `used by N apps >`).

### 6.2 Outline Button Variants & Interactive States

| Variant / State | Background Fill | `1px` Outline Stroke | Text / Glyph Foreground | Typical Use |
|---|---|---|---|---|
| **`PrimaryOutline`** (rest) | `pill_blue_fill` (`#253141` / `#E2F4FC`) | `accent_line` (`#69CBFC` / `#0090D9`) | `accent_line` (`#69CBFC` / `#0090D9`) | **Single** global primary CTA of a view (`Rescan` / `Rescan needed` in main header) |
| **`PodPrimaryOutline`** (rest) | `pill_pink_fill` (`#382C3C` / `#FCECF6`) | `accent_2` (`#FA80CB` / `#FA29AA`) | `accent_2` (`#FA80CB` / `#FA29AA`) | **Repeated** local primary CTA (`Rescan` on each drive pod, `y-storage §11`) |
| **`SecondaryOutline`** (rest) | `TRANSPARENT` | `border` (`#26263A` / `#D9E4F0`) | `text` (`#EDEDF2` / `#1C2733`) | **Neutral** view/utility/overlay/card buttons (`Settings`, `Back to Apps`, `ThemeToggle`, overlay `All` / `Close`, `Add exclusion`) |
| **`SubtleOutline` / Row-Level** (rest) | `TRANSPARENT` | `TRANSPARENT` (no border) | `muted` (`#9A9AB0` / `#5F7186`) | **Quiet** repeated row actions & inline resets (`Locate`, `Web search`, `used by N apps >`, `Remove`, `Clear`) — stacked rows read as one quiet text column |
| **Hovered** (`PrimaryOutline` / `SecondaryOutline`) | `widget_hover` (`#1F1F31` / `#E8F4FB`) | `accent_line` (`#69CBFC` / `#0090D9`) | `accent_line` (`#69CBFC` / `#0090D9`) | Hover feedback promotes border & label/glyph to blue accent |
| **Hovered** (`PodPrimaryOutline`) | `widget_hover` (`#1F1F31` / `#E8F4FB`) | `accent_2` (`#FA80CB` / `#FA29AA`) | `accent_2` (`#FA80CB` / `#FA29AA`) | Hover feedback keeps local pod primary button in `accent_2` pink |
| **Hovered** (`SubtleOutline` / Row-Level) | `widget_hover` (`#1F1F31` / `#E8F4FB`) | `TRANSPARENT` (no border) | `accent_line` (`#69CBFC` / `#0090D9`) | Muted surface fill (`4px`/`6px` pill) + accent label with zero border box |
| **Pressed** (`is_pointer_button_down_on`) | `widget_active` (`#142832` / `#D8EEF9`) | `accent_2` (`#FA80CB` / `#FA29AA`; `TRANSPARENT` on `SubtleOutline`) | `text` (`#EDEDF2` / `#1C2733`) | Active pointer press feedback |
| **Hovered List / Ledger Row** | `widget_hover` (`#1F1F31` / `#E8F4FB`) | `TRANSPARENT` | — | Full-row highlight across collapsed table rows and expanded ledger rows (including action cells) |
| **Active-Highlight** (open/selected trigger) | `pill_blue_fill` (`#253141` / `#E2F4FC`) | `accent_line` (`#69CBFC` / `#0090D9`) | `accent_line` (`#69CBFC` / `#0090D9`) | Open/latched cross-reference trigger (e.g., active `used by N apps >`) |
| **Disabled** (`!enabled`) | `TRANSPARENT` | `border` (`TRANSPARENT` on `SubtleOutline`) | `muted` (`lerp(bg, muted, 0.55)` on `SubtleOutline`) | Unavailable action (`Scanning...`, missing-on-disk `Locate` with zero box stroke) |

### 6.3 Shared Header & Form Controls
- **`ThemeToggle` Icon Button**: Square `28x28 px` (`HEADER_CONTROL_H`) `6.0px` `SecondaryOutline` container sharing the exact `SecondaryOutline` Neutral-rest / Accent-hover / Pink-press states above.
  - Dark mode draws an 8-ray sun glyph in `text` at rest (`accent_line` on hover, `text` on press).
  - Light mode draws a crescent moon in `text` at rest (`accent_line` on hover, `text` on press; `r = 5.3`), punched cleanly by an offset cutout circle (`r = 4.4`) painted in `bg` when rest/transparent, or the active hover/press `fill` during interaction.
- **Text Inputs**: Recessed `RADIUS_CONTROL = 6.0px` frame filled with `input_bg` (or `header_input_bg` `#FFFFFF` when sitting directly on `#EEF4FA` Light header canvas), `1px` `border` (`accent_line` while focused), `12.0pt` `text` value, `12.0pt` `hint_text` visible **only** when unfocused and empty.
- **Checkboxes**: `16x16 px` `RADIUS_PILL = 4.0px` box (`pill_blue_fill` + `accent_line` + `1.75pt` `accent_line` check stroke when checked; `widget_hover` + `accent_line` hovered; `input_bg` + `border` unchecked).
- **Scrollbars**: Non-floating `6.0px` vertical thumb (`scrollbar_thumb` -> `scrollbar_thumb_hover` -> `accent_line`), `handle_min_length = 24.0`, `SPACE_8` (`8px`) inner gutter (`bar_outer_margin = 0.0`).

### 6.4 WT Header Lockup & Application Icon Standard
- **Original Brand Mark Only**: Every WT application embeds the original unmodified WT mark (`assets/logo-mark.png`, `include_bytes!`, transparent background — no added background tile, container border, or app-specific glyph fusion) for both the multi-size `.exe`/window icon (`16px`–`256px` across title bar, taskbar, Alt-Tab, and Explorer) and the in-app header lockup.
- **Header Lockup Geometry**: Left-aligned on the top header row (`HEADER_CONTROL_H = 28.0px` vertical centreline):
  1. **Logo Mark**: **`24 x 24 px`** logical (`24 * pixels_per_point` physical, premultiplied-alpha area-downsampled from source artwork), transparent backdrop, optically centred vertically inside the `28.0px` row (`2.0px` top/bottom margin).
  2. **Logo-to-Name Gap**: **`SPACE_8` (`8.0px`)**.
  3. **App Name**: **`T1` `16.0pt` `Segoe UI` Regular** in primary `text` (`#EDEDF2` Dark / `#1C2733` Light), optically centred on the row centreline.
  4. **Name-to-Caption Gap**: **`SPACE_8` (`8.0px`)**.
  5. **Muted Inline Caption**: **`T4` `11.5pt` `Segoe UI` Regular** in `muted` (`#9A9AB0` Dark / `#5F7186` Light; semantic status colour permitted only during active transient operations), optically centred cleanly and single-line truncated before the right-hand action control cluster.
- **Settings / About Surface**: Any Settings or About surface repeats the same `24px` WT logo mark + `16.0pt` app name + `11.5pt` muted descriptor lockup.

---

## 7. Chip Discipline

1. **Pill Badges Exist Only for Decisions or State**: Non-interactive semantic badges render as full **pill shapes** (`corner_radius = 0.5 * height`, e.g., `9.0px` on `18px` pills; `10.5pt` Regular) strictly when encoding an actionable constraint, safety boundary, verification confidence, qualifier, or status mark (`§8`). Never render large heavy dark solid badge fills in Light mode — both Dark and Light modes use outline (`1px`) + soft tinted backdrop + high-contrast ink (`#E2F4FC` / `#0090D9` / `#0090D9` for Light blue `proven` pills; `#253141` / `#69CBFC` / `#AFE3FD` for Dark; `#FCECF6` / `#FA29AA` / `#FA29AA` for Light pink qualifiers; `#382C3C` / `#FA80CB` / `#FBBBE3` for Dark).
2. **Forbidden Static Chips**: Never wrap persistent modes (`READ-ONLY`), hardware/OS classifications (`SYSTEM DRIVE`), prose category names (`Category` column text, `shared runtime` in tables), internal spec terms (`Constitutional`), or quiet steady-state labels in bordered badge chips.
3. **Interactive Filter Buttons**: Single-select filter controls use `RADIUS_PILL = 4.0px` (`pill_blue_fill` + `1px accent_line` + `text` when active; `widget` + `1px border` + `muted` when inactive) so interactive controls remain visually distinct from non-interactive pill badges.

---

## 8. Status-Colour Saturation Rule

Across every WT instrument, **saturated green/red (`STATUS_GREEN` / `STATUS_RED`) is reserved exclusively for real semantic status**, while structural accents and metadata stay at calm pastel or neutral contrast levels.

| Category | Element Examples | Palette Treatment |
|---|---|---|
| **Takes Saturated Status** | • `8px` (`r = 4.0`) health, device-link, and reconciliation status dots (`diff 0 B`) + paired status text<br>• Critical/threshold-driven capacity & battery meters (`z-controller` battery `> 20%` `STATUS_GREEN` / `<= 20%` `STATUS_RED`; `y-storage` drive bar `>= 85%` full / `<= 15%` free `STATUS_RED`)<br>• Signed change deltas (`>= 1 MB`: `-` shrank/favorable `status_green`, `+` grew/unfavorable `status_red`; deltas `< 1 MB` stay blank with exact bytes on hover)<br>• New baseline state pill (`new`) using `status_green` `1px` outline + text over `pill_green_fill`<br>• Actual errors, safety locks, alerts, and missing-resource badges (`NO-TOUCH`, `LOCKED`, `missing`, `diff != 0`) using `status_red` outline/text over `pill_red_fill` (NEVER stale warnings or `Pin: Ignore`)<br>• Muted warning state (`status_amber` text/stroke over `pill_amber_fill`, e.g., stale scan scope)<br>• Explicit safety-zone badges (`● GREEN`, `● RED`) featuring a `5px` saturated dot over `pill_green_fill` / `pill_red_fill` | **`STATUS_GREEN`** (`#28D66C` Dark / `#0A7A3C` Light text & stroke; `#28D66C` solid dot; fill `#102E20` / `#E4F9EC`)<br>**`STATUS_RED`** (`#F04458` Dark / `#E02444` Light text & stroke; `#F04458` solid dot & bar; fill `#2C121E` / `#FDE8ED`)<br>**`status_amber`** (`#F5B83D` Dark / `#A66C07` Light warning copy; fill `#382E25` / `#FDF0DA`) |
| **Stays Pastel or Quiet Neutral** | • Healthy drive allocation meters (`WT_RIM_GRADIENT_STOPS` 6-stop sweep)<br>• Active container gradient rims (`#54B9F2 → ... → #FFC7E3`) and pressed `accent_2` (`#FA80CB` Dark / `#FA29AA` Light) borders<br>• Provenance & qualifier badges (`proven` in `pill_blue_fill` + `accent_line` + `proven_text`; location qualifiers in `pill_pink_fill` + `accent_2` + `pill_pink_text`)<br>• Dependency kind chips (`service or driver`, `anti-cheat`) and structural/decision badges (`BODY`, `shared`, `inferred`, all `Manual pin` kinds `Pin: App`/`Group`/`Personal`/`Ignore`) in `TRANSPARENT`/`border`/`muted`<br>• Inline `<= 2px` proportional table bars, hairline dividers, and `T4` metadata | **Pastel (`accent_1`, `accent_2`)** or **Quiet Neutral (`border`, `muted`, `subtle_text`)** |

---

## 9. Shared Layout & Family Critique Checklist (`FC1`–`FC9`)

Tabular instruments align numbers in fixed right-aligned monospace columns under a sticky `panel_alt` sortable column-header row (`T4` `11.0pt` labels acting directly as sort triggers with a single directional arrow, never duplicated in a separate bar).

Before accepting screenshots of any WT application, verify **`FC1`–`FC9`**:
- **FC1 (No Card Soup)**: List rows and summary lines sit inside hairline-divided ledger panels rather than repeating stacked individual cards.
- **FC2 (Restrained Numeral Weight & Clear Row Hierarchy)**: Right-aligned `T2` totals lead cleanly by scale and luminance (`13.0pt`–`13.5pt` Regular/Medium, never heavy Bold slabs) alongside `T3` `Segoe UI Semibold` app names and quiet `T4` metadata.
- **FC3 (State-Signal Gradient Rim Only)**: Diagonal gradient rims (`#54B9F2 -> #7CD0FA -> #A5E6FF -> #D8A6FF -> #F7A8D8 -> #FFC7E3`) appear strictly on active/expanded containers; zero collapsed containers or non-status pills use solid pastel fills.
- **FC4 (Multi-Tier Corner Radii)**: Visible separation across `8px` panels/rims, `6px` buttons/inputs, `4px` filter pills/checkboxes, `3.5px–4px` chunky meter tracks, full-pill (`0.5 * h`) status badges, and `0px` hairline ledger rows.
- **FC5 (Saturated Status Discipline)**: Saturated `#28D66C` / `#F04458` (`#0A7A3C` / `#E02444` Light) marks appear strictly on live status dots, threshold/battery alerts, signed deltas, safety locks, and zone marks — while metadata, proportions, dependency kind chips, and qualifiers stay calm pastel (`accent_1`/`accent_2`) or quiet neutral.
- **FC6 (Outline Button Hierarchy & Scope-Split Accents)**: Global `Rescan` (`PrimaryOutline`) uses `accent_1` blue (`#69CBFC` / `#0090D9` + `pill_blue_fill`); repeated per-pod `Rescan` (`PodPrimaryOutline`) uses `accent_2` pink (`#FA80CB` on `#382C3C` Dark / `#FA29AA` on `#FCECF6` Light); buttons never carry gradient borders; `ThemeToggle` & `Settings` (`SecondaryOutline`) render neutral `border` + `text` at rest with smooth anti-aliased `1px` perimeter rings and clean moon/sun contrast; row-level `SubtleOutline` actions (`Locate`, `Web search`, `used by N apps >`, `Exclusions`) stay borderless and `muted` at rest (`widget_hover` + `accent_line` text on hover).
- **FC7 (`>= 10.5pt` Floor)**: All visible labels, badges, captions, and tooltips meet or exceed `10.5pt`.
- **FC8 (Dual-Mode Surface & Input Separation)**: Both Dark (`#0A0A10` / `#10101A` / `#14141F`) and Light (`#EEF4FA` / `#F4F8FC` / `#FFFFFF`) preserve distinct surface tiers and recessed `input_bg` wells.
- **FC9 (Content-Hugging & Top-Left Sparse States)**: Empty or sparse panels align top-left cleanly without floating centred whitespace or orphaned headers.
- **FC10 (WT Brand Lockup & Unmodified Mark)**: Main header, Settings/About surface, and OS window chrome render the original transparent WT mark (`24px` in-app lockup + `SPACE_8` + `16.0pt` app name + `SPACE_8` + `11.5pt` muted caption) with crisp alpha anti-aliasing in both Dark and Light modes and zero added tile or border.

---

# Part II — App-Specific Sections

Each section below documents only what is unique to an individual WT application beyond Part I.

## 10. App-Specific: `z-controller`

- **Active Controller Cards**: The active virtual controller card renders the `8.0px` Active Gradient Rim (`§5`); inactive/standby cards use standard `panel` surfaces with `1px border`.
- **Battery Instrument Meter**: `124.0 x 8.0 px` single-colour chunky rounded track (`Rounding 4.0` `bg` + `1px border` outer, `Rounding 3.0` inner fill) using `STATUS_GREEN` (`#28D66C`) above `20%` and `STATUS_RED` (`#F04458`) at or below `20%` (never the 3-stop gradient).
- **Live Device Status**: Paired with `8px` (`r = 4.0`) saturated `STATUS_GREEN` / `STATUS_RED` connection indicator dots.

---

## 11. App-Specific: `y-storage`

- **Masthead & Settings Lockup**: Top masthead band (`Main`, `Settings`, `Dependencies`) renders `[24x24 WT mark] + 8px + "y-storage" (16.0pt text) + 8px + status caption` (`11.5pt muted` for `"Read-only · Last scan: <timestamp> · <duration>s"`, `accent_line` during active scans, or `status_amber` `"Read-only · Scope changed — rescan needed"` on stale scope with last-scan timestamp preserved on hover), paired with right-aligned `[ThemeToggle] [Rescan / Rescan needed] [Settings / Back to Apps]`. `Settings` houses naturally stacked, non-occluding sections (`Scan Scope & Exclusions` per-drive scope cards, unified **Attribution Zone Rulebook** grouping chips under `"App container"` / `"Personal files"` subheads with neutral muted `Manual pins`, and the `8px` `panel` About card repeating the `24px` WT lockup).
- **Adaptive Drive Instrument Header & In-Place Scope**: Physical fixed drives sit inside `CentralPanel` above the search/filter bar in a responsive `8px` `panel` pod grid (`1..=4` drives in 1 row, `5..=8` in 2 balanced rows, `9+` in a 5-column wrap; `1px border`, `Margin::symmetric(SPACE_12, SPACE_8)`, `SPACE_4`/`SPACE_8` multiples-of-4 gaps). Excluded drives are never walked or sized at all (including registered `InstallLocation` folders), and apps whose folders all live on excluded drives are omitted from the report; the Settings scope section and pod `Include` tooltip explain drive exclusion, while the `Exclusions` popover explains folder-level exclusion. Each pod uses consistent production copy across all drive counts (`1..10`):
  1. `"C:\"` (`13.0pt` `Segoe UI Semibold` `text`) + right-aligned `"{used} of {total}"` (`11.0pt` / `10.5pt` compact `Consolas Regular`);
  2. `"Scanned HH:MM"` (or `"Not scanned"` when excluded / `"Scanning…"` during active per-drive rescan) + right-aligned `"{free} free"`;
  3. Flush `2.0px` proportional bar in the 6-stop WT logo gradient when included (`lerp(pod_fill, muted, 0.35)` greyed bar + dimmed surface/border when excluded; `STATUS_RED` only if drive read fails/`0` capacity);
  4. Compact in-place control strip (`24px`, single `"Hx"` optical baseline across `"Include"` and `"Exclusions"` sharing one `11.0pt` / compact `10.5pt` font size): quiet `"Include"` checkbox + quiet `"Exclusions"` / `"Exclusions (N)"` (`SubtleOutline`) overlay trigger + right-aligned per-drive `"Rescan"` (`PodPrimaryOutline`: `accent_2` pink stroke + `accent_2` label over `pill_pink_fill`, `24px` min height — scope-split local CTA per `§6`). Scope or zone-rulebook edits mark the cached report stale (`Rescan needed`) cleanly without auto-triggering a surprise walk.
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
  - `Total size` / `Size on X:` embeds a quiet `<= 2px` (`PROP_BAR_W = 76.0`, `PROP_BAR_H = 2.0`) proportional bar relative to the largest app in the active mode (`max_bar_bytes`), renders a smaller muted `10.5pt` `"of <global>"` companion **after** the per-drive total inside the size cell whenever an app spans multiple drives (never in the Growth delta column), and updates the filter summary line to `"N apps · <sum> on X:"`. Missing or first-seen drive baselines display the `"new"` green status pill (`status_green` border + text over `pill_green_fill`); numeric deltas `< 1 MB` (`1,048,576 B`) render blank with exact signed bytes on hover; signed `>= 1 MB` deltas render as `+` red / `−` green monospace values.
  - Expanding an app row converts it into an `8px` `panel_alt` inspection drawer wrapped in the continuous **WT Active Gradient Rim** (`§5`), suppressing the row hairline above it, showing its `Category` + rule basis (plus a quiet neutral `inferred` pill when all bases are inferred, a quiet `SubtleOutline` `used by N apps >` trigger when referenced as a dependency, and word-capped `"contains <Child> — counted under <Child>"` carve-out note lines), and aligning two sub-tables cleanly to the main size column's right edge with word/segment-boundary `"…"` truncation and full-text hover throughout:
    - `Locations (N)`: `Zone · Path · Basis · Locate / Web search · Size` (`● GREEN` / `● RED` full-pill zone badges, `BODY` quiet structural pill, `ProgramData · shared` / `loose files in the install folder` / `loose files` pink qualifier pills using `pill_pink_fill`/`accent_2`/`pill_pink_text`, word/segment-boundary `"…"` paths & basis with full hover tooltips, full-row `widget_hover`, plus `SubtleOutline` `Locate` & `Web search` actions with `"no longer on disk"` disabled guard and natural-question hover preview).
    - `Dependencies`: `Dependency name · Kind · Confidence · Used by · Own size` (`service or driver` vs `anti-cheat` quiet neutral kind chips, `proven` / `inferred` / `shared` / `missing` pills, quiet borderless `SubtleOutline` `used by N apps >` overlay triggers, word/segment-boundary `"…"` name, kind, & basis with hover tooltips).
- **Collapsible & Full-Height Resizable Reconciliation Ledger Footer**: Pinned storage groups (`Windows & system`, `Your files` with `"Shell folders & Personal-files zones · read-only"` subtitle, `Shared & system caches`, `Unattributed`) sit in one `8px` closed `1px`-bordered `panel` scroll container whose height strictly reserves space below it for the per-drive reconciliation block at every window size (`760x520` minimum up) and any ledger resize height, with top `6px` drag-resize handle (`max(min_ledger_h, client_height - masthead_height - 4px)`), a `"Pinned storage ledger"` collapse chevron folding to a single inline summary line (`Win/sys · Your files · Shared caches · Unattributed` + `● diff 0 B`), and semantic `2px` colour ticks (`muted`, `muted`, `STATUS_GREEN`, muted neutral). Beneath the closed frame sits the live per-drive reconciliation equation (accounting for NTFS `hardlink deduplication offset` where applicable) ending with an `8px` (`r = 4.0`) saturated `STATUS_GREEN` dot + green `diff 0 B` text (or `STATUS_RED` alert pill if `diff != 0`), while the main app list paints a soft `22px` bottom gradient fade above the footer when scrollable content overflows. Expanded folder rows in **`Your files`**, **`Shared & system caches`**, and **`Unattributed`** share `Locate` + `Web search` (`SubtleOutline`, natural question hover query + `"no longer on disk"` guard) and paint full-row `widget_hover` across the row including action cells (`Windows & system` omits folder actions because its rows include synthetic remainders, hardlink offsets, and locked OS paths). Expanding **Unattributed** additionally provides a compact view-only toolbar with text search, `Clear`, 4 sort chips (`Size ↓ / ↑`, `Name A–Z`, `Path A–Z`), three `SecondaryOutline` facet dropdowns (`Drive`, `Size`, `Zone` covering `AppData Local`, `AppData Roaming`, `AppData LocalLow`, `ProgramData`, `Program Files`, `User profile root`, `Other drive location`), `Reset filters`, and a live `"N of M folders · <subtotal>"` counter (preserving `Unattributed` group totals unchanged; the other three single-purpose ledger groups intentionally omit facet bars).
- **`y-storage` Domain Critique Checks (`YS1`–`YS3`)**:
  - **YS1 (Shared Size Column Alignment)**: Sticky header `Total size`, main row totals, expanded `Locations`/`Dependencies` size columns, and pinned summary totals share one exact right vertical edge (accounting for the `14px` `6px + SPACE_8` scrollbar gutter).
  - **YS2 (Calm Drive Pod Hierarchy)**: Each drive pod gives `"C:\"` (`13.0pt` Strong) and `"{used} of {total}"` (`11.0pt Consolas Regular`) quiet structural primacy over timestamps, `"{free} free"`, flush `2.0px` bar, and the quiet `Include` / `Exclusions` / `Rescan` control strip.
  - **YS3 (Subordinate Shared Dependencies)**: Inside expanded drawers, `Dependencies` own sizes (`11.5pt` `muted`) visually recede behind the app's own total (`13.0pt` `text`) so shared runtimes never read as additive footprint.
