# Critique Round 1 — Pre-Fix Baseline (`design-pass-1`)

Evaluated strictly against [DESIGN.md](file:///Y:/Repos/y-storage/DESIGN.md) checklist items `C1`–`C12` across:
- [before-main-dark.png](file:///Y:/Repos/y-storage/specs/008-design-pass-1/before-main-dark.png)
- [before-main-light.png](file:///Y:/Repos/y-storage/specs/008-design-pass-1/before-main-light.png)
- [before-expanded-fifa23.png](file:///Y:/Repos/y-storage/specs/008-design-pass-1/before-expanded-fifa23.png)
- [before-settings.png](file:///Y:/Repos/y-storage/specs/008-design-pass-1/before-settings.png)

## Checklist Evaluation (Round 1)
- **C1 (No Uniform Card Soup)**: FAIL (`F1`)
- **C2 (One Dominant Row Number)**: FAIL (`F3`)
- **C3 (Shared Size Column Alignment)**: FAIL (`F5`)
- **C4 (Headline Drive Strip)**: FAIL (`F3`)
- **C5 (Quiet Shared Dependency Sizes)**: FAIL (`F3`)
- **C6 (Chip Discipline)**: FAIL (`F2`)
- **C7 (No Solid Pastel Fills)**: FAIL (`F2`)
- **C8 (Multi-Tier Corner Radii)**: FAIL (`F4`)
- **C9 (`>= 10.5pt` Type Floor)**: FAIL (`F5`)
- **C10 (Dark & Light Input Contrast)**: FAIL (`F6`)
- **C11 (Content-Hugging Settings & Alignment)**: FAIL (`F7`)
- **C12 (`z-controller` Control Sync Intact)**: PASS (`ui-polish-2` button/scrollbar sync intact)

## Findings List

- **F1 [P0 — Uniform equal-weight card soup & nested sub-cards]** (`C1`, `before-main-dark`, `before-main-light`, `before-expanded-fifa23`): Every collapsed app row (`9` visible rows), all `4` pinned storage group rows, and all `8` sub-rows inside expanded `FIFA 23` are rendered as identical bordered cards/sub-boxes of equal weight.
- **F2 [P0 — Decorative chips & solid pastel badge fill]** (`C6`, `C7`, all `4` views):
  - `[READ-ONLY]` is an app-wide static pill in the header (`READ-ONLY` belongs in quiet metadata text).
  - `[SYSTEM DRIVE]` in Settings duplicates static drive identity; `[LOCKED · Constitutional]` leaks spec jargon in lavender instead of a concise `[LOCKED]` outline badge.
  - Every dependency row stacks 3–4 chips (`[shared runtime]` lavender pill + solid pastel `#54B9F2` `[proven]` badge with dark text + `[shared]` + glowing `[used by N apps >]` pill), violating both Chip Discipline and the No-Solid-Pastel-Fill rule.
  - `[diff 0 B]` in the reconciliation footer wears a full bright-green bordered pill during normal steady state rather than reading as a quiet ledger line.
- **F3 [P0 — Flat type hierarchy across Drive Strip, App Rows, and Dependencies]** (`C2`, `C4`, `C5`, all `4` views):
  - Drive free space (`94.61 GB free`, `12.5pt`) is smaller than app row labels (`13.0pt`) instead of serving as the `16.0pt` headline number of the instrument.
  - Collapsed app rows render `display_name` and `total_bytes` at identical `13.0pt` Bold `pal.text`, so there is no single dominant quantity (`total_bytes` should be `13.5pt` Strong while `display_name` steps down to `13.0pt` Regular and publisher steps down to `11.0pt` `subtle_text`).
  - Expanded dependency own sizes (`594.97 MB`, `3.03 GB`, `1.34 GB`, `41.97 MB`) render in `12.0pt` Bold `pal.text`, competing visually with the app's own footprint (`47.84 GB`).
- **F4 [P1 — Single-radius `6px` uniformity]** (`C8`, all `4` views): Drive pods, app cards, pinned cards, Settings cards, buttons, and inputs all share `6px` corner radius, and all chips use `4px`–`5px`. Must differentiate: `8px` panels / `6px` shared controls / `4px` filter pills / `2px` state badges & meter tracks / `0px` hairline ledger rows.
- **F5 [P1 — Right size-column misalignment & sub-floor `10.0pt` label]** (`C3`, `C9`, `before-main-dark`, `before-main-light`): `pinned_groups_panel` lacks the `14px` (`6px` bar + `8px` gutter) right offset of `ScrollArea`, shifting pinned totals (`42.93 GB`, `2.99 GB`, `11.01 GB`, `1.38 GB`) out of vertical alignment with `47.84 GB`; `"PINNED STORAGE GROUPS"` uses `10.0pt` (below the `10.5pt` floor).
- **F6 [P1 — Light-mode text input merges with `#EEF4FA` canvas]** (`C10`, `before-main-light`): `draw_crisp_text_input` uses `pal.bg` (`#EEF4FA`) inside the `#EEF4FA` Light header, giving zero fill differentiation vs surrounding background instead of crisp `#FFFFFF` `input_bg`.
- **F7 [P1 — Settings orphan `"Exclusions:"` header & centred empty-state alignment]** (`C11`, `before-settings`): Drives `D:\`, `Y:\`, and `Z:\` have `0` exclusions yet allocate a standalone `"Exclusions:"` label line above their input row; empty list states use centred text floating in empty space.
- **F8 [P2 — `BODY` zone badge competes visually with `GREEN`/`RED` action zones]** (`C6`, `before-expanded-fifa23`): Using full tinted blue fill on `[BODY]` puts visual weight equal to `GREEN`/`RED`; demoting `BODY` to a quiet border-only `2px` outline focuses colour signal on `GREEN` caches and `RED` user data.
