# Implementation Plan: `ui-polish-1`

- **Data & Persistence (`crates/y-core/src/config.rs`, `apps/y-storage/src/win32_host.rs`)**:
  - Add `WindowPlacementConfig` (`x`, `y`, `width`, `height`, `maximized`) to `AppConfig`.
  - Restore saved window bounds after validating against virtual screen metrics; save placement on `WM_EXITSIZEMOVE` and `WM_CLOSE`.
  - Bind multi-size icons via `WM_SETICON` (`ICON_BIG`, `ICON_SMALL`) at window creation.
- **Full-Bleed Icon (`apps/y-storage/build.rs`)**:
  - Update procedural SDF icon artwork to `hx = 124.0, hy = 124.0` (`248 / 256 ≈ 96.9%` of tile width) across sizes `[16, 20, 24, 32, 40, 48, 64, 128, 256]`, overwriting `assets/y-storage.ico` whenever bytes change.
- **UI Overhaul (`apps/y-storage/src/ui.rs`, `apps/y-storage/src/main.rs`)**:
  - Multiples-of-4 layout system with fixed-height row allocations (`24/28/32px`), aligned `SIZE_COL_W = 88px` numeric column, universal `draw_truncated_text` hover tooltips, unified chip renderer, non-crowding `Area`-based reverse-dependency overlay, borderless header with theme toggle immediately left of `Rescan`, and prominent free-space drive tiles.
- **Light Verification**:
  - Existing `cargo test --workspace` (`18` tests), `cargo build --release --workspace`, one live CLI scan regression check, and visual inspection of all 5 screenshots (`main-dark`, `main-light`, `expanded-fifa23`, `reverse-ea-app`, `settings`).
