use std::fs::File;
use std::path::Path;

pub use y_core::pe::PeVersionStrings;

/// Reads PE `VS_VERSION_INFO` string fields (`CompanyName`, `ProductName`, `FileDescription`)
/// from an on-disk binary via bounded seek-only I/O.
#[must_use]
pub fn read_pe_version_strings(path: impl AsRef<Path>) -> PeVersionStrings {
    let Ok(mut file) = File::open(path.as_ref()) else {
        return PeVersionStrings::default();
    };
    y_core::pe::parse_pe_version_strings(&mut file)
}
