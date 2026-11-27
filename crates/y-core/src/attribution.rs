use std::collections::{BTreeMap, HashMap, HashSet};

use crate::config::{ContainerRulebook, PinKind, ZoneRulebook};
use crate::discovery::{
    AppTokenMatchKind, contains_whole_token_sequence, extract_guid, is_subpath_of,
    match_app_predicate, normalize_app_token, normalize_path_key, path_drive_letter,
    split_alnum_words, versioned_family_identity_key,
};
use crate::history::{HistoryEntryRef, ScanHistory, compute_growth, recompute_report_growth_from_history};
use crate::model::{
    AttributedApp, AttributionBasis, BreakdownItem, DriveInfo, GrowthDelta, InstalledApp,
    LocationQualifier, ManifestAppEntry, NoTouchCategory, RegistryDiscoveryStats, ScanReport,
    ScannedUnit, ScannedUnitKind, SystemDriveBreakdown, UnattributedItem, ZoneKind,
};
use crate::tree::{is_non_app_internal_subdir_name, is_program_files_inbox_folder};
use crate::zones::classify_zone;

#[derive(Debug, Clone)]
pub struct RootExeCandidate {
    pub exe_path: String,
    pub product_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AttributionOptions<'a> {
    pub containers: Option<&'a ContainerRulebook>,
    pub zones: Option<&'a ZoneRulebook>,
    pub manifest_entries: &'a [ManifestAppEntry],
    pub steam_library_roots: &'a [String],
    pub root_exe_candidates: Option<&'a HashMap<String, RootExeCandidate>>,
}

struct AppBucket {
    app: InstalledApp,
    bases: Vec<AttributionBasis>,
    items: Vec<BreakdownItem>,
    total_bytes: u64,
}

