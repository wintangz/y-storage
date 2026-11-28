# y-storage Status

- **Branch / Latest Code Commit**: `main` at `e317234` (`settings-layout`)
- **Verification Report**: `specs/032-settings-layout/verification.md`
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `30493754C889DDAA256B467099A88E45D2306EDF2950901CE134F2353B7E9FBA`)

## Tests
- **`y-storage` (`cargo test -p y-storage`, touched crate this round)**: **47 / 47 passed**, **0 failed**, **0 warnings**
- **`y-core` / `y-platform-windows` (untouched since `0d7a280`)**: **74 / 74** / **9 / 9** (`128` prior workspace total + `2` new = `130` total)

## Perf (`--bench-perf`, 1275x900, 120 DPI, unchanged since `0d7a280`)
- **Scroll collapsed / expanded / resize**: `6.93 ms` / `8.02 ms` / `8.95 ms` (`0.0%` idle CPU)

## Standing Live-Scan Invariants (`--ephemeral --all-drives`, unchanged since `0d7a280`)
- **`EA SPORTS™ FIFA 23`** / **`Store: MSTeams`**: `107,199,710,486 B` / `279,428,566 B` (`0 B` delta)
- **Drive Reconciliation**: `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`); `296` apps (`0` misattributions)

## Next Queued Rounds
1. `app-family-rollup`
