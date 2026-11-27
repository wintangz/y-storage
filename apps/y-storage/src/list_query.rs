use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use y_core::config::{
    AppFilterChip, AppSortMode, DepStateFilter, FacetFilters, FootprintZoneFilter,
    GrowthFacetFilter, NO_PUBLISHER_FACET_KEY, SizeBandFilter, SortDirection,
    UnattributedFacetFilters, UnattributedLocationZone,
};
use y_core::model::{
    AppCategory, AttributedApp, AttributionBasis, GrowthDelta, ScanReport, UnattributedItem,
    ZoneKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnattributedSortMode {
    #[default]
    SizeDesc,
    SizeAsc,
    NameAsc,
    NameDesc,
}

impl UnattributedSortMode {
    pub const ALL_OPTIONS: [Self; 4] = [
        Self::SizeDesc,
        Self::SizeAsc,
        Self::NameAsc,
        Self::NameDesc,
    ];

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::SizeDesc => "Size ↓",
            Self::SizeAsc => "Size ↑",
            Self::NameAsc => "Name A→Z",
            Self::NameDesc => "Name Z→A",
        }
    }
}

fn unattrib_leaf_lower(path: &str) -> String {
    let trimmed = path.trim_end_matches('\\');
    trimmed
        .rsplit('\\')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase()
}

/// View-only substring filter & sort over `ScanReport.unattributed` without modifying report items or totals.
#[cfg(test)]
#[must_use]
pub fn filter_and_sort_unattributed<'a>(
    items: &'a [UnattributedItem],
    query: &str,
    sort: UnattributedSortMode,
) -> (Vec<(usize, &'a UnattributedItem)>, u64) {
    filter_and_sort_unattributed_with_facets(
        items,
        query,
        &UnattributedFacetFilters::default(),
        sort,
    )
}

/// View-only search, multi-facet (`Drive`, `Size`, `Location zone`) AND-filter, and 4-mode sort
/// over `ScanReport.unattributed`, preserving original report items and group totals unchanged.
#[must_use]
pub fn filter_and_sort_unattributed_with_facets<'a>(
    items: &'a [UnattributedItem],
    query: &str,
    facets: &UnattributedFacetFilters,
    sort: UnattributedSortMode,
) -> (Vec<(usize, &'a UnattributedItem)>, u64) {
    let q = query.trim().to_ascii_lowercase();
    let mut out: Vec<(usize, &'a UnattributedItem)> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| {
            if !q.is_empty() && !it.path.to_ascii_lowercase().contains(&q) {
                return false;
            }
            if let Some(drv) = facets.drive {
                if it.drive_letter.to_ascii_uppercase() != drv.to_ascii_uppercase() {
                    return false;
                }
            }
            if facets.size_band != SizeBandFilter::All && !facets.size_band.matches_bytes(it.bytes) {
                return false;
            }
            if facets.location_zone != UnattributedLocationZone::All
                && UnattributedLocationZone::classify_path(&it.path) != facets.location_zone
            {
                return false;
            }
            true
        })
        .collect();
    match sort {
        UnattributedSortMode::SizeDesc => {
            // Preserve exact report ordering (bytes desc, path tie-break).
        }
        UnattributedSortMode::SizeAsc => {
            out.sort_by(|(_, a), (_, b)| {
                a.bytes
                    .cmp(&b.bytes)
                    .then_with(|| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()))
            });
        }
        UnattributedSortMode::NameAsc => {
            out.sort_by(|(_, a), (_, b)| {
                unattrib_leaf_lower(&a.path)
                    .cmp(&unattrib_leaf_lower(&b.path))
                    .then_with(|| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()))
                    .then_with(|| b.bytes.cmp(&a.bytes))
            });
        }
        UnattributedSortMode::NameDesc => {
            out.sort_by(|(_, a), (_, b)| {
                unattrib_leaf_lower(&b.path)
                    .cmp(&unattrib_leaf_lower(&a.path))
                    .then_with(|| b.path.to_ascii_lowercase().cmp(&a.path.to_ascii_lowercase()))
                    .then_with(|| b.bytes.cmp(&a.bytes))
            });
        }
    }
    let subtotal = out.iter().map(|(_, it)| it.bytes).sum();
    (out, subtotal)
}

/// Extracts the signed growth delta vs the previous scan, returning `None` for apps with no prior baseline (`NoBaseline` or `New`).
#[must_use]
pub const fn growth_delta_bytes(growth: GrowthDelta) -> Option<i64> {
    match growth {
        GrowthDelta::Changed(d) => Some(d),
        GrowthDelta::NoBaseline | GrowthDelta::New(_) => None,
    }
}

/// Returns `true` for standalone/toolchain/user-profile dot-folder entries (never registry apps).
#[must_use]
pub fn is_standalone_entry(app: &AttributedApp) -> bool {
    app.bases.contains(&AttributionBasis::StandaloneFolder)
        || app.id.starts_with("standalone:")
        || app.id.starts_with("dot:")
}

/// Returns the active single-drive measurement target when either `facets.drive`
/// OR the `HasSystemDriveFootprint` chip alone restricts the list to one drive.
#[must_use]
pub fn active_per_drive_mode(
    chip: AppFilterChip,
    facets: &FacetFilters,
    system_drive_letter: char,
) -> Option<char> {
    let sys = system_drive_letter.to_ascii_uppercase();
    let facet_drv = facets.drive.map(|d| d.to_ascii_uppercase());
    let has_c_chip = chip == AppFilterChip::HasSystemDriveFootprint;
    match (facet_drv, has_c_chip) {
        (Some(d), false) => Some(d),
        (None, true) => Some(sys),
        (Some(d), true) if d == sys => Some(sys),
        _ => None,
    }
}

/// Pure predicate checking whether an app matches the single-select filter chip.
#[must_use]
pub fn app_matches_chip(
    app: &AttributedApp,
    chip: AppFilterChip,
    system_drive_letter: char,
) -> bool {
    app_matches_chip_for_mode(app, chip, system_drive_letter, None)
}