#[must_use]
pub fn attribute_scan(
    timestamp_unix: u64,
    timestamp_display: String,
    duration_ms: u64,
    drives: Vec<DriveInfo>,
    registry_stats: RegistryDiscoveryStats,
    installed_apps: &[InstalledApp],
    guid_to_app_name: &HashMap<String, String>,
    scanned_units: &[ScannedUnit],
    previous_history: Option<&HistoryEntryRef>,
) -> ScanReport {
    attribute_scan_with_options(
        timestamp_unix,
        timestamp_display,
        duration_ms,
        drives,
        registry_stats,
        installed_apps,
        guid_to_app_name,
        scanned_units,
        previous_history,
        &AttributionOptions::default(),
    )
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn attribute_scan_with_options(
    timestamp_unix: u64,
    timestamp_display: String,
    duration_ms: u64,
    drives: Vec<DriveInfo>,
    registry_stats: RegistryDiscoveryStats,
    installed_apps: &[InstalledApp],
    guid_to_app_name: &HashMap<String, String>,
    scanned_units: &[ScannedUnit],
    previous_history: Option<&HistoryEntryRef>,
    options: &AttributionOptions<'_>,
) -> ScanReport {
    let mut buckets: Vec<AppBucket> = installed_apps
        .iter()
        .cloned()
        .map(|app| AppBucket {
            app,
            bases: Vec::new(),
            items: Vec::new(),
            total_bytes: 0,
        })
        .collect();

    let excluded_drives: HashSet<char> = drives
        .iter()
        .filter(|d| !d.full_walk_enabled)
        .map(|d| d.letter.to_ascii_uppercase())
        .collect();

    let effective_zones: ZoneRulebook = if let Some(z) = options.zones {
        z.clone()
    } else if let Some(c) = options.containers {
        ZoneRulebook::from_legacy_containers(c)
    } else {
        ZoneRulebook::default()
    };

    let mut store_buckets: HashMap<String, usize> = HashMap::new();
    let mut manifest_buckets_by_dir: HashMap<String, (usize, AttributionBasis)> = HashMap::new();
    for m in options.manifest_entries {
        let mk = normalize_path_key(&m.install_dir);
        if mk.len() <= 3 {
            continue;
        }
        let m_tok = normalize_app_token(&m.display_name);
        let existing_idx = buckets.iter().position(|b| {
            b.app
                .all_install_locations
                .iter()
                .any(|loc| normalize_path_key(loc) == mk)
                || (!m_tok.is_empty() && normalize_app_token(&b.app.display_name) == m_tok)
                || (!m_tok.is_empty()
                    && b.app
                        .aliases
                        .iter()
                        .any(|al| normalize_app_token(al) == m_tok))
        });
        let idx = if let Some(pos) = existing_idx {
            if !buckets[pos]
                .app
                .all_install_locations
                .iter()
                .any(|loc| normalize_path_key(loc) == mk)
            {
                buckets[pos]
                    .app
                    .all_install_locations
                    .push(m.install_dir.clone());
            }
            if buckets[pos].app.direct_install_location.is_none() {
                buckets[pos].app.direct_install_location = Some(m.install_dir.clone());
            }
            if buckets[pos].app.publishers.is_empty() {
                if let Some(ref pub_name) = m.publisher {
                    buckets[pos].app.publishers.push(pub_name.clone());
                }
            }
            if buckets[pos].app.display_icon.is_none() && m.display_icon.is_some() {
                buckets[pos].app.display_icon = m.display_icon.clone();
            }
            pos
        } else {
            let idx = buckets.len();
            buckets.push(AppBucket {
                app: InstalledApp {
                    id: m.id.clone(),
                    display_name: m.display_name.clone(),
                    display_version: String::new(),
                    uninstall_guids: Vec::new(),
                    aliases: Vec::new(),
                    publishers: m.publisher.clone().into_iter().collect(),
                    direct_install_location: Some(m.install_dir.clone()),
                    inferred_install_location: None,
                    all_install_locations: vec![m.install_dir.clone()],
                    no_touch: None,
                    is_store_fallback: false,
                    display_icon: m.display_icon.clone(),
                },
                bases: Vec::new(),
                items: Vec::new(),
                total_bytes: 0,
            });
            store_buckets.insert(m.id.clone(), idx);
            idx
        };
        manifest_buckets_by_dir.insert(mk, (idx, m.basis));
    }

    let mut shared_caches: Vec<BreakdownItem> = Vec::new();
    let mut your_files: Vec<BreakdownItem> = Vec::new();
    let mut unattributed: Vec<UnattributedItem> = Vec::new();
    let mut root_sys_by_drive: HashMap<char, Vec<BreakdownItem>> = HashMap::new();
    let mut os_folders_by_drive: HashMap<char, Vec<BreakdownItem>> = HashMap::new();
    let mut scanned_non_system_by_drive: HashMap<char, u64> = HashMap::new();
    let steam_lib_keys: Vec<String> = options
        .steam_library_roots
        .iter()
        .map(|p| normalize_path_key(p))
        .filter(|s| !s.is_empty())
        .collect();
    let mut manifest_container_keys: HashSet<String> = HashSet::new();
    for lk in &steam_lib_keys {
        manifest_container_keys.insert(lk.clone());
        manifest_container_keys.insert(format!("{lk}\\steamapps"));
        manifest_container_keys.insert(format!("{lk}\\steamapps\\common"));
    }
    for m in options.manifest_entries {
        let ck = normalize_path_key(&m.container_dir);
        if ck.len() > 3 {
            manifest_container_keys.insert(ck);
        }
    }
    let mut exe_children_by_parent: HashMap<String, Vec<(&String, &RootExeCandidate)>> =
        HashMap::new();
    if let Some(exe_map) = options.root_exe_candidates {
        for (pk, cand) in exe_map {
            if let Some((parent, _)) = pk.trim_end_matches('\\').rsplit_once('\\') {
                let p_key = if parent.len() == 2 && parent.ends_with(':') {
                    format!("{parent}\\")
                } else {
                    parent.to_string()
                };
                exe_children_by_parent.entry(p_key).or_default().push((pk, cand));
            }
        }
    }

    // If C:\XboxGames\Minecraft Launcher exists as a scanned non-empty MSIXVC unit without a Win32 Uninstall
    // registry entry, pre-seed the "Minecraft Launcher" bucket so AppData\Roaming\.minecraft (visited earlier
    // alphabetically than C:\XboxGames) routes to "Minecraft Launcher" regardless of NTFS unit order.
    if find_app_matching(&buckets, &["minecraft launcher"]).is_none()
        && scanned_units.iter().any(|u| {
            u.bytes > 0
                && u.kind == ScannedUnitKind::Regular
                && !excluded_drives.contains(&u.drive_letter.to_ascii_uppercase())
                && normalize_path_key(&u.path) == "c:\\xboxgames\\minecraft launcher"
        })
    {
        ensure_synthetic_app(
            &mut buckets,
            &mut store_buckets,
            "xbox:minecraft launcher",
            "Minecraft Launcher",
            "Microsoft Studios",
            None,
            false,
        );
    }

    for unit in scanned_units {
        if unit.bytes == 0 && unit.kind != ScannedUnitKind::RootSystemFile {
            continue;
        }
        let d_letter = unit.drive_letter.to_ascii_uppercase();
        if excluded_drives.contains(&d_letter) {
            continue;
        }

        let is_loose = unit.kind == ScannedUnitKind::ContainerLooseFiles;
        match unit.kind {
            ScannedUnitKind::RootSystemFile => {
                root_sys_by_drive
                    .entry(d_letter)
                    .or_default()
                    .push(BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: None,
                        qualifier: None,
                        basis: None,
                        basis_detail: None,
                        no_touch: Some(NoTouchCategory::SystemRuntime),
                        drive_letter: d_letter,
                    });
                continue;
            }
            ScannedUnitKind::PersonalShell => {
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
                your_files.push(BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: None,
                    qualifier: None,
                    basis: None,
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: d_letter,
                });
                continue;
            }
            ScannedUnitKind::RegistryUserHive => {
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
                let reg_idx = ensure_synthetic_app(
                    &mut buckets,
                    &mut store_buckets,
                    "synth:ntuser-registry",
                    "Windows User Registry (NTUSER.DAT)",
                    "Microsoft Corporation",
                    Some(NoTouchCategory::Registry),
                    false,
                );
                let (zone, _) = classify_zone(&unit.path, false);
                push_item_to_bucket(
                    &mut buckets[reg_idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: Some(LocationQualifier::LooseFiles),
                        basis: Some(AttributionBasis::Token),
                        basis_detail: None,
                        no_touch: Some(NoTouchCategory::Registry),
                        drive_letter: d_letter,
                    },
                    AttributionBasis::Token,
                );
                continue;
            }
            ScannedUnitKind::Regular => {
                let path_lower_pre = normalize_path_key(&unit.path);
                let leaf_pre = leaf_name(&unit.path).to_ascii_lowercase();
                if let Some(os_label) = classify_drive_root_os_folder(&path_lower_pre, &leaf_pre) {
                    os_folders_by_drive
                        .entry(d_letter)
                        .or_default()
                        .push(BreakdownItem {
                            path: unit.path.clone(),
                            bytes: unit.bytes,
                            zone: None,
                            qualifier: None,
                            basis: None,
                            basis_detail: Some(os_label.to_string()),
                            no_touch: Some(NoTouchCategory::SystemRuntime),
                            drive_letter: d_letter,
                        });
                    continue;
                }
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
            }
            ScannedUnitKind::ContainerLooseFiles => {
                *scanned_non_system_by_drive.entry(d_letter).or_default() += unit.bytes;
            }
        }

        // Bottom-up attribution pipeline.
        let path_lower = normalize_path_key(&unit.path);
        let leaf = leaf_name(&unit.path);
        let leaf_lower = leaf.to_ascii_lowercase();

        // Shared/system GREEN caches (including Steam shader/download temp caches when not installed game roots).
        if !is_loose && (is_shared_green_cache(&path_lower) || is_manifest_green_cache(&path_lower, &steam_lib_keys)) {
            let qual = if path_lower.contains(":\\programdata\\") {
                Some(LocationQualifier::MachineShared)
            } else {
                None
            };
            shared_caches.push(BreakdownItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                zone: Some(ZoneKind::Green),
                qualifier: qual,
                basis: None,
                basis_detail: None,
                no_touch: None,
                drive_letter: d_letter,
            });
            continue;
        }

        // Part B(7) early pin check: PinKind::Personal beats even unowned attribution.
        let pin_here = if is_loose {
            None
        } else {
            effective_zones.pin_for_path(&unit.path)
        };
        if pin_here == Some(PinKind::Personal) {
            your_files.push(BreakdownItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                zone: None,
                qualifier: None,
                basis: None,
                basis_detail: None,
                no_touch: None,
                drive_letter: d_letter,
            });
            continue;
        }

        // Step (1): Exact Registered Install Root (deepest exact path first; subtrees claimed later in Step 6).
        if let Some(idx) = find_by_exact_install_root(&buckets, &path_lower) {
            let (zone, default_qual) = classify_zone(&unit.path, true);
            let qual = if is_loose {
                Some(LocationQualifier::InstallLooseFiles)
            } else {
                default_qual
            };
            let nt = buckets[idx].app.no_touch;
            let basis = if buckets[idx].app.id.starts_with("steam:") {
                AttributionBasis::SteamManifest
            } else {
                AttributionBasis::InstallLocation
            };
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(basis),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                basis,
            );
            continue;
        }

        // Step (2): Manifest Container Entries (Steam ACF / Epic .item) + Steam library loose files.
        if let Some(&(idx, m_basis)) = manifest_buckets_by_dir.get(&path_lower) {
            let (zone, default_qual) = classify_zone(&unit.path, true);
            let qual = if is_loose {
                Some(LocationQualifier::InstallLooseFiles)
            } else {
                default_qual
            };
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(m_basis),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                m_basis,
            );
            continue;
        }

        if is_loose && is_steam_library_loose_dir(&path_lower, &steam_lib_keys) {
            if let Some(idx) = find_app_matching(&buckets, &["steam"]) {
                let (zone, _) = classify_zone(&unit.path, true);
                let nt = buckets[idx].app.no_touch;
                push_item_to_bucket(
                    &mut buckets[idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: Some(LocationQualifier::LooseFiles),
                        basis: Some(AttributionBasis::SteamManifest),
                        basis_detail: None,
                        no_touch: nt,
                        drive_letter: d_letter,
                    },
                    AttributionBasis::SteamManifest,
                );
                continue;
            }
        }

        // Part B(3): Remainder loose files of a non-install-root container never pretend to be apps,
        // keep their real path, check curated single-app containers (e.g. ProgramData\NVIDIA) or Step (6)
        // enclosing install root, and otherwise stay honestly Unattributed with `is_loose_files = true`.
        if is_loose {
            if !is_generic_container_path_for_loose(&path_lower, &effective_zones) {
                if let Some((idx, is_install_override)) = find_by_token(
                    &mut buckets,
                    &mut store_buckets,
                    guid_to_app_name,
                    &path_lower,
                    &leaf,
                    &leaf_lower,
                ) {
                    let (zone, _) = classify_zone(&unit.path, is_install_override);
                    let nt = buckets[idx].app.no_touch;
                    push_item_to_bucket(
                        &mut buckets[idx],
                        BreakdownItem {
                            path: unit.path.clone(),
                            bytes: unit.bytes,
                            zone: Some(zone),
                            qualifier: Some(LocationQualifier::LooseFiles),
                            basis: Some(AttributionBasis::Token),
                            basis_detail: None,
                            no_touch: nt,
                            drive_letter: d_letter,
                        },
                        AttributionBasis::Token,
                    );
                    continue;
                }
            }
            if let Some(idx) = find_by_enclosing_install_location(&buckets, &path_lower) {
                let (zone, _) = classify_zone(&unit.path, true);
                let nt = buckets[idx].app.no_touch;
                let basis = if buckets[idx].app.id.starts_with("steam:") {
                    AttributionBasis::SteamManifest
                } else {
                    AttributionBasis::InstallLocation
                };
                push_item_to_bucket(
                    &mut buckets[idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: Some(LocationQualifier::InstallLooseFiles),
                        basis: Some(basis),
                        basis_detail: None,
                        no_touch: nt,
                        drive_letter: d_letter,
                    },
                    basis,
                );
                continue;
            }
            let (zone, _) = classify_zone(&unit.path, false);
            unattributed.push(UnattributedItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                drive_letter: d_letter,
                zone_hint: Some(zone),
                is_loose_files: true,
            });
            continue;
        }

        // Part B(7): Manual Pin = App (when no registered/manifest entry already claimed exact root).
        if pin_here == Some(PinKind::App) {
            let clean_name = clean_container_folder_name(&leaf);
            let app_key = format!("pin:{path_lower}");
            let icon = options
                .root_exe_candidates
                .and_then(|m| m.get(&path_lower))
                .map(|c| c.exe_path.clone());
            let idx = ensure_folder_only_app(
                &mut buckets,
                &mut store_buckets,
                &app_key,
                &clean_name,
                &unit.path,
                icon,
            );
            let (zone, qual) = classify_zone(&unit.path, true);
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::ManualPin),
                    basis_detail: Some("manual pin: App".to_string()),
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::ManualPin,
            );
            continue;
        }

        // Exclude OS metadata like $Recycle.Bin, Config.Msi, System Volume Information from force-matching.
        if is_unattributable_os_noise(&path_lower, &leaf_lower) {
            let (zone, _) = classify_zone(&unit.path, false);
            unattributed.push(UnattributedItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                drive_letter: d_letter,
                zone_hint: Some(zone),
                is_loose_files: false,
            });
            continue;
        }

        // Part B(7): Manual Pin = Ignore overrides all inference below (falls through to Unattributed).
        if pin_here == Some(PinKind::Ignore) {
            let (zone, _) = classify_zone(&unit.path, false);
            unattributed.push(UnattributedItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                drive_letter: d_letter,
                zone_hint: Some(zone),
                is_loose_files: false,
            });
            continue;
        }

        // Step (4): Two-Segment Publisher\Product Composition BEFORE leaf-only matching.
        match find_by_two_segment_composition(&buckets, &unit.path) {
            TwoSegmentMatch::Unique(idx) => {
                let (zone, qual) = classify_zone(&unit.path, unit.is_direct_install_root);
                let nt = buckets[idx].app.no_touch;
                push_item_to_bucket(
                    &mut buckets[idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: qual,
                        basis: Some(AttributionBasis::PublisherProduct),
                        basis_detail: None,
                        no_touch: nt,
                        drive_letter: d_letter,
                    },
                    AttributionBasis::PublisherProduct,
                );
                continue;
            }
            TwoSegmentMatch::AmbiguousTie => {
                let (zone, _) = classify_zone(&unit.path, false);
                unattributed.push(UnattributedItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    drive_letter: d_letter,
                    zone_hint: Some(zone),
                    is_loose_files: false,
                });
                continue;
            }
            TwoSegmentMatch::NoMatch => {}
        }

        // 2b. ExactName leaf match.
        if let Some(idx) = find_by_exact_name(&buckets, &leaf) {
            let (zone, qual) = classify_zone(&unit.path, unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::ExactName),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::ExactName,
            );
            continue;
        }

        // 3. Token match (Package Cache GUID lookup, curated rules, versioned publisher products, and <X>Data rule).
        if let Some((idx, is_install_override)) = find_by_token(
            &mut buckets,
            &mut store_buckets,
            guid_to_app_name,
            &path_lower,
            &leaf,
            &leaf_lower,
        ) {
            let (zone, qual) =
                classify_zone(&unit.path, is_install_override || unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::Token),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::Token,
            );
            continue;
        }

        // 4. Publisher match (specific non-generic publishers only, never "Microsoft Corporation").
        if let Some(idx) = find_by_publisher(&buckets, &leaf) {
            let (zone, qual) = classify_zone(&unit.path, unit.is_direct_install_root);
            let nt = buckets[idx].app.no_touch;
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(AttributionBasis::Publisher),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                AttributionBasis::Publisher,
            );
            continue;
        }

        // 5a. Store Packages fallback (`AppData\Local\Packages\<Pkg>` or `Program Files\WindowsApps\<Pkg>`).
        if (path_lower.contains("\\appdata\\local\\packages\\")
            && !path_lower.ends_with("\\appdata\\local\\packages"))
            || (path_lower.contains("\\program files\\windowsapps\\")
                && !path_lower.ends_with("\\program files\\windowsapps"))
        {
            let is_winapps = path_lower.contains("\\program files\\windowsapps\\");
            if let Some(idx) = attribute_store_package(&mut buckets, &mut store_buckets, &leaf) {
                let (zone, qual) = classify_zone(&unit.path, is_winapps);
                let nt = buckets[idx].app.no_touch;
                push_item_to_bucket(
                    &mut buckets[idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: qual,
                        basis: Some(AttributionBasis::PackageFallback),
                        basis_detail: None,
                        no_touch: nt,
                        drive_letter: d_letter,
                    },
                    AttributionBasis::PackageFallback,
                );
                continue;
            }
        }

        // 5b. Unmatched ProgramData\Package Cache items belong to Shared & system caches (GREEN).
        if path_lower.contains("\\programdata\\package cache\\") {
            shared_caches.push(BreakdownItem {
                path: unit.path.clone(),
                bytes: unit.bytes,
                zone: Some(ZoneKind::Green),
                qualifier: Some(LocationQualifier::MachineShared),
                basis: None,
                basis_detail: None,
                no_touch: None,
                drive_letter: d_letter,
            });
            continue;
        }

        // 5c. User-profile dot-folders and standalone SDKs.
        if is_user_profile_dot_folder(&path_lower, &leaf_lower) {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                &format!("dot:{leaf_lower}"),
                &leaf,
                "User profile dot-folder",
                None,
                false,
            );
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Red),
                    qualifier: None,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        if path_lower.ends_with("\\appdata\\roaming\\flutter")
            || path_lower.ends_with("\\appdata\\local\\flutter")
        {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                "standalone:flutter",
                "flutter",
                "Standalone SDK / toolchain",
                None,
                false,
            );
            let qual = if path_lower.contains("\\appdata\\roaming\\") {
                Some(LocationQualifier::RoamingConfig)
            } else {
                None
            };
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Body),
                    qualifier: qual,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        if path_lower.ends_with("\\appdata\\local\\ms-playwright") {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                "standalone:playwright",
                "Playwright",
                "Standalone toolchain · regenerable dev-tool cache (downloaded browser binaries)",
                None,
                false,
            );
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Green),
                    qualifier: None,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        if path_lower.ends_with("\\appdata\\local\\gitkrakencli") {
            let idx = ensure_synthetic_app(
                &mut buckets,
                &mut store_buckets,
                "standalone:gitkraken-cli",
                "GitKraken CLI",
                "Standalone CLI toolchain (gk.exe)",
                None,
                false,
            );
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::StandaloneFolder),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: d_letter,
                },
                AttributionBasis::StandaloneFolder,
            );
            continue;
        }

        // Step (5): Three-way candidate inference on unclaimed remainder (never promotes manifest non-member siblings).
        if !is_unclaimed_manifest_sibling(&path_lower, &manifest_container_keys) {
            if let Some((display, publisher_ctx, icon, basis, detail)) = infer_step5_app_candidate(
                &unit.path,
                &path_lower,
                &leaf,
                &leaf_lower,
                &effective_zones,
                options.root_exe_candidates,
                &exe_children_by_parent,
            ) {
                let prefix = if basis == AttributionBasis::ContainerRule {
                    "container"
                } else {
                    "exe"
                };
                let app_key = format!("{prefix}:{path_lower}");
                let idx = ensure_folder_only_app(
                    &mut buckets,
                    &mut store_buckets,
                    &app_key,
                    &display,
                    &unit.path,
                    icon,
                );
                if let Some(pub_str) = publisher_ctx {
                    if !buckets[idx].app.publishers.contains(&pub_str) {
                        buckets[idx].app.publishers.push(pub_str);
                    }
                }
                let (zone, qual) = classify_zone(&unit.path, true);
                push_item_to_bucket(
                    &mut buckets[idx],
                    BreakdownItem {
                        path: unit.path.clone(),
                        bytes: unit.bytes,
                        zone: Some(zone),
                        qualifier: qual,
                        basis: Some(basis),
                        basis_detail: Some(detail),
                        no_touch: None,
                        drive_letter: d_letter,
                    },
                    basis,
                );
                continue;
            }
        }

        // Step (6): Containment is the weakest claim — claims only residual unattributed subtrees
        // inside an app's InstallLocation left over after Steps (1)–(5) and Pins.
        if let Some(idx) = find_by_enclosing_install_location(&buckets, &path_lower) {
            let (zone, qual) = classify_zone(&unit.path, true);
            let nt = buckets[idx].app.no_touch;
            let basis = if buckets[idx].app.id.starts_with("steam:") {
                AttributionBasis::SteamManifest
            } else {
                AttributionBasis::InstallLocation
            };
            push_item_to_bucket(
                &mut buckets[idx],
                BreakdownItem {
                    path: unit.path.clone(),
                    bytes: unit.bytes,
                    zone: Some(zone),
                    qualifier: qual,
                    basis: Some(basis),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: d_letter,
                },
                basis,
            );
            continue;
        }

        // Honest remainder: stays Unattributed.
        let (zone, _) = classify_zone(&unit.path, false);
        unattributed.push(UnattributedItem {
            path: unit.path.clone(),
            bytes: unit.bytes,
            drive_letter: d_letter,
            zone_hint: Some(zone),
            is_loose_files: false,
        });
    }

    // Capture all registered display names before filtering zero-byte rows so OS-managed MSI runtimes
    // (such as VC++ 2008/2010 in WinSxS) still resolve `present = true` during dependency analysis.
    let mut installed_registry_names: Vec<String> = Vec::with_capacity(installed_apps.len());
    for app in installed_apps {
        if !installed_registry_names.contains(&app.display_name) {
            installed_registry_names.push(app.display_name.clone());
        }
    }

    // Build AttributedApps (keep all apps with > 0 bytes, disambiguating rare duplicate display names).
    let mut raw_apps: Vec<AttributedApp> = buckets
        .into_iter()
        .filter(|b| b.total_bytes > 0)
        .map(|mut b| {
            b.items.sort_by(|a, c| c.bytes.cmp(&a.bytes));
            b.bases.sort();
            b.bases.dedup();
            let prev = previous_history.and_then(|h| h.get(&b.app.display_name).copied());
            let growth = compute_growth(previous_history.is_some(), prev, b.total_bytes);
            let install_location = b
                .app
                .primary_install_location()
                .filter(|p| {
                    path_drive_letter(p)
                        .is_none_or(|dl| !excluded_drives.contains(&dl.to_ascii_uppercase()))
                })
                .map(|s| s.to_string())
                .or_else(|| {
                    b.items
                        .iter()
                        .find(|i| i.zone == Some(ZoneKind::Body))
                        .map(|i| i.path.clone())
                });
            AttributedApp {
                id: b.app.id,
                display_name: b.app.display_name,
                publishers: b.app.publishers,
                install_location,
                total_bytes: b.total_bytes,
                previous_bytes: prev,
                growth,
                per_drive_growth: BTreeMap::new(),
                no_touch: b.app.no_touch,
                bases: b.bases,
                items: b.items,
                carved_out_apps: Vec::new(),
                dependencies: Vec::new(),
                display_icon: b.app.display_icon,
                icon_rgba: None,
                category: crate::model::AppCategory::default(),
                category_basis: String::new(),
            }
        })
        .collect();

    // Record carved-out nested apps on enclosing apps ("contains <Child> — counted under <Child>").
    let root_entries: Vec<(String, String, Vec<String>)> = raw_apps
        .iter()
        .map(|a| {
            let mut roots: Vec<String> = Vec::new();
            if let Some(ref inst) = a.install_location {
                let k = normalize_path_key(inst);
                if k.len() > 3 {
                    roots.push(k);
                }
            }
            (a.id.clone(), a.display_name.clone(), roots)
        })
        .collect();
    for app_a in &mut raw_apps {
        let Some(ref loc_a) = app_a.install_location else {
            continue;
        };
        let ak = normalize_path_key(loc_a);
        if ak.len() <= 3 {
            continue;
        }
        let mut children_names: Vec<String> = Vec::new();
        for (id_b, name_b, roots_b) in &root_entries {
            if id_b == &app_a.id {
                continue;
            }
            if roots_b
                .iter()
                .any(|bk| bk != &ak && is_subpath_of(bk, &ak))
                && !children_names.contains(name_b)
            {
                children_names.push(name_b.clone());
            }
        }
        children_names.sort();
        app_a.carved_out_apps = children_names;
    }

    // If two folder-only apps happen to share the exact same cleaned display name across folders
    // (e.g. D:\Games\Minecraft vs Z:\Games\Minecraft), add a concise drive/leaf suffix so every row is unique.
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for app in &raw_apps {
        *name_counts.entry(app.display_name.to_ascii_lowercase()).or_default() += 1;
    }
    for app in &mut raw_apps {
        if (app.id.starts_with("container:") || app.id.starts_with("exe:") || app.id.starts_with("pin:"))
            && name_counts
                .get(&app.display_name.to_ascii_lowercase())
                .copied()
                .unwrap_or(0)
                > 1
        {
            if let Some(drv) = app.items.first().map(|i| i.drive_letter) {
                app.display_name = format!("{} ({drv}:)", app.display_name);
            }
        }
    }

    let mut apps = raw_apps;
    apps.sort_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    shared_caches.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let shared_caches_total_bytes = shared_caches.iter().map(|i| i.bytes).sum();

    your_files.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let your_files_total_bytes = your_files.iter().map(|i| i.bytes).sum();

    unattributed.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let unattributed_total_bytes = unattributed.iter().map(|i| i.bytes).sum();

    // Compute per-drive Windows & system breakdown and reconciliation totals.
    let mut windows_system = Vec::new();
    let mut windows_system_total_bytes = 0u64;
    for drv in &drives {
        let d = drv.letter.to_ascii_uppercase();
        let mut root_files = root_sys_by_drive.remove(&d).unwrap_or_default();
        root_files.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        let mut os_folders = os_folders_by_drive.remove(&d).unwrap_or_default();
        os_folders.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        let root_sum: u64 = root_files.iter().map(|i| i.bytes).sum();
        let os_sum: u64 = os_folders.iter().map(|i| i.bytes).sum();
        let scanned_non_sys = *scanned_non_system_by_drive.get(&d).unwrap_or(&0);
        let walked_sum = scanned_non_sys
            .saturating_add(root_sum)
            .saturating_add(os_sum);
        let (remainder_bytes, hardlink_overlap_bytes) = if drv.full_walk_enabled {
            if walked_sum <= drv.used_bytes {
                (drv.used_bytes - walked_sum, 0)
            } else {
                (0, walked_sum - drv.used_bytes)
            }
        } else {
            (0, 0)
        };
        let sys_total = root_sum
            .saturating_add(os_sum)
            .saturating_add(remainder_bytes);
        windows_system_total_bytes = windows_system_total_bytes.saturating_add(sys_total);

        if drv.full_walk_enabled || sys_total > 0 {
            windows_system.push(SystemDriveBreakdown {
                drive_letter: d,
                root_files,
                os_folders,
                remainder_bytes,
                hardlink_overlap_bytes,
                total_bytes: sys_total,
                scanned_non_system_bytes: scanned_non_sys,
                drive_used_bytes: drv.used_bytes,
                drive_free_bytes: drv.free_bytes,
                drive_total_bytes: drv.total_bytes,
            });
        }
    }

    let _ = path_drive_letter; // referenced via model helpers

    ScanReport {
        scan_timestamp_unix: timestamp_unix,
        scan_timestamp_display: timestamp_display,
        duration_ms,
        drives,
        registry_stats,
        apps,
        shared_caches,
        shared_caches_total_bytes,
        your_files,
        your_files_total_bytes,
        windows_system,
        windows_system_total_bytes,
        unattributed,
        unattributed_total_bytes,
        reverse_dependencies: Vec::new(),
        installed_registry_names,
        catalog_source: String::new(),
        warnings: Vec::new(),
        applied_containers: options.containers.cloned(),
        applied_zones: Some(effective_zones),
    }
}

