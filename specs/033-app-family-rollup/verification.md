# Verification 033 — `app-family-rollup`

**Scope:** Presentation-only product family rollup computed in the `apps/y-storage` app layer over the finished `ScanReport` (`apps/y-storage/src/{family.rs,list_query.rs,main.rs,ui.rs}` — **zero** changes to `crates/y-core` or `crates/y-platform-windows`; no screenshots captured per round instructions).

---

## 1. Synthetic Fixture Matrix

| # | Fixture | Verdict | Test Name & Verified Result |
|---|---|---|---|
| 1 | Browser (`10 GB`) + NO-TOUCH runtime (`2.4 GB`) + 2 tiny satellites (`2.5 MB`, `78 KB`), same publisher -> 1 family with exact total | **PASS** | `family::tests::fixture_1_browser_runtime_and_two_satellites_group_with_exact_total` — 1 family (`4` members, `total_bytes == 12_402_578_000`, `bases == [Stem]`, `shared_category == None`, `NO-TOUCH · Runtime` preserved on runtime member) |
| 2 | Two same-publisher products with different stems & no dependency stay separate | **PASS** | `family::tests::fixture_2_same_publisher_different_stems_without_dep_or_root_stay_separate` — 2 ungrouped `LedgerEntry::Unit`s, 0 families |
| 3 | Dependency-linked pair from different publishers does NOT group | **PASS** | `family::tests::fixture_3_cross_publisher_dependency_does_not_group` — 2 ungrouped `LedgerEntry::Unit`s, 0 families |
| 4 | Single unit (and publisherless units) create no family | **PASS** | `family::tests::fixture_4_single_unit_and_publisherless_units_create_no_family` — 3 ungrouped `LedgerEntry::Unit`s, 0 families |
| 5 | Family sum always equals sum of its units (no byte created or lost) | **PASS** | `family::tests::fixture_5_family_sum_always_equals_member_sum_and_conserves_bytes` — every `fam.total_bytes == sum(members)`, every app index appears once, `sum(entries) == sum(apps)` |
| 6 | Pagination counts a family as one top-level row | **PASS** | `ui::tests::fixture_6_pagination_counts_family_as_one_top_level_row` — 50 units + 1 family of 4 units (`54` apps) -> `visible_entries.len() == 51`, page 1 has `50`, page 2 has `1` |

---

## 2. UI & View-Model Requirements Matrix

| # | UI / View-Model Item | Verdict | Implementation |
|---|---|---|---|
| 1 | Presentation-only view model over finished `ScanReport`; zero `y-core` attribution changes | **PASS** | Pure `derive_app_families(&rep.apps)` in `apps/y-storage/src/family.rs`; `git diff --stat` confirms `crates/y-core` and `crates/y-platform-windows` untouched |
| 2 | Normalised publisher key (no publisher -> never groups); same-publisher grouping by shared stem / dependency / shared install parent (transitive closure); cross-publisher never merges | **PASS** | `normalized_publisher_key` + per-publisher `DisjointSet` in `family.rs`; Microsoft Store dot-delimited vendor prefix mapped cleanly (`Microsoft.*` / `MicrosoftCorporationII.*` -> `microsoft`, generic/hex prefixes excluded); 1-token publisher-only stems rejected |
| 3 | Family >= 2 units; family total = sum of member totals; shown basis (`stem` / `dependency` / `shared root`) | **PASS** | Components `< 2` emit `LedgerEntry::Unit`; header displays `"{N} apps"` neutral pill + `fam.basis_label()` accent pill + hover/expanded `"Family basis: ..."` detail |
| 4 | Expanding family lists member units unchanged (own bytes, own badges, NO-TOUCH badge preserved, expandable member drawers) | **PASS** | `draw_family_ledger_row` renders recessed `pal.panel_alt` container + `3px` `pal.accent_line` left bar + indented `draw_unit_ledger_row`; verified in Dark & Light via `ui::tests::family_header_expanded_members_and_notouch_drawer_render_cleanly` |
| 5 | Top-level sort (families by family total), search/filter surfacing on member match, top-level pagination | **PASS** | `filter_and_sort_ledger_entries` + `compare_ledger_entries` in `apps/y-storage/src/list_query.rs`; `list_query::tests::family_top_level_sort_and_member_search_surfacing` + `draw_main_app_list` paging over `visible_entries` |
| 6 | Family header shows category only if all members share it, else none | **PASS** | `AppFamily::shared_category` is `Some(cat)` iff all members share `cat`, else `None` (blank category cell on family row) |

---

## 3. Live Rescan Verification (`--ephemeral --all-drives`, Same Scan)

Executed directly against the deployed binary (`Y:\Apps\y-storage\y-storage.exe --scan-cli ... --ephemeral --all-drives` + `--dump-families`):

- **Scan totals**: `296` apps -> `229` top-level entries (`23` families); `sum_apps_bytes = 592,447,800,423 B` == `sum_entries_bytes = 592,447,800,423 B` (**`byte_diff = 0 B`**).
- **`Microsoft Edge` family (`family:microsoft:microsoft-edge`)**:
  - **Family Total**: `9,258,259,238 B` (`== member_sum_bytes`)
  - **Basis**: `stem · dependency` (`shared name stem "Edge" + same-publisher dependency (Microsoft Corporation)`)
  - **Shared Category**: `None` (blank on header; members span 4 categories)
  - **Member Units (all 6 verified byte-identical to `report.apps` in the same scan)**:
    1. `Microsoft Edge` (`reg:microsoft-edge`): `8,044,660,862 B` · `Browsers`
    2. `Microsoft Edge WebView2 Runtime` (`sys:edge-webview2`): `1,208,194,645 B` · `System & Runtimes` · `NO-TOUCH · Runtime`
    3. `Store: MicrosoftCorporationII.MicrosoftFamily` (`store:microsoftcorporationii.microsoftfamily_8wekyb3d8bbwe`): `2,717,542 B` · `Productivity`
    4. `Store: Microsoft.Edge.GameAssist` (`store:microsoft.edge.gameassist_8wekyb3d8bbwe`): `2,591,421 B` · `Utilities`
    5. `Store: Microsoft.MicrosoftEdge.Stable` (`store:microsoft.microsoftedge.stable_8wekyb3d8bbwe`): `78,384 B` · `Browsers`
    6. `Store: Microsoft.MicrosoftEdgeDevToolsClient` (`store:microsoft.microsoftedgedevtoolsclient_8wekyb3d8bbwe`): `16,384 B` · `Development`

---

## 4. Touched-Crate Tests, Tripwire & Deployment

- **Tripwire (`git diff --stat`)**: Only `apps/y-storage/src/{family.rs,list_query.rs,main.rs,ui.rs}` modified in code (`crates/y-core` and `crates/y-platform-windows` untouched).
- **Touched-crate tests (`cargo test -p y-storage`)**: **`55 / 55` passed**, **`0` failed**, **`0` warnings** (`47` prior + `8` new).
- **Release build (`cargo build --release --bin y-storage`)**: **0 warnings** (`49.70s`).
- **Deployed binary**: `Y:\Apps\y-storage\y-storage.exe`
  - **SHA-256**: `7B035A5C83D807F6F2806EC3CB04E5331DC54AB585336110BD9E18DFE8F58F2F`
- **GUI smoke test**: `Y:\Apps\y-storage\y-storage.exe --smoke-gui-ms 500` -> `ExitCode = 0`.
