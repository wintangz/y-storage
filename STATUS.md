# y-storage Status

- **Branch / Latest Code Commit**: `main` at `b1ffbd5` (`settings-layout` Rulebook Browse... picker)
- **Verification Report**: `specs/033-app-family-rollup/verification.md` (+ `specs/032-settings-layout/verification.md`)
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `ADA28A1D4C0B2CEA2ADC68AE264D8211624677A518B2BDDF79F858D246D8EB0B`)

## Tests (`cargo test --workspace`)
- **`y-storage` / `y-core` / `y-platform-windows`**: **58 / 58** / **74 / 74** / **9 / 9** (**141 / 141 total**, **0 failed**, **0 warnings**)

## Perf (`--bench-perf`, 1275x900, 120 DPI, unchanged since `0d7a280`)
- **Scroll collapsed / expanded / resize**: `6.93 ms` / `8.02 ms` / `8.95 ms` (`0.0%` idle CPU)

## Live Rescan (`--ephemeral --all-drives`)
- **Family Rollup (`app-family-rollup`)**: `296` apps -> `229` top-level entries (`23` families); `sum_entries == sum_apps` (`592,447,800,423 B`, `byte_diff = 0 B`); **`Microsoft Edge`** family `9,258,259,238 B` (`6` members, `stem · dependency`, every member unit byte-identical to scan)
- **Standing Invariants (attribution untouched since `0d7a280`)**: `EA SPORTS™ FIFA 23` / `Store: MSTeams` `0 B` delta; `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`)

## Next Queued Rounds
- None queued.
