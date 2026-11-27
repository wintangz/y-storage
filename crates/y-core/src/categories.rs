use std::path::Path;

use serde::Deserialize;

use crate::discovery::{AppTokenMatchKind, contains_whole_token_sequence, match_app_predicate};
use crate::model::{AppCategory, AttributedApp, ScanReport};

pub const EMBEDDED_APP_CATEGORIES: &str = include_str!("../../../data/app_categories.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryMappingEntry {
    pub category: AppCategory,
    #[serde(default)]
    pub app_matches: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryKeywordRule {
    pub category: AppCategory,
    #[serde(default)]
    pub name_contains: Vec<String>,
    #[serde(default)]
    pub publisher_contains: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CategoryCatalog {
    #[serde(default)]
    pub mappings: Vec<CategoryMappingEntry>,
    #[serde(default)]
    pub rules: Vec<CategoryKeywordRule>,
}

impl CategoryCatalog {
    pub fn parse_and_validate(raw: &str) -> Result<Self, String> {
        let cat = toml::from_str::<Self>(raw).map_err(|e| format!("invalid categories TOML: {e}"))?;
        if cat.mappings.is_empty() && cat.rules.is_empty() {
            return Err("categories catalog defines neither mappings nor rules".into());
        }
        Ok(cat)
    }

    pub fn load_for_exe_dir(exe_dir: Option<&Path>) -> Self {
        let embedded = Self::parse_and_validate(EMBEDDED_APP_CATEGORIES)
            .expect("embedded data/app_categories.toml must parse");
        if let Some(dir) = exe_dir {
            for cand in [
                dir.join("app_categories.toml"),
                dir.join("data").join("app_categories.toml"),
            ] {
                if cand.exists() {
                    match std::fs::read_to_string(&cand)
                        .map_err(|e| e.to_string())
                        .and_then(|raw| Self::parse_and_validate(&raw))
                    {
                        Ok(cat) => return cat,
                        Err(err) => {
                            eprintln!(
                                "WARN [y-storage]: invalid app_categories override '{}': {err}; using embedded.",
                                cand.display()
                            );
                        }
                    }
                }
            }
        }
        embedded
    }

    pub fn load_from_current_exe() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        Self::load_for_exe_dir(exe_dir.as_deref())
    }
}

fn is_store_system_runtime_name(name_lower: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "store: microsoft.windowsappruntime",
        "store: microsoftcorporationii.winappruntime",
        "store: microsoft.vclibs",
        "store: microsoft.ui.xaml",
        "store: microsoft.net.native",
        "store: microsoft.directxruntime",
        "store: microsoft.gamingservices",
        "store: microsoft.advertising.xaml",
        "store: microsoft.widgetsplatformruntime",
        "store: microsoft.applicationcompatibilityenhancements",
    ];
    PREFIXES.iter().any(|p| name_lower.starts_with(p))
}

/// Classifies an app using strict evidence precedence:
/// (a) authored [[mappings]] -> (b) hard filesystem/no-touch signals -> (c) [[rules]] keyword match -> Uncategorized.
pub fn classify_app<F>(
    app: &AttributedApp,
    catalog: &CategoryCatalog,
    has_game_manifest_dir: F,
) -> (AppCategory, String)
where
    F: Fn(&str) -> bool,
{
    let name_lower = app.display_name.to_ascii_lowercase();

    // (a) Authored explicit mapping (highest precedence): prefer exact full-token match over whole-word slice
    let mut slice_mapping: Option<&CategoryMappingEntry> = None;
    for mapping in &catalog.mappings {
        for m in &mapping.app_matches {
            let kind = match_app_predicate(&app.display_name, m)
                .into_iter()
                .chain(match_app_predicate(&app.id, m))
                .max();
            match kind {
                Some(AppTokenMatchKind::ExactFullToken) => {
                    return (mapping.category, format!("mapping: {}", mapping.reason));
                }
                Some(AppTokenMatchKind::WholeWordSlice) if slice_mapping.is_none() => {
                    slice_mapping = Some(mapping);
                }
                _ => {}
            }
        }
    }
    if let Some(mapping) = slice_mapping {
        return (mapping.category, format!("mapping: {}", mapping.reason));
    }

    // (b) Hard signals: game library install tree OR system/driver/service/runtime NO-TOUCH identity
    let in_steamapps = app.install_location.as_deref().is_some_and(|p| {
        p.to_ascii_lowercase().contains("\\steamapps\\common\\")
    }) || app
        .items
        .iter()
        .any(|it| it.path.to_ascii_lowercase().contains("\\steamapps\\common\\"));
    if in_steamapps {
        return (
            AppCategory::Games,
            "signal: installed inside Steam game library (steamapps\\common)".into(),
        );
    }

    let has_launcher_manifest = app
        .install_location
        .as_deref()
        .is_some_and(&has_game_manifest_dir)
        || app.items.iter().any(|it| has_game_manifest_dir(&it.path));
    if has_launcher_manifest {
        return (
            AppCategory::Games,
            "signal: Epic/GOG game manifest in install directory".into(),
        );
    }

    if let Some(nt) = app.no_touch {
        return (
            AppCategory::SystemAndRuntimes,
            format!("signal: {} component", nt.label()),
        );
    }
    if app.id.starts_with("sys:")
        || app.id.starts_with("synth:")
        || is_store_system_runtime_name(&name_lower)
    {
        return (
            AppCategory::SystemAndRuntimes,
            "signal: OS runtime / system framework package".into(),
        );
    }

    // (c) Publisher / name keyword rules from catalog (whole-word token boundary)
    for rule in &catalog.rules {
        let name_hit = !rule.name_contains.is_empty()
            && rule
                .name_contains
                .iter()
                .any(|kw| contains_whole_token_sequence(&app.display_name, kw));
        let pub_hit = !rule.publisher_contains.is_empty()
            && rule.publisher_contains.iter().any(|kw| {
                app.publishers
                    .iter()
                    .any(|p| contains_whole_token_sequence(p, kw))
            });
        if name_hit || pub_hit {
            return (rule.category, format!("rule: {}", rule.reason));
        }
    }

    (
        AppCategory::Uncategorized,
        "none: insufficient evidence for classification".into(),
    )
}

pub fn classify_report_apps<F>(report: &mut ScanReport, has_game_manifest_dir: F)
where
    F: Fn(&str) -> bool,
{
    let catalog = CategoryCatalog::load_from_current_exe();
    for app in &mut report.apps {
        let (cat, basis) = classify_app(app, &catalog, &has_game_manifest_dir);
        app.category = cat;
        app.category_basis = basis;
    }
}
