# Implementation Plan: `ui-polish-2`

## Modified Files (UI & Screenshot Harness Only)
- `.specify/memory/constitution.md`: register canonical slug `ui-polish-2`.
- `apps/y-storage/src/ui.rs`:
  1. Sync `ThemePalette` & `apply_theme_visuals` with `z-controller` (`bg`, `panel`, `panel_alt`, `border`, `text`, `muted`, `hint_text`, `widget`, `widget_hover`, `widget_active`, `accent_line`, `pill_blue_fill`, `scrollbar_thumb`, `scrollbar_thumb_hover`, `override_text_color = None`).
  2. Implement physical-pixel-snapped concentric filled rounded-rect painters (`snap_rect_to_pixels`, `paint_crisp_bordered_box`, `draw_crisp_frame`) so 1px rounded borders are solid, continuous, and uniform at 96 DPI and high DPI.
  3. Implement `draw_z_button` mirroring `z-controller` neutral/hover/pressed/disabled states and sync `draw_theme_toggle_button` (`Rounding::same(6.0)`, `1px` crisp border, sun/crescent vectors). Route every button (`Rescan`, `Settings`/`Back to Apps`, `Clear`, `Add exclusion`, `Remove`, `All`, `Close`, `used by N apps >`) and chip/card through crisp primitives.
  4. Expand `draw_split_row` clip rectangles vertically by `1.0` px so child 1px borders are never clipped.
  5. Implement `draw_crisp_text_input` and `draw_crisp_checkbox` with true muted hint rendering hidden on focus or non-empty content.
  6. Replace drive-strip progress bars with full-inner-width `6.0` px bordered proportional bars (`fill_px = round(inner_px * clamp(used / total, 0.0, 1.0))`).
  7. Update `format_delta_display` to return `Option<_>`, leaving zero deltas completely blank while keeping right-aligned column alignment.
  8. Wrap vertical `ScrollArea`s with `draw_synced_scroll_area` (`6.0` px bar width, `SPACE_8` inner gutter, `floating = false`, `foreground_color = false`, `pal.scrollbar_thumb` colors).
- `apps/y-storage/src/main.rs`:
  - Add `--view settings-typed` arm populating `new_exclusion_inputs.insert('C', r"C:\TempBuildCache".to_string())` for screenshot proof of Fix 2.

## Verification Plan
1. `cargo test --workspace` (expect `18` passed, `0` warnings).
2. `cargo build --release --workspace`.
3. Live scan via `y-storage.exe --scan-cli <scratch.json> --ephemeral` (or live scan) checking `FIFA 23` size and `C:\` reconciliation diff = `0 B`.
4. Render the 4 headless screenshots at `1020x720` (`--dpi 96`), generate 4x nearest-neighbor zoomed crops of the header buttons in Dark & Light modes, measure drive-bar pixel fills directly from `screenshot-main-dark.png`, and inspect all 6 PNGs visually with `read_file`.
5. Record results in `specs/006-ui-polish-2/verification.md`, copy release exe to `Y:\Apps\y-storage\y-storage.exe`, and commit cleanly to `main`.
