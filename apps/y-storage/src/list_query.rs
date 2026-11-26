use std::cmp::Ordering;

use y_core::config::{AppFilterChip, AppSortMode, SortDirection};
use y_core::model::{AttributedApp, AttributionBasis, GrowthDelta};

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

/// Pure predicate checking whether an app matches the single-select filter chip.
#[must_use]
pub fn app_matches_chip(
    app: &AttributedApp,
    chip: AppFilterChip,
    system_drive_letter: char,
) -> bool {
    match chip {
        AppFilterChip::All => true,
        AppFilterChip::Grew => matches!(app.growth, GrowthDelta::Changed(d) if d > 0),
        AppFilterChip::NoTouch => app.no_touch.is_some(),
        AppFilterChip::Standalone => is_standalone_entry(app),
        AppFilterChip::HasSystemDriveFootprint => app
            .items
            .iter()
            .any(|it| it.drive_letter.eq_ignore_ascii_case(&system_drive_letter)),
    }
}

/// Pure predicate checking whether an app matches the case-insensitive search query on name, publisher, or path.
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

/// Combines the active filter chip and search text using logical AND.
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
    match mode {
        AppSortMode::Size => match dir {
            SortDirection::Desc => b
                .total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| a.display_name.cmp(&b.display_name)),
            SortDirection::Asc => a
                .total_bytes
                .cmp(&b.total_bytes)
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
                .then_with(|| b.total_bytes.cmp(&a.total_bytes))
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
                .then_with(|| b.total_bytes.cmp(&a.total_bytes))
        }
        AppSortMode::Category => {
            let ca = a.category.label().to_lowercase();
            let cb = b.category.label().to_lowercase();
            let primary = match dir {
                SortDirection::Asc => ca.cmp(&cb),
                SortDirection::Desc => cb.cmp(&ca),
            };
            primary
                .then_with(|| b.total_bytes.cmp(&a.total_bytes))
                .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
                .then_with(|| a.display_name.cmp(&b.display_name))
        }
        AppSortMode::Growth => match (growth_delta_bytes(a.growth), growth_delta_bytes(b.growth)) {
            (Some(da), Some(db)) => {
                let primary = match dir {
                    SortDirection::Desc => db.cmp(&da),
                    SortDirection::Asc => da.cmp(&db),
                };
                primary
                    .then_with(|| b.total_bytes.cmp(&a.total_bytes))
                    .then_with(|| a.display_name.cmp(&b.display_name))
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => b
                .total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| a.display_name.cmp(&b.display_name)),
        },
    }
}

/// Filters and sorts the app list without mutating report totals or original report ordering.
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
}
