# Implementation Plan: `design-pass-1`

## Artifacts & Code Targets
- Repo root `DESIGN.md`: WT family design system + `y-storage` signature elements + `C1`–`C12` screenshot critique checklist.
- `.specify/memory/constitution.md`: register `design-pass-1`.
- `specs/008-design-pass-1/`: `before-*.png`, `critique-round-1.md`, `after-*.png`, `critique-round-2.md`, `verification.md`.
- `apps/y-storage/src/ui.rs`: implement visual/typographic/chip/surface fixes closing `F1`–`F8` while keeping `z-controller` shared control sync (`RADIUS_CONTROL = 6.0`, crisp `1px` borders, `draw_synced_scroll_area`).
