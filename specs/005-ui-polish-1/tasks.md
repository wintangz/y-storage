# Tasks: `ui-polish-1`

- [x] T001 Update `.specify/memory/constitution.md` and author `specs/005-ui-polish-1/{spec.md,plan.md,tasks.md}`
- [x] T002 Implement `WindowPlacementConfig` persistence in `config.rs` + `win32_host.rs` and regenerate full-bleed (`~97%`) `y-storage.ico` in `build.rs` with `WM_SETICON`
- [x] T003 Polish `apps/y-storage/src/ui.rs` and screenshot modes in `main.rs` covering Polish Items 1–8 without changing any scan/attribution/dependency math
- [x] T004 Run `cargo test --workspace`, build release exe, run one live scan regression check, verify icon/persistence, and capture + inspect all 5 screenshots
- [x] T005 Write `specs/005-ui-polish-1/verification.md`, deploy release exe to `Y:\Apps\y-storage\`, and commit to `main`
