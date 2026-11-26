# Third-Party Licenses

`y-storage` statically links open-source Rust crates from crates.io. This file summarizes their licenses for distribution alongside `y-storage.exe`.

| Crate | License | Purpose |
|---|---|---|
| `egui`, `epaint`, `emath`, `ecolor`, `epaint_default_fonts` | MIT OR Apache-2.0 | Retained/immediate-mode UI layout and font mesh generation (`default-features = false`) |
| `ab_glyph`, `ab_glyph_rasterizer`, `owned_ttf_parser`, `ttf-parser` | Apache-2.0 / MIT | Pure-Rust TTF glyph rasterization |
| `windows-sys`, `windows-targets`, `windows_x86_64_msvc`, `windows-link` | MIT OR Apache-2.0 | Windows API bindings (`FindFirstFileExW`, Registry, GDI, User32, ProcessStatus) |
| `serde`, `serde_derive`, `serde_json`, `serde_core` | MIT OR Apache-2.0 | Scan report, folder cache, and scan history serialization |
| `toml`, `toml_edit`, `toml_datetime`, `toml_write`, `winnow`, `serde_spanned` | MIT OR Apache-2.0 | Persistent TOML settings (`%APPDATA%\y-storage\config.toml`) |
| `thiserror`, `ahash`, `hashbrown`, `indexmap`, `parking_lot`, `once_cell`, `memchr`, `itoa`, `zmij`, `zerocopy`, `smallvec`, `cfg-if`, `scopeguard` | MIT OR Apache-2.0 | Core Rust utilities & lock primitives |
