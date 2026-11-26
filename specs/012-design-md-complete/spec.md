# Feature Specification: `design-md-complete`

**Feature Branch**: `main` (documentation-only add-on on top of `e8fe9af`)
**Created**: 2026-11-26
**Status**: Active
**Input**: ADD-ON 2 to `visual-identity-addon` (`slug: design-md-complete`): complete `DESIGN.md` as the single canonical design standard for the entire WT ecosystem (`z-controller`, `y-storage`, `WT Manager`, and future WT apps), documentation only with zero code changes.

## Requirements

1. **Canonical Governance Header**: Add a top header to `DESIGN.md` stating it is the canonical design document for all WT apps (`z-controller`, `y-storage`, `WT Manager`, and future apps), that `z-controller` (`Y:\Repos\z-controller`) is the reference implementation for shared controls, and that any app adopting it copies `DESIGN.md` and implements the Shared Core unchanged, placing any deviations solely inside its own app-specific section.
2. **Outline-Button Standard in Shared Core (`ADD 2`)**: Move the full `ButtonVariant` outline-button specification (`PrimaryOutline`, `SecondaryOutline`, `SubtleOutline`, `Hovered`, `Pressed`, `Active-Highlight`, `Disabled`, `ThemeToggle`, crisp `1px` `paint_crisp_ring_mesh` transparent geometry, padding, radius) into the Shared Core, citing `z-controller` as reference implementation.
3. **Status-Saturation Rule in Shared Core (`ADD 3`)**: Document in the Shared Core which UI elements take saturated status colours (`STATUS_GREEN` / `STATUS_RED` and soft tints) vs which remain pastel or quiet neutral, reflecting the exact implemented rules across `z-controller` and `y-storage`.
4. **Complete Shared Core vs App-Specific Split**: Ensure Part I (Shared Core) covers in one place: North Star & Anti-Patterns, Colour Tokens (Dark & Light), Typography Scale, Spacing Scale, Corner Radii, Gradient-Rim State Rule, Family Button & Interactive Control Standard, Chip Discipline, Status-Colour Saturation Rule, and Family Layout/Critique Checklist; ensure Part II holds only genuinely app-specific patterns (`z-controller` and `y-storage`).
5. **Documentation-Only Verification & Commit**: Verify zero `.rs`, `Cargo.*`, or binary edits; list `DESIGN.md` headings as evidence; commit cleanly on `main`.
