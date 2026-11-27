# Plan: `substring-matcher-fix`

1. Update `.specify/memory/constitution.md` and `specs/020-substring-matcher-fix/{spec.md, plan.md, tasks.md}`.
2. Update `crates/y-core/src/discovery.rs`: skip protocol URIs (`://`) and `\steam\games\` in `infer_install_root_from_exe`, refine `normalize_app_token`, and export `split_alnum_words`, `AppTokenMatchKind`, `match_app_predicate`, `contains_whole_token_sequence`.
3. Update `crates/y-core/src/attribution.rs`: replace raw `contains` in `find_app_matching` (exact-first, whole-word boundary, ambiguity fallthrough), `find_by_publisher`, and Package Cache token matching.
4. Update `crates/y-core/src/categories.rs`, `data/app_categories.toml`, and `crates/y-core/src/dependencies.rs` to use token-boundary matching across mappings, rules, folder kinship, and dependency target/app rules.
5. Add regression tests in `crates/y-core/tests/round3_tests.rs` and `--view msteams-drawer` in `apps/y-storage/src/main.rs`.
6. Run `cargo test --workspace`, build release, execute live scan + full collision sweep vs `ledger_usability_1_scan.json`, inspect `screenshot-msteams-drawer-dark.png`, write `verification.md`, copy release `.exe` to `Y:\Apps\y-storage\`, and commit locally.
