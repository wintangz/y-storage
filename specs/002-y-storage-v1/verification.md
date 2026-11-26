# Verification Report: `y-storage` V1

**Date**: 2026-11-26  
**Toolchain**: `rustc 1.100.0` (`x86_64-pc-windows-msvc`, Edition 2024, `windows-sys = "=0.59.0"`, `egui 0.29.1`)  
**Release Deliverable**: `Y:\Apps\y-storage\y-storage.exe` (`3,648,512` bytes standalone portable binary)

---

## Milestone Results Summary

| Milestone / Feature Slug | Result | Proving Number / Evidence |
|---|---|---|
| `feasibility-spike` — (1) Registry enumeration | **PASS** | `319` raw `DisplayName` entries (`70` with `InstallLocation`); `85` canonical user-facing apps (`41` direct `InstallLocation`, `15` inferred install location) (`specs/001-feasibility-spike/evidence.md`). |
| `feasibility-spike` — (2) Default-scope scan speed | **PASS** | **`8.797 s`** (`8,797 ms`) non-elevated default walk (`C:\` excluding `C:\Windows` + secondary `InstallLocation`s), well under `150 s` threshold (zero MFT/USN admin elevation needed). |
| `feasibility-spike` — (3) Top-50 attribution coverage | **PASS** | `42/50` (`84.0%`) attributed in default scope (`40/50` / `80.0%` on `C:\`-only); top 5 unattributed: `Local\Temp` (`2.24 GB`), `EdgeCore` (`1.91 GB`), `Roaming\flutter` (`1.89 GB`), `.gemini` (`1.62 GB`), `EdgeWebView` (`1.12 GB`). |
| `app-discovery` & `attribution-zones` | **PASS** | Refined segment matching attributed `EdgeCore`/`EdgeUpdate` (`Microsoft Edge`) & `EdgeWebView` (`NO-TOUCH · Runtime`), `WindowsApps` Store packages, `Package Cache` `{GUID}` links, `BODY`/`GREEN`/`RED`/`Roaming · Config`/`ProgramData · Machine-shared` zones, and `NO-TOUCH` badges (`245` total entries incl. Store packages). |
| `drive-scanner` | **PASS** | `Scan1 = 8.604 s` (`WS = 8.63 MB`), `Scan2 = 8.373 s` (`WS = 8.76 MB`); skips directory junctions (`windows_scanner_tests.rs`), hard-excludes `Windows` & `WinSxS`, runs at `THREAD_MODE_BACKGROUND_BEGIN`. |
| `scan-history` | **PASS** | Two consecutive live scans persisted `scan_history.json`, `folder_cache.json`, `last_scan.json`, and `config.toml`; Scan 2 computed live deltas (`{"Changed":0}` on `Discord`). |
| `storage-ui` | **PASS** | Live Win32 window `--smoke-gui-ms 2000` loaded `245` cached apps with zero auto-scan (`WS = 25.66 MB`, `PrivateBytes = 6.41 MB`); all 4 screenshots (`main-dark`, `expanded-dark`, `settings-dark`, `main-light`) visually inspected cleanly. |
| `cargo test --workspace` | **PASS** | `10 passed; 0 failed` (`5` in `y-core`, `2` in `y-platform-windows`, `3` in `y-storage`), `0` compiler warnings. |

---

## V1 Acceptance Hand-Check Comparisons (Dev Machine)

| Acceptance Check | `y-storage` Total | Independent Ground Truth | Delta | Verdict |
|---|---|---|---|---|
| (a) EA App + FIFA 23 combined across all locations | `54,627,471,178 B` (`50.88 GB`) | `54,627,471,178 B` (`50.88 GB`) | **`0.0000%`** | **PASS** (`< 5%`) |
| (b) Visual Studio total (`A-001`: Community 2022 + Installer) | `3,246,635,065 B` (`3.02 GB`) | `3,246,639,463 B` (`3.02 GB`) | **`0.0001%`** | **PASS** (`< 5%`) |
| (c) Per-user app (`A-002`: Discord across `Local\Discord`, `Roaming\discord`, `Common Files\Discord`, `SquirrelTemp`) | `1,074,461,668 B` (`1,024.69 MB`) | `1,074,461,668 B` (`1,024.69 MB`) | **`0.0000%`** | **PASS** (`< 5%`) |
| (c2) Per-user cross-check (`Microsoft Visual Studio Code (User)`) | `2,993,806,872 B` (`2.79 GB`) | `2,993,804,382 B` (`2.79 GB`) | **`0.0001%`** | **PASS** |
| (d) Drive `C:\` Reconciliation | `apps 84,477,462,024 + your_files 3,210,721,812 + unattr 14,884,258,917 + win_sys 45,957,397,375 (rem 34,129,447,807) + free 99,702,910,976` | `248,232,751,104 B` (`drive_total_bytes`) | **`0 B`** | **PASS** |

*Note on (a)*: `C:\Users\Win\Documents\FIFA 23` (`57,223,884 B` / `54.57 MB`) is intentionally routed to `Your files` (not attributed to FIFA 23) in accordance with the constitutional personal-shell isolation rule.

---

## Logged Assumptions

- **A-001 (Visual Studio SKU on Dev Machine)**: The dev machine has `Visual Studio Community 2022` (`C:\Program Files\Microsoft Visual Studio\2022\Community`) and `Microsoft Visual Studio Installer` (`C:\Program Files (x86)\Microsoft Visual Studio\Installer`) installed rather than standalone `Visual Studio Build Tools`. Acceptance (b) verifies the combined Visual Studio footprint across all install, ProgramData, and AppData locations.
- **A-002 (Per-User App in `AppData\Local`)**: `C:\Users\Win\AppData\Local\Programs` contains only an empty `Common` directory on this machine, while `Discord` installs its binary body in `C:\Users\Win\AppData\Local\Discord` (`BODY`) and user data in `C:\Users\Win\AppData\Roaming\discord` (`RED · Roaming config`). Acceptance (c) verifies `Discord`, alongside a cross-check of `Microsoft Visual Studio Code (User)` (`Y:\Apps\Microsoft VS Code` + `Roaming\Code` + `.vscode`).
- **A-003 (Windows & System Estimate)**: `C:\Windows` and `WinSxS` are never walked to prevent hardlink inflation; `Windows & system` sums directly sized root files (`pagefile.sys`, `swapfile.sys`, `DumpStack.log.tmp`) and the remainder (`drive_used - everything_scanned`, labelled as an estimate).
