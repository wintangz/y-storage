# Implementation Plan: `feasibility-spike`

**Branch**: `001-feasibility-spike` | **Date**: 2026-11-26 | **Spec**: [spec.md](spec.md)

## Summary

Implement `crates/y-core` (pure-Rust domain model, container decomposition, attribution engine, zone classification, scan history, config/cache) and `crates/y-platform-windows` (Win32 registry reader, `FindFirstFileExW` low-I/O scanner, drive/shell-folder resolver, and `spike_runner` binary) to gather exact live measurements on the dev machine before building `storage-ui`.

## Technical Context

- **Language/Version**: Rust `1.100.0` (`x86_64-pc-windows-msvc`, Edition 2024)
- **Dependencies**: `windows-sys = "=0.59.0"`, `serde`, `serde_json`, `toml`, `thiserror`, `bitflags`
- **Performance Gate**: Default-scope walk (`C:\` minus `C:\Windows` + secondary `InstallLocation`s) target `<= 150s` without admin elevation.

## Project Structure for Spike

```text
crates/
├── y-core/                     # Zero-Win32 domain types, attribution, zones, tree partitioning
└── y-platform-windows/         # Registry Uninstall enum, FindFirstFileExW low-IO scanner
    └── src/bin/
        └── spike_runner.rs     # Executes items (1), (2), (3) and prints proving telemetry
```
