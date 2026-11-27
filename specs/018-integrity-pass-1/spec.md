# Feature Specification: `integrity-pass-1`

## Summary
Eliminate over-merging of independently installable/uninstallable versioned product families (Visual C++ Redistributables, .NET runtimes/Desktop Runtimes/SDKs, Windows SDKs, Python/Java) and close dependency-detection gaps across all previously zero-dependency applications while surfacing reverse dependency counts prominently on shared runtimes.

## STEP 0 — Confirmed Diagnosis
1. `crates/y-core/src/discovery.rs` (`normalize_app_token` lines `259–268`, `build_registry_catalog` lines `347–399`): `normalize_app_token` strips version digits and architecture tokens (`x64`, `x86`, `64`, `version`), collapsing every `Microsoft Visual C++ <year> ... Redistributable` record to `"microsoft visual c redistributable"` (`Microsoft Windows Desktop Runtime` to `"microsoft windows desktop runtime"`, `Microsoft .NET SDK` to `"microsoft net sdk"`) and merging all records into the first enumerated entry.
2. `crates/y-core/src/attribution.rs` (`find_by_token` lines `719–761`): `ProgramData\Package Cache` folders match `{GUID}` -> `guid_to_app_name` and route by coarse name substring (`"visual c++"` / `".net"` / `"windows software development kit"`) to `find_app_matching`, sending all payloads into the single merged app (and even routing `.NET Runtime 8.0.21` caches into `.NET SDK 7.0.407`) without consulting the cache folder's version or architecture.

## Scope & Requirements
1. **FIX A — Split Versioned Families**:
   - Independently installable/uninstallable versioned families (`Visual C++ Redistributable`, `Microsoft Windows Desktop Runtime`, `Microsoft .NET Runtime`, `Microsoft ASP.NET Core Runtime`, `Microsoft .NET SDK`, `Windows Software Development Kit`, side-by-side `Python 3.x`, `Java`) keep one distinct canonical row per `(product, version, arch)` carrying its real display name and version.
   - `HKCU-64` / `HKCU-32` Uninstall reflection mirrors sharing the same non-redirected subkey deduplicate cleanly.
   - `ProgramData\Package Cache` folders route first by bundle `uninstall_guids` match, then by `(version, arch)` extracted from `{GUID}v<ver>` + `guid_to_app_name` — conserving exact family byte totals (`0 B` loss/drift) while eliminating cross-version misattribution.
   - Scope guard stays intact: `system_component`, `parent_key_name`, `release_type` (`update`/`hotfix`/`security`) guards remain enforced, and shared-folder/driver-suite/companion pairs stay merged.
2. **FIX B — Dependency Coverage Sweep**:
   - Version-specific targets for `VC++` (`2008`, `2010`, `2012`, `2013`, `v14`), `.NET` (`6`, `7`, `8`, `9`, `10` via `.deps.json` / `.runtimeconfig.json`, plus `dotnet-runtime` fallback), and documented shared runtimes (`OpenAL`, `PhysX`, `Python Runtime`, `WinUI XAML`, `.NET Native`, etc.) authored strictly from official Microsoft/vendor documentation.
   - `dep_scan.rs` traverses up to depth 3 into non-cache subdirectories, falls back to scanning non-`Body` app directories when `body_roots` is empty, raises the PE header-seek file-size guard to `512 MiB`, prioritizes `.exe` headers over `.dll` headers, and detects UWP `*_app.dll` VC++ imports and `.runtimeconfig.json` TFMs.
   - Generic installer manifest requests (`-req:vc`) coalesce onto an already-proven `vcpp-*` target so `EA SPORTS™ FIFA 23` maintains exact `51,372,195,955 B` with `7` dependencies.
   - Shared runtimes display a prominent first-class `"Used by N apps"` section directly in the expanded drawer (`ui.rs`) with an overlay trigger for consumer inspection.
