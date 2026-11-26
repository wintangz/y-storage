# Implementation Plan: `button-discipline-1`

1. Register `button-discipline-1` in `.specify/memory/constitution.md` and create `specs/017-button-discipline-1/{spec,plan,tasks}.md`.
2. Update `apps/y-storage/src/ui.rs`:
   - Skip `paint_crisp_ring_mesh` in `paint_crisp_bordered_box` when `border_color == Color32::TRANSPARENT` and paint `fill` cleanly over `snapped` when opaque.
   - Extract `resolve_button_colors` and update `draw_z_button` to enforce the 3-tier rest/hover/active/disabled mapping across both Dark and Light palettes.
   - Update `draw_theme_toggle_button` (`SecondaryOutline`) and `draw_filter_radio_chip` hover contrast (`pal.accent_line` on hover).
   - Reassign `used by N apps >` (companion & Dependencies rows), `Locate`, `Web search` -> `SubtleOutline`, and overlay `All` / Settings `Add exclusion` -> `SecondaryOutline`.
   - Keep `draw_reverse_group_header` chevron neutral (`pal.muted`) when collapsed and `pal.accent_line` when expanded.
   - Add unit test `button_hierarchy_reserves_accent_for_primary_active_and_hover`.
3. Update `apps/y-storage/src/main.rs` with `--view unattributed-hover-btn` so the Unattributed hovered button state renders without tooltip occlusion over adjacent rows.
4. Update `DESIGN.md` (`Part I §6`, `§2`, `§4.2`, `§9 FC6`, `Part II §11`).
5. Run `cargo test --workspace`, `cargo build --release --workspace`, one live `--scan-cli --ephemeral` check, render and inspect all 4 default-size screenshots, write `verification.md`, deploy to `Y:\Apps\y-storage\y-storage.exe`, and commit on `main`.
