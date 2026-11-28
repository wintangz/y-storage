# y-storage Status

- **Branch / Latest Code Commit**: `main` at `0d7a280` (`ledger-paging-perf`)
- **Verification Report**: `specs/031-ledger-paging-perf/verification.md`
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `DB0706894F8BF2DD08AFB09722F9231D9E1909550477D7412180F2AF20017EDA`)

## Workspace Tests (`cargo test --workspace`)
- **Total**: **128 / 128 passed**, **0 failed**, **0 warnings**
- **`y-core`**: **74 / 74** (`lib` 3, `core` 6, `round2` 6, `round3` 8, `round4` 5, `round5` 11, `round6` 9, `round7` 7, `round8` 8, `round9_evidence` 11)
- **`y-platform-windows`**: **9 / 9** (`lib` 1, `evidence_platform` 5, `windows_scanner` 3)
- **`y-storage`**: **45 / 45** (`main.rs` unit tests)

## Perf (`--bench-perf`, 1275x900, 120 DPI)
- **Scroll collapsed / expanded / resize**: `8.73 -> 6.93 ms` / `9.71 -> 8.02 ms` / `10.43 -> 8.95 ms` (`0.0%` idle CPU)

## Standing Live-Scan Invariants (`--ephemeral --all-drives`, unchanged in UI/perf round)
- **`EA SPORTS™ FIFA 23`** / **`Store: MSTeams`**: `107,199,710,486 B` / `279,428,566 B` (`0 B` delta)
- **Drive Reconciliation**: `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`); `296` apps (`0` misattributions)

## Next Queued Rounds
1. `settings-layout`
