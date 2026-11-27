# Tasks `023-ui-polish-3`

- [x] Capture pre-change `before-*.png` baseline screenshots from `2a9ee96`
- [x] Split `DependencyKind` (`AntiCheat` vs `ServiceOrDriver`) & dedup trailing `(service_name)` suffixes (`FIX 5`, `FIX 12`)
- [x] Expand `icons.rs` to extract icons for `epic:`, `store:`, `sys:`, service binaries, and `AppxManifest.xml` `<Logo>` PNGs (`FIX 13`)
- [x] Promote pod `Rescan` to `PodPrimaryOutline` (`24px`, `accent_line`) & unify all pod strings across `1/2/5/10` sims (`FIX 1`, `FIX 11`)
- [x] Fix pod `Exclusions` overlay copy & add drive-exclusion tooltip to `Include` (`FIX 4`)
- [x] Bound ledger scroll viewport inside closed `1px` frame strictly above reconciliation + update `Your files` subtitle (`FIX 2`, `FIX 10`)
- [x] Stack Settings inside `settings_scroll` with 2-column drive grid, grouped zone subheads (`name + Remove` only), and neutral pin badges (`FIX 3`, `FIX 8`)
- [x] Add `status_amber` warning token and shorten stale masthead caption with hover timestamp (`FIX 9`)
- [x] Render `"new"` as a green status pill, suppress numeric `< 1 MB` deltas visually, and place `"of <global>"` after per-drive totals (`FIX 6`, `FIX 7`, `FIX 14`)
- [x] Enforce word-boundary drawer truncation & hash `font_id.family` (`FIX 12`)
- [x] Update `DESIGN.md` (`§2.2`, `§6`, `§6.2`, `§8`, `§9`, `§11`)
- [x] Verify `cargo test --workspace` (`74/74` pass, 0 warnings), release build (0 warnings), live invariance scans (`diff = 0 B`), and visually inspect all before/after/sim PNGs
- [x] Deploy `Y:\Apps\y-storage\y-storage.exe`, verify SHA-256 & GUI smoke test, write verification report, commit, and push `origin/main`
- [x] Re-anchor palette to `logo-mark-flat` (`#69CBFC`/`#FA80CB` Dark, `#0090D9`/`#FA29AA` Light, 6-stop rim/meter gradient, status family updates, delete named violet family, render & inspect `fix15-*` Dark/Light PNGs, update `DESIGN.md`, test, rebuild, redeploy, commit & push) (`FIX 15`)
