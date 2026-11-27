# Tasks: `attribution-engine-2`

- [x] T000 Capture pre-change live 4-drive baseline scan and scaffold specs
- [x] T001 Part A & B(7): `y-core` `model.rs` & unified `ZoneRulebook` + `FolderPin` in `config.rs` with legacy migration
- [x] T002 Part B(1)–(7): Epic manifest parser (`dependencies.rs`), bottom-up decomposition (`tree.rs`), and 7-step claim resolution (`attribution.rs`)
- [x] T003 Windows scanner (`scanner.rs`): `ZoneRulebook` shell integration, Epic/Steam `ManifestAppEntry`, candidate exe probing, and retention for nested carve-outs/pins
- [x] T004 Settings `Attribution Zone Rulebook` UI (`ui.rs`), drawer carve-out notes, loose-files qualifiers, and screenshot views (`main.rs`)
- [x] T005 Update `DESIGN.md` Part II `§11` with unified Zone Rulebook, bottom-up resolution order, Two-Segment composition, and Three-Way inference
- [x] T006 Add `round5_tests.rs` end-to-end `FolderNode` fixture tests + UI rulebook tests
- [x] T007 Verify `cargo test --workspace` (0 warnings), release build, live scans (`D:\Media`, `FIFA 23`, `MSTeams`, `diff = 0 B`, zero `[files]`), inspect PNGs, deploy exe, `verification.md`, git commit
