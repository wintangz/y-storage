# y-storage Status

- **Branch / Latest Code Commit**: `main` at `7ade0ff` (`settings-layout` semantic red + modal + global reset)
- **Verification Report**: `specs/033-app-family-rollup/verification.md` (+ `specs/032-settings-layout/verification.md`)
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `727BAE7B1D99805079D62F49735385A52A573785C25EA36E57B7090D1AF45214`)

## Tests
- **`y-storage` (`cargo test -p y-storage`, touched crate this round)**: **60 / 60 passed**, **0 failed**, **0 warnings**
- **`y-core` / `y-platform-windows` (untouched since `e2ed91f` / `0d7a280`)**: **74 / 74** / **9 / 9** (`141` prior workspace total + `2` new = `143` total)

## Perf (`--bench-perf`, 1275x900, 120 DPI, unchanged since `0d7a280`)
- **Scroll collapsed / expanded / resize**: `6.93 ms` / `8.02 ms` / `8.95 ms` (`0.0%` idle CPU)

## Live Rescan (`--ephemeral --all-drives`)
- **Family Rollup (`app-family-rollup`)**: `296` apps -> `229` top-level entries (`23` families); `sum_entries == sum_apps` (`592,447,800,423 B`, `byte_diff = 0 B`); **`Microsoft Edge`** family `9,258,259,238 B` (`6` members, `stem · dependency`, every member unit byte-identical to scan)
- **Standing Invariants (attribution untouched since `0d7a280`)**: `EA SPORTS™ FIFA 23` / `Store: MSTeams` `0 B` delta; `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`)

## Next Queued Rounds
- None queued.
