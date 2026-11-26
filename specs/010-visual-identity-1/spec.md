# Feature Specification: `visual-identity-1`

**Scope**: Visuals + icon metadata ONLY (`ui.rs`, `DESIGN.md`, Win32 icon extraction cached once per scan, additive `#[serde(default)]` icon fields in `y-core`). Zero changes to scan/walk, attribution, dependency detection, history, sort/filter semantics, or computed byte totals.

## Fixes

1. **FIX 1 — Colour Sync with `z-controller` + Token Table**:
   - Audit every colour in `Y:\Repos\z-controller` (`ui.rs`, `diagram.rs`, `tray_win32.rs`) vs `y-storage` across both Dark and Light modes.
   - Sync `y-storage`'s `ThemePalette`, `apply_theme_visuals`, `pill_red_fill` (`pill_pink_fill`), `input_bg` (on `panel` vs `bg` canvas), neutral violet-grey `subtle_text`/`hint_text` axis (`#82829A`/`#64647A`), muted `pal.text` foreground on `pill_blue_fill` badges/buttons, hairline `pal.border` drawer top/bottom rules, and `STATUS_GREEN`/`LOGO_PINK` indicator ticks/dots.
   - Update `DESIGN.md` with the synced token values and named app-specific exceptions.

2. **FIX 2 — Real App Icons (`18x18 px`, Cached Once Per Scan)**:
   - Extract real Windows shell/exe icons for registry apps (`DisplayIcon` or `install_location` exe fallback via `SHDefExtractIconW` / `ExtractIconExW` + `GetDIBits` box-downsampled to `18x18` RGBA).
   - Store icon bitmaps in `ScanReport` (`AttributedApp::icon_rgba`, base64-encoded RGBA) and cache decoded `TextureHandle`s per scan in `StorageUiState` so scrolling never re-extracts.
   - Standalone/dot-folder/service/iconless entries render one consistent neutral `18x18` muted folder-tile glyph.
   - Keep `MainTableColumns` fixed column x-positions unchanged.

3. **FIX 3 — Instrument Monospace Numerals & Proportion Bar**:
   - Load system-installed `Consolas` (`C:\Windows\Fonts\consola.ttf` / `consolab.ttf`, fallback `CascadiaMono.ttf`) as `FontFamily::Monospace` / `"MonospaceBold"` — zero font downloads or installs.
   - Render all numeric columns (`Total size`, `Growth delta`, `Own size`, breakdown sizes, drive `used`/`free` counters, header attributed total) in monospace numerals.
   - Add a quiet `2px` (`<= 3 px`) proportion bar underneath each app's `Total size` (`app.total_bytes / max_unfiltered_app_bytes`) and expose exact bytes + `%` share of attributed total on hover.

4. **FIX 4 — Word-Boundary Capped Basis Lines (`…`)**:
   - Cap `Dependencies` basis second lines and `Locations` basis cells to one line ending at a word boundary with `"…"`, preserving the full basis text in the hover tooltip.
