# Implementation Plan: `y-storage` V1

**Branch**: `002-y-storage-v1` | **Date**: 2026-11-26 | **Spec**: [spec.md](spec.md)

## Summary

Build `y-storage` as a single standalone read-only Windows desktop executable (`y-storage.exe`) in Rust (`rustc 1.100.0`, `x86_64-pc-windows-msvc`), organized into three workspace crates: `y-core` (OS-independent discovery normalization, container decomposition, attribution, zone tagging, scan history, config/cache), `y-platform-windows` (Win32 registry reader, non-elevated `FindFirstFileExW` low-I/O scanner, drive & shell-folder discovery), and `y-storage` (background scan worker, pure-CPU `egui 0.29` GDI `SoftwarePainter`, PerMonitorV2 Win32 window host, and pastel dark/light theme matching `z-controller`).

## Technical Context

- **Language/Version**: Rust `1.100.0` (Edition 2024, `x86_64-pc-windows-msvc`)
- **Primary Dependencies**: `windows-sys = "=0.59.0"`, `egui = "0.29"` (`default-features = false`, `default_fonts`), `serde`, `serde_json`, `toml`, `thiserror`, `bitflags`
- **Persistent State**: `%APPDATA%\y-storage\config.toml`, `%LOCALAPPDATA%\y-storage\{scan_history.json,last_scan.json,folder_cache.json}`
- **Scanner Choice**: Non-elevated multi-threaded `FindFirstFileExW(FindExInfoBasic, FIND_FIRST_EX_LARGE_FETCH)` at `THREAD_MODE_BACKGROUND_BEGIN`, verified at `8.797 s` in `feasibility-spike` (`<= 150 s` gate, avoiding NTFS MFT/USN admin UAC trade-off).
- **Testing**: `cargo test --workspace` + live V1 acceptance (`EA App + FIFA 23`, `Visual Studio`, `Discord`, `C:\` reconciliation, live GUI memory & screenshot inspection)

## Project Structure

```text
y:\Repos\y-storage\
├── Cargo.toml
├── icon.svg
├── crates/
│   ├── y-core/                 # app-discovery, attribution-zones, scan-history, config, cache, tree
│   └── y-platform-windows/     # registry, paths, drive-scanner, spike_runner
└── apps/
    └── y-storage/              # storage-ui (sw_painter, win32_host, ui, icon_gen, main)
```
