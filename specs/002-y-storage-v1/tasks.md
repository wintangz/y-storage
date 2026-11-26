# Tasks: `y-storage` V1

**Input**: `specs/002-y-storage-v1/{spec.md,plan.md,data-model.md}`

## Phase 1: Core & Platform Hardening (`app-discovery`, `drive-scanner`, `attribution-zones`, `scan-history`)

- [x] T001 Refine `attribution.rs` segment matching (`EdgeCore`/`EdgeUpdate`, `EdgeWebView` runtime, `EasyAntiCheat` isolation)
- [x] T002 Write unit tests in `crates/y-core/tests/core_tests.rs` covering registry normalization/dedup, zone/qualifier classification, no-touch classification, personal shell isolation, nested `InstallLocation` subtraction, byte conservation, and `scan-history` growth deltas
- [x] T003 Write Win32 filesystem integration tests in `crates/y-platform-windows/tests/windows_scanner_tests.rs` covering reparse point skip (directory junction), `Windows`/`WinSxS` hard exclusion, and `target/test-tmp/` fixture cleanup

## Phase 2: Desktop UI & Single-Binary Host (`storage-ui`)

- [x] T004 Create `icon.svg`, `apps/y-storage/assets/y-storage.ico` (multi-size 16..256), `app.rc`, `y-storage.manifest` (PerMonitorV2, `asInvoker`), and `build.rs`
- [x] T005 Implement `apps/y-storage/src/sw_painter.rs` (pure-CPU GDI DIB `SoftwarePainter` + offscreen RGBA screenshot export)
- [x] T006 Implement `apps/y-storage/src/win32_host.rs` (PerMonitorV2 Win32 window, DWM theme sync, mouse/wheel/keyboard/clipboard, non-blocking background scan progress updates)
- [x] T007 Implement `apps/y-storage/src/ui.rs` (header with per-drive Used/Free + last scan + Rescan + search filter, expandable sorted app list with growth deltas + zone/basis/no-touch badges, pinned `Windows & system` / `Your files` / `Unattributed` + reconciliation summary, Settings view for scan targets and exclusions, dark/light theme toggle)
- [x] T008 Wire CLI flags (`--scan-cli`, `--smoke-gui-ms`, `--render-screenshot`) and normal GUI startup in `apps/y-storage/src/main.rs`

## Phase 3: Verification, V1 Acceptance & Delivery (`test`)

- [x] T009 Run `cargo test --workspace` and verify 0 failures / 0 warnings
- [x] T010 Build `cargo build --release --workspace` and run live V1 acceptance on dev machine: (a) EA App + FIFA 23, (b) Visual Studio, (c) Discord vs independent Win32/PowerShell ground-truth sizes within ~5%, plus `C:\` reconciliation and two-scan history growth delta
- [x] T011 Capture & visually inspect UI screenshots (`main-dark`, `expanded-dark`, `settings-dark`, `main-light`), measure live GUI working set, write `LICENSE`, `THIRD-PARTY-LICENSES.md`, `README.md`, `specs/002-y-storage-v1/verification.md`, copy release exe to `Y:\Apps\y-storage\`, and commit