/// Merges a single-drive scan (`refreshed_drive`) into `base_report` without touching other drives'
/// items, timestamps, or unchanged growth baselines, then recomputes totals, reconciliation, and
/// affected per-drive growth deltas against `history`.
#[must_use]
pub fn merge_drive_rescan(
    base_report: &ScanReport,
    refreshed_drive: char,
    single_drive_report: ScanReport,
    history: &ScanHistory,
) -> ScanReport {
    let target = refreshed_drive.to_ascii_uppercase();

    // 1. Drives: preserve untouched drives (including their `last_scanned_*`), replace `target`.
    let mut merged_drives: Vec<DriveInfo> = Vec::new();
    for base_drv in &base_report.drives {
        let dl = base_drv.letter.to_ascii_uppercase();
        if dl == target {
            if let Some(fresh) = single_drive_report
                .drives
                .iter()
                .find(|d| d.letter.to_ascii_uppercase() == target)
            {
                merged_drives.push(fresh.clone());
            } else {
                merged_drives.push(base_drv.clone());
            }
        } else {
            merged_drives.push(base_drv.clone());
        }
    }
    for fresh in &single_drive_report.drives {
        let dl = fresh.letter.to_ascii_uppercase();
        if !merged_drives
            .iter()
            .any(|d| d.letter.to_ascii_uppercase() == dl)
        {
            merged_drives.push(fresh.clone());
        }
    }
    merged_drives.sort_by_key(|d| d.letter.to_ascii_uppercase());

    // 2. Apps: preserve untouched drives' items and untouched apps verbatim; merge fresh items on `target`.
    let mut touched_app_ids: HashSet<String> = HashSet::new();
    let mut merged_apps: Vec<AttributedApp> = Vec::new();

    for old_app in &base_report.apps {
        let had_on_target = old_app
            .items
            .iter()
            .any(|i| i.drive_letter.to_ascii_uppercase() == target);
        let remaining_items: Vec<BreakdownItem> = old_app
            .items
            .iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() != target)
            .cloned()
            .collect();
        if remaining_items.is_empty() && !had_on_target {
            continue;
        }
        if !remaining_items.is_empty() {
            let mut updated = old_app.clone();
            updated.items = remaining_items;
            updated.total_bytes = updated.items.iter().map(|i| i.bytes).sum();
            if had_on_target {
                touched_app_ids.insert(updated.id.clone());
                let mut bases: Vec<AttributionBasis> =
                    updated.items.iter().filter_map(|i| i.basis).collect();
                bases.sort();
                bases.dedup();
                updated.bases = bases;
            }
            merged_apps.push(updated);
        }
    }

    for fresh_app in single_drive_report.apps {
        let fresh_items_on_target: Vec<BreakdownItem> = fresh_app
            .items
            .into_iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target)
            .collect();
        if fresh_items_on_target.is_empty() {
            continue;
        }
        let existing_pos = merged_apps
            .iter()
            .position(|a| a.id == fresh_app.id)
            .or_else(|| {
                let fn_tok = normalize_app_token(&fresh_app.display_name);
                (!fn_tok.is_empty()).then(|| {
                    merged_apps.iter().position(|a| {
                        a.display_name.eq_ignore_ascii_case(&fresh_app.display_name)
                    })
                }).flatten()
            });

        if let Some(pos) = existing_pos {
            let ex = &mut merged_apps[pos];
            touched_app_ids.insert(ex.id.clone());
            ex.items.extend(fresh_items_on_target);
            ex.items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
            ex.total_bytes = ex.items.iter().map(|i| i.bytes).sum();
            let mut bases: Vec<AttributionBasis> =
                ex.items.iter().filter_map(|i| i.basis).collect();
            bases.sort();
            bases.dedup();
            ex.bases = bases;
            if ex.icon_rgba.is_none() {
                ex.icon_rgba = fresh_app.icon_rgba;
            }
            if ex.install_location.is_none() {
                ex.install_location = fresh_app.install_location;
            }
            if ex.dependencies.is_empty() && !fresh_app.dependencies.is_empty() {
                ex.dependencies = fresh_app.dependencies;
            }
        } else {
            let total_bytes: u64 = fresh_items_on_target.iter().map(|i| i.bytes).sum();
            let mut bases: Vec<AttributionBasis> =
                fresh_items_on_target.iter().filter_map(|i| i.basis).collect();
            bases.sort();
            bases.dedup();
            touched_app_ids.insert(fresh_app.id.clone());
            merged_apps.push(AttributedApp {
                id: fresh_app.id,
                display_name: fresh_app.display_name,
                publishers: fresh_app.publishers,
                install_location: fresh_app.install_location,
                total_bytes,
                previous_bytes: fresh_app.previous_bytes,
                growth: fresh_app.growth,
                per_drive_growth: fresh_app.per_drive_growth,
                no_touch: fresh_app.no_touch,
                bases,
                items: fresh_items_on_target,
                carved_out_apps: fresh_app.carved_out_apps,
                dependencies: fresh_app.dependencies,
                display_icon: fresh_app.display_icon,
                icon_rgba: fresh_app.icon_rgba,
                category: fresh_app.category,
                category_basis: fresh_app.category_basis,
            });
        }
    }

    merged_apps.sort_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    // 3. Shared caches, Your files, Unattributed, Windows & system: replace only `target`.
    let mut merged_caches: Vec<BreakdownItem> = base_report
        .shared_caches
        .iter()
        .filter(|i| i.drive_letter.to_ascii_uppercase() != target)
        .cloned()
        .collect();
    merged_caches.extend(
        single_drive_report
            .shared_caches
            .into_iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target),
    );
    merged_caches.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let shared_caches_total_bytes = merged_caches.iter().map(|i| i.bytes).sum();

    let mut merged_your_files: Vec<BreakdownItem> = base_report
        .your_files
        .iter()
        .filter(|i| i.drive_letter.to_ascii_uppercase() != target)
        .cloned()
        .collect();
    merged_your_files.extend(
        single_drive_report
            .your_files
            .into_iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target),
    );
    merged_your_files.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let your_files_total_bytes = merged_your_files.iter().map(|i| i.bytes).sum();

    let mut merged_unattrib: Vec<UnattributedItem> = base_report
        .unattributed
        .iter()
        .filter(|i| i.drive_letter.to_ascii_uppercase() != target)
        .cloned()
        .collect();
    merged_unattrib.extend(
        single_drive_report
            .unattributed
            .into_iter()
            .filter(|i| i.drive_letter.to_ascii_uppercase() == target),
    );
    merged_unattrib.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let unattributed_total_bytes = merged_unattrib.iter().map(|i| i.bytes).sum();

    let mut merged_sys: Vec<SystemDriveBreakdown> = base_report
        .windows_system
        .iter()
        .filter(|s| s.drive_letter.to_ascii_uppercase() != target)
        .cloned()
        .collect();
    merged_sys.extend(
        single_drive_report
            .windows_system
            .into_iter()
            .filter(|s| s.drive_letter.to_ascii_uppercase() == target),
    );
    merged_sys.sort_by_key(|s| s.drive_letter.to_ascii_uppercase());
    let windows_system_total_bytes = merged_sys.iter().map(|s| s.total_bytes).sum();

    let mut merged = ScanReport {
        scan_timestamp_unix: single_drive_report.scan_timestamp_unix,
        scan_timestamp_display: single_drive_report.scan_timestamp_display,
        duration_ms: single_drive_report.duration_ms,
        drives: merged_drives,
        registry_stats: single_drive_report.registry_stats,
        apps: merged_apps,
        shared_caches: merged_caches,
        shared_caches_total_bytes,
        your_files: merged_your_files,
        your_files_total_bytes,
        windows_system: merged_sys,
        windows_system_total_bytes,
        unattributed: merged_unattrib,
        unattributed_total_bytes,
        reverse_dependencies: base_report.reverse_dependencies.clone(),
        installed_registry_names: if single_drive_report.installed_registry_names.is_empty() {
            base_report.installed_registry_names.clone()
        } else {
            single_drive_report.installed_registry_names
        },
        catalog_source: if single_drive_report.catalog_source.is_empty() {
            base_report.catalog_source.clone()
        } else {
            single_drive_report.catalog_source
        },
        warnings: single_drive_report.warnings,
        applied_containers: single_drive_report
            .applied_containers
            .or_else(|| base_report.applied_containers.clone()),
        applied_zones: single_drive_report
            .applied_zones
            .or_else(|| base_report.applied_zones.clone()),
    };

    // Preserve growth baselines for untouched apps, and recompute per-drive growth strictly for
    // apps touched on `target` against prior snapshots that actually scanned each drive.
    let saved_untouched: HashMap<String, (Option<u64>, GrowthDelta)> = merged
        .apps
        .iter()
        .filter(|a| !touched_app_ids.contains(&a.id))
        .map(|a| (a.id.clone(), (a.previous_bytes, a.growth)))
        .collect();
    recompute_report_growth_from_history(&mut merged, history);
    for app in &mut merged.apps {
        if let Some(&(prev_b, prev_g)) = saved_untouched.get(&app.id) {
            app.previous_bytes = prev_b;
            app.growth = prev_g;
        }
    }

    merged
}

