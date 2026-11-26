# Data Model Additions: `y-storage` Round 2

- **`DependencyKind`**: `PlatformLauncher`, `AntiCheatServiceDriver`, `SharedRuntime`, `SharedData`.
- **`DependencyConfidence`**: `Proven` (PE import / `.deps.json` / manifest / Windows service or driver), `Inferred` (folder fingerprint / curated catalog).
- **`DependencyRecord`**: `target_id`, `target_name`, `kind`, `present`, `own_size_bytes: Option<u64>`, `shared`, `basis`, `confidence`, `imported_by_binary`.
- **`ReverseDependencyEntry`**: `target_id`, `target_name`, `kind`, `present`, `own_size_bytes`, `dependent_count`, `binary_importer_count`, `consumers: Vec<ReverseConsumer>`.
- **`ScanReport` additions**: `shared_caches: Vec<BreakdownItem>`, `shared_caches_total_bytes: u64`, `reverse_dependencies: Vec<ReverseDependencyEntry>` (all `#[serde(default)]` for backwards compatibility).
- **`AttributionBasis::StandaloneFolder`**: Marks user-profile dot-folders (e.g. `.gemini`) and standalone SDKs (`flutter`).
