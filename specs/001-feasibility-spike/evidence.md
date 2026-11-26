# Evidence Report: `feasibility-spike`

**Date**: 2026-11-26  
**Toolchain**: `rustc 1.100.0` (`x86_64-pc-windows-msvc`, Edition 2024, `windows-sys = "=0.59.0"`)  
**Elevation**: Standard non-elevated user (`IsAdmin: False`)

## Summary Results

| Spike Item | Result | Proving Evidence |
|---|---|---|
| (1) Registry enumeration | **PASS** | `319` raw `DisplayName` entries (`70` with non-empty `InstallLocation`); `85` canonical user-facing apps (`41` direct `InstallLocation`, `15` inferred from `UninstallString`/`DisplayIcon`/`SteamPath`). |
| (2) Default-scope scan speed | **PASS** | **`8.797 s`** (`8,797 ms`) for `C:\` (excluding `C:\Windows`, capturing `pagefile.sys`/`swapfile.sys`/`DumpStack.log.tmp`) + secondary-drive `InstallLocation`s (`D:\`, `Y:\`, `Z:\`), using low I/O priority (`THREAD_MODE_BACKGROUND_BEGIN`) and `FindFirstFileExW(FindExInfoBasic, FIND_FIRST_EX_LARGE_FETCH)`. Well under `150 s` threshold — no NTFS MFT/USN admin elevation needed. |
| (3) Attribution coverage (Top 50) | **PASS** | Default scope (`C:\` + secondary `InstallLocation`s): **`42/50` (`84.0%`)** attributed, `8` (`16.0%`) `Unattributed`. `C:\`-only: **`40/50` (`80.0%`)** attributed, `10` (`20.0%`) `Unattributed`. |
| Drive `C:\` reconciliation | **PASS** | `apps = 80,881,190,370 (75.33 GB)` + `your_files = 3,210,721,812 (2.99 GB)` + `unattributed = 18,472,560,283 (17.20 GB)` + `win_sys = 45,956,307,311 (42.80 GB)` + `free = 99,711,971,328 (92.86 GB)` = `248,232,751,104` vs `drive_total = 248,232,751,104` (**diff = `0` bytes**). |

## Top 5 Unattributed Folders (Live Dev Machine)

1. `C:\Users\Win\AppData\Local\Temp` — `2,401,477,344` bytes (`2.24 GB`)
2. `C:\Program Files (x86)\Microsoft\EdgeCore` — `2,050,554,248` bytes (`1.91 GB`)
3. `C:\Users\Win\AppData\Roaming\flutter` — `2,031,983,212` bytes (`1.89 GB`)
4. `C:\Users\Win\.gemini` — `1,743,432,999` bytes (`1.62 GB`)
5. `C:\Program Files (x86)\Microsoft\EdgeWebView` — `1,207,499,322` bytes (`1.12 GB`)

## Architecture Decision Logged

- Because standard non-elevated `FindFirstFileExW` at `THREAD_MODE_BACKGROUND_BEGIN` completes the full default-scope walk in `8.797 s` (`< 150 s` gate), `002-y-storage-v1` adopts this standard-user scanner without requiring admin UAC elevation or raw volume MFT/USN handles (`FSCTL_ENUM_USN_DATA`).