fn ensure_folder_only_app(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    key: &str,
    display_name: &str,
    install_location: &str,
    display_icon: Option<String>,
) -> usize {
    if let Some(&idx) = store_buckets.get(key) {
        return idx;
    }
    let idx = buckets.len();
    let loc = install_location.to_string();
    buckets.push(AppBucket {
        app: InstalledApp {
            id: key.to_string(),
            display_name: display_name.to_string(),
            display_version: String::new(),
            uninstall_guids: Vec::new(),
            aliases: Vec::new(),
            publishers: Vec::new(),
            direct_install_location: Some(loc.clone()),
            inferred_install_location: None,
            all_install_locations: vec![loc],
            no_touch: None,
            is_store_fallback: false,
            display_icon,
        },
        bases: Vec::new(),
        items: Vec::new(),
        total_bytes: 0,
    });
    store_buckets.insert(key.to_string(), idx);
    idx
}

/// Cleans a container child or folder name into a readable app title while preserving distinct versions.
#[must_use]
pub fn clean_container_folder_name(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    for suffix in ["_LinkNeverDie.Com", ".LinkNeverDie.Com", "_LinkNeverDie", "-LinkNeverDie"] {
        if let Some(prefix) = s.strip_suffix(suffix) {
            s = prefix.to_string();
        }
    }
    s = s.replace('_', " ");
    // Convert dots acting as word separators (adjacent to ASCII letters, except `Node.js` / `v1.3`) into spaces.
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(chars.len());
    for i in 0..chars.len() {
        let ch = chars[i];
        if ch == '.' {
            let prev = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
            let next = chars.get(i + 1).copied();
            let is_version_dot = prev.is_some_and(|c| c.is_ascii_digit())
                && next.is_some_and(|c| c.is_ascii_digit());
            let prev_is_v = prev.is_some_and(|c| c == 'v' || c == 'V')
                && next.is_some_and(|c| c.is_ascii_digit());
            if is_version_dot || prev_is_v {
                out.push('.');
            } else if prev.is_some() && next.is_some() {
                out.push(' ');
            } else {
                out.push('.');
            }
        } else {
            out.push(ch);
        }
    }
    let clean = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        raw.trim().to_string()
    } else {
        clean
    }
}

fn is_drive_root_child(path_lower: &str) -> bool {
    let trimmed = path_lower.trim_end_matches('\\');
    let parts: Vec<&str> = trimmed.split('\\').collect();
    parts.len() == 2 && parts[0].len() == 2 && parts[0].ends_with(':')
}

fn is_steam_library_loose_dir(path_lower: &str, steam_lib_keys: &[String]) -> bool {
    let norm = path_lower.trim_end_matches('\\');
    steam_lib_keys
        .iter()
        .any(|lk| norm == lk || norm == format!("{lk}\\steamapps"))
}

fn is_manifest_green_cache(path_lower: &str, steam_lib_keys: &[String]) -> bool {
    steam_lib_keys.iter().any(|lk| {
        path_lower == format!("{lk}\\steamapps\\shadercache")
            || path_lower == format!("{lk}\\steamapps\\temp")
    })
}

fn is_unclaimed_manifest_sibling(path_lower: &str, manifest_containers: &HashSet<String>) -> bool {
    let trimmed = path_lower.trim_end_matches('\\');
    let Some((parent, _)) = trimmed.rsplit_once('\\') else {
        return false;
    };
    manifest_containers.contains(parent)
}

fn is_generic_container_path_for_loose(path_lower: &str, zones: &ZoneRulebook) -> bool {
    let pk = path_lower.trim_end_matches('\\');
    if pk.len() <= 2 {
        return true;
    }
    if zones.matches_app_container_dir(path_lower).is_some()
        || zones.matches_personal_dir(path_lower, &[])
    {
        return true;
    }
    matches!(
        pk.split('\\').last().unwrap_or(""),
        "users"
            | "appdata"
            | "local"
            | "roaming"
            | "locallow"
            | "programs"
            | "packages"
            | "windowsapps"
            | "program files"
            | "program files (x86)"
            | "common files"
            | "programdata"
    ) || pk.ends_with("\\common files\\steam")
        || pk.ends_with("\\appdata\\local\\steam")
        || pk.ends_with("\\appdata\\roaming\\inkscape")
        || pk.ends_with("\\microsoft\\edge")
        || pk.ends_with(":\\program files\\amd")
        || pk.ends_with(":\\program files\\nvidia corporation")
}

fn infer_step5_app_candidate(
    unit_path: &str,
    path_lower: &str,
    leaf: &str,
    leaf_lower: &str,
    zones: &ZoneRulebook,
    exe_candidates: Option<&HashMap<String, RootExeCandidate>>,
    exe_children_by_parent: &HashMap<String, Vec<(&String, &RootExeCandidate)>>,
) -> Option<(String, Option<String>, Option<String>, AttributionBasis, String)> {
    let trimmed = unit_path.trim_end_matches('\\');
    let parent_path = trimmed
        .rsplit_once('\\')
        .map(|(p, _)| if p.len() == 2 && p.ends_with(':') { format!("{p}\\") } else { p.to_string() })
        .unwrap_or_default();
    let parent_key = normalize_path_key(&parent_path);
    let cand_here = exe_candidates.and_then(|m| m.get(path_lower));
    let child_exes_here = exe_children_by_parent
        .get(path_lower)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    // 1. Child of a Group node (explicit PinKind::Group, OR Drive-Root / AppContainer child whose
    // parent has no own-level exe and >= 2 non-internal children passing 5(a)). An AppContainer itself
    // is never treated as a publisher Group node.
    let parent_pin_group = zones.pin_for_path(&parent_path) == Some(PinKind::Group);
    let parent_is_app_container = if let Some((gp, _)) = parent_path.trim_end_matches('\\').rsplit_once('\\') {
        zones.matches_app_container_dir(gp).is_some()
    } else {
        false
    };
    let parent_auto_group = zones.matches_app_container_dir(&parent_path).is_none()
        && (is_drive_root_child(&parent_key) || parent_is_app_container)
        && exe_candidates.is_some_and(|m| !m.contains_key(&parent_key))
        && exe_children_by_parent.get(&parent_key).is_some_and(|v| {
            v.iter()
                .filter(|(cpk, _)| {
                    !is_non_app_internal_subdir_name(&leaf_name(cpk).to_ascii_lowercase())
                })
                .count()
                >= 2
        });
    if parent_pin_group || parent_auto_group {
        if is_program_files_inbox_folder(path_lower, leaf_lower) {
            return None;
        }
        if cand_here.is_some() || parent_pin_group {
            let group_name = clean_container_folder_name(&leaf_name(&parent_path));
            let display = cand_here
                .and_then(|c| c.product_name.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| clean_container_folder_name(leaf));
            let icon = cand_here.map(|c| c.exe_path.clone());
            let detail = if parent_pin_group {
                format!("manual pin Group: {group_name}")
            } else {
                format!("group node: {group_name}")
            };
            return Some((
                display,
                (!group_name.is_empty()).then_some(group_name),
                icon,
                AttributionBasis::ExecutableScan,
                detail,
            ));
        }
        return None;
    }

    // 2. User-declared AppContainer immediate children: preserve user-organized folder names and never overwrite with wrapper exe names.
    if let Some(container_name) = zones.matches_app_container_dir(&parent_path) {
        if leaf_lower == "steam"
            || leaf_lower == "steamlibrary"
            || is_program_files_inbox_folder(path_lower, leaf_lower)
        {
            return None;
        }
        let icon = cand_here
            .map(|c| c.exe_path.clone())
            .or_else(|| child_exes_here.first().map(|(_, c)| c.exe_path.clone()));
        return Some((
            clean_container_folder_name(leaf),
            None,
            icon,
            AttributionBasis::ContainerRule,
            format!("layout rule: {container_name}"),
        ));
    }

    // 3. Drive-root candidate (Part A Drive Root Rule + Part B 5(a) own exe / 5(c) unsplit wrapper).
    if is_drive_root_child(path_lower) {
        if let Some(c) = cand_here {
            let display = c
                .product_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| clean_container_folder_name(leaf));
            return Some((
                display,
                None,
                Some(c.exe_path.clone()),
                AttributionBasis::ExecutableScan,
                "executable scan (inferred)".to_string(),
            ));
        }
        if !child_exes_here.is_empty() {
            return Some((
                clean_container_folder_name(leaf),
                None,
                Some(child_exes_here[0].1.exe_path.clone()),
                AttributionBasis::ExecutableScan,
                "single-app wrapper (inferred)".to_string(),
            ));
        }
    }

    None
}

enum TwoSegmentMatch {
    Unique(usize),
    AmbiguousTie,
    NoMatch,
}

fn is_structural_two_segment_parent(p_lower: &str) -> bool {
    (p_lower.len() == 2 && p_lower.ends_with(':'))
        || matches!(
            p_lower,
            "users"
                | "appdata"
                | "local"
                | "roaming"
                | "locallow"
                | "programdata"
                | "program files"
                | "program files (x86)"
                | "common files"
                | "programs"
                | "packages"
                | "windowsapps"
        )
}

fn find_by_two_segment_composition(buckets: &[AppBucket], path: &str) -> TwoSegmentMatch {
    let trimmed = path.trim_end_matches('\\');
    let parts: Vec<&str> = trimmed.split('\\').collect();
    if parts.len() < 3 {
        return TwoSegmentMatch::NoMatch;
    }
    let parent_seg = parts[parts.len() - 2];
    let leaf_seg = parts[parts.len() - 1];
    let parent_lower = parent_seg.to_ascii_lowercase();
    if is_structural_two_segment_parent(&parent_lower) {
        return TwoSegmentMatch::NoMatch;
    }
    let p_words = split_alnum_words(parent_seg);
    let l_words = split_alnum_words(leaf_seg);
    if p_words.is_empty() || l_words.is_empty() {
        return TwoSegmentMatch::NoMatch;
    }
    let mut composed = p_words.clone();
    composed.extend(l_words.clone());

    let p_norm = normalize_app_token(parent_seg);
    let l_norm = normalize_app_token(leaf_seg);
    let composed_norm = if !p_norm.is_empty() && !l_norm.is_empty() {
        format!("{p_norm} {l_norm}")
    } else {
        String::new()
    };

    let mut matched: Vec<(usize, bool)> = Vec::new();
    for (i, b) in buckets.iter().enumerate() {
        if b.app.is_store_fallback || b.app.id.starts_with("synth:") {
            continue;
        }
        let exact_words = split_alnum_words(&b.app.display_name) == composed
            || b.app
                .aliases
                .iter()
                .any(|al| split_alnum_words(al) == composed);
        let norm_eq = !composed_norm.is_empty()
            && (normalize_app_token(&b.app.display_name) == composed_norm
                || b.app
                    .aliases
                    .iter()
                    .any(|al| normalize_app_token(al) == composed_norm));
        if exact_words || norm_eq {
            let pub_hit = b
                .app
                .publishers
                .iter()
                .any(|p| contains_whole_token_sequence(p, parent_seg));
            matched.push((i, pub_hit));
        }
    }

    if matched.is_empty() {
        TwoSegmentMatch::NoMatch
    } else if matched.len() == 1 {
        TwoSegmentMatch::Unique(matched[0].0)
    } else {
        TwoSegmentMatch::AmbiguousTie
    }
}

