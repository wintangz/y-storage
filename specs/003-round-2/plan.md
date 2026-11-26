# Implementation Plan: `y-storage` Round 2

**Branch**: `003-round-2` | **Date**: 2026-11-26 | **Spec**: [spec.md](spec.md)

## Summary

Add forward and reverse dependency discovery (`dependencies`), tighten `Unattributed` categorization (`attribution-coverage-2`), and execute non-circular Windows Explorer Folder Properties ground-truth verification (`ground-truth`), keeping `y-storage` strictly read-only with minimal UI additions and zero restyling.

## Architecture Additions

1. **`data/dependency_catalog.toml`**: Authored catalog loaded at runtime (with compile-time fallback) defining shared runtimes (`vcpp-runtime`, `directx-runtime`, `dotnet-runtime`, `webview2-runtime`), major launchers (`steam`, `ea-app`, `epic-games`, `gog-galaxy`, `battlenet`, `riot-client`, `xbox`), anti-cheats (`ea-anticheat`, `easy-anticheat`, `battleye`, `vanguard`, `denuvo`), DLL->runtime import rules, folder fingerprints, and curated app rules.
2. **`crates/y-core/src/pe.rs`**: Seek-only PE32 / PE32+ `IMAGE_IMPORT_DESCRIPTOR` parser reading imported DLL names in `< 1 ms` per binary without reading full file payloads.
3. **`crates/y-platform-windows/src/services.rs`**: `HKLM\SYSTEM\CurrentControlSet\Services` enumerator extracting `ImagePath`, `DisplayName`, `Type`, and `DependOnService`.
4. **`crates/y-core/src/dependencies.rs` & `crates/y-platform-windows/src/dep_scan.rs`**: Four-source forward resolver (1. PE/deps.json/AppxManifest/installerdata `Proven`, 2. Windows Services/Drivers `Proven`, 3. Fingerprints & Steam `appmanifest_*.acf` `Proven`/`Inferred`, 4. Curated catalog `Inferred`) + graph inversion for reverse view (`dependent_count` and `binary_importer_count`), with strict non-inflation of `AttributedApp.total_bytes`.
5. **`crates/y-core/src/attribution.rs`**: `Shared & system caches (GREEN)` pinned group, standalone top-level user-profile dot-folders, standalone `flutter` SDK entry, and verified non-forced token fixes (`Display.PhysX`, `NVIDIA App`, `ollama app.exe`, `oice_16_*`, `BattlEye`).
6. **`apps/y-storage/src/ui.rs`**: Minimal additions only — `"Dependencies"` header view toggle, expandable `"Dependencies"` section inside app cards, `ActiveView::Dependencies` reverse view, and `Shared & system caches (GREEN)` pinned group.
