use crate::discovery::normalize_path_key;
use crate::model::{LocationQualifier, ZoneKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinWindowsDefaultLocation {
    pub name: &'static str,
    pub relative_path: &'static str,
}

impl BuiltinWindowsDefaultLocation {
    #[must_use]
    pub fn resolved_path(self, system_drive_letter: char) -> String {
        format!(
            "{}:\\{}",
            system_drive_letter.to_ascii_uppercase(),
            self.relative_path
        )
    }
}

pub const BUILTIN_WINDOWS_DEFAULT_LOCATIONS: &[BuiltinWindowsDefaultLocation] = &[
    BuiltinWindowsDefaultLocation {
        name: "Program Files",
        relative_path: "Program Files",
    },
    BuiltinWindowsDefaultLocation {
        name: "Program Files (x86)",
        relative_path: "Program Files (x86)",
    },
    BuiltinWindowsDefaultLocation {
        name: "ProgramData",
        relative_path: "ProgramData",
    },
    BuiltinWindowsDefaultLocation {
        name: "Users",
        relative_path: "Users",
    },
    BuiltinWindowsDefaultLocation {
        name: "WindowsApps",
        relative_path: r"Program Files\WindowsApps",
    },
];

#[must_use]
pub fn is_builtin_windows_default_name(seg_lower: &str) -> bool {
    BUILTIN_WINDOWS_DEFAULT_LOCATIONS
        .iter()
        .any(|loc| loc.name.eq_ignore_ascii_case(seg_lower))
}

#[must_use]
pub fn classify_zone(
    path: &str,
    is_install_location: bool,
) -> (ZoneKind, Option<LocationQualifier>) {
    let p = normalize_path_key(path);

    let is_program_data = p.contains(":\\programdata\\") || p.ends_with(":\\programdata");
    let is_roaming = p.contains("\\appdata\\roaming\\") || p.ends_with("\\appdata\\roaming");

    // GREEN: safely regenerable caches & temp directories.
    let is_green = p.ends_with("\\temp")
        || p.contains("\\appdata\\local\\temp")
        || p.contains("\\d3dscache")
        || p.contains("\\nvidia\\dxcache")
        || p.contains("\\nvidia\\glcache")
        || p.contains("\\amd\\dxcache")
        || p.contains("\\amd\\dxccache")
        || p.contains("\\amd\\vkcache")
        || p.contains("\\crashdumps")
        || p.contains("\\squirreltemp")
        || p.contains("\\epicinstallertemp")
        || p.contains("\\programdata\\package cache")
        || p.contains("\\appdata\\local\\package cache")
        || p.contains("\\shadercache")
        || p.contains("\\local\\cache")
        || p.contains("\\inetcache")
        || p.contains("\\feedscache")
        || p.contains("\\fontcache");

    if is_green {
        let qual = if is_program_data {
            Some(LocationQualifier::MachineShared)
        } else {
            None
        };
        return (ZoneKind::Green, qual);
    }

    // BODY: program install bodies & per-user Local\Programs.
    let is_body = is_install_location
        || p.contains(":\\program files\\")
        || p.contains(":\\program files (x86)\\")
        || p.contains("\\appdata\\local\\programs\\")
        || p.contains("\\steamapps\\common\\")
        || (p.contains(":\\xboxgames\\") && !p.ends_with("\\gamesave"))
        || p.contains(":\\gog games\\")
        || p.contains(":\\riot games\\")
        || p.contains(":\\apps\\");

    if is_body {
        let qual = if is_program_data {
            Some(LocationQualifier::MachineShared)
        } else {
            None
        };
        return (ZoneKind::Body, qual);
    }

    // RED: real user/app data, Store Packages, Roaming configs, ProgramData shared state.
    let qual = if is_program_data {
        Some(LocationQualifier::MachineShared)
    } else if is_roaming {
        Some(LocationQualifier::RoamingConfig)
    } else {
        None
    };

    (ZoneKind::Red, qual)
}
