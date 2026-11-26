# Specification: `design-pass-1`

**Canonical Slug**: `design-pass-1`
**Type**: Visual Design Pass (Impeccable Method: `DESIGN.md` -> Critique Round 1 -> Visual Fixes -> Critique Round 2)
**Scope**: Visuals only — zero changes to scan, attribution, dependency, history, sort, or filter logic, and zero computed byte totals changed.

## Method
1. Write `DESIGN.md` at the repo root as a shared WT family document (North Star, pastel/surface/type/spacing/radius tokens, hierarchy rules, chip discipline, anti-pattern list, `y-storage` signature elements, and the `C1`–`C12` screenshot critique checklist).
2. Run **Critique Round 1** directly against the pre-fix baseline screenshots (`before-main-dark.png`, `before-main-light.png`, `before-expanded-fifa23.png`, `before-settings.png`), logging severities `P0` / `P1` / `P2`.
3. Fix every `P0` and `P1` finding (and free `P2`s) in `apps/y-storage/src/ui.rs` without breaking `ui-polish-2` control parity with `z-controller`.
4. Capture post-fix `after-*.png` screenshots across all four views and run **Critique Round 2** proving all `P0` / `P1` findings closed.

## Logged Assumptions
- Shared interactive control parity (`z-controller` 1px crisp borders, `6px` button/input corners, sun/crescent icon toggle, slim `6px` palette scrollbar) from `ui-polish-2` is non-negotiable and preserved identically.
- No new unit tests are needed because logic (`y-core`, `y-platform-windows`, `list_query`) is completely untouched; existing `23` workspace tests + `cargo build --release` + live CLI scan + visual before/after inspection satisfy the pass.
