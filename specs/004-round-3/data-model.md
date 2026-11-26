# Data Model Additions: `y-storage` Round 3

## `y_core::model::ScanReport`
- `catalog_source: String` (`#[serde(default)]`) — Human-readable origin of the dependency catalog used during the scan (`"embedded"`, `"override: <path>"`, or `"embedded (fallback after invalid override: <path>)"`).
- `warnings: Vec<String>` (`#[serde(default)]`) — Non-fatal scan warnings surfaced in CLI output, log file, and UI (such as invalid external catalog override fallback notices).

## `y_core::dependencies`
- `CatalogSourceKind`: `Embedded | ExternalOverride | EmbeddedFallbackAfterError`
- `CatalogLoadResult`: `{ catalog: DependencyCatalog, source_kind: CatalogSourceKind, source_label: String, override_path: Option<PathBuf>, warning: Option<String> }`

## Attribution Basis & Labels (`attribution-coverage-3`)
- Versioned products under publisher folders (`Local|Roaming|LocalLow\<Publisher>\<Product><version>`) and `<X>Data` folders attach via `AttributionBasis::Token` (preferring an existing installed `InstalledApp`, or creating one shared `product:<stem>` / `app:<stem>` bucket so multiple versions sum into one total).
- `Local\ms-playwright` attaches as `"Playwright"` (`StandaloneFolder`, `ZoneKind::Green`, publisher note `"Standalone toolchain · regenerable dev-tool cache (downloaded browser binaries)"`).
- `Local\GitKrakenCLI` attaches as `"GitKraken CLI"` (`StandaloneFolder`, `ZoneKind::Body`, publisher note `"Standalone CLI toolchain (gk.exe)"`).