#[must_use]
pub fn classify_drive_root_os_folder(path_lower: &str, leaf_lower: &str) -> Option<&'static str> {
    if !is_drive_root_child(path_lower) {
        return None;
    }
    if leaf_lower.starts_with("$recycle.bin") {
        return Some("Recycle Bin (OS system folder)");
    }
    if leaf_lower == "system volume information" {
        return Some("System Volume Information (Shadow Copy / restore points)");
    }
    if leaf_lower == "recovery" || leaf_lower == "$winreagent" {
        return Some("Windows Recovery Environment (OS system folder)");
    }
    if leaf_lower == "config.msi" {
        return Some("Windows Installer rollback cache");
    }
    if leaf_lower == "$windows.~bt" || leaf_lower == "$windows.~ws" || leaf_lower == "$sysreset" {
        return Some("Windows Setup / Reset temporary system folder");
    }
    if leaf_lower == "perflogs" || leaf_lower == "msocache" || leaf_lower == ".gamingroot" || leaf_lower == "documents and settings" {
        return Some("Windows OS system folder / metadata");
    }
    if leaf_lower.starts_with("found.") && leaf_lower[6..].chars().all(|c| c.is_ascii_digit()) {
        return Some("CHKDSK recovery fragments");
    }
    None
}

fn push_item_to_bucket(bucket: &mut AppBucket, item: BreakdownItem, basis: AttributionBasis) {
    bucket.total_bytes = bucket.total_bytes.saturating_add(item.bytes);
    if !bucket.bases.contains(&basis) {
        bucket.bases.push(basis);
    }
    bucket.items.push(item);
}

fn leaf_name(path: &str) -> String {
    let trimmed = path.trim_end_matches('\\');
    trimmed
        .rsplit('\\')
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

fn is_shared_green_cache(path_lower: &str) -> bool {
    let basic = path_lower.ends_with("\\appdata\\local\\temp")
        || path_lower.contains("\\appdata\\local\\temp\\")
        || path_lower.ends_with("\\appdata\\locallow\\temp")
        || path_lower.contains("\\appdata\\locallow\\temp\\")
        || path_lower.ends_with("\\appdata\\local\\d3dscache")
        || path_lower.ends_with("\\appdata\\local\\nvidia\\dxcache")
        || path_lower.ends_with("\\appdata\\local\\nvidia\\glcache")
        || path_lower.ends_with("\\appdata\\local\\amd\\dxcache")
        || path_lower.ends_with("\\appdata\\local\\amd\\dxccache")
        || path_lower.ends_with("\\appdata\\local\\amd\\vkcache")
        || path_lower.ends_with("\\appdata\\local\\crashdumps")
        || path_lower.ends_with("\\appdata\\local\\squirreltemp")
        || path_lower.ends_with("\\appdata\\local\\cache")
        || path_lower.ends_with("\\programdata\\epicinstallertemp");
    if basic {
        return true;
    }

    let win_caches = [
        "\\appdata\\local\\microsoft\\windows\\webcache",
        "\\appdata\\local\\microsoft\\windows\\explorer",
        "\\appdata\\local\\microsoft\\windows\\inetcache",
        "\\appdata\\local\\microsoft\\windows\\caches",
        "\\appdata\\local\\microsoft\\windows\\actioncentercache",
        "\\appdata\\local\\microsoft\\windows\\pricache",
        "\\appdata\\local\\microsoft\\windows\\appcache",
        "\\appdata\\local\\microsoft\\windows\\iecompatcache",
        "\\appdata\\local\\microsoft\\windows\\iecompatuacache",
        "\\appdata\\local\\microsoft\\windows\\ppbcompatcache",
        "\\appdata\\local\\microsoft\\windows\\ppbcompatuacache",
        "\\appdata\\local\\microsoft\\windows\\temporary internet files",
    ];
    win_caches
        .iter()
        .any(|sfx| path_lower.ends_with(sfx) || path_lower.contains(&format!("{sfx}\\")))
}

fn is_user_profile_dot_folder(path_lower: &str, leaf_lower: &str) -> bool {
    if !leaf_lower.starts_with('.') || leaf_lower == ".[files]" || leaf_lower == "." || leaf_lower == ".." {
        return false;
    }
    let parts: Vec<&str> = path_lower.trim_end_matches('\\').split('\\').collect();
    parts.len() == 4 && parts[1] == "users"
}

fn is_unattributable_os_noise(path_lower: &str, leaf_lower: &str) -> bool {
    if leaf_lower.starts_with("$recycle.bin")
        || leaf_lower == "config.msi"
        || leaf_lower == "system volume information"
        || leaf_lower == "recovery"
        || leaf_lower == ".gamingroot"
        || leaf_lower == "usoshared"
        || leaf_lower == "usoprivate"
        || leaf_lower == "softwaredistribution"
        || leaf_lower == "whesvc"
        || leaf_lower == "windowsholographicdevices"
        || leaf_lower == "elevateddiagnostics"
        || leaf_lower == "connecteddevicesplatform"
    {
        return true;
    }
    // Generic shared Temp / CrashDumps / Package Cache root files without app owner stay unattributed.
    if path_lower.ends_with("\\appdata\\local\\temp")
        || path_lower.ends_with("\\appdata\\locallow\\temp")
        || path_lower.ends_with("\\appdata\\local\\d3dscache")
        || path_lower.ends_with("\\appdata\\local\\crashdumps")
        || path_lower.ends_with("\\appdata\\roaming\\microsoft\\windows")
        || path_lower.ends_with("\\appdata\\roaming\\microsoft\\installer")
        || path_lower.ends_with("\\appdata\\roaming\\microsoft\\crypto")
        || path_lower.ends_with("\\appdata\\local\\packages")
        || path_lower.ends_with("\\program files\\windowsapps")
    {
        return true;
    }
    false
}

fn find_by_exact_install_root(buckets: &[AppBucket], path_lower: &str) -> Option<usize> {
    for (i, b) in buckets.iter().enumerate() {
        for loc in &b.app.all_install_locations {
            let lk = normalize_path_key(loc);
            if lk.len() >= 4 && path_lower == lk {
                return Some(i);
            }
        }
    }
    None
}

fn find_by_enclosing_install_location(buckets: &[AppBucket], path_lower: &str) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, b) in buckets.iter().enumerate() {
        if b.app.id.starts_with("container:")
            || b.app.id.starts_with("exe:")
            || b.app.id.starts_with("pin:")
            || b.app.id.starts_with("synth:")
        {
            continue;
        }
        for loc in &b.app.all_install_locations {
            let lk = normalize_path_key(loc);
            if lk.len() < 4 {
                continue;
            }
            if path_lower != lk && is_subpath_of(path_lower, &lk) {
                if best.is_none_or(|(_, best_len)| lk.len() > best_len) {
                    best = Some((i, lk.len()));
                }
            }
        }
    }
    best.map(|(idx, _)| idx)
}

fn find_by_exact_name(buckets: &[AppBucket], leaf: &str) -> Option<usize> {
    let leaf_tok = normalize_app_token(leaf);
    if leaf_tok.len() < 3 {
        return None;
    }
    for (i, b) in buckets.iter().enumerate() {
        let name_tok = normalize_app_token(&b.app.display_name);
        if name_tok == leaf_tok {
            return Some(i);
        }
        for alias in &b.app.aliases {
            if normalize_app_token(alias) == leaf_tok {
                return Some(i);
            }
        }
    }
    None
}

/// Selects an `InstalledApp` index using token-boundary matching.
/// Prefers exact full-token matches over whole-word slice matches, never matches substrings inside
/// longer words, and returns `None` when multiple unrelated apps tie ambiguously at the winning tier.
#[must_use]
pub fn match_installed_app_index(apps: &[InstalledApp], predicates: &[&str]) -> Option<usize> {
    match_installed_app_by(apps.iter(), predicates)
}

fn match_installed_app_by<'a>(
    apps: impl Iterator<Item = &'a InstalledApp> + Clone,
    predicates: &[&str],
) -> Option<usize> {
    for &pred in predicates {
        let mut exact_hits: Vec<(usize, &'a InstalledApp)> = Vec::new();
        let mut slice_hits: Vec<(usize, &'a InstalledApp)> = Vec::new();
        for (i, app) in apps.clone().enumerate() {
            let mut best = match_app_predicate(&app.display_name, pred);
            for alias in &app.aliases {
                if let Some(kind) = match_app_predicate(alias, pred) {
                    if best.is_none_or(|prev| kind > prev) {
                        best = Some(kind);
                    }
                }
            }
            match best {
                Some(AppTokenMatchKind::ExactFullToken) => exact_hits.push((i, app)),
                Some(AppTokenMatchKind::WholeWordSlice) => slice_hits.push((i, app)),
                None => {}
            }
        }
        let candidates = if !exact_hits.is_empty() {
            exact_hits
        } else {
            slice_hits
        };
        if candidates.is_empty() {
            continue;
        }
        if candidates.len() == 1 {
            return Some(candidates[0].0);
        }
        // Only resolve multi-row ties automatically when every candidate is a split row of the
        // exact same canonical versioned product family (e.g. .NET SDK 7 & 10); otherwise refuse
        // to force-match so genuinely ambiguous folders fall through cleanly to Unattributed.
        let family_keys: Vec<Option<String>> = candidates
            .iter()
            .map(|(_, app)| {
                versioned_family_identity_key(&app.display_name, &app.display_version)
                    .and_then(|fk| fk.split(':').next().map(str::to_string))
            })
            .collect();
        if let Some(Some(first_fam)) = family_keys.first() {
            if family_keys.iter().all(|k| k.as_deref() == Some(first_fam.as_str())) {
                return Some(candidates[0].0);
            }
        }
        return None;
    }
    None
}

fn find_app_matching(buckets: &[AppBucket], predicates: &[&str]) -> Option<usize> {
    match_installed_app_by(buckets.iter().map(|b| &b.app), predicates)
}

fn ensure_synthetic_app(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    key: &str,
    display_name: &str,
    publisher: &str,
    no_touch: Option<NoTouchCategory>,
    is_store_fallback: bool,
) -> usize {
    if let Some(&idx) = store_buckets.get(key) {
        return idx;
    }
    let idx = buckets.len();
    buckets.push(AppBucket {
        app: InstalledApp {
            id: key.to_string(),
            display_name: display_name.to_string(),
            display_version: String::new(),
            uninstall_guids: Vec::new(),
            aliases: Vec::new(),
            publishers: if publisher.is_empty() {
                Vec::new()
            } else {
                vec![publisher.to_string()]
            },
            direct_install_location: None,
            inferred_install_location: None,
            all_install_locations: Vec::new(),
            no_touch,
            is_store_fallback,
            display_icon: None,
        },
        bases: Vec::new(),
        items: Vec::new(),
        total_bytes: 0,
    });
    store_buckets.insert(key.to_string(), idx);
    idx
}

fn extract_pkg_cache_version(leaf_lower: &str) -> Option<String> {
    let idx = leaf_lower.rfind("}v")?;
    let ver = leaf_lower[idx + 2..].trim();
    if ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        Some(ver.to_string())
    } else {
        None
    }
}

fn extract_arch_from_text(lower: &str) -> Option<&'static str> {
    if lower.contains("x64") || lower.contains("amd64") || lower.contains("64-bit") {
        Some("x64")
    } else if lower.contains("arm64") {
        Some("arm64")
    } else if lower.contains("x86") || lower.contains("32-bit") {
        Some("x86")
    } else {
        None
    }
}

fn extract_ver_token_from_text(lower: &str) -> Option<String> {
    for raw_tok in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')) {
        let tok = raw_tok.trim_start_matches('v').trim_matches('.');
        if tok.contains('.') && tok.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if tok.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return Some(tok.to_string());
            }
        }
    }
    None
}

fn parse_version_tuple(ver: &str) -> [u64; 4] {
    let mut nums = [0u64; 4];
    for (i, part) in ver.trim_start_matches('v').split('.').take(4).enumerate() {
        nums[i] = part.parse::<u64>().unwrap_or(0);
    }
    nums
}

fn versions_match(ver_a: &str, ver_b: &str) -> bool {
    let a = parse_version_tuple(ver_a);
    let b = parse_version_tuple(ver_b);
    if a == [0; 4] || b == [0; 4] {
        return false;
    }
    // Match exact 3-part (major.minor.build) when 4th part is optional (e.g. 12.0.40664.0 vs 12.0.40664)
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && (a[3] == b[3] || a[3] == 0 || b[3] == 0)
}

fn app_effective_version(app: &InstalledApp) -> Option<String> {
    let dn_lower = app.display_name.to_ascii_lowercase();
    extract_ver_token_from_text(&dn_lower).or_else(|| {
        let dv = app.display_version.trim();
        (!dv.is_empty()).then(|| dv.to_ascii_lowercase())
    })
}

