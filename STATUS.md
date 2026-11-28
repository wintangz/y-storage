# y-storage Status

- **Branch / Latest Code Commit**: `main` (`icon-sizebar-fixes`: `31d82cb` folder-fallback icon candidate ranking + 4px solid ledger size bar)
- **Verification Report**: `specs/033-app-family-rollup/verification.md` (+ `specs/032-settings-layout/verification.md`)
- **Deployed Binary**: `Y:\Apps\y-storage\y-storage.exe` (SHA-256 `D1C6FDD4401277D388F0466B382E7BAFF127C09FC791970FB9DAE8BDDAB47E87`)

## Tests
- **`cargo test --workspace` (tripwire battery, `crates/y-platform-windows/src/icons.rs` touched)**: **146 / 146 passed**, **0 failed**, **0 warnings**
- **`y-storage`**: **61 / 61 passed** (`60` prior + `1` new `ledger_size_bar_renders_proportional_widths_and_uniform_4px_thickness`)
- **`y-platform-windows`** (touched in `31d82cb`): **11 / 11 passed** (`9` prior + `2` new icon candidate ranking tests)
- **`y-core`** (untouched since `e2ed91f`): **74 / 74 passed**

## Perf (`--bench-perf`, 1275x900, 120 DPI, unchanged since `0d7a280`)
- **Scroll collapsed / expanded / resize**: `6.93 ms` / `8.02 ms` / `8.95 ms` (`0.0%` idle CPU)

## Live Rescan (`--ephemeral --all-drives`)
- **Family Rollup (`app-family-rollup`)**: `296` apps -> `229` top-level entries (`23` families); `sum_entries == sum_apps` (`592,447,800,423 B`, `byte_diff = 0 B`); **`Microsoft Edge`** family `9,258,259,238 B` (`6` members, `stem · dependency`, every member unit byte-identical to scan)
- **Standing Invariants (`icon-sizebar-fixes` tripwire scan, `296` apps, `25,511 ms`)**: `EA SPORTS™ FIFA 23` (`107,199,710,486 B`) / `Store: MSTeams` (`279,428,566 B`); `diff = 0 B` on all drives (`C:`, `D:`, `Y:`, `Z:`)

## Next Queued Rounds
- None queued.
