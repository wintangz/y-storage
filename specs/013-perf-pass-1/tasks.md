# Tasks: `perf-pass-1`

- [x] T001 Append `perf-pass-1` to `.specify/memory/constitution.md` and create `specs/013-perf-pass-1/{spec,plan,tasks}.md`
- [x] T002 Add `--bench-perf` real-window 4-metric harness (`idle_5s`, `scroll_collapsed_5s`, `scroll_expanded_5s`, `resize_drag_5s`) and capture unmodified baseline numbers
- [x] T003 Implement FIX A (Regular `Consolas` numerals, `ProportionalStrong` `Segoe UI Semibold` only on row app names & expanded drawer titles) in `apps/y-storage/src/ui.rs` and update `DESIGN.md` §3 / §9 / §11
- [x] T004 Implement FIX B: row virtualization, strict repaint-on-demand, `Arc<ScanReport>` + report/filter/rim/text caches, pooled DIB resize capacity, and scanline-clipped rasterization + unit tests
- [x] T005 Re-measure all 4 metrics (`perf_after.json`), run `cargo test --workspace`, release build, live scan invariant check, Dark/Light screenshots, write `verification.md`, deploy `Y:\Apps\y-storage\y-storage.exe`, and commit on `main`