#[must_use]
fn app_matches_chip_for_mode(
    app: &AttributedApp,
    chip: AppFilterChip,
    system_drive_letter: char,
    per_drive: Option<char>,
) -> bool {
    match chip {
        AppFilterChip::All => true,
        AppFilterChip::Grew => {
            let g = per_drive.map_or(app.growth, |dl| app.growth_for_drive(dl));
            matches!(g, GrowthDelta::Changed(d) if d > 0)
        }
        AppFilterChip::NoTouch => app.no_touch.is_some(),
        AppFilterChip::Standalone => is_standalone_entry(app),
        AppFilterChip::HasSystemDriveFootprint => app.bytes_on_drive(system_drive_letter) > 0,
    }
}

/// Pure predicate checking whether an app matches the case-insensitive search query on name, publisher, category, or path.
#[must_use]
pub fn app_matches_search(app: &AttributedApp, search_query: &str) -> bool {
    let q = search_query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    let name_hit = app.display_name.to_ascii_lowercase().contains(&q);
    if name_hit {
        return true;
    }
    if app.publishers.is_empty() && "(no publisher)".contains(&q) {
        return true;
    }
    let pub_hit = app
        .publishers
        .iter()
        .any(|p| p.to_ascii_lowercase().contains(&q));
    if pub_hit {
        return true;
    }
    if app.category.label().to_ascii_lowercase().contains(&q) {
        return true;
    }
    app.items
        .iter()
        .any(|it| it.path.to_ascii_lowercase().contains(&q))
}

#[must_use]
pub fn has_growth_baseline(report: &ScanReport) -> bool {
    report
        .apps
        .iter()
        .any(|a| !matches!(a.growth, GrowthDelta::NoBaseline))
}

/// Drops persisted facet selections that no longer exist in `report` so launches never land on empty stale filters.
pub fn sanitize_facets_for_report(facets: &mut FacetFilters, report: &ScanReport) {
    if let Some(ref pub_key) = facets.publisher {
        let exists = if pub_key == NO_PUBLISHER_FACET_KEY {
            report.apps.iter().any(|a| a.publishers.is_empty())
        } else {
            report.apps.iter().any(|a| {
                a.publishers
                    .iter()
                    .any(|p| p.trim().eq_ignore_ascii_case(pub_key))
            })
        };
        if !exists {
            facets.publisher = None;
        }
    }
    if let Some(dl) = facets.drive {
        let dl_u = dl.to_ascii_uppercase();
        let has_drive = report
            .drives
            .iter()
            .any(|d| d.letter.to_ascii_uppercase() == dl_u)
            || report.apps.iter().any(|a| {
                a.items
                    .iter()
                    .any(|it| it.drive_letter.to_ascii_uppercase() == dl_u)
            });
        if !has_drive {
            facets.drive = None;
        }
    }
    if facets.growth != GrowthFacetFilter::All && !has_growth_baseline(report) {
        facets.growth = GrowthFacetFilter::All;
    }
}

/// Builds `(facet_key, display_label, app_count, total_bytes)` ordered by summed bytes descending,
/// grouping apps without a publisher under `"(no publisher)"`.
#[must_use]
pub fn publisher_facet_options(apps: &[AttributedApp]) -> Vec<(String, String, usize, u64)> {
    let mut map: HashMap<String, (String, usize, u64)> = HashMap::new();
    for app in apps {
        if app.publishers.is_empty() {
            let e = map
                .entry(NO_PUBLISHER_FACET_KEY.to_string())
                .or_insert_with(|| ("(no publisher)".to_string(), 0, 0));
            e.1 += 1;
            e.2 = e.2.saturating_add(app.total_bytes);
        } else {
            let mut seen_app_pubs = HashSet::new();
            for p in &app.publishers {
                let pt = p.trim();
                if pt.is_empty() {
                    continue;
                }
                let kl = pt.to_ascii_lowercase();
                if seen_app_pubs.insert(kl) {
                    let e = map
                        .entry(pt.to_string())
                        .or_insert_with(|| (pt.to_string(), 0, 0));
                    e.1 += 1;
                    e.2 = e.2.saturating_add(app.total_bytes);
                }
            }
        }
    }
    let mut out: Vec<(String, String, usize, u64)> = map
        .into_iter()
        .map(|(k, (lbl, cnt, bytes))| (k, lbl, cnt, bytes))
        .collect();
    out.sort_by(|a, b| {
        b.3.cmp(&a.3)
            .then_with(|| a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase()))
    });
    out
}

#[must_use]
pub fn category_facet_options(apps: &[AttributedApp]) -> Vec<(AppCategory, usize)> {
    let all_cats = [
        AppCategory::Games,
        AppCategory::PlatformsAndLaunchers,
        AppCategory::Browsers,
        AppCategory::Communication,
        AppCategory::Development,
        AppCategory::MediaAndCreative,
        AppCategory::Productivity,
        AppCategory::Utilities,
        AppCategory::SystemAndRuntimes,
        AppCategory::Uncategorized,
    ];
    let mut counts: BTreeMap<AppCategory, usize> = BTreeMap::new();
    for a in apps {
        *counts.entry(a.category).or_default() += 1;
    }
    all_cats
        .into_iter()
        .map(|c| (c, counts.get(&c).copied().unwrap_or(0)))
        .collect()
}

