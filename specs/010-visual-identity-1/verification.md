# Verification Report: `visual-identity-1`

**Date**: 2026-11-26  
**Scope**: Visual identity, token sync with `z-controller`, cached shell app icons, monospace numerals, size proportion bars, and word-boundary basis capping. Zero changes to scan, attribution, dependency, history, sort/filter logic, or computed byte totals.

## 1. Token Comparison Table (`z-controller` vs `y-storage`)

Audited directly from `Y:\Repos\z-controller\apps\z-controller\src\ui.rs`:

| Token / Role | `z-controller` Dark | `y-storage` Before (Dark) | `y-storage` After (Dark) | `z-controller` Light | `y-storage` Before (Light) | `y-storage` After (Light) | Notes / Drift / Named Exceptions |
|---|---|---|---|---|---|---|---|
| `bg` | `#0A0A10` | `#0A0A10` | `#0A0A10` | `#EEF4FA` | `#EEF4FA` | `#EEF4FA` | Exact sync (`z-controller` Dark uses `R==G<B`, Light uses cool slate `R<G<B`). |
| `panel` | `#14141F` | `#14141F` | `#14141F` | `#FFFFFF` | `#FFFFFF` | `#FFFFFF` | Exact sync. |
| `panel_alt` (`faint_bg`) | `#10101A` | `#10101A` | `#10101A` | `#F4F8FC` | `#F4F8FC` | `#F4F8FC` | Exact sync. |
| `border` | `#26263A` | `#26263A` | `#26263A` | `#D9E4F0` | `#D9E4F0` | `#D9E4F0` | Exact sync. |
| `text` | `#EDEDF2` | `#EDEDF2` | `#EDEDF2` | `#1C2733` | `#1C2733` | `#1C2733` | Exact sync. |
| `muted` | `#9A9AB0` | `#9A9AB0` | `#9A9AB0` | `#5F7186` | `#5F7186` | `#5F7186` | Exact sync; `widgets.noninteractive.fg_stroke` also synced to `pal.muted`. |
| `subtle_text` | *(uses `muted`)* | `#72748C` | `#82829A` | *(uses `muted`)* | `#748499` | `#748499` | **Named Exception (`DESIGN.md` §2.3 `T4`)**: quiet inline table tier; Dark fixed to neutral `R==G` violet-grey `#82829A`. |
| `hint_text` | *(none)* | `#56586E` | `#64647A` | *(none)* | `#98A6B8` | `#98A6B8` | **Named Exception (`C10`)**: dim placeholder hint; Dark fixed to `#64647A`. |
| `widget` / `hover` / `active` | `#181826` / `#1F1F31` / `#182230` | `#181826` / `#1F1F31` / `#182230` | `#181826` / `#1F1F31` / `#182230` | `#F5F8FC` / `#E9F2FA` / `#DCEBF8` | `#F5F8FC` / `#E9F2FA` / `#DCEBF8` | `#F5F8FC` / `#E9F2FA` / `#DCEBF8` | Exact sync. |
| `accent_line` | `#54B9F2` | `#54B9F2` | `#54B9F2` | `#2F9BE8` | `#2F9BE8` | `#2F9BE8` | Exact sync. Text on `pill_blue_fill` badges/buttons switched from `#54B9F2`/`#2F9BE8` to `pal.text` (`#EDEDF2`/`#1C2733`) matching `z-controller`. |
| `LOGO_LAVENDER` / `lavender_text` | `#D8A6FF` | `#D8A6FF` | `#D8A6FF` | `#D8A6FF` | `#D8A6FF` bar / `#783EB8` text | `#D8A6FF` bar / `#783EB8` text | **`z-controller` drift**: single `#D8A6FF` const across modes. **Exception (`C10`)**: `#783EB8` on Light chip text. |
| `LOGO_PINK` / `delta_grew` | `#F7A8D8` | `#F7A8D8` | `#F7A8D8` | `#F7A8D8` | `#C43058` | `#F7A8D8` tick / `#C43058` text | **`z-controller` drift**: single `#F7A8D8` const; `Unattributed` tick synced to `#F7A8D8`; **Exception (`C9`/`C10`)**: `#C43058` for Light `+delta` text. |
| `STATUS_GREEN` / `status_green` | `#28D66C` | `#28D66C` | `#28D66C` | `#28D66C` | `#148A44` | `#28D66C` dot/tick / `#148A44` text | **`z-controller` drift**: single `#28D66C` const; steady-state dot & cache tick synced to `#28D66C`; **Exception (`C9`/`C10`)**: `#148A44` for Light text/badges. |
| `STATUS_RED` / `no_touch` | `#F04458` | `#F04458` / `#CE5C6C` | `#F04458` / `#CE5C6C` | `#F04458` | `#CE2840` / `#BC4050` | `#F04458` dot / `#CE2840` & `#BC4050` | **`z-controller` drift**: single `#F04458` const; **Exception (`DESIGN.md` §2.1 & `C10`)**: `#CE2840` & muted `#CE5C6C`/`#BC4050` outlines. |
| `pill_blue_fill` / `pill_red_fill` (`pill_pink_fill`) | `#121C28` / `#261622` | `#121C28` / `#2E141A` | `#121C28` / `#261622` | `#EBF6FF` / `#FDF1F9` | `#EBF6FF` / `#FDEAED` | `#EBF6FF` / `#FDF1F9` | Synced `pill_red_fill` directly to `z-controller`'s `pill_pink_fill`. |
| `input_bg` | `#0A0A10` | `#0A0A10` | `#0A0A10` | `#EEF4FA` | `#FFFFFF` | `#EEF4FA` on `panel` / `#FFFFFF` on `bg` | Matches `z-controller` inside white Settings cards (`#EEF4FA`); **Exception (`C10`)**: header filter input on `#EEF4FA` canvas keeps `#FFFFFF`. |