fn find_best_versioned_bucket(
    buckets: &[AppBucket],
    filter_pred: impl Fn(&str) -> bool,
    target_ver: Option<&str>,
    target_arch: Option<&str>,
) -> Option<usize> {
    let mut best: Option<(usize, i32, [u64; 4])> = None;
    for (i, b) in buckets.iter().enumerate() {
        let dn = b.app.display_name.to_ascii_lowercase();
        if !filter_pred(&dn) {
            continue;
        }
        let b_ver = app_effective_version(&b.app);
        let b_arch = extract_arch_from_text(&dn);
        let ver_score = match (target_ver, b_ver.as_deref()) {
            (Some(tv), Some(bv)) if versions_match(tv, bv) => 100,
            (Some(tv), Some(bv)) => {
                let ta = parse_version_tuple(tv);
                let ba = parse_version_tuple(bv);
                if ta[0] != 0 && ta[0] == ba[0] && ta[1] == ba[1] {
                    30
                } else if ta[0] != 0 && ta[0] == ba[0] {
                    10
                } else {
                    0
                }
            }
            _ => 0,
        };
        let arch_score = match (target_arch, b_arch) {
            (Some(ta), Some(ba)) if ta == ba => 20,
            (Some(_), Some(_)) => -5,
            _ => 0,
        };
        let score = ver_score + arch_score;
        let vt = b_ver.as_deref().map(parse_version_tuple).unwrap_or([0; 4]);
        if best.is_none_or(|(_, bs, bvt)| score > bs || (score == bs && vt > bvt)) {
            best = Some((i, score, vt));
        }
    }
    best.map(|(i, _, _)| i)
}