fn app_occupies_zone_facet(app: &AttributedApp, zf: FootprintZoneFilter) -> bool {
    match zf {
        FootprintZoneFilter::All => true,
        FootprintZoneFilter::InstallFolder => {
            app.bases.contains(&AttributionBasis::InstallLocation)
                || app.items.iter().any(|it| {
                    it.zone == Some(ZoneKind::Body)
                        || it.basis == Some(AttributionBasis::InstallLocation)
                })
        }
        FootprintZoneFilter::AppData => app.items.iter().any(|it| {
            let pl = it.path.to_ascii_lowercase();
            pl.contains("\\appdata\\local\\")
                || pl.contains("\\appdata\\roaming\\")
                || pl.contains("\\appdata\\locallow\\")
        }),
        FootprintZoneFilter::ProgramData => app
            .items
            .iter()
            .any(|it| it.path.to_ascii_lowercase().contains("\\programdata\\")),
        FootprintZoneFilter::ProfileOrDotFolder => {
            app.bases.contains(&AttributionBasis::StandaloneFolder)
                || app.id.starts_with("dot:")
                || app.items.iter().any(|it| {
                    if it.basis == Some(AttributionBasis::StandaloneFolder) {
                        return true;
                    }
                    let pl = it.path.trim_end_matches('\\').to_ascii_lowercase();
                    let parts: Vec<&str> = pl.split('\\').collect();
                    parts.len() == 4
                        && parts[1] == "users"
                        && parts[3] != "appdata"
                })
        }
    }
}

#[must_use]
fn app_matches_facets_for_mode(
    app: &AttributedApp,
    facets: &FacetFilters,
    shared_app_ids: &HashSet<String>,
    per_drive: Option<char>,
) -> bool {
    if let Some(ref pub_key) = facets.publisher {
        let hit = if pub_key == NO_PUBLISHER_FACET_KEY {
            app.publishers.is_empty()
        } else {
            app.publishers
                .iter()
                .any(|p| p.trim().eq_ignore_ascii_case(pub_key))
        };
        if !hit {
            return false;
        }
    }
    if let Some(cat) = facets.category {
        if app.category != cat {
            return false;
        }
    }
    if let Some(drv) = facets.drive {
        if app.bytes_on_drive(drv) == 0 {
            return false;
        }
    }
    let eff_bytes = per_drive.map_or(app.total_bytes, |dl| app.bytes_on_drive(dl));
    if !facets.size_band.matches_bytes(eff_bytes) {
        return false;
    }
    match facets.dep_state {
        DepStateFilter::All => {}
        DepStateFilter::HasDependencies => {
            if app.dependencies.is_empty() {
                return false;
            }
        }
        DepStateFilter::NoKnownDependencies => {
            if !app.dependencies.is_empty() {
                return false;
            }
        }
        DepStateFilter::SharedUsedByOthers => {
            if !shared_app_ids.contains(&app.id) {
                return false;
            }
        }
    }
    if !app_occupies_zone_facet(app, facets.footprint_zone) {
        return false;
    }
    let eff_growth = per_drive.map_or(app.growth, |dl| app.growth_for_drive(dl));
    match facets.growth {
        GrowthFacetFilter::All => {}
        GrowthFacetFilter::Grew => {
            if !matches!(eff_growth, GrowthDelta::Changed(d) if d > 0)
                && !matches!(eff_growth, GrowthDelta::New(_))
            {
                return false;
            }
        }
        GrowthFacetFilter::Shrank => {
            if !matches!(eff_growth, GrowthDelta::Changed(d) if d < 0) {
                return false;
            }
        }
        GrowthFacetFilter::Unchanged => {
            if !matches!(eff_growth, GrowthDelta::Changed(0)) {
                return false;
            }
        }
    }
    true
}

/// Combines the active filter chip, search text, and general facets using logical AND,
/// switching size, growth, and Size/Growth sorting to per-drive bytes when a single drive filter is active.
#[must_use]
pub fn filter_and_sort_apps_with_facets<'a>(
    apps: &'a [AttributedApp],
    mode: AppSortMode,
    dir: SortDirection,
    chip: AppFilterChip,
    search_query: &str,
    facets: &FacetFilters,
    shared_app_ids: &HashSet<String>,
    system_drive_letter: char,
) -> Vec<&'a AttributedApp> {
    let per_drive = active_per_drive_mode(chip, facets, system_drive_letter);
    let mut out: Vec<&'a AttributedApp> = apps
        .iter()
        .filter(|app| {
            app_matches_chip_for_mode(app, chip, system_drive_letter, per_drive)
                && app_matches_search(app, search_query)
                && app_matches_facets_for_mode(app, facets, shared_app_ids, per_drive)
        })
        .collect();
    if per_drive.is_some() || (mode, dir) != (AppSortMode::Size, SortDirection::Desc) {
        out.sort_by(|a, b| compare_apps_for_mode(a, b, mode, dir, per_drive));
    }
    out
}

/// Combines the active filter chip and search text using logical AND.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn app_matches_filter(
    app: &AttributedApp,
    chip: AppFilterChip,
    search_query: &str,
    system_drive_letter: char,
) -> bool {
    app_matches_chip(app, chip, system_drive_letter) && app_matches_search(app, search_query)
}

/// Pure comparator for Size, Name (case-insensitive), and Growth (no-baseline/new apps always sort after apps with deltas, ordered by size descending among themselves).
#[must_use]
pub fn compare_apps(
    a: &AttributedApp,
    b: &AttributedApp,
    mode: AppSortMode,
    dir: SortDirection,
) -> Ordering {
    compare_apps_for_mode(a, b, mode, dir, None)
}