## 2. Automated Tests & Build

- `cargo test --workspace`: **25 passed, 0 failed** (6 `core_tests` incl. `cached_app_icon_base64_and_json_roundtrip`, 4 `round2_tests`, 4 `round3_tests`, 2 `windows_scanner_tests`, 9 `y-storage` unit tests incl. `word_boundary_truncation_never_splits_mid_word_and_ends_with_ellipsis`), **0 warnings**.
- `cargo build --release --workspace`: **Succeeded (exit code 0), 0 warnings**.

## 3. Live CLI Scan Verification (`--scan-cli ... --ephemeral`)

- Apps: `262` total (`74` registry `reg:*` entries; `59` extracted real shell icons; standalone/service/dot-folder entries use consistent neutral folder-tile fallback)
- `EA SPORTS™ FIFA 23`: `51,372,195,955 B` (`FIFA_HAS_ICON = True`)
- Total attributed across drives: `161,356,708,470 B`
- `C:\` reconciliation: **`diff = 0 B`**

## 4. Fix Status & Screenshots (`1020x720`, `96` DPI)

- **FIX 1 (Colour Sync with `z-controller` + Token Table)**: **PASS** — Verified in `screenshot-main-dark.png` & `screenshot-main-light.png`.
- **FIX 2 (Real App Icons `18x18 px`, Cached Once Per Scan + Neutral Fallback)**: **PASS** — Verified in `screenshot-main-dark.png`, `screenshot-main-light.png`, and `screenshot-expanded-fifa23-dark.png`.
- **FIX 3 (`Consolas` Monospace Numerals + `2px` Proportion Bar + Hover `%` Share)**: **PASS** — Verified vertical numeral alignment, `2px` proportion bars, and `51,372,195,955 bytes · 31.84% share` tooltip in `screenshot-hover-size-dark.png` (as well as full non-clipped `used` / `free` drive pod labels after adjusting split width).
- **FIX 4 (Word-Boundary Capped Basis Lines `…`)**: **PASS** — Verified in `screenshot-expanded-fifa23-dark.png` (`eaanticheat.gameservicelauncher.exe…`, `eaappinstaller_installscript.vdf…`) and unit test.