fn find_by_token(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    guid_to_app_name: &HashMap<String, String>,
    path_lower: &str,
    leaf: &str,
    leaf_lower: &str,
) -> Option<(usize, bool)> {
    // 1. Package Cache {GUID} + version/arch-aware routing (never cross-route between split versions).
    if path_lower.contains("\\programdata\\package cache\\") {
        let opt_guid = extract_guid(leaf_lower);
        let opt_cache_ver = extract_pkg_cache_version(leaf_lower);

        // 1a. Exact Uninstall bundle GUID match for split versioned families.
        if let Some(ref g) = opt_guid {
            if let Some(idx) = buckets.iter().position(|b| {
                b.app.uninstall_guids.contains(g)
                    && crate::discovery::versioned_family_identity_key(
                        &b.app.display_name,
                        &b.app.display_version,
                    )
                    .is_some()
            }) {
                return Some((idx, false));
            }
        }

        if let Some(ref g) = opt_guid {
            if let Some(app_name) = guid_to_app_name.get(g) {
                let an_lower = app_name.to_ascii_lowercase();
                if contains_whole_token_sequence(&an_lower, "ea app") {
                    if let Some(idx) = find_app_matching(buckets, &["ea app"]) {
                        return Some((idx, false));
                    }
                }
                if contains_whole_token_sequence(&an_lower, "visual studio") {
                    if let Some(idx) = find_app_matching(
                        buckets,
                        &["visual studio build tools", "visual studio community", "visual studio"],
                    ) {
                        return Some((idx, false));
                    }
                }
                if contains_whole_token_sequence(&an_lower, "windows software development kit")
                    || contains_whole_token_sequence(&an_lower, "windows sdk")
                    || contains_whole_token_sequence(&an_lower, "windows app certification")
                    || contains_whole_token_sequence(&an_lower, "universal crt")
                    || contains_whole_token_sequence(&an_lower, "windows team extension")
                    || contains_whole_token_sequence(&an_lower, "universal general midi")
                    || contains_whole_token_sequence(&an_lower, "winrt intellisense")
                    || contains_whole_token_sequence(&an_lower, "kits configuration")
                    || contains_whole_token_sequence(&an_lower, "application verifier")
                    || contains_whole_token_sequence(&an_lower, "msi development tools")
                    || contains_whole_token_sequence(&an_lower, "sdk arm64")
                    || contains_whole_token_sequence(&an_lower, "windows desktop extension")
                    || contains_whole_token_sequence(&an_lower, "windows iot extension")
                    || contains_whole_token_sequence(&an_lower, "windows mobile extension")
                    || contains_whole_token_sequence(&an_lower, "winappdeploy")
                {
                    let ver = opt_cache_ver
                        .clone()
                        .or_else(|| extract_ver_token_from_text(&an_lower));
                    if let Some(idx) = find_best_versioned_bucket(
                        buckets,
                        |dn| contains_whole_token_sequence(dn, "windows software development kit"),
                        ver.as_deref(),
                        None,
                    ) {
                        return Some((idx, false));
                    }
                }
                if contains_whole_token_sequence(&an_lower, "visual c++") {
                    let target_ver = opt_cache_ver
                        .clone()
                        .or_else(|| extract_ver_token_from_text(&an_lower));
                    let target_arch = extract_arch_from_text(&an_lower);
                    if let Some(idx) = find_best_versioned_bucket(
                        buckets,
                        |dn| {
                            contains_whole_token_sequence(dn, "visual c++")
                                && contains_whole_token_sequence(dn, "redistributable")
                        },
                        target_ver.as_deref(),
                        target_arch,
                    ) {
                        return Some((idx, false));
                    }
                }
                if contains_whole_token_sequence(&an_lower, "windows desktop runtime") {
                    let target_ver = extract_ver_token_from_text(&an_lower)
                        .or_else(|| opt_cache_ver.clone());
                    let target_arch = extract_arch_from_text(&an_lower);
                    if let Some(idx) = find_best_versioned_bucket(
                        buckets,
                        |dn| contains_whole_token_sequence(dn, "windows desktop runtime"),
                        target_ver.as_deref(),
                        target_arch,
                    ) {
                        return Some((idx, false));
                    }
                }
                if contains_whole_token_sequence(&an_lower, ".net") {
                    let target_arch = extract_arch_from_text(&an_lower);
                    let named_ver = extract_ver_token_from_text(&an_lower);
                    // Prefer exact .NET Runtime - <ver> match when component belongs to an installed runtime (e.g. 8.0.21)
                    if let Some(ref nv) = named_ver {
                        if let Some(idx) = buckets.iter().position(|b| {
                            let dn = b.app.display_name.to_ascii_lowercase();
                            contains_whole_token_sequence(&dn, ".net runtime")
                                && !contains_whole_token_sequence(&dn, "desktop")
                                && app_effective_version(&b.app)
                                    .as_deref()
                                    .is_some_and(|bv| versions_match(nv, bv))
                        }) {
                            return Some((idx, false));
                        }
                    }
                    // Route to matching major .NET SDK version (7.* vs 10.*, mapping MSI v56.* -> 7, v80.*/v81.* -> 10).
                    let inferred_sdk_ver: Option<String> = named_ver
                        .as_deref()
                        .and_then(|v| {
                            if v.starts_with("6.") || v.starts_with("7.") || an_lower.contains("net6") || an_lower.contains("net7") {
                                Some("7.0.0".to_string())
                            } else if v.starts_with("10.") || v.starts_with("26.") || an_lower.contains("net10") || an_lower.contains("net8") || an_lower.contains("net9") {
                                Some("10.0.0".to_string())
                            } else {
                                None
                            }
                        })
                        .or_else(|| {
                            opt_cache_ver.as_deref().and_then(|cv| {
                                let maj = parse_version_tuple(cv)[0];
                                if matches!(maj, 28 | 56) {
                                    Some("7.0.0".to_string())
                                } else if matches!(maj, 40 | 80 | 81) {
                                    Some("10.0.0".to_string())
                                } else {
                                    None
                                }
                            })
                        });
                    if let Some(idx) = find_best_versioned_bucket(
                        buckets,
                        |dn| contains_whole_token_sequence(dn, ".net sdk"),
                        inferred_sdk_ver.as_deref(),
                        target_arch,
                    ) {
                        return Some((idx, false));
                    }
                    if let Some(idx) = find_app_matching(buckets, &["microsoft .net sdk", "microsoft .net"]) {
                        return Some((idx, false));
                    }
                }
                // Direct match by token against canonical apps.
                let tok = normalize_app_token(app_name);
                if let Some(idx) = buckets.iter().position(|b| normalize_app_token(&b.app.display_name) == tok) {
                    return Some((idx, false));
                }
            }
        }
        // Version-suffixed Windows 10/11 SDK Package Cache entries (`{...}v10.1.26100.9457`).
        if leaf_lower.contains("v10.1.26100.") {
            if let Some(idx) = find_app_matching(buckets, &["windows software development kit"]) {
                return Some((idx, false));
            }
        }
        // VS 2022 Package Cache bundles (`...v17.*` or `Microsoft.VisualStudio.*`).
        if leaf_lower.contains("v17.") || leaf_lower.contains("visualstudio") {
            if let Some(idx) = find_app_matching(
                buckets,
                &["visual studio build tools", "visual studio community", "visual studio"],
            ) {
                return Some((idx, false));
            }
        }
    }

    // 2. Curated deterministic token rules -> installed apps.
    let rules: &[(&[&str], &[&str], bool)] = &[
        // Rust toolchain
        (&[".cargo", ".rustup"], &["rustup"], false),
        // VS Code
        (&[".vscode", ".vscode-shared", "vscode-sqltools"], &["visual studio code"], false),
        (&["\\appdata\\roaming\\code"], &["visual studio code"], false),
        // Visual Studio ecosystem
        (
            &[
                "\\microsoft visual studio\\shared",
                "\\microsoft\\visualstudio",
                "\\appdata\\roaming\\visual studio setup",
                "\\appdata\\local\\microsoft\\vsapplicationinsights",
                "\\appdata\\local\\microsoft\\vscommon",
                "\\appdata\\local\\microsoft\\vstelemetry",
                "\\programdata\\microsoft devdiv",
                "\\programdata\\microsoft\\visualstudio",
                "\\program files (x86)\\microsoft visual studio",
            ],
            &["visual studio build tools", "visual studio community", "microsoft visual studio"],
            false,
        ),
        // Windows SDK / Kits
        (
            &[
                "\\program files (x86)\\windows kits",
                "\\program files (x86)\\microsoft sdks",
                "\\appdata\\local\\microsoft sdks",
                "\\programdata\\windows app certification kit",
            ],
            &["windows software development kit", "windows sdk"],
            true,
        ),
        // .NET SDK & NuGet
        (
            &[
                "\\.dotnet",
                "\\program files\\dotnet",
                "\\program files (x86)\\microsoft.net",
                "\\appdata\\local\\nuget",
                "\\appdata\\roaming\\nuget",
                "\\appdata\\local\\asp.net",
                "\\programdata\\microsoft\\netframework",
            ],
            &["microsoft .net sdk", "microsoft .net runtime", "microsoft windows desktop runtime"],
            false,
        ),
        (&["\\programdata\\asus"], &["asus framework service", "armoury crate"], false),
        (&["\\program files\\ea", "\\program files\\electronic arts", "\\program files\\common files\\eainstaller", "\\programdata\\ea desktop", "\\programdata\\electronic arts", "\\programdata\\origin", "\\programdata\\eaanticheat", "\\appdata\\local\\eadesktop", "\\appdata\\local\\ealaunchhelper", "\\appdata\\local\\electronic arts", "\\appdata\\local\\origin", "\\appdata\\roaming\\ea"], &["ea app"], false),
        (&["\\programdata\\frostbite"], &["ea app"], false),
        // Discord
        (
            &[
                "\\appdata\\local\\discord",
                "\\appdata\\roaming\\discord",
                "\\common files\\discord",
            ],
            &["discord"],
            false,
        ),
        // Steam
        (&["\\common files\\steam", "\\appdata\\local\\steam"], &["steam"], false),
        // Epic Games
        (
            &[
                "\\program files (x86)\\epic games",
                "\\programdata\\epic",
                "\\appdata\\local\\epicgameslauncher",
                "\\appdata\\local\\epic games",
            ],
            &["epic games launcher"],
            false,
        ),
        // Google Chrome
        (
            &["\\appdata\\local\\google\\chrome", "\\program files\\google\\chrome", "\\program files (x86)\\google\\chrome"],
            &["google chrome"],
            false,
        ),
        // Microsoft Edge & WebView2
        (
            &[
                "\\appdata\\local\\microsoft\\edgewebview",
                "\\program files (x86)\\microsoft\\edgewebview",
            ],
            &["microsoft edge webview2"],
            false,
        ),
        (
            &[
                "\\appdata\\local\\microsoft\\edge",
                "\\program files (x86)\\microsoft\\edge",
                "\\program files (x86)\\microsoft\\edgecore",
                "\\program files (x86)\\microsoft\\edgeupdate",
                "\\programdata\\microsoft\\edgeupdate",
            ],
            &["microsoft edge"],
            false,
        ),
        // NVIDIA
        (
            &["\\program files\\nvidia corporation\\installer2\\display.physx"],
            &["nvidia physx"],
            true,
        ),
        (
            &["\\appdata\\local\\nvidia corporation"],
            &["nvidia app", "nvidia graphics driver"],
            false,
        ),
        (
            &[
                "\\program files\\nvidia corporation\\nv",
                "\\program files\\nvidia corporation\\display",
                "\\appdata\\roaming\\nvidia",
                "\\programdata\\nvidia corporation",
                "\\programdata\\nvidia",
            ],
            &["nvidia graphics driver", "nvidia app"],
            false,
        ),
        // AMD
        (
            &[
                "c:\\amd",
                "\\appdata\\local\\amd",
                "\\appdata\\local\\amd_common",
                "\\appdata\\local\\amdsoftwareinstaller",
                "\\appdata\\roaming\\amd",
                "\\programdata\\amd",
                "\\program files\\amd",
            ],
            &["amd software", "amd chipset"],
            false,
        ),
        // Ollama
        (&["\\.ollama", "\\appdata\\local\\ollama", "\\appdata\\roaming\\ollama", "\\appdata\\roaming\\ollama app.exe"], &["ollama"], false),
        // Unity
        (
            &[
                "\\.unity",
                "\\unity user templates",
                "\\appdata\\local\\unityhub",
                "\\appdata\\local\\unityhub-updater",
                "\\appdata\\roaming\\unityhub",
            ],
            &["unity hub", "unity"],
            false,
        ),
        (&["\\appdata\\local\\unity", "\\appdata\\roaming\\unity", "\\appdata\\locallow\\unity", "\\programdata\\unity"], &["unity"], false),
        // Office
        (
            &[
                "\\programdata\\microsoft\\clicktorun",
                "\\programdata\\microsoft\\office",
                "\\appdata\\local\\microsoft\\office",
                "\\appdata\\roaming\\microsoft\\office",
            ],
            &["microsoft office ltsc", "microsoft 365", "microsoft office professional", "microsoft office"],
            false,
        ),
        // Logitech G Hub
        (&["\\program files\\lghub", "\\program files\\logi", "\\programdata\\lghub", "\\appdata\\local\\lghub", "\\appdata\\roaming\\lghub", "\\appdata\\roaming\\g hub"], &["logitech g hub"], false),
        // Copilot CLI
        (&["\\.copilot", "\\appdata\\local\\githubcopilotcli", "\\appdata\\local\\copilot"], &["github copilot cli"], false),
        // HoYoPlay / Genshin
        (&["\\appdata\\local\\hoyoverse", "\\appdata\\locallow\\mihoyo", "\\appdata\\roaming\\cognosphere"], &["hoyoplay", "genshin impact"], false),
        // Rockstar
        (&["\\program files\\rockstar games", "\\program files (x86)\\rockstar games", "\\programdata\\rockstar games", "\\appdata\\local\\rockstar games"], &["rockstar games launcher"], false),
        // Minecraft Java / Launcher (Bedrock routes directly to Store: Microsoft.MinecraftUWP below)
        (&["c:\\xboxgames\\minecraft launcher"], &["minecraft launcher"], true),
        (&["\\appdata\\roaming\\.minecraft", "\\appdata\\local\\minecraftinstaller"], &["minecraft launcher"], false),
        // Wacom
        (&["\\appdata\\roaming\\wtablet", "\\appdata\\roaming\\wpersistent"], &["wacom tablet"], false),
        // WinRAR, IDM, TreeSize, Tailscale, TeamViewer, RustDesk, Spotify, Inkscape, Tiled, Node, Python
        (&["\\appdata\\roaming\\winrar"], &["winrar"], false),
        (&["\\programdata\\idm", "\\appdata\\roaming\\idm", "\\program files (x86)\\internet download manager"], &["internet download manager"], false),
        (&["\\appdata\\roaming\\jam software", "\\program files\\jam software"], &["treesize"], false),
        (&["\\programdata\\tailscale", "\\appdata\\local\\tailscale"], &["tailscale"], false),
        (&["\\appdata\\local\\teamviewer", "\\appdata\\roaming\\teamviewer"], &["teamviewer"], false),
        (&["\\appdata\\local\\rustdesk", "\\appdata\\roaming\\rustdesk"], &["rustdesk"], false),
        (&["\\appdata\\local\\spotify", "\\appdata\\roaming\\spotify"], &["spotify"], false),
        (&["\\appdata\\roaming\\inkscape"], &["inkscape"], false),
        (&["\\appdata\\local\\tiled", "\\appdata\\roaming\\tiled"], &["tiled"], false),
        (&["\\appdata\\roaming\\npm", "\\appdata\\local\\pnpm"], &["node.js"], false),
        (&["\\appdata\\local\\pip", "\\appdata\\local\\uv", "\\appdata\\roaming\\uv"], &["python"], false),
        (&["\\appdata\\local\\easyanticheat", "\\appdata\\roaming\\easyanticheat", "\\program files (x86)\\easyanticheat"], &["easyanticheat"], false),
        (&["\\appdata\\local\\battleye", "\\common files\\battleye"], &["battleye"], false),
        (&["\\appdata\\local\\artcraft"], &["artcraft"], false),
        (&["\\appdata\\roaming\\filmcraft"], &["filmcraft"], false),
        (&["\\appdata\\roaming\\photocraft"], &["photocraft"], false),
        (&["\\appdata\\local\\mister download manager", "\\appdata\\roaming\\misterdownloadmanager"], &["mister download manager"], false),
        (&["\\appdata\\local\\theisle"], &["the isle"], false),
        (
            &[
                "\\appdata\\roaming\\apple computer",
                "\\appdata\\local\\apple computer",
                "\\programdata\\apple computer",
                "\\programdata\\apple",
            ],
            &["appleinc.itunes", "itunes", "appledevices"],
            false,
        ),
    ];

    for (path_needles, app_needles, is_body) in rules {
        let matched_path = path_needles.iter().any(|needle| {
            path_matches_token(path_lower, leaf_lower, needle)
        });
        if matched_path {
            if let Some(idx) = find_app_matching(buckets, app_needles) {
                return Some((idx, *is_body));
            }
        }
    }

    // Minecraft Bedrock product routes to the Store: Microsoft.MinecraftUWP package bucket.
    let is_bedrock_install =
        path_matches_token(path_lower, leaf_lower, "c:\\xboxgames\\minecraft for windows");
    if is_bedrock_install
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\roaming\\minecraft bedrock")
    {
        if let Some(idx) = attribute_store_package(
            buckets,
            store_buckets,
            "Microsoft.MinecraftUWP_8wekyb3d8bbwe",
        ) {
            return Some((idx, is_bedrock_install));
        }
    }

    // General rule: versioned per-product folder under a publisher folder (e.g. Local|Roaming\Google\AndroidStudio2024.2).
    if let Some((idx, is_body)) =
        match_versioned_publisher_product(buckets, store_buckets, path_lower, leaf)
    {
        return Some((idx, is_body));
    }

    // General rule: AppData\Roaming|Local\<X>Data attributes to app X when X is installed or a curated known app.
    if let Some((idx, is_body)) =
        match_xdata_folder(buckets, store_buckets, path_lower, leaf, leaf_lower)
    {
        return Some((idx, is_body));
    }

    // Verified known-app fallbacks (data folders of standalone or un-registered apps resolved cleanly).
    let known_app_fallbacks: &[(&[&str], &[&str], &str, &str, &str, bool)] = &[
        (
            &["\\appdata\\roaming\\tabnine", "\\appdata\\local\\tabnine"],
            &["tabnine"],
            "app:tabnine",
            "TabNine",
            "Tabnine",
            false,
        ),
        (
            &[
                "\\appdata\\local\\zalo",
                "\\appdata\\local\\zalopc",
                "\\appdata\\roaming\\zalopc",
            ],
            &["zalo"],
            "app:zalo",
            "Zalo",
            "VNG Corp.",
            false,
        ),
        (
            &[
                "\\appdata\\local\\jellyfin",
                "\\programdata\\jellyfin",
                "\\appdata\\local\\jellyfinmediaplayer",
            ],
            &["jellyfin"],
            "app:jellyfin",
            "Jellyfin",
            "Jellyfin",
            false,
        ),
        (
            &[
                "\\appdata\\roaming\\postman",
                "\\appdata\\local\\postman",
                "\\appdata\\roaming\\postmanagent",
            ],
            &["postman"],
            "app:postman",
            "Postman",
            "Postman, Inc.",
            false,
        ),
        (
            &["\\microsoft\\power bi desktop store app"],
            &["power bi desktop"],
            "app:power-bi-desktop",
            "Microsoft Power BI Desktop",
            "Microsoft Corporation",
            false,
        ),
        (
            &[
                "\\common files\\adobe",
                "\\appdata\\roaming\\adobe",
                "\\appdata\\local\\adobe",
                "\\appdata\\locallow\\adobe",
                "\\appdata\\roaming\\com.adobe.dunamis",
            ],
            &["adobe creative cloud", "adobe acrobat"],
            "app:adobe-creative-cloud",
            "Adobe Creative Cloud",
            "Adobe Inc.",
            false,
        ),
        (
            &["\\colossal order\\cities skylines ii"],
            &["cities: skylines ii", "cities skylines ii"],
            "app:cities-skylines-ii",
            "Cities: Skylines II",
            "Colossal Order",
            false,
        ),
        (
            &["\\appdata\\roaming\\riot-client-ux", "\\appdata\\local\\riot games"],
            &["riot client"],
            "app:riot-client",
            "Riot Client",
            "Riot Games, Inc.",
            false,
        ),
    ];

    for (path_needles, app_needles, synth_key, display_name, publisher, is_body) in
        known_app_fallbacks
    {
        if path_needles
            .iter()
            .any(|needle| path_matches_token(path_lower, leaf_lower, needle))
        {
            if let Some(idx) = find_app_matching(buckets, app_needles) {
                return Some((idx, *is_body));
            }
            let idx = ensure_synthetic_app(
                buckets,
                store_buckets,
                synth_key,
                display_name,
                publisher,
                None,
                false,
            );
            return Some((idx, *is_body));
        }
    }

    // EdgeWebView2 Runtime when SystemComponent=1 filtered it out of canonical registry apps.
    if path_matches_token(path_lower, leaf_lower, "\\program files (x86)\\microsoft\\edgewebview")
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\local\\microsoft\\edgewebview")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:edge-webview2",
            "Microsoft Edge WebView2 Runtime",
            "Microsoft Corporation",
            Some(NoTouchCategory::SystemRuntime),
            false,
        );
        return Some((idx, path_lower.contains("\\program files")));
    }

    // Standalone EasyAntiCheat data/install when no separate Uninstall entry exists.
    if path_matches_token(path_lower, leaf_lower, "\\appdata\\roaming\\easyanticheat")
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\local\\easyanticheat")
        || path_matches_token(path_lower, leaf_lower, "\\program files (x86)\\easyanticheat_eos")
        || path_matches_token(path_lower, leaf_lower, "\\program files (x86)\\easyanticheat")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:easyanticheat",
            "EasyAntiCheat",
            "Epic Games, Inc.",
            Some(NoTouchCategory::AntiCheat),
            false,
        );
        return Some((idx, path_lower.contains("\\program files")));
    }

    // Special handling for `C:\XboxGames\Minecraft for Windows` -> Store package fallback or dedicated Minecraft entry if Xbox Service isn't desired.
    if path_lower.starts_with("c:\\xboxgames\\") {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            &format!("xbox:{leaf_lower}"),
            &format!("Xbox Game — {leaf_lower}"),
            "Microsoft Xbox",
            None,
            true,
        );
        return Some((idx, true));
    }

    // Also attribute `Program Files (x86)\Reference Assemblies` / `MSBuild` to Visual Studio if installed.
    if path_lower.ends_with("\\program files (x86)\\msbuild")
        || path_lower.ends_with("\\program files\\msbuild")
        || path_lower.ends_with("\\reference assemblies")
    {
        if let Some(idx) = find_app_matching(
            buckets,
            &["visual studio build tools", "visual studio community", "visual studio"],
        ) {
            return Some((idx, true));
        }
    }

    // Office Click-to-Run / Office Add-in Local\Packages (`oice_16_*`) & Microsoft Shared
    if path_lower.contains("\\program files\\microsoft office")
        || path_lower.contains("\\program files (x86)\\microsoft office")
        || path_lower.contains("\\common files\\microsoft shared")
        || path_lower.contains("\\appdata\\local\\packages\\oice_16_")
    {
        if let Some(idx) = find_app_matching(
            buckets,
            &["microsoft office ltsc", "microsoft 365", "microsoft office professional", "microsoft office"],
        ) {
            return Some((idx, !path_lower.contains("\\appdata\\local\\packages\\")));
        }
    }

    // Standalone BattlEye anti-cheat files when no separate Uninstall entry exists.
    if path_matches_token(path_lower, leaf_lower, "\\common files\\battleye")
        || path_matches_token(path_lower, leaf_lower, "\\appdata\\local\\battleye")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:battleye",
            "BattlEye",
            "BattlEye Innovations",
            Some(NoTouchCategory::AntiCheat),
            false,
        );
        return Some((idx, path_lower.contains("\\program files")));
    }

    // NVIDIA PhysX installer folder fallback (`Display.PhysX.{...}`)
    if path_lower.contains("\\nvidia corporation\\installer2\\display.physx") {
        if let Some(idx) = find_app_matching(buckets, &["nvidia physx"]) {
            return Some((idx, true));
        }
    }

    // Windows Defender & System services in ProgramData / Program Files
    if path_lower.contains("\\program files\\windows defender")
        || path_lower.contains("\\programdata\\microsoft\\windows defender")
        || path_lower.contains("\\programdata\\microsoft\\windows security health")
    {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            "sys:windows-defender",
            "Windows Defender",
            "Microsoft Corporation",
            Some(NoTouchCategory::Service),
            false,
        );
        return Some((idx, true));
    }

    None
}

fn publisher_display_for_folder(pub_lower: &str) -> Option<&'static str> {
    match pub_lower {
        "google" => Some("Google LLC"),
        "jetbrains" => Some("JetBrains s.r.o."),
        "autodesk" => Some("Autodesk"),
        "adobe" => Some("Adobe Inc."),
        _ => None,
    }
}

