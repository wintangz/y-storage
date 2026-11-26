# Tasks: `ui-polish-2`

- [x] Register `ui-polish-2` in `.specify/memory/constitution.md` and author `spec.md`, `plan.md`, `tasks.md`.
- [x] Sync `ThemePalette` and `apply_theme_visuals` in `apps/y-storage/src/ui.rs` with `z-controller` and remove `override_text_color`.
- [x] Implement pixel-snapped crisp border helpers (`paint_crisp_bordered_box`, `draw_crisp_frame`), `draw_z_button`, synced `draw_theme_toggle_button`, `paint_chip`, and expanded `draw_split_row` clips (FIX 1).
- [x] Implement `draw_crisp_text_input` & `draw_crisp_checkbox` with true muted hint behaviour, and add `--view settings-typed` in `apps/y-storage/src/main.rs` (FIX 2).
- [x] Update drive strip bars to full-width bordered proportional tracks using `fill_px = round(inner_px * (used_bytes / total_bytes))` (FIX 3).
- [x] Update `format_delta_display` and `draw_main_app_list` to leave `0` deltas completely empty (FIX 4).
- [x] Implement `draw_synced_scroll_area` with slim palette-coloured non-overlapping scrollbar across all scroll views (FIX 5).
- [x] Run `cargo test --workspace`, `cargo build --release --workspace`, live scan verification, capture & visually inspect all 6 PNG screenshots + pixel-measured drive bar table, deploy to `Y:\Apps\y-storage\y-storage.exe`, write `verification.md`, and commit to `main`.
