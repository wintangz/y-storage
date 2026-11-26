# Implementation Plan: `design-md-complete`

- **Slug**: `design-md-complete`
- **Mode**: Documentation-only (no Rust code, tests, binaries, or screenshots modified).
- **Primary Artifact**: [DESIGN.md](file:///Y:/Repos/y-storage/DESIGN.md)
- **Reference Sources**:
  - [Y:\Repos\y-storage\apps\y-storage\src\ui.rs](file:///Y:/Repos/y-storage/apps/y-storage/src/ui.rs) (implemented tokens, outline button variants, transparent crisp ring mesh, gradient rim, status saturation)
  - [Y:\Repos\z-controller\apps\z-controller\src\ui.rs](file:///Y:/Repos/z-controller/apps/z-controller/src/ui.rs) (reference WT palette, gradient rim, buttons, battery bar, status dots)

## Execution Phases

1. Update `.specify/memory/constitution.md` and author `specs/012-design-md-complete/{spec,plan,tasks}.md`.
2. Rewrite `DESIGN.md` with:
   - Canonical Governance Header + Adoption Rule
   - Part I — Shared Core (`§1`–`§9`) consolidating all 8 shared family pillars
   - Part II — App-Specific Sections (`§10 z-controller`, `§11 y-storage`)
3. Write `specs/012-design-md-complete/verification.md`, check `git status -s` / `git diff --name-only` (docs only), extract the exact `DESIGN.md` heading outline, and commit on `main`.
