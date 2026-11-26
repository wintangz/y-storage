# Verification Report: `unattributed-assist-1`

**Commit Parent**: `6d8b121` (`main` / `origin/main`)
**Live Ephemeral Scan Date**: `2026-11-26` (`259` attributed apps, `303` unattributed folders)

## 1. Per-Fix Result Matrix

| Item | Result | Verification Summary |
|---|---|---|
| **FIX 1 — Locate** | **PASS** | `draw_unattributed_item_row` adds a `18.0px` `SecondaryOutline` `"Locate"` button on every row inside the expanded `Unattributed` group. Existing paths spawn `explorer.exe /select,"<full path>"` via `CommandExt::raw_arg` (re-checking `Path::new(&norm).exists()` in `win32_host::launch_explorer_select` before launching). Missing-on-disk paths disable the button with hover text `"no longer on disk"` (covered by `ui::tests::unattributed_rows_render_locate_and_disabled_missing_folder_cleanly`). |
| **FIX 2 — Web Search** | **PASS** | Every `Unattributed` row also renders a `18.0px` `SecondaryOutline` `"Web search"` button building `https://www.google.com/search?q=<RFC3986>` from `"<folder_name>" "<parent_name>" "what is this folder"` (or `"<folder_name>" "what is this folder"` when at drive root). Exact constructed query renders on button hover (`screenshot-unattributed-hover-query-dark.png`). Opens default browser via OS `ShellExecuteW(..., "open", ...)` — zero network requests inside `y-storage`. |
| **FIX 3 — Placement + Scope** | **PASS** | Buttons appear strictly on `Unattributed` folder rows inside the pinned ledger immediately left of `SIZE_COL_W` (`YS1` alignment preserved), and do NOT appear on attributed app `Locations`, `Windows & system`, `Your files`, or `Shared & system caches`. Documented in `DESIGN.md` Part II (`§11`). |

### Search Engine Selection
- **Engine**: Google Search (`https://www.google.com/search?q=...`)
- **Why**: Highest coverage of obscure Windows system/vendor folder names, Visual Studio remote debugger directories (`onecoremsvsmon`), Store package IDs, and OEM telemetry caches, while requiring zero API keys, zero authentication, and zero network calls from `y-storage` itself (launched strictly through OS `ShellExecuteW`).

---

## 2. Automated Tests, Build & Live Scan Invariants

- **`cargo test --workspace`**: **`37 passed, 0 failed, 0 ignored`**, `0` compiler warnings:
  - `y-core`: `16 passed` (`core_tests`: `6`, `round2_tests`: `6`, `round3_tests`: `4`)
  - `y-platform-windows`: `2 passed` (`windows_scanner_tests`)
  - `y-storage`: `19 passed` (including `search_query_quotes_folder_parent_and_phrase_cleanly`, `search_query_and_url_handle_special_characters_safely`, `locate_command_quotes_paths_with_spaces_commas_and_unicode`, and `unattributed_rows_render_locate_and_disabled_missing_folder_cleanly`)
- **`cargo build --release --workspace`**: Succeeded cleanly (`24.72s`, `0` warnings); deployed to `Y:\Apps\y-storage\y-storage.exe`.
- **Live Ephemeral Scan (`--scan-cli ... --ephemeral`)**:
  - `259` attributed apps · `303` unattributed folders
  - `EA SPORTS™ FIFA 23`: **`51,372,195,955 B`**, **`7` dependencies**
  - `C:\` reconciliation: **`diff = 0 B`**

---

## 3. Screenshots Inspected (`specs/016-unattributed-assist-1/`)
- [screenshot-unattributed-expanded-dark.png](file:///Y:/Repos/y-storage/specs/016-unattributed-assist-1/screenshot-unattributed-expanded-dark.png)
- [screenshot-unattributed-hover-query-dark.png](file:///Y:/Repos/y-storage/specs/016-unattributed-assist-1/screenshot-unattributed-hover-query-dark.png)

## 4. Logged Assumptions
- **Per-Scan Disk Check Caching**: `StorageUiState::unattrib_exists_cache` caches `Path::exists()` per `(scan_timestamp_unix, path)` in UI memory for smooth scrolling, while `win32_host::launch_explorer_select` re-verifies `Path::exists()` at click time so a folder deleted mid-session never launches Explorer to an invalid target.
- **Virtualized Full List**: Replaced `.take(40)` on expanded `Unattributed` rows with viewport clip virtualization inside `pinned_groups_scroll` so all unattributed folders (`303` on the reference machine) can be located/searched without UI lag.
