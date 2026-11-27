# Plan `023-ui-polish-3`

1. **Pre-Change Screenshots (`before-*.png`)**: Capture dark/light before evidence from `main` (`2a9ee96`) on real 4-drive scan JSON (`after_ae2_4drive.json`) + synthetic delta contrast fixture (`growth_contrast_report.json`).
2. **Core & Platform Changes**:
   - Split `DependencyKind::AntiCheatServiceDriver` into `AntiCheat` & `ServiceOrDriver` (`#[serde(alias = "AntiCheatServiceDriver")]`) in `crates/y-core/src/model.rs`, update `data/dependency_catalog.toml`, and classify discovered services via `is_anticheat_service` + `format_service_dep_name` in `crates/y-core/src/dependencies.rs`.
   - Upgrade `crates/y-platform-windows/src/icons.rs` (`FIX 13`) to scan `epic:`, `store:`, `sys:`, service binaries, depth-4 `Body`/`InstallLocation` `.exe`s (skipping `\AppData\` / `\Package Cache`), and MSIX `AppxManifest.xml` `<Logo>` PNGs via Win32 `gdiplus`.
3. **UI, Harness & Design System Changes**:
   - Update `ThemePalette` (`status_amber`), `ButtonVariant::PodPrimaryOutline`, `DeltaCellDisplay` (`NewPill` green pill, `< 1 MB` `Blank` with exact byte hover, `>= 1 MB` `SignedValue`), `PER_DRIVE_SIZE_COL_W` & companion placement (`"2.28 GB of 163.39 GB"`), `draw_drive_pods_grid` (`Include` / `Exclusions` / `Scanned HH:MM` / `Rescan` everywhere, `3+2` / `5+5` grids), `draw_pod_scope_overlay` folder copy, bounded closed `draw_crisp_frame` ledger scroll container above reconciliation, `"Shell folders & Personal-files zones · read-only"` `Your files` subtitle, natural-scrolling 2-column Settings + grouped subheads (`App container` / `Personal files`, no per-chip type badges, neutral muted pin badges), and word-boundary drawer truncation in `apps/y-storage/src/ui.rs` and `apps/y-storage/src/main.rs`.
   - Update `DESIGN.md` (`§2.2`, `§6`, `§6.2`, `§8`, `§9 FC6`, `§11`).
4. **Verification, Screenshots, Deploy & Push**:
   - Run `cargo test --workspace` + `cargo build --release --bin y-storage` (0 warnings).
   - Run live `4-drive` and `C:`-only scans to verify attribution & reconciliation invariance (`diff = 0 B`, `EA SPORTS™ FIFA 23`, `Store: MSTeams`, and `FIX 13` icon gains).
   - Render and inspect all `before-*`, `after-*`, and `sim-*` screenshots via `read_file`.
   - Deploy `Y:\Apps\y-storage\y-storage.exe`, run `--smoke-gui-ms 500`, write specs, `git commit`, and `git push origin main`.