#[must_use]
pub fn compare_apps_for_mode(
    a: &AttributedApp,
    b: &AttributedApp,
    mode: AppSortMode,
    dir: SortDirection,
    per_drive: Option<char>,
) -> Ordering {
    let eff_size = |app: &AttributedApp| -> u64 {
        per_drive.map_or(app.total_bytes, |dl| app.bytes_on_drive(dl))
    };
    let eff_growth = |app: &AttributedApp| -> GrowthDelta {
        per_drive.map_or(app.growth, |dl| app.growth_for_drive(dl))
    };
    match mode {
        AppSortMode::Size => match dir {
            SortDirection::Desc => eff_size(b)
                .cmp(&eff_size(a))
                .then_with(|| a.display_name.cmp(&b.display_name)),
            SortDirection::Asc => eff_size(a)
                .cmp(&eff_size(b))
                .then_with(|| a.display_name.cmp(&b.display_name)),
        },
        AppSortMode::Name => {
            let la = a.display_name.to_lowercase();
            let lb = b.display_name.to_lowercase();
            let primary = match dir {
                SortDirection::Asc => la.cmp(&lb),
                SortDirection::Desc => lb.cmp(&la),
            };
            primary
                .then_with(|| a.display_name.cmp(&b.display_name))
                .then_with(|| eff_size(b).cmp(&eff_size(a)))
        }
        AppSortMode::Publisher => {
            let pa = a
                .publishers
                .first()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let pb = b
                .publishers
                .first()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let primary = match (pa, pb) {
                (Some(sa), Some(sb)) => {
                    let la = sa.to_lowercase();
                    let lb = sb.to_lowercase();
                    match dir {
                        SortDirection::Asc => la.cmp(&lb),
                        SortDirection::Desc => lb.cmp(&la),
                    }
                }
                // Publisher-less apps group at end in A-Z and at start in Z-A.
                (Some(_), None) => match dir {
                    SortDirection::Asc => Ordering::Less,
                    SortDirection::Desc => Ordering::Greater,
                },
                (None, Some(_)) => match dir {
                    SortDirection::Asc => Ordering::Greater,
                    SortDirection::Desc => Ordering::Less,
                },
                (None, None) => Ordering::Equal,
            };
            primary
                .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
                .then_with(|| a.display_name.cmp(&b.display_name))
                .then_with(|| eff_size(b).cmp(&eff_size(a)))
        }
        AppSortMode::Category => {
            let ca = a.category.label().to_lowercase();
            let cb = b.category.label().to_lowercase();
            let primary = match dir {
                SortDirection::Asc => ca.cmp(&cb),
                SortDirection::Desc => cb.cmp(&ca),
            };
            primary
                .then_with(|| eff_size(b).cmp(&eff_size(a)))
                .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
                .then_with(|| a.display_name.cmp(&b.display_name))
        }
        AppSortMode::Growth => match (
            growth_delta_bytes(eff_growth(a)),
            growth_delta_bytes(eff_growth(b)),
        ) {
            (Some(da), Some(db)) => {
                let primary = match dir {
                    SortDirection::Desc => db.cmp(&da),
                    SortDirection::Asc => da.cmp(&db),
                };
                primary
                    .then_with(|| eff_size(b).cmp(&eff_size(a)))
                    .then_with(|| a.display_name.cmp(&b.display_name))
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => eff_size(b)
                .cmp(&eff_size(a))
                .then_with(|| a.display_name.cmp(&b.display_name)),
        },
    }
}

/// Filters and sorts the app list without mutating report totals or original report ordering.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn filter_and_sort_apps<'a>(
    apps: &'a [AttributedApp],
    mode: AppSortMode,
    dir: SortDirection,
    chip: AppFilterChip,
    search_query: &str,
    system_drive_letter: char,
) -> Vec<&'a AttributedApp> {
    let mut out: Vec<&'a AttributedApp> = apps
        .iter()
        .filter(|app| app_matches_filter(app, chip, search_query, system_drive_letter))
        .collect();
    // Preserve exact byte-for-byte original report order when in default Size desc mode.
    if (mode, dir) != (AppSortMode::Size, SortDirection::Desc) {
        out.sort_by(|a, b| compare_apps(a, b, mode, dir));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use y_core::config::AppConfig;
    use y_core::model::{AppCategory, BreakdownItem, NoTouchCategory, ZoneKind};

    fn make_app(
        id: &str,
        name: &str,
        publisher: &str,
        total_bytes: u64,
        growth: GrowthDelta,
        no_touch: Option<NoTouchCategory>,
        bases: Vec<AttributionBasis>,
        drive_letters: &[char],
    ) -> AttributedApp {
        let items = drive_letters
            .iter()
            .enumerate()
            .map(|(i, &dl)| BreakdownItem {
                path: format!("{dl}:\\Apps\\{name}\\part{i}"),
                bytes: total_bytes / (drive_letters.len().max(1) as u64),
                zone: Some(ZoneKind::Body),
                qualifier: None,
                basis: bases.first().copied(),
                basis_detail: None,
                no_touch,
                drive_letter: dl,
            })
            .collect();
        AttributedApp {
            id: id.to_string(),
            display_name: name.to_string(),
            publishers: vec![publisher.to_string()],
            install_location: None,
            total_bytes,
            previous_bytes: None,
            growth,
            per_drive_growth: Default::default(),
            no_touch,
            bases,
            items,
            dependencies: Vec::new(),
            display_icon: None,
            icon_rgba: None,
            category: AppCategory::default(),
            category_basis: String::new(),
        }
    }

    #[test]
    fn size_desc_all_empty_search_preserves_exact_order_and_flips_asc() {
        let apps = vec![
            make_app(
                "reg:alpha",
                "Alpha",
                "Pub A",
                500,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "reg:beta",
                "Beta",
                "Pub B",
                300,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['D'],
            ),
            make_app(
                "reg:gamma",
                "Gamma",
                "Pub C",
                100,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
        ];

        let default_res = filter_and_sort_apps(
            &apps,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            'C',
        );
        let default_ids: Vec<&str> = default_res.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(default_ids, vec!["reg:alpha", "reg:beta", "reg:gamma"]);

        let asc_res = filter_and_sort_apps(
            &apps,
            AppSortMode::Size,
            SortDirection::Asc,
            AppFilterChip::All,
            "   ",
            'C',
        );
        let asc_ids: Vec<&str> = asc_res.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(asc_ids, vec!["reg:gamma", "reg:beta", "reg:alpha"]);
    }

    #[test]
    fn name_sort_is_case_insensitive_and_click_transitions_work() {
        assert_eq!(
            AppSortMode::Name.click_transition(AppSortMode::Size, SortDirection::Desc),
            (AppSortMode::Name, SortDirection::Asc)
        );
        assert_eq!(
            AppSortMode::Name.click_transition(AppSortMode::Name, SortDirection::Asc),
            (AppSortMode::Name, SortDirection::Desc)
        );
        assert_eq!(
            AppSortMode::Growth.click_transition(AppSortMode::Name, SortDirection::Asc),
            (AppSortMode::Growth, SortDirection::Desc)
        );
        assert_eq!(
            AppSortMode::Growth.click_transition(AppSortMode::Growth, SortDirection::Desc),
            (AppSortMode::Growth, SortDirection::Asc)
        );

        let apps = vec![
            make_app(
                "reg:zeta",
                "Zeta",
                "Pub",
                100,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "dot:.gemini",
                ".gemini",
                "User profile dot-folder",
                200,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::StandaloneFolder],
                &['C'],
            ),
            make_app(
                "reg:alpha",
                "alpha",
                "Pub",
                300,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "reg:beta",
                "Beta",
                "Pub",
                400,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
        ];

        let az: Vec<&str> = filter_and_sort_apps(
            &apps,
            AppSortMode::Name,
            SortDirection::Asc,
            AppFilterChip::All,
            "",
            'C',
        )
        .into_iter()
        .map(|a| a.display_name.as_str())
        .collect();
        assert_eq!(az, vec![".gemini", "alpha", "Beta", "Zeta"]);

        let za: Vec<&str> = filter_and_sort_apps(
            &apps,
            AppSortMode::Name,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            'C',
        )
        .into_iter()
        .map(|a| a.display_name.as_str())
        .collect();
        assert_eq!(za, vec!["Zeta", "Beta", "alpha", ".gemini"]);
    }

    #[test]
    fn growth_sort_places_no_baseline_and_new_after_all_deltas_ordered_by_size() {
        let apps = vec![
            make_app(
                "reg:nb-small",
                "NoBaseline Small",
                "Pub",
                10_000,
                GrowthDelta::NoBaseline,
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "reg:shrank",
                "ShrankApp",
                "Pub",
                5_000,
                GrowthDelta::Changed(-2_000),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "reg:new-big",
                "NewApp Big",
                "Pub",
                900_000,
                GrowthDelta::New(900_000),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "reg:zero",
                "ZeroChange",
                "Pub",
                50_000,
                GrowthDelta::Changed(0),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
            make_app(
                "reg:grew",
                "BigGrower",
                "Pub",
                20_000,
                GrowthDelta::Changed(15_000),
                None,
                vec![AttributionBasis::InstallLocation],
                &['C'],
            ),
        ];

        // Desc: biggest positive delta -> 0 -> negative -> no-baseline/new (ordered by size desc: NewApp Big before NoBaseline Small)
        let desc_ids: Vec<&str> = filter_and_sort_apps(
            &apps,
            AppSortMode::Growth,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            'C',
        )
        .into_iter()
        .map(|a| a.id.as_str())
        .collect();
        assert_eq!(
            desc_ids,
            vec![
                "reg:grew",
                "reg:zero",
                "reg:shrank",
                "reg:new-big",
                "reg:nb-small"
            ]
        );

        // Asc: most negative delta -> 0 -> positive -> no-baseline/new STILL after every delta, ordered by size desc among themselves
        let asc_ids: Vec<&str> = filter_and_sort_apps(
            &apps,
            AppSortMode::Growth,
            SortDirection::Asc,
            AppFilterChip::All,
            "",
            'C',
        )
        .into_iter()
        .map(|a| a.id.as_str())
        .collect();
        assert_eq!(
            asc_ids,
            vec![
                "reg:shrank",
                "reg:zero",
                "reg:grew",
                "reg:new-big",
                "reg:nb-small"
            ]
        );
    }

    #[test]
    fn filter_chips_and_search_combine_with_and() {
        let apps = vec![
            make_app(
                "sys:xbox",
                "Xbox Service",
                "Microsoft Corporation",
                800,
                GrowthDelta::Changed(100),
                Some(NoTouchCategory::Service),
                vec![AttributionBasis::Token],
                &['C'],
            ),
            make_app(
                "sys:battleye",
                "BattlEye",
                "BattlEye Innovations",
                400,
                GrowthDelta::Changed(0),
                Some(NoTouchCategory::AntiCheat),
                vec![AttributionBasis::Token],
                &['D'],
            ),
            make_app(
                "dot:.gemini",
                ".gemini",
                "User profile dot-folder",
                300,
                GrowthDelta::Changed(50),
                None,
                vec![AttributionBasis::StandaloneFolder],
                &['C'],
            ),
            make_app(
                "standalone:flutter",
                "flutter",
                "Standalone SDK / toolchain",
                200,
                GrowthDelta::NoBaseline,
                None,
                vec![AttributionBasis::StandaloneFolder],
                &['C'],
            ),
            make_app(
                "reg:fifa23",
                "FIFA 23",
                "EA",
                900,
                GrowthDelta::New(900),
                None,
                vec![AttributionBasis::InstallLocation],
                &['D'],
            ),
        ];

        assert_eq!(
            filter_and_sort_apps(&apps, AppSortMode::Size, SortDirection::Desc, AppFilterChip::Grew, "", 'C')
                .into_iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["sys:xbox", "dot:.gemini"]
        );

        assert_eq!(
            filter_and_sort_apps(&apps, AppSortMode::Size, SortDirection::Desc, AppFilterChip::NoTouch, "microsoft", 'C')
                .into_iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["sys:xbox"]
        );

        assert_eq!(
            filter_and_sort_apps(&apps, AppSortMode::Name, SortDirection::Asc, AppFilterChip::Standalone, "flut", 'C')
                .into_iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["standalone:flutter"]
        );

        assert_eq!(
            filter_and_sort_apps(&apps, AppSortMode::Size, SortDirection::Desc, AppFilterChip::HasSystemDriveFootprint, "", 'C')
                .into_iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["sys:xbox", "dot:.gemini", "standalone:flutter"]
        );
    }

    #[test]
    fn config_persists_and_restores_sort_filter_and_search_state() {
        let dir = std::env::temp_dir().join("y_storage_sort_filter_cfg_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let cfg_path = dir.join("config.toml");

        let mut cfg = AppConfig::default();
        cfg.sort_mode = AppSortMode::Growth;
        cfg.sort_direction = SortDirection::Asc;
        cfg.filter_chip = AppFilterChip::Standalone;
        cfg.search_query = "flut".to_string();
        cfg.save_atomic(&cfg_path).expect("save config");

        let decoded = AppConfig::load_or_default(&cfg_path, &[], "C:\\Windows");
        assert_eq!(decoded.sort_mode, AppSortMode::Growth);
        assert_eq!(decoded.sort_direction, SortDirection::Asc);
        assert_eq!(decoded.filter_chip, AppFilterChip::Standalone);
        assert_eq!(decoded.search_query, "flut");

        // Backward compatibility: older config without sort/filter keys deserializes to defaults.
        let legacy_path = dir.join("legacy_config.toml");
        let legacy = "version = 1\ntheme_mode = \"Dark\"\n[[targets]]\nletter = 'C'\nenabled = true\nexclusions = [\"C:\\\\Windows\"]\n";
        std::fs::write(&legacy_path, legacy).expect("write legacy config");
        let legacy_cfg = AppConfig::load_or_default(&legacy_path, &[], "C:\\Windows");
        assert_eq!(legacy_cfg.sort_mode, AppSortMode::Size);
        assert_eq!(legacy_cfg.sort_direction, SortDirection::Desc);
        assert_eq!(legacy_cfg.filter_chip, AppFilterChip::All);
        assert_eq!(legacy_cfg.search_query, "");
        assert!(!legacy_cfg.ledger_collapsed);
        assert_eq!(legacy_cfg.ledger_height_px, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publisher_sort_orders_tie_breaks_by_name_and_groups_empty_publishers() {
        let apps = vec![
            make_app("reg:no1", "Zebra Tool", "", 100, GrowthDelta::Changed(0), None, vec![AttributionBasis::ExactName], &['C']),
            make_app("reg:no2", "Alpha Tool", "   ", 500, GrowthDelta::Changed(0), None, vec![AttributionBasis::ExactName], &['C']),
            make_app("reg:b1", "Beta App", "BetaPub", 200, GrowthDelta::Changed(0), None, vec![AttributionBasis::ExactName], &['C']),
            make_app("reg:a2", "Zoo App", "acme inc", 100, GrowthDelta::Changed(0), None, vec![AttributionBasis::ExactName], &['C']),
            make_app("reg:a1", "Ant App", "Acme Inc", 50, GrowthDelta::Changed(0), None, vec![AttributionBasis::ExactName], &['C']),
        ];

        // A->Z: Acme (Ant before Zoo), BetaPub (Beta), then publisher-less at end (Alpha before Zebra)
        let asc: Vec<&str> = filter_and_sort_apps(&apps, AppSortMode::Publisher, SortDirection::Asc, AppFilterChip::All, "", 'C')
            .into_iter()
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(asc, vec!["reg:a1", "reg:a2", "reg:b1", "reg:no2", "reg:no1"]);

        // Z->A: publisher-less at start (Alpha before Zebra tie-broken A-Z), then BetaPub, then Acme (Ant before Zoo)
        let desc: Vec<&str> = filter_and_sort_apps(&apps, AppSortMode::Publisher, SortDirection::Desc, AppFilterChip::All, "", 'C')
            .into_iter()
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(desc, vec!["reg:no2", "reg:no1", "reg:b1", "reg:a1", "reg:a2"]);
    }

    #[test]
    fn category_sort_and_search_group_by_category_then_size_desc_then_name() {
        let mut c = make_app("reg:fifa", "EA SPORTS FIFA 23", "EA", 50_000, GrowthDelta::Changed(0), None, vec![AttributionBasis::InstallLocation], &['D']);
        c.category = AppCategory::Games;
        let mut d = make_app("reg:unity", "Unity", "Unity", 1_000, GrowthDelta::Changed(0), None, vec![AttributionBasis::InstallLocation], &['C']);
        d.category = AppCategory::Development;
        let mut b = make_app("reg:edge", "Microsoft Edge", "Microsoft", 900, GrowthDelta::Changed(0), None, vec![AttributionBasis::InstallLocation], &['C']);
        b.category = AppCategory::Browsers;
        let mut a = make_app("reg:chrome", "Google Chrome", "Google", 400, GrowthDelta::Changed(0), None, vec![AttributionBasis::InstallLocation], &['C']);
        a.category = AppCategory::Browsers;

        // Input mirrors a real ScanReport sorted descending by total_bytes.
        let apps = vec![c, d, b, a];
        let asc: Vec<&str> = filter_and_sort_apps(&apps, AppSortMode::Category, SortDirection::Asc, AppFilterChip::All, "", 'C')
            .into_iter()
            .map(|x| x.id.as_str())
            .collect();
        // Browsers (Edge 900 before Chrome 400), Development, Games
        assert_eq!(asc, vec!["reg:edge", "reg:chrome", "reg:unity", "reg:fifa"]);

        // Search filter matches category labels cleanly
        let filtered: Vec<&str> = filter_and_sort_apps(&apps, AppSortMode::Size, SortDirection::Desc, AppFilterChip::All, "browsers", 'C')
            .into_iter()
            .map(|x| x.id.as_str())
            .collect();
        assert_eq!(filtered, vec!["reg:edge", "reg:chrome"]);
    }

    #[test]
    fn unattributed_filter_and_sort_preserves_totals_and_matches_substrings() {
        let items = vec![
            UnattributedItem {
                path: r"C:\Users\Win\AppData\Local\AlphaTool".into(),
                bytes: 5_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"C:\ProgramData\Microsoft\Shared".into(),
                bytes: 9_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"C:\Users\Win\AppData\Roaming\zeta_cache".into(),
                bytes: 1_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
        ];

        // Default SizeDesc with empty query preserves order and sums full total
        let (all, total) = filter_and_sort_unattributed(&items, "", UnattributedSortMode::SizeDesc);
        assert_eq!(all.len(), 3);
        assert_eq!(total, 15_000);

        // Name ascending puts AlphaTool first
        let (name_asc, _) = filter_and_sort_unattributed(&items, "", UnattributedSortMode::NameAsc);
        assert_eq!(name_asc[0].1.path, r"C:\Users\Win\AppData\Local\AlphaTool");
        assert_eq!(name_asc[2].1.path, r"C:\Users\Win\AppData\Roaming\zeta_cache");

        // Substring query matches parent path segments case-insensitively
        let (appdata, sub_bytes) =
            filter_and_sort_unattributed(&items, "appdata", UnattributedSortMode::SizeAsc);
        assert_eq!(appdata.len(), 2);
        assert_eq!(sub_bytes, 6_000);
        assert_eq!(appdata[0].1.bytes, 1_000);
        assert_eq!(appdata[1].1.bytes, 5_000);
    }

    #[test]
    fn facet_filters_combine_with_and_and_fallback_cleanly_on_missing_values() {
        const GIB: u64 = 1024 * 1024 * 1024;
        let mut ms_big = make_app(
            "reg:xbox",
            "Xbox Service",
            "Microsoft Corporation",
            7 * GIB,
            GrowthDelta::Changed(500),
            None,
            vec![AttributionBasis::InstallLocation],
            &['C'],
        );
        ms_big.category = AppCategory::Games;
        ms_big.dependencies.push(y_core::model::DependencyRecord {
            target_id: "directx-runtime".into(),
            target_name: "DirectX Runtime".into(),
            kind: y_core::model::DependencyKind::SharedRuntime,
            present: true,
            own_size_bytes: None,
            shared: true,
            basis: "PE import".into(),
            confidence: y_core::model::DependencyConfidence::Proven,
            imported_by_binary: true,
        });

        let mut ms_small = make_app(
            "reg:sdk",
            "Windows SDK",
            "Microsoft Corporation",
            200 * 1024 * 1024,
            GrowthDelta::Changed(-100),
            None,
            vec![AttributionBasis::Token],
            &['D'],
        );
        ms_small.items[0].zone = Some(ZoneKind::Green);
        ms_small.items[0].path = r"D:\ProgramData\Package Cache\SDK".into();
        ms_small.category = AppCategory::Development;

        let mut ea_huge = make_app(
            "reg:fifa",
            "FIFA 23",
            "EA Canada",
            48 * GIB,
            GrowthDelta::Changed(0),
            None,
            vec![AttributionBasis::InstallLocation],
            &['C', 'D'],
        );
        ea_huge.category = AppCategory::Games;

        let apps = vec![ms_big, ms_small, ea_huge];
        let mut shared_ids = HashSet::new();
        shared_ids.insert("reg:sdk".to_string());

        let facets = FacetFilters {
            publisher: Some("Microsoft Corporation".into()),
            size_band: SizeBandFilter::Ge1Gb,
            drive: Some('C'),
            ..FacetFilters::default()
        };
        let matched = filter_and_sort_apps_with_facets(
            &apps,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            &facets,
            &shared_ids,
            'C',
        );
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "reg:xbox");

        // ProgramData + Shared dependency matches reg:sdk only
        let f2 = FacetFilters {
            footprint_zone: FootprintZoneFilter::ProgramData,
            dep_state: DepStateFilter::SharedUsedByOthers,
            growth: GrowthFacetFilter::Shrank,
            ..FacetFilters::default()
        };
        let m2 = filter_and_sort_apps_with_facets(
            &apps,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            &f2,
            &shared_ids,
            'C',
        );
        assert_eq!(m2.len(), 1);
        assert_eq!(m2[0].id, "reg:sdk");

        // Stale persisted publisher/drive resets to None instead of yielding empty list
        let rep = ScanReport {
            scan_timestamp_unix: 1,
            scan_timestamp_display: "now".into(),
            duration_ms: 1,
            drives: vec![y_core::model::DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 100,
                free_bytes: 50,
                used_bytes: 50,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            }],
            registry_stats: y_core::model::RegistryDiscoveryStats {
                raw_display_name_entries: 3,
                raw_with_install_location: 2,
                canonical_apps: 3,
                canonical_with_direct_install_location: 2,
                canonical_with_inferred_install_location: 0,
            },
            apps,
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![],
            your_files_total_bytes: 0,
            windows_system: vec![],
            windows_system_total_bytes: 0,
            unattributed: vec![],
            unattributed_total_bytes: 0,
            reverse_dependencies: vec![],
            installed_registry_names: vec![],
            catalog_source: String::new(),
            warnings: vec![],
            applied_containers: None,
        };
        let mut stale = FacetFilters {
            publisher: Some("Vanished Publisher".into()),
            drive: Some('Z'),
            ..FacetFilters::default()
        };
        sanitize_facets_for_report(&mut stale, &rep);
        assert_eq!(stale.publisher, None);
        assert_eq!(stale.drive, None);
    }

    #[test]
    fn unattributed_facets_combine_with_and_and_classify_all_seven_zones() {
        let items = vec![
            UnattributedItem {
                path: r"C:\Users\Win\AppData\Local\Firaxis Games".into(),
                bytes: 2_000_000_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"C:\Users\Win\AppData\Roaming\UserBenchmark".into(),
                bytes: 200_000_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"C:\Users\Win\AppData\LocalLow\CampfireStudio".into(),
                bytes: 50_000_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"C:\ProgramData\USOShared".into(),
                bytes: 120_000_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"C:\Program Files\IIS".into(),
                bytes: 15_000_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Body),
            },
            UnattributedItem {
                path: r"C:\Users\Win\onecoremsvsmon".into(),
                bytes: 96_000_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
            },
            UnattributedItem {
                path: r"D:\Media".into(),
                bytes: 150_000_000_000,
                drive_letter: 'D',
                zone_hint: Some(ZoneKind::Red),
            },
        ];

        assert_eq!(
            UnattributedLocationZone::classify_path(&items[0].path),
            UnattributedLocationZone::AppDataLocal
        );
        assert_eq!(
            UnattributedLocationZone::classify_path(&items[1].path),
            UnattributedLocationZone::AppDataRoaming
        );
        assert_eq!(
            UnattributedLocationZone::classify_path(&items[2].path),
            UnattributedLocationZone::AppDataLocalLow
        );
        assert_eq!(
            UnattributedLocationZone::classify_path(&items[3].path),
            UnattributedLocationZone::ProgramData
        );
        assert_eq!(
            UnattributedLocationZone::classify_path(&items[4].path),
            UnattributedLocationZone::ProgramFiles
        );
        assert_eq!(
            UnattributedLocationZone::classify_path(&items[5].path),
            UnattributedLocationZone::UserProfileRoot
        );
        assert_eq!(
            UnattributedLocationZone::classify_path(&items[6].path),
            UnattributedLocationZone::OtherDriveLocation
        );

        let f = UnattributedFacetFilters {
            drive: Some('C'),
            size_band: SizeBandFilter::Between100MbAnd1Gb,
            location_zone: UnattributedLocationZone::AppDataRoaming,
        };
        let (rows, sub) = filter_and_sort_unattributed_with_facets(
            &items,
            "user",
            &f,
            UnattributedSortMode::SizeDesc,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(sub, 200_000_000);
        assert_eq!(rows[0].1.path, r"C:\Users\Win\AppData\Roaming\UserBenchmark");
    }

    #[test]
    fn per_drive_size_mode_switches_size_and_sorting_for_drive_facet_and_has_c_chip() {
        let mut two_drive_app = make_app(
            "reg:giant-d",
            "GiantOnD",
            "Pub",
            50_200_000_000,
            GrowthDelta::Changed(1_000_000),
            None,
            vec![AttributionBasis::InstallLocation],
            &['C'],
        );
        two_drive_app.items = vec![
            BreakdownItem {
                path: r"C:\Users\Win\AppData\Local\GiantOnD".into(),
                bytes: 200_000_000,
                zone: Some(ZoneKind::Red),
                qualifier: None,
                basis: Some(AttributionBasis::ExactName),
                basis_detail: None,
                no_touch: None,
                drive_letter: 'C',
            },
            BreakdownItem {
                path: r"D:\Games\GiantOnD".into(),
                bytes: 50_000_000_000,
                zone: Some(ZoneKind::Body),
                qualifier: None,
                basis: Some(AttributionBasis::InstallLocation),
                basis_detail: None,
                no_touch: None,
                drive_letter: 'D',
            },
        ];

        let big_c_app = make_app(
            "reg:big-c",
            "BigOnC",
            "Pub",
            4_000_000_000,
            GrowthDelta::Changed(500_000),
            None,
            vec![AttributionBasis::InstallLocation],
            &['C'],
        );

        let apps = vec![two_drive_app, big_c_app];
        let shared = HashSet::new();

        // Global mode: GiantOnD (50.2 GB) leads BigOnC (4.0 GB).
        let global = filter_and_sort_apps_with_facets(
            &apps,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            &FacetFilters::default(),
            &shared,
            'C',
        );
        assert_eq!(global.iter().map(|a| a.display_name.as_str()).collect::<Vec<_>>(), vec!["GiantOnD", "BigOnC"]);
        assert_eq!(active_per_drive_mode(AppFilterChip::All, &FacetFilters::default(), 'C'), None);

        // Drive = C: facet switches to per-drive bytes -> BigOnC (4.0 GB) leads GiantOnD (200 MB).
        let c_facets = FacetFilters {
            drive: Some('C'),
            ..Default::default()
        };
        assert_eq!(active_per_drive_mode(AppFilterChip::All, &c_facets, 'C'), Some('C'));
        let on_c = filter_and_sort_apps_with_facets(
            &apps,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::All,
            "",
            &c_facets,
            &shared,
            'C',
        );
        assert_eq!(on_c.iter().map(|a| a.display_name.as_str()).collect::<Vec<_>>(), vec!["BigOnC", "GiantOnD"]);
        assert_eq!(on_c[1].bytes_on_drive('C'), 200_000_000);
        assert_eq!(on_c[1].total_bytes, 50_200_000_000);

        // "Has C: footprint" chip alone ALSO activates C: per-drive mode (FIX 7c).
        assert_eq!(
            active_per_drive_mode(AppFilterChip::HasSystemDriveFootprint, &FacetFilters::default(), 'C'),
            Some('C')
        );
        let has_c = filter_and_sort_apps_with_facets(
            &apps,
            AppSortMode::Size,
            SortDirection::Desc,
            AppFilterChip::HasSystemDriveFootprint,
            "",
            &FacetFilters::default(),
            &shared,
            'C',
        );
        assert_eq!(has_c.iter().map(|a| a.display_name.as_str()).collect::<Vec<_>>(), vec!["BigOnC", "GiantOnD"]);

        // Combining Has C: footprint + Drive = D: gives >1 distinct drive constraint -> reverts to global totals.
        let d_facets = FacetFilters {
            drive: Some('D'),
            ..Default::default()
        };
        assert_eq!(
            active_per_drive_mode(AppFilterChip::HasSystemDriveFootprint, &d_facets, 'C'),
            None
        );
    }
}
