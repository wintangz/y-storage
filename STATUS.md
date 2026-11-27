# y-storage Status

- **Branch / Latest Code Commit**: `main` at `5c3969e` (`evidence-pipeline-1`)
- **Verification Report**: `specs/030-evidence-pipeline-1/verification.md`
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `CBF8F7E615C6164679253E0194647EB56A78D74D48C0DEEDD50CEA853F3373C0`)

## Workspace Tests (`cargo test --workspace`)
- **Total**: **124 / 124 passed**, **0 failed**, **0 warnings**
- **`y-core`**: **74 / 74** (`lib` 3, `core` 6, `round2` 6, `round3` 8, `round4` 5, `round5` 11, `round6` 9, `round7` 7, `round8` 8, `round9_evidence` 11)
- **`y-platform-windows`**: **9 / 9** (`lib` 1, `evidence_platform` 5, `windows_scanner` 3)
- **`y-storage`**: **41 / 41** (`main.rs` unit tests)

## Standing Live-Scan Invariants (`--ephemeral --all-drives`)
- **`EA SPORTS™ FIFA 23`**: `107,199,710,486 B` (`0 B` delta vs `specs/026` baseline)
- **`Store: MSTeams`**: `279,428,566 B` (`0 B` delta)
- **Drive Reconciliation**: `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`)
- **Live Apps / Duration**: `296` apps (`0` misattributions, `1` logged curated conflict), `21,656 ms` warm / `24,339 ms` cold

## Next Queued Rounds
1. `ledger-paging-perf`
2. `settings-layout`