fn split_camel_case(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len() + 4);
    let mut prev: Option<char> = None;
    for ch in stem.chars() {
        if let Some(p) = prev {
            if p.is_ascii_lowercase() && ch.is_ascii_uppercase() {
                out.push(' ');
            }
        }
        out.push(ch);
        prev = Some(ch);
    }
    out
}

fn canonicalize_product_stem(stem: &str) -> String {
    let compact = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match compact.as_str() {
        "androidstudio" => "Android Studio".to_string(),
        "intellijidea" | "ideaic" | "ideau" => "IntelliJ IDEA".to_string(),
        "pycharm" | "pycharmce" => "PyCharm".to_string(),
        "webstorm" => "WebStorm".to_string(),
        "goland" => "GoLand".to_string(),
        "rustrover" => "RustRover".to_string(),
        "clion" => "CLion".to_string(),
        "phpstorm" => "PhpStorm".to_string(),
        "datagrip" => "DataGrip".to_string(),
        "rider" => "Rider".to_string(),
        _ => split_camel_case(stem),
    }
}

fn parse_versioned_product_name(leaf: &str) -> Option<String> {
    let bytes = leaf.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii_digit() {
            continue;
        }
        let suffix = &leaf[i..];
        let valid_ver_chars = suffix
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
        let has_version_shape = suffix.len() >= 2 && (suffix.contains('.') || suffix.chars().filter(|c| c.is_ascii_digit()).count() >= 2);
        if valid_ver_chars && has_version_shape {
            let mut stem_end = i;
            let trimmed_sep = leaf[..stem_end].trim_end_matches([' ', '-', '_']);
            stem_end = trimmed_sep.len();
            if stem_end >= 2 {
                let last = leaf.as_bytes()[stem_end - 1];
                let prev = leaf.as_bytes()[stem_end - 2];
                if matches!(last, b'v' | b'V') && !prev.is_ascii_alphabetic() {
                    stem_end -= 1;
                }
            }
            let stem = leaf[..stem_end].trim_end_matches([' ', '-', '_']);
            let alpha_count = stem.chars().filter(|c| c.is_ascii_alphabetic()).count();
            if alpha_count >= 4 {
                return Some(canonicalize_product_stem(stem));
            }
            return None;
        }
    }
    None
}

fn match_versioned_publisher_product(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    path_lower: &str,
    leaf: &str,
) -> Option<(usize, bool)> {
    let parts: Vec<&str> = path_lower.trim_end_matches('\\').split('\\').collect();
    if parts.len() < 4 {
        return None;
    }
    let tier = parts[parts.len() - 3];
    let pub_dir = parts[parts.len() - 2];
    if !matches!(tier, "local" | "roaming" | "locallow") {
        return None;
    }
    if parts.len() < 5 || parts[parts.len() - 4] != "appdata" {
        return None;
    }
    let pub_display = publisher_display_for_folder(pub_dir)?;
    let product_name = parse_versioned_product_name(leaf)?;
    let product_tok = normalize_app_token(&product_name);
    if product_tok.is_empty() {
        return None;
    }
    if let Some(idx) = buckets
        .iter()
        .position(|b| normalize_app_token(&b.app.display_name) == product_tok)
    {
        return Some((idx, false));
    }
    let idx = ensure_synthetic_app(
        buckets,
        store_buckets,
        &format!("product:{product_tok}"),
        &product_name,
        pub_display,
        None,
        false,
    );
    Some((idx, false))
}

fn lookup_known_xdata_app(stem_tok: &str) -> Option<(&'static str, &'static str)> {
    match stem_tok {
        "zalo" => Some(("Zalo", "VNG Corp.")),
        "tabnine" => Some(("TabNine", "Tabnine")),
        "jellyfin" => Some(("Jellyfin", "Jellyfin")),
        "postman" => Some(("Postman", "Postman, Inc.")),
        "riot client" => Some(("Riot Client", "Riot Games, Inc.")),
        _ => None,
    }
}

fn match_xdata_folder(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    path_lower: &str,
    leaf: &str,
    leaf_lower: &str,
) -> Option<(usize, bool)> {
    let parts: Vec<&str> = path_lower.trim_end_matches('\\').split('\\').collect();
    if parts.len() < 3 {
        return None;
    }
    if parts[parts.len() - 3] != "appdata"
        || !matches!(parts[parts.len() - 2], "roaming" | "local")
    {
        return None;
    }
    if !leaf_lower.ends_with("data")
        || leaf.len() < 7
        || matches!(leaf_lower, "appdata" | "application data" | "programdata")
    {
        return None;
    }
    let x_stem = leaf[..leaf.len() - 4].trim_end_matches(['-', '_', ' ']);
    if x_stem.len() < 3 {
        return None;
    }
    if let Some(idx) = find_by_exact_name(buckets, x_stem) {
        return Some((idx, false));
    }
    let stem_tok = normalize_app_token(x_stem);
    if let Some((display_name, publisher)) = lookup_known_xdata_app(&stem_tok) {
        let idx = ensure_synthetic_app(
            buckets,
            store_buckets,
            &format!("app:{stem_tok}"),
            display_name,
            publisher,
            None,
            false,
        );
        return Some((idx, false));
    }
    None
}

fn path_matches_token(path_lower: &str, leaf_lower: &str, needle: &str) -> bool {
    if needle.starts_with('.') {
        return leaf_lower == needle;
    }
    if !needle.contains('\\') {
        return leaf_lower == needle;
    }
    if path_lower == needle || path_lower.ends_with(needle) {
        return true;
    }
    if needle.starts_with('\\') {
        let with_slash = format!("{needle}\\");
        path_lower.contains(&with_slash)
    } else {
        let with_slash = format!("{needle}\\");
        path_lower.starts_with(&with_slash)
    }
}

fn find_by_publisher(buckets: &[AppBucket], leaf: &str) -> Option<usize> {
    let leaf_tok = normalize_app_token(leaf);
    if leaf_tok.len() < 4
        || matches!(
            leaf_tok.as_str(),
            "microsoft"
                | "microsoft corporation"
                | "studio"
                | "studios"
                | "software"
                | "games"
                | "interactive"
                | "entertainment"
                | "technologies"
                | "systems"
                | "digital"
                | "corporation"
                | "company"
                | "limited"
        )
    {
        return None;
    }
    let mut strong_matches: Vec<usize> = Vec::new();
    let mut slice_matches: Vec<usize> = Vec::new();
    for (i, b) in buckets.iter().enumerate() {
        let mut best: Option<AppTokenMatchKind> = None;
        for pub_name in &b.app.publishers {
            let p_tok = normalize_app_token(pub_name);
            // Never attribute solely on generic "Microsoft" / "Microsoft Corporation".
            if p_tok.is_empty()
                || p_tok == "microsoft"
                || p_tok == "microsoft corporation"
                || p_tok.len() < 4
            {
                continue;
            }
            if let Some(kind) = match_app_predicate(&p_tok, &leaf_tok) {
                let effective = if p_tok == leaf_tok
                    || p_tok.starts_with(&format!("{leaf_tok} "))
                {
                    AppTokenMatchKind::ExactFullToken
                } else {
                    kind
                };
                if best.is_none_or(|prev| effective > prev) {
                    best = Some(effective);
                }
            }
        }
        match best {
            Some(AppTokenMatchKind::ExactFullToken) => strong_matches.push(i),
            Some(AppTokenMatchKind::WholeWordSlice) => slice_matches.push(i),
            None => {}
        }
    }
    let winners = if !strong_matches.is_empty() {
        strong_matches
    } else {
        slice_matches
    };
    // Only attribute when unambiguous (1 owning app at the winning tier).
    if winners.len() == 1 {
        Some(winners[0])
    } else {
        None
    }
}

fn is_known_backup_suffix(seg: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    let trimmed = lower.trim_end_matches(|c: char| c.is_ascii_digit());
    matches!(trimmed, "bak" | "old" | "backup" | "copy")
}

fn is_publisher_id_shaped(seg: &str) -> bool {
    if seg.is_empty() || is_known_backup_suffix(seg) || !seg.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if seg.len() == 13 && seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        return true;
    }
    (10..=16).contains(&seg.len())
        && seg.chars().any(|c| c.is_ascii_alphabetic())
        && seg.chars().any(|c| c.is_ascii_digit())
}

fn is_windowsapps_five_segment_form(parts: &[&str]) -> bool {
    parts.len() == 5
        && parts[1].contains('.')
        && parts[1].chars().any(|c| c.is_ascii_digit())
        && is_publisher_id_shaped(parts[4])
}

fn resolve_store_package_stem_and_pub_id(pkg_folder: &str) -> (&str, &str) {
    let parts: Vec<&str> = pkg_folder.split('_').collect();
    let stem = parts.first().copied().unwrap_or(pkg_folder);
    if parts.len() < 2 {
        return (stem, "");
    }
    if is_windowsapps_five_segment_form(&parts) {
        return (stem, parts[4]);
    }
    let mut idx = parts.len() - 1;
    while idx >= 2 {
        let prev = parts[idx - 1];
        let curr = parts[idx];
        if is_publisher_id_shaped(prev) && (is_known_backup_suffix(curr) || !is_publisher_id_shaped(curr)) {
            idx -= 1;
        } else {
            break;
        }
    }
    (stem, parts[idx])
}

fn attribute_store_package(
    buckets: &mut Vec<AppBucket>,
    store_buckets: &mut HashMap<String, usize>,
    pkg_folder: &str,
) -> Option<usize> {
    let lower = pkg_folder.to_ascii_lowercase();
    // Skip non-package synthetic sub-items
    if lower == "activesync" || lower == "packages" || lower == "windowsapps" || lower.starts_with("sandbox.") || lower.starts_with("oice_") {
        return None;
    }

    // Store package names are either `<FamilyStem>_<PubId>` (AppData\Local\Packages, possibly with backup suffix)
    // or `<FamilyStem>_<Version>_<Arch>_<ResourceId>_<PubId>` (Program Files\WindowsApps).
    let (stem, pub_id) = resolve_store_package_stem_and_pub_id(pkg_folder);
    let friendly = stem
        .strip_prefix("Microsoft.")
        .or_else(|| stem.strip_prefix("MicrosoftWindows."))
        .or_else(|| stem.strip_prefix("MicrosoftCorporationII."))
        .or_else(|| stem.strip_prefix("AppleInc."))
        .or_else(|| stem.strip_prefix("NVIDIACorp."))
        .or_else(|| stem.strip_prefix("RealtekSemiconductorCorp."))
        .or_else(|| stem.strip_prefix("B9ECED6F."))
        .or_else(|| stem.strip_prefix("40459File-New-Project."))
        .or_else(|| stem.strip_prefix("JAMSoftware."))
        .unwrap_or(stem);

    // Match against existing registry apps first if unambiguously named (e.g. TreeSize, ArmouryCrate, Edge).
    let friendly_tok = normalize_app_token(friendly);
    if friendly_tok.len() >= 5 {
        if let Some(idx) = buckets.iter().position(|b| {
            let dn = normalize_app_token(&b.app.display_name);
            dn == friendly_tok
        }) {
            return Some(idx);
        }
    }

    let is_runtime_or_system = lower.starts_with("microsoft.windows.")
        || lower.starts_with("microsoftwindows.")
        || lower.starts_with("microsoft.windowsappruntime")
        || lower.starts_with("microsoftcorporationii.winappruntime")
        || lower.starts_with("microsoft.net.native")
        || lower.starts_with("microsoft.directxruntime")
        || lower.starts_with("microsoft.vclibs")
        || lower.starts_with("microsoft.ui.xaml")
        || lower.starts_with("microsoft.aad.brokerplugin")
        || lower.starts_with("microsoft.accountscontrol")
        || lower.starts_with("microsoft.creddialoghost")
        || lower.starts_with("microsoft.bioenrollment")
        || lower.starts_with("microsoft.lockapp")
        || lower.starts_with("windows.")
        || lower.starts_with("1527c705-")
        || lower.starts_with("c5e2524a-")
        || lower.starts_with("e2a4f912-")
        || lower.starts_with("f46d4000-");

    let no_touch = if lower.starts_with("microsoft.net.native")
        || lower.starts_with("microsoft.windowsappruntime")
        || lower.starts_with("microsoftcorporationii.winappruntime")
        || lower.starts_with("microsoft.directxruntime")
        || lower.starts_with("microsoft.vclibs")
        || lower.starts_with("microsoft.ui.xaml")
    {
        Some(NoTouchCategory::SystemRuntime)
    } else if is_runtime_or_system {
        Some(NoTouchCategory::Service)
    } else {
        None
    };

    let display = format!("Store: {stem}");
    let key = if pub_id.is_empty() {
        format!("store:{}", stem.to_ascii_lowercase())
    } else {
        format!("store:{}_{}", stem.to_ascii_lowercase(), pub_id.to_ascii_lowercase())
    };
    let idx = ensure_synthetic_app(buckets, store_buckets, &key, &display, "Microsoft Store", no_touch, true);
    Some(idx)
}

pub fn build_store_package_map(items: &[(&str, &str)]) -> BTreeMap<String, String> {
    items
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}
