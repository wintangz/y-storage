# y-storage Status

- **Branch / Latest Code Commit**: `main` at `6f8e6d8` (`app-family-rollup`)
- **Verification Report**: `specs/033-app-family-rollup/verification.md`
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `7B035A5C83D807F6F2806EC3CB04E5331DC54AB585336110BD9E18DFE8F58F2F`)

## Tests
- **`y-storage` (`cargo test -p y-storage`, touched crate this round)**: **55 / 55 passed**, **0 failed**, **0 warnings**
- **`y-core` / `y-platform-windows` (untouched since `0d7a280`)**: **74 / 74** / **9 / 9** (`130` prior workspace total + `8` new = `138` total)

## Perf (`--bench-perf`, 1275x900, 120 DPI, unchanged since `0d7a280`)
- **Scroll collapsed / expanded / resize**: `6.93 ms` / `8.02 ms` / `8.95 ms` (`0.0%` idle CPU)

## Live Rescan (`--ephemeral --all-drives`)
- **Family Rollup (`app-family-rollup`)**: `296` apps -> `229` top-level entries (`23` families); `sum_entries == sum_apps` (`592,447,800,423 B`, `byte_diff = 0 B`); **`Microsoft Edge`** family `9,258,259,238 B` (`6` members, `stem · dependency`, every member unit byte-identical to scan)
- **Standing Invariants (attribution untouched since `0d7a280`)**: `EA SPORTS™ FIFA 23` / `Store: MSTeams` `0 B` delta; `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`)

## Next Queued Rounds
- None queued.
