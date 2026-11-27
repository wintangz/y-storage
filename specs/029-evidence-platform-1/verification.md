# Verification 029 — `evidence-platform-1`

**Scope:** `crates/y-platform-windows` + minimal `FileDescription` extension in `crates/y-core/src/pe.rs` (**PLATFORM ONLY** — no attribution, model, scanner, or UI behaviour changes).

## 1. Capabilities Implemented & Logged Assumptions

1. **Authenticode Signer Extraction** (`crates/y-platform-windows/src/authenticode.rs` -> `extract_authenticode_signer_org`):
   - Extracts the embedded PKCS#7 signer certificate via `CryptQueryObject(CERT_QUERY_OBJECT_FILE, ..., CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED, CERT_QUERY_FORMAT_FLAG_BINARY)` + `CryptMsgGetParam(CMSG_SIGNER_INFO_PARAM)` + `CertFindCertificateInStore(CERT_FIND_SUBJECT_CERT)` + `CertGetNameStringW(CERT_NAME_ATTR_TYPE, szOID_ORGANIZATION_NAME)`.
   - **Assumption (offline / catalog-only):** Fully offline by design — reads embedded CMS metadata directly from disk without `WinVerifyTrust`, CRL/OCSP, or network calls. Unsigned binaries and Windows catalog-only binaries without an embedded PKCS#7 signature return `None`.
   - **Assumption (normalization & fallback):** Strips control characters and collapses internal whitespace runs; falls back to `CERT_NAME_SIMPLE_DISPLAY_TYPE` (`CN`) only if `O` is absent.

2. **PE Version-Resource Reader** (`crates/y-core/src/pe.rs` + `crates/y-platform-windows/src/pe_version.rs` -> `read_pe_version_strings`):
   - Verified and reused the existing `companion-reunion-global` bounded (`<= 64 KB`) seek-only `VS_VERSION_INFO` parser in `crates/y-core/src/pe.rs`.
   - Extended `PeVersionStrings` solely with `pub file_description: Option<String>` populated alongside `company_name` and `product_name`; preserved existing `is_generic_runtime_or_engine_stub` filtering unchanged.

3. **App Paths Enumeration** (`crates/y-platform-windows/src/app_paths.rs` -> `enumerate_app_paths`):
   - Enumerates `SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths` across `HKLM` (64-bit & 32-bit registry views) and `HKCU`, expanding environment variables, unquoting, normalizing Windows paths, deduplicating, and returning sorted `AppPathEntry { exe_name, target_path }`.
   - **Assumption:** Subkeys whose default registry value is empty (e.g. legacy stubs without a path) are omitted.
   - Live check result on this machine: **48** entries (including `chrome.exe -> C:\Program Files\Google\Chrome\Application\chrome.exe`, `AppleMusic.exe`, `artcraft-launcher.exe`).

4. **Start Menu `.lnk` Resolution** (`crates/y-platform-windows/src/start_menu.rs` -> `resolve_lnk_target_exe`, `enumerate_start_menu_shortcuts`):
   - Pure-Rust `[MS-SHLLINK]` parser (`<= 256 KB` bounded read) reading `LinkInfo` (Unicode/ANSI local base path + common suffix), `StringData` (`RelativePath`), and `ExtraData` (`EnvironmentVariableDataBlock`, expanding env vars).
   - Scans `%ProgramData%\Microsoft\Windows\Start Menu\Programs` and `%APPDATA%\Microsoft\Windows\Start Menu\Programs` recursively (depth <= 8, skipping symlinks/reparse points), returning `StartMenuShortcut { display_name, target_path, shortcut_path }`.
   - **Assumption:** Only shortcuts resolving to an `.exe` target are returned.
   - Live check result on this machine: **129** shortcuts resolved (e.g. `About Java`, `Ahk2Exe`, `App Recovery`).

5. **Service `ImagePath` List** (`crates/y-platform-windows/src/services.rs` -> `enumerate_service_image_paths`):
   - Exposed the existing `enumerate_windows_services()` scanner via `enumerate_service_image_paths() -> Vec<ServiceImagePath>` (`service_name`, `display_name`, `image_path`), omitting entries with an empty normalized `ImagePath`.
   - Live check result on this machine: **796** service `ImagePath` entries returned.

## 2. Live Spot-Check Table (`evidence_platform_tests`)

Compared against Windows File Properties / `Get-AuthenticodeSignature` and `(Get-Item <path>).VersionInfo`:

| Executable | Windows Signer / `VersionInfo` Properties | Extracted Signer Org | Extracted `CompanyName` | Extracted `ProductName` | Extracted `FileDescription` | Result |
|---|---|---|---|---|---|---|
| `C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe` | Signed (`CN=Microsoft Corporation, O=Microsoft Corporation, ...`), Company=`Microsoft Corporation`, Product=`Microsoft Edge`, Desc=`Microsoft Edge` | `"Microsoft Corporation"` | `"Microsoft Corporation"` | `"Microsoft Edge"` | `"Microsoft Edge"` | **PASS** |
| `C:\Program Files\Google\Chrome\Application\chrome.exe` | Signed (`CN=Google LLC, O=Google LLC, ...`), Company=`Google LLC`, Product=`Google Chrome`, Desc=`Google Chrome` | `"Google LLC"` | `"Google LLC"` | `"Google Chrome"` | `"Google Chrome"` | **PASS** |
| `Y:\Apps\nodejs\node.exe` | Signed (`CN=OpenJS Foundation, O=OpenJS Foundation, ...`), Company=`Node.js`, Product=`Node.js`, Desc=`Node.js JavaScript Runtime` | `"OpenJS Foundation"` | `"Node.js"` | `"Node.js"` | `"Node.js JavaScript Runtime"` | **PASS** |
| `Y:\Apps\y-storage\y-storage.exe` *(unsigned)* | `NotSigned` (empty `SignerCertificate`) | `None` | — | — | — | **PASS** |
| `C:\Windows\System32\notepad.exe` *(catalog-only bonus check)* | `IsOSBinary=True` (catalog-signed, no embedded PKCS#7) | `None` | — | — | — | **PASS** |

## 3. Workspace Test Suite (`cargo test --workspace`)

Ran `cargo test --workspace` after the final code edit with **0 compiler warnings** (`EXIT=0`):

- **`y-core`** — **63 / 63 passed**:
  - `src/lib.rs` unit tests: **3 / 3 passed**
  - `core_tests`: **6 / 6 passed**
  - `round2_tests`: **6 / 6 passed**
  - `round3_tests`: **8 / 8 passed**
  - `round4_tests`: **5 / 5 passed**
  - `round5_tests`: **11 / 11 passed**
  - `round6_tests`: **9 / 9 passed**
  - `round7_tests`: **7 / 7 passed**
  - `round8_tests`: **8 / 8 passed**
- **`y-platform-windows`** — **9 / 9 passed**:
  - `src/lib.rs` unit tests: **1 / 1 passed**
  - `src/bin/spike_runner.rs`: **0 / 0**
  - `evidence_platform_tests`: **5 / 5 passed**
  - `windows_scanner_tests`: **3 / 3 passed**
- **`y-storage`** — **41 / 41 passed**:
  - `src/main.rs` unit tests: **41 / 41 passed**
- **Workspace Total**: **113 / 113 passed**, **0 failed**, **0 ignored**, **0 warnings**.
