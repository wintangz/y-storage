# Specification: `substring-matcher-fix`

**Slug**: `substring-matcher-fix`  
**Status**: Implementing

## Problem Statement

1. **`find_app_matching` raw-substring collision**: `crates/y-core/src/attribution.rs` used `dn.contains(pred)`, causing curated path rules (e.g. `\appdata\local\steam` and `\common files\steam` with predicate `"steam"`) to match `"Store: MSTeams"` (`"msteams"` contains `"steam"`), misattributing `768.02 MB` of Steam's folders (`htmlcache`, `cefdata`, `Common Files\Steam\drivers`) to `MSTeams`.
2. **Protocol launcher URI shadowing**: `infer_install_root_from_exe` in `crates/y-core/src/discovery.rs` extracted `Z:\Games\Steam` from `"Z:\Games\Steam\steam.exe" steam://uninstall/376210` and `Z:\Games\Steam\steam\games\*.ico`, causing `"The Isle"` to capture `Z:\Games\Steam` (`2.06 GB`) and merge `"Steam"` (`Z:\Games\Steam\uninstall.exe`) as an alias.
3. **Downstream `contains` leakage**: `categories.rs` (`classify_app` `app_matches` / `name_contains` / `publisher_contains`) and `dependencies.rs` (`folder_kinship`, `app_rules`, `is_self_dependency`) used raw `contains`, assigning `MSTeams` the `"Valve Steam PC gaming client"` category and inferring a spurious `MSTeams -> steam` dependency.

## Requirements

- Replace raw-substring app, publisher, category, and dependency matching with token-boundary / whole-word matching (`split_alnum_words`, `match_app_predicate`, `contains_whole_token_sequence`).
- Prefer exact full-token matches over whole-word slice matches; when multiple unrelated apps tie ambiguously at the winning tier, do NOT force-match — fall through to `Unattributed` (except split versioned rows of the same canonical product family).
- Do not infer install directories from protocol launcher URIs (`://`) or Steam shared icon caches (`\steam\games\`).
- Complete a full installed-app substring collision sweep comparing pre-fix vs post-fix attribution, category, dependencies, and exact before/after totals.
- Verify with regression tests (including `MSTeams`/`Steam` and generic substring/ambiguity fixtures), live scan (`FIFA 23 = 51,372,195,955 B` / `7` deps, `C:\ diff = 0 B`, `MSTeams ≈ 266.49 MB` and `Steam` `+768.02 MB`), and dark screenshot of the corrected `Store: MSTeams` drawer.
