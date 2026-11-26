# Implementation Plan: `visual-identity-addon`

1. Update `.specify/memory/constitution.md`, `DESIGN.md`, and `specs/011-visual-identity-addon/{spec,plan,tasks}.md`.
2. In `apps/y-storage/src/ui.rs`:
   - Add `ThemePalette` saturated status fields (`#28D66C`/`#0FA84F`, `#F04458`/`#E02444`, tinted status fills).
   - Add `paint_crisp_ring_mesh` so `paint_crisp_bordered_box` renders unbroken `1px` outlines at 96 DPI even when `fill == Color32::TRANSPARENT`.
   - Implement `ButtonVariant::{PrimaryOutline, SecondaryOutline, SubtleOutline}` across `draw_z_button` and `draw_theme_toggle_button`, updating all action buttons.
   - Port `push_rounded_rect_perimeter`, `build_rounded_rect_polygon`, and `paint_active_card_gradient_border` from `z-controller`, replacing the flat bar/rules on expanded app drawers in `draw_main_app_list`.
   - Upgrade drive meters to chunky `7px` rounded tracks with `<= 15%` free headroom alert, upgrade `diff 0 B` to an `8px` `STATUS_GREEN` dot + green text, and add `5px` saturated status dots inside `GREEN`/`RED` zone badges.
3. In `apps/y-storage/src/main.rs`: add `--view header-buttons-zoom` (`3x` nearest-neighbor crop of top-right header controls `[756..1020, 0..52]`).
4. Run `cargo test --workspace`, `cargo build --release --workspace`, one `--scan-cli ... --ephemeral` verification, and render + inspect all 6 screenshots (`main-dark`, `main-light`, `expanded-fifa23-dark`, `expanded-fifa23-light`, `header-buttons-zoom-dark`, `header-buttons-zoom-light`).
5. Write `specs/011-visual-identity-addon/verification.md`, mark all tasks done, deploy to `Y:\Apps\y-storage\y-storage.exe`, and commit cleanly on `main`.
