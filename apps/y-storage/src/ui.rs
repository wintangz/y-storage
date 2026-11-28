use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use egui::{
    Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Key, Layout, Margin,
    Order, Pos2, Rect, RichText, Rounding, Sense, Stroke, TextStyle, UiBuilder, Vec2,
};
use y_core::config::{
    AppConfig, AppFilterChip, AppSortMode, DepStateFilter, FacetFilters, FootprintZoneFilter,
    GrowthFacetFilter, NO_PUBLISHER_FACET_KEY, PinKind, SizeBandFilter, SortDirection, ThemeMode,
    ThemePresetChoice, UnattributedFacetFilters, UnattributedLocationZone, ZoneRuleKind,
    ZoneRulebook, default_custom_theme_rgb,
};
use y_core::discovery::{normalize_app_token, normalize_path_key};
use y_core::model::{
    AttributionBasis, DependencyConfidence, DependencyKind, GrowthDelta, LocationQualifier,
    NO_KNOWN_DEPENDENCIES_LABEL, ReverseDependencyEntry, ScanReport, ZoneKind,
};
use y_platform_windows::scanner::ScanProgressSnapshot;

use crate::family::{AppFamily, LedgerEntry, derive_app_families};
use crate::list_query::{
    UnattributedSortMode, active_per_drive_mode, category_facet_options,
    filter_and_sort_ledger_entries, filter_and_sort_unattributed_with_facets, has_growth_baseline,
    publisher_facet_options, sanitize_facets_for_report,
};
use crate::theme::{
    ThemeLadder, ThemePalette, compute_theme_key, contrast_ratio, format_hex_rgb, lerp_color,
    parse_hex_rgb,
};

// Single multiples-of-4 spacing scale used uniformly everywhere.
pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;

// Multi-tier corner-radius scale from DESIGN.md (paired with z-controller synced controls).
const RADIUS_PANEL: f32 = 8.0;
const RADIUS_CONTROL: f32 = 6.0;
const RADIUS_PILL: f32 = 4.0;
const RADIUS_TABLE_CELL: f32 = 3.0;
const BADGE_PILL_H: f32 = 18.0;
const RADIUS_BADGE_PILL: f32 = BADGE_PILL_H * 0.5;

pub const LEDGER_PAGE_SIZE: usize = 50;
const LEDGER_FOOTER_H: f32 = 34.0;

const SIZE_COL_W: f32 = 96.0;
const PER_DRIVE_SIZE_COL_W: f32 = 186.0;
const DELTA_COL_W: f32 = 96.0;
const GROWTH_NOISE_FLOOR_BYTES: u64 = 1_048_576; // 1 MB binary (DESIGN.md / FIX 6 noise floor)
const APP_ICON_SIZE: f32 = 18.0;
const PROP_BAR_W: f32 = 76.0;
const PROP_BAR_H: f32 = 2.0;
const SCROLL_GUTTER_W: f32 = 6.0 + SPACE_8;
const HEADER_CONTROL_H: f32 = 28.0;
const TABLE_HEADER_H: f32 = 26.0;
const SUB_HEADER_H: f32 = 20.0;
const APP_ROW_H: f32 = 30.0;
const PINNED_ROW_H: f32 = 24.0;
const SUB_LINE1_H: f32 = 22.0;
const SUB_LINE2_H: f32 = 16.0;
const COLLAPSED_ROW_OUTER_H: f32 = APP_ROW_H + SPACE_4 * 2.0;
const SETTINGS_TWO_COL_MIN_W: f32 = 560.0;
const SETTINGS_POD_MIN_W: f32 = 320.0;
const FORM_GRID_ACTION_W: f32 = 112.0;
const FORM_GRID_ROW_H: f32 = 26.0;
const SEGOE_UI_FONT_PATH: &str = r"C:\Windows\Fonts\segoeui.ttf";
const SEGOE_UI_SEMIBOLD_PATH: &str = r"C:\Windows\Fonts\seguisb.ttf";
const SEGOE_UI_BOLD_PATH: &str = r"C:\Windows\Fonts\segoeuib.ttf";
const CONSOLA_FONT_PATH: &str = r"C:\Windows\Fonts\consola.ttf";
const CASCADIA_MONO_FONT_PATH: &str = r"C:\Windows\Fonts\CascadiaMono.ttf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    Main,
    Dependencies,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    ScanScope,
    Rulebook,
}

impl SettingsSection {
    pub const ALL: [Self; 3] = [Self::Appearance, Self::ScanScope, Self::Rulebook];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance & Theme",
            Self::ScanScope => "Scan Scope & Exclusions",
            Self::Rulebook => "Attribution Zone Rulebook",
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiAction {
    StartRescan,
    StartDriveRescan(char),
    ToggleTheme(ThemeMode),
    SetTheme {
        mode: ThemeMode,
        preset: ThemePresetChoice,
        custom_rgb: [u8; 3],
    },
    SaveConfig(AppConfig),
    UpdateListView {
        sort_mode: AppSortMode,
        sort_direction: SortDirection,
        filter_chip: AppFilterChip,
        search_query: String,
        facets: FacetFilters,
    },
    UpdateLedgerLayout {
        collapsed: bool,
        height_px: Option<f32>,
    },
    LocateInExplorer(String),
    OpenWebSearch(String),
}

#[derive(Debug, Clone, Copy)]
struct ChipStyle {
    fill: Color32,
    stroke: Color32,
    text: Color32,
}

#[derive(Debug, Clone)]
enum DeltaCellKind {
    Blank,
    NewPill,
    SignedValue { text: String, color: Color32 },
}

#[derive(Debug, Clone)]
struct DeltaCellDisplay {
    kind: DeltaCellKind,
    tip: Option<String>,
}

struct CachedAppMeta {
    total_str: String,
    total_tip: String,
    delta_display: DeltaCellDisplay,
    name_w_px: Option<f32>,
    pub_w_px: Option<f32>,
}

struct CachedReportData {
    scan_ts: u64,
    theme_key: u32,
    apps_count: usize,
    apps: Vec<CachedAppMeta>,
    ledger_entries: Vec<LedgerEntry>,
    max_unfiltered_bytes: u64,
    total_attributed_bytes: u64,
    attributed_label: String,
    attributed_tip: String,
    rev_counts: HashMap<String, usize>,
    rev_by_app_token: HashMap<String, (String, usize)>,
    rev_by_app_id: HashMap<String, (Option<String>, usize)>,
    shared_app_ids: HashSet<String>,
    has_growth_baseline: bool,
    sys_reconcile: Vec<(char, String, String, i128)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy)]
enum LedgerRowBadge {
    Accent(&'static str),
    Status(StatusKind, &'static str),
}

struct CachedUnattributedList {
    scan_ts: u64,
    count: usize,
    search_query: String,
    facets: UnattributedFacetFilters,
    sort: UnattributedSortMode,
    visible_indices: Vec<usize>,
    filtered_sub_bytes: u64,
}

struct CachedFilteredList {
    scan_ts: u64,
    sort_mode: AppSortMode,
    sort_direction: SortDirection,
    filter_chip: AppFilterChip,
    search_query: String,
    facets: FacetFilters,
    sys_drive: char,
    visible_entries: Vec<usize>,
    visible_indices: Vec<usize>,
}

struct CachedRimMesh {
    w_q: i32,
    h_q: i32,
    r_q: i32,
    ppp_q: i32,
    theme_key: u32,
    mesh: egui::epaint::Mesh,
    fill_poly: Vec<Pos2>,
}

pub struct StorageUiState {
    pub active_view: ActiveView,
    pub sort_mode: AppSortMode,
    pub sort_direction: SortDirection,
    pub filter_chip: AppFilterChip,
    pub search_query: String,
    pub facets: FacetFilters,
    pub open_facet_popup: Option<&'static str>,
    pub unattrib_search_query: String,
    pub unattrib_facets: UnattributedFacetFilters,
    pub open_unattrib_facet_popup: Option<&'static str>,
    pub unattrib_sort: UnattributedSortMode,
    pub open_pod_scope_drive: Option<char>,
    pub new_container_name_input: String,
    pub new_container_name_kind: ZoneRuleKind,
    pub new_container_path_input: String,
    pub new_container_path_kind: ZoneRuleKind,
    pub new_pin_path_input: String,
    pub new_pin_kind: PinKind,
    pub settings_section: SettingsSection,
    pub settings_focus_containers: bool,
    pub dep_search_query: String,
    pub expanded_apps: HashSet<String>,
    pub expanded_families: HashSet<String>,
    pub expanded_deps: HashSet<String>,
    pub open_reverse_dep_id: Option<String>,
    pub expand_shared_caches: bool,
    pub expand_windows_system: bool,
    pub expand_your_files: bool,
    pub expand_unattributed: bool,
    pub new_exclusion_inputs: HashMap<char, String>,
    pub applied_theme_key: Option<u32>,
    pub custom_hex_input: String,
    pub custom_hex_valid: bool,
    pub has_prop_strong: bool,
    pub ledger_collapsed: bool,
    pub ledger_height_px: Option<f32>,
    ledger_drag_start_h: Option<f32>,
    pub scroll_ledger_bottom: bool,
    pub ledger_page: usize,
    pub ledger_scroll_to_top: bool,
    pub header_rects: [Option<(AppSortMode, Rect)>; 5],
    pub icon_textures: HashMap<String, egui::TextureHandle>,
    pub icon_cache_scan_ts: Option<u64>,
    pub first_row_size_rect: Option<Rect>,
    pub first_unattrib_web_search_rect: Option<Rect>,
    pub unattrib_mid_row_rect: Option<Rect>,
    pub first_drive_pod_rect: Option<Rect>,
    wt_logo_tex: Option<(u32, egui::TextureHandle)>,
    unattrib_exists_cache: HashMap<String, bool>,
    cached_report: Option<CachedReportData>,
    cached_filtered: Option<CachedFilteredList>,
    cached_unattrib: Option<CachedUnattributedList>,
    cached_rim: Option<CachedRimMesh>,
    expanded_row_heights: HashMap<String, f32>,
    expanded_family_heights: HashMap<String, f32>,
    truncate_cache: HashMap<(u64, u32, u16), (String, bool)>,
    cached_measure_ppp: Option<f32>,
}

impl Default for StorageUiState {
    fn default() -> Self {
        Self {
            active_view: ActiveView::Main,
            sort_mode: AppSortMode::Size,
            sort_direction: SortDirection::Desc,
            filter_chip: AppFilterChip::All,
            search_query: String::new(),
            facets: FacetFilters::default(),
            open_facet_popup: None,
            unattrib_search_query: String::new(),
            unattrib_facets: UnattributedFacetFilters::default(),
            open_unattrib_facet_popup: None,
            unattrib_sort: UnattributedSortMode::SizeDesc,
            open_pod_scope_drive: None,
            new_container_name_input: String::new(),
            new_container_name_kind: ZoneRuleKind::AppContainer,
            new_container_path_input: String::new(),
            new_container_path_kind: ZoneRuleKind::PersonalFiles,
            new_pin_path_input: String::new(),
            new_pin_kind: PinKind::App,
            settings_section: SettingsSection::Appearance,
            settings_focus_containers: false,
            dep_search_query: String::new(),
            expanded_apps: HashSet::new(),
            expanded_families: HashSet::new(),
            expanded_deps: HashSet::new(),
            open_reverse_dep_id: None,
            expand_shared_caches: false,
            expand_windows_system: false,
            expand_your_files: false,
            expand_unattributed: false,
            new_exclusion_inputs: HashMap::new(),
            applied_theme_key: None,
            custom_hex_input: format_hex_rgb(default_custom_theme_rgb()),
            custom_hex_valid: true,
            has_prop_strong: false,
            ledger_collapsed: false,
            ledger_height_px: None,
            ledger_drag_start_h: None,
            scroll_ledger_bottom: false,
            ledger_page: 0,
            ledger_scroll_to_top: false,
            header_rects: [None; 5],
            icon_textures: HashMap::new(),
            icon_cache_scan_ts: None,
            first_row_size_rect: None,
            first_unattrib_web_search_rect: None,
            unattrib_mid_row_rect: None,
            first_drive_pod_rect: None,
            wt_logo_tex: None,
            unattrib_exists_cache: HashMap::new(),
            cached_report: None,
            cached_filtered: None,
            cached_unattrib: None,
            cached_rim: None,
            expanded_row_heights: HashMap::new(),
            expanded_family_heights: HashMap::new(),
            truncate_cache: HashMap::new(),
            cached_measure_ppp: None,
        }
    }
}

impl StorageUiState {
    #[must_use]
    pub fn from_config(cfg: &AppConfig) -> Self {
        Self {
            sort_mode: cfg.sort_mode,
            sort_direction: cfg.sort_direction,
            filter_chip: cfg.filter_chip,
            search_query: cfg.search_query.clone(),
            facets: cfg.facets.clone(),
            ledger_collapsed: cfg.ledger_collapsed,
            ledger_height_px: cfg.ledger_height_px,
            custom_hex_input: format_hex_rgb(cfg.theme_custom_rgb),
            custom_hex_valid: true,
            ..Self::default()
        }
    }
}

fn get_or_load_wt_logo_tex(
    ctx: &egui::Context,
    slot: &mut Option<(u32, egui::TextureHandle)>,
    logical_px: f32,
) -> egui::TextureId {
    let ppp = ctx.pixels_per_point().max(0.5);
    let phys_px = (logical_px * ppp).round().clamp(16.0, 128.0) as u32;
    let needs_reload = slot.as_ref().is_none_or(|(sz, _)| *sz != phys_px);
    if needs_reload {
        let px = phys_px as usize;
        let rgba = crate::logo::rasterize_wt_logo_rgba(px);
        let img = egui::ColorImage::from_rgba_unmultiplied([px, px], &rgba);
        let handle = ctx.load_texture(
            format!("wt_logo_mark_{phys_px}"),
            img,
            egui::TextureOptions::LINEAR,
        );
        *slot = Some((phys_px, handle));
    }
    slot.as_ref().unwrap().1.id()
}

/// Shared WT header lockup (`§6.4`): unmodified `24px` WT mark + `SPACE_8` + `16.0pt` app name + `SPACE_8` + `11.5pt` muted caption.
fn draw_wt_header_lockup(
    ui: &mut egui::Ui,
    logo_slot: &mut Option<(u32, egui::TextureHandle)>,
    app_name: &str,
    caption: &str,
    caption_color: Color32,
    pal: &ThemePalette,
) -> Option<egui::Response> {
    const LOGO_PX: f32 = 24.0;
    let row_h = ui.max_rect().height().clamp(LOGO_PX, HEADER_CONTROL_H);
    let (icon_slot, _) = ui.allocate_exact_size(Vec2::new(LOGO_PX, row_h), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let icon_rect = snap_rect_to_pixels(
        Rect::from_center_size(icon_slot.center(), Vec2::splat(LOGO_PX)),
        ppp,
    );
    let tex_id = get_or_load_wt_logo_tex(ui.ctx(), logo_slot, LOGO_PX);
    ui.painter().image(
        tex_id,
        icon_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    let title_font = FontId::new(16.0, FontFamily::Proportional);
    let title_galley = ui.fonts(|f| f.layout_no_wrap(app_name.to_owned(), title_font, pal.text));
    let (title_slot, _) =
        ui.allocate_exact_size(Vec2::new(title_galley.size().x.ceil(), row_h), Sense::hover());
    let title_pos = optical_galley_pos(ui, title_slot, &title_galley, None, false);
    ui.painter().galley(title_pos, title_galley, pal.text);

    if !caption.is_empty() {
        Some(draw_truncated_text(
            ui,
            caption,
            FontId::new(11.5, FontFamily::Proportional),
            caption_color,
            false,
        ))
    } else {
        None
    }
}

#[must_use]
pub fn format_size_2dec(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[must_use]
pub fn format_exact_bytes(bytes: u64) -> String {
    let raw = bytes.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3 + 6);
    for (idx, ch) in raw.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let formatted: String = out.chars().rev().collect();
    format!("{formatted} bytes")
}

fn format_delta_display(delta: GrowthDelta, pal: &ThemePalette) -> DeltaCellDisplay {
    match delta {
        GrowthDelta::NoBaseline => DeltaCellDisplay {
            kind: DeltaCellKind::NewPill,
            tip: Some("No baseline from a previous scan".to_string()),
        },
        GrowthDelta::New(b) => DeltaCellDisplay {
            kind: DeltaCellKind::NewPill,
            tip: Some(format!(
                "New since previous scan ({} total, no prior baseline)",
                format_exact_bytes(b)
            )),
        },
        GrowthDelta::Changed(0) => DeltaCellDisplay {
            kind: DeltaCellKind::Blank,
            tip: None,
        },
        GrowthDelta::Changed(d) if d.unsigned_abs() < GROWTH_NOISE_FLOOR_BYTES => {
            let sign = if d > 0 { "+" } else { "-" };
            let abs = d.unsigned_abs();
            DeltaCellDisplay {
                kind: DeltaCellKind::Blank,
                tip: Some(format!(
                    "{}{} ({}) since previous scan — below 1 MB display floor",
                    sign,
                    format_size_2dec(abs),
                    format_exact_bytes(abs)
                )),
            }
        }
        GrowthDelta::Changed(d) if d > 0 => DeltaCellDisplay {
            kind: DeltaCellKind::SignedValue {
                text: format!("+{}", format_size_2dec(d as u64)),
                color: pal.delta_grew,
            },
            tip: Some(format!("+{} since previous scan", format_exact_bytes(d as u64))),
        },
        GrowthDelta::Changed(d) => DeltaCellDisplay {
            kind: DeltaCellKind::SignedValue {
                text: format!("-{}", format_size_2dec(d.unsigned_abs())),
                color: pal.status_green,
            },
            tip: Some(format!(
                "-{} since previous scan",
                format_exact_bytes(d.unsigned_abs())
            )),
        },
    }
}

pub fn configure_fonts_and_theme(ctx: &egui::Context, mode: ThemeMode) {
    configure_fonts_and_theme_with(
        ctx,
        mode,
        ThemePresetChoice::Blue,
        default_custom_theme_rgb(),
    );
}

pub fn configure_fonts_and_theme_with(
    ctx: &egui::Context,
    mode: ThemeMode,
    preset: ThemePresetChoice,
    custom_rgb: [u8; 3],
) {
    let mut fonts = FontDefinitions::default();
    let prop_regular_bytes = std::fs::read(SEGOE_UI_FONT_PATH).ok();
    if let Some(ref bytes) = prop_regular_bytes {
        fonts
            .font_data
            .insert("segoe_ui".to_owned(), FontData::from_owned(bytes.clone()));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe_ui".to_owned());
        }
    }
    // Reserve Strong weight (`ProportionalStrong` = Segoe UI Semibold) strictly for row app names & drawer titles.
    let prop_strong_bytes = std::fs::read(SEGOE_UI_SEMIBOLD_PATH)
        .or_else(|_| std::fs::read(SEGOE_UI_BOLD_PATH))
        .ok()
        .or(prop_regular_bytes);
    if let Some(strong_bytes) = prop_strong_bytes {
        fonts
            .font_data
            .insert("segoe_ui_strong".to_owned(), FontData::from_owned(strong_bytes));
        fonts.families.insert(
            FontFamily::Name("ProportionalStrong".into()),
            vec!["segoe_ui_strong".to_owned(), "segoe_ui".to_owned()],
        );
    }
    // Consolas on Windows provides Regular (`consola.ttf`) and Bold (`consolab.ttf`) with no Medium face;
    // keep all numerals on Regular Consolas so numerals never render as heavy Bold slabs (DESIGN.md §3).
    let mono_regular_bytes = std::fs::read(CONSOLA_FONT_PATH)
        .or_else(|_| std::fs::read(CASCADIA_MONO_FONT_PATH))
        .ok();
    if let Some(mono_bytes) = mono_regular_bytes {
        fonts
            .font_data
            .insert("consola_mono".to_owned(), FontData::from_owned(mono_bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "consola_mono".to_owned());
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(16.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
    ]
    .into_iter()
    .collect();
    style.interaction.selectable_labels = false;
    ctx.set_style(style);
    ctx.tessellation_options_mut(|opts| {
        opts.feathering = true;
        opts.feathering_size_in_pixels = 1.0;
        opts.bezier_tolerance = 0.015;
    });
    apply_theme_visuals_with(ctx, mode, preset, custom_rgb);
}

#[inline]
fn mono_font_for(_ui: &egui::Ui, size_px: f32, _bold: bool) -> FontId {
    FontId::new(size_px, FontFamily::Monospace)
}

#[inline]
fn prop_strong_font(has_prop_strong: bool, size_px: f32) -> (FontId, bool) {
    if has_prop_strong {
        (FontId::new(size_px, FontFamily::Name("ProportionalStrong".into())), false)
    } else {
        (FontId::new(size_px, FontFamily::Proportional), true)
    }
}

pub fn apply_theme_visuals_with(
    ctx: &egui::Context,
    mode: ThemeMode,
    preset: ThemePresetChoice,
    custom_rgb: [u8; 3],
) {
    let pal = ThemePalette::from_config(mode, preset, custom_rgb);
    let mut style = (*ctx.style()).clone();
    let mut visuals = match mode {
        ThemeMode::Dark => egui::Visuals::dark(),
        ThemeMode::Light => egui::Visuals::light(),
    };
    visuals.dark_mode = mode == ThemeMode::Dark;
    visuals.panel_fill = pal.bg;
    visuals.window_fill = pal.panel;
    visuals.extreme_bg_color = pal.bg;
    visuals.faint_bg_color = pal.panel_alt;
    visuals.code_bg_color = pal.bg;
    // Never force override_text_color so TextEdit hints keep muted contrast while real text uses pal.text.
    visuals.override_text_color = None;
    visuals.window_rounding = Rounding::same(8.0);
    visuals.menu_rounding = Rounding::same(6.0);
    visuals.window_stroke = Stroke::new(1.0_f32, pal.border);
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;

    visuals.widgets.noninteractive.bg_fill = pal.panel;
    visuals.widgets.noninteractive.weak_bg_fill = pal.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.muted);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = pal.widget;
    visuals.widgets.inactive.weak_bg_fill = pal.widget;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = pal.widget_hover;
    visuals.widgets.hovered.weak_bg_fill = pal.widget_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, pal.accent_border);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = pal.widget_active;
    visuals.widgets.active.weak_bg_fill = pal.widget_active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, pal.accent_border);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    visuals.widgets.open.bg_fill = pal.widget_hover;
    visuals.widgets.open.weak_bg_fill = pal.widget_hover;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, pal.accent_border);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.open.rounding = Rounding::same(6.0);

    visuals.selection.bg_fill = lerp_color(pal.panel, pal.accent_border, 0.48);
    visuals.selection.stroke = Stroke::new(1.0_f32, pal.accent_line);

    style.visuals = visuals;
    style.interaction.selectable_labels = false;
    style.animation_time = 0.0;
    style.spacing.item_spacing = Vec2::new(SPACE_8, SPACE_4);
    style.spacing.button_padding = Vec2::new(SPACE_12, SPACE_4);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.bar_inner_margin = SPACE_8;
    style.spacing.scroll.bar_outer_margin = 0.0;
    style.spacing.scroll.handle_min_length = 24.0;
    ctx.set_style(style);
}

#[inline]
fn paint_solid_px_rect(painter: &egui::Painter, rect: Rect, color: Color32) {
    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices = vec![
        egui::epaint::Vertex {
            pos: rect.left_top(),
            uv: egui::epaint::WHITE_UV,
            color,
        },
        egui::epaint::Vertex {
            pos: rect.right_top(),
            uv: egui::epaint::WHITE_UV,
            color,
        },
        egui::epaint::Vertex {
            pos: rect.right_bottom(),
            uv: egui::epaint::WHITE_UV,
            color,
        },
        egui::epaint::Vertex {
            pos: rect.left_bottom(),
            uv: egui::epaint::WHITE_UV,
            color,
        },
    ];
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    painter.add(egui::Shape::mesh(mesh));
}

#[inline]
fn snap_rect_to_pixels(rect: Rect, ppp: f32) -> Rect {
    let inv = if ppp > 0.0 { 1.0 / ppp } else { 1.0 };
    let x0 = (rect.min.x * ppp).round() * inv;
    let y0 = (rect.min.y * ppp).round() * inv;
    let x1 = ((rect.max.x * ppp).round() * inv).max(x0 + 2.0 * inv);
    let y1 = ((rect.max.y * ppp).round() * inv).max(y0 + 2.0 * inv);
    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
}

const CORNER_ARC_PTS: usize = 7;

fn corner_unit_arcs() -> &'static [[(f32, f32); CORNER_ARC_PTS]; 4] {
    static ARCS: OnceLock<[[(f32, f32); CORNER_ARC_PTS]; 4]> = OnceLock::new();
    ARCS.get_or_init(|| {
        let ranges = [
            (std::f32::consts::PI, 1.5 * std::f32::consts::PI),
            (1.5 * std::f32::consts::PI, 2.0 * std::f32::consts::PI),
            (0.0, 0.5 * std::f32::consts::PI),
            (0.5 * std::f32::consts::PI, std::f32::consts::PI),
        ];
        let mut out = [[(0.0_f32, 0.0_f32); CORNER_ARC_PTS]; 4];
        let denom = (CORNER_ARC_PTS - 1) as f32;
        for (c_idx, &(a0, a1)) in ranges.iter().enumerate() {
            for k in 0..CORNER_ARC_PTS {
                let theta = a0 + (a1 - a0) * (k as f32 / denom);
                out[c_idx][k] = (theta.cos(), theta.sin());
            }
        }
        out
    })
}

fn push_rounded_rect_perimeter(rect: Rect, radius: f32, out: &mut Vec<(Pos2, Vec2)>) {
    let r = radius.clamp(0.0, 0.5 * rect.width().min(rect.height()));
    let centers = [
        Pos2::new(rect.left() + r, rect.top() + r),
        Pos2::new(rect.right() - r, rect.top() + r),
        Pos2::new(rect.right() - r, rect.bottom() - r),
        Pos2::new(rect.left() + r, rect.bottom() - r),
    ];
    let arcs = corner_unit_arcs();
    for (center, arc) in centers.into_iter().zip(arcs.iter()) {
        for &(nx, ny) in arc {
            let n = Vec2::new(nx, ny);
            let p = center + n * r;
            if let Some(&(prev_p, _)) = out.last() {
                if (p - prev_p).length_sq() < 1e-5 {
                    continue;
                }
            }
            out.push((p, n));
        }
    }
    if out.len() >= 2 && (out[0].0 - out[out.len() - 1].0).length_sq() < 1e-5 {
        out.pop();
    }
}

/// Emits fixed-count (`4 * 17 = 68`) rounded-rect perimeter vertices so concentric rings align 1:1.
fn sample_rounded_rect_ring(rect: Rect, radius: f32, diag_pad: f32, out: &mut Vec<Pos2>) {
    out.clear();
    let r = radius.clamp(0.0, 0.5 * rect.width().min(rect.height()));
    let eff_diag = if r > 0.5 { diag_pad } else { 0.0 };
    let centers = [
        Pos2::new(rect.left() + r, rect.top() + r),
        Pos2::new(rect.right() - r, rect.top() + r),
        Pos2::new(rect.right() - r, rect.bottom() - r),
        Pos2::new(rect.left() + r, rect.bottom() - r),
    ];
    let arcs = corner_unit_arcs();
    for (center, arc) in centers.into_iter().zip(arcs.iter()) {
        for &(nx, ny) in arc {
            let factor = ((nx.abs() + ny.abs()) - 1.0) / 0.414_213_56;
            let rad = (r + eff_diag * factor).max(0.0);
            out.push(center + Vec2::new(nx * rad, ny * rad));
        }
    }
}

fn stitch_closed_ring_quads(
    mesh: &mut egui::epaint::Mesh,
    outer_pts: &[Pos2],
    outer_col: Color32,
    inner_pts: &[Pos2],
    inner_col: Color32,
) {
    let n = outer_pts.len();
    if n < 4 || inner_pts.len() != n {
        return;
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.reserve(n * 2);
    mesh.indices.reserve(n * 6);
    for i in 0..n {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: outer_pts[i],
            uv: egui::epaint::WHITE_UV,
            color: outer_col,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: inner_pts[i],
            uv: egui::epaint::WHITE_UV,
            color: inner_col,
        });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let o0 = base + (i * 2) as u32;
        let i0 = o0 + 1;
        let o1 = base + (j * 2) as u32;
        let i1 = o1 + 1;
        mesh.indices.extend_from_slice(&[o0, o1, i0, i0, o1, i1]);
    }
}

/// Stitches ONLY the 4 rounded corner arcs between concentric `4 * CORNER_ARC_PTS` rings,
/// leaving the 4 straight H/V edges to axis-aligned `add_solid_rect_quad` bars.
fn stitch_corner_arcs_quads(
    mesh: &mut egui::epaint::Mesh,
    outer_pts: &[Pos2],
    outer_col: Color32,
    inner_pts: &[Pos2],
    inner_col: Color32,
) {
    let n = outer_pts.len();
    if n != 4 * CORNER_ARC_PTS || inner_pts.len() != n {
        stitch_closed_ring_quads(mesh, outer_pts, outer_col, inner_pts, inner_col);
        return;
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.reserve(n * 2);
    mesh.indices.reserve(4 * (CORNER_ARC_PTS - 1) * 6);
    for i in 0..n {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: outer_pts[i],
            uv: egui::epaint::WHITE_UV,
            color: outer_col,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: inner_pts[i],
            uv: egui::epaint::WHITE_UV,
            color: inner_col,
        });
    }
    for corner in 0..4 {
        let start = corner * CORNER_ARC_PTS;
        for k in 0..(CORNER_ARC_PTS - 1) {
            let i = start + k;
            let j = i + 1;
            let o0 = base + (i * 2) as u32;
            let i0 = o0 + 1;
            let o1 = base + (j * 2) as u32;
            let i1 = o1 + 1;
            mesh.indices.extend_from_slice(&[o0, o1, i0, i0, o1, i1]);
        }
    }
}

fn add_ring_center_fan(
    mesh: &mut egui::epaint::Mesh,
    ring_pts: &[Pos2],
    center: Pos2,
    col: Color32,
) {
    if col == Color32::TRANSPARENT || ring_pts.len() < 4 {
        return;
    }
    let n = ring_pts.len();
    let c_idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: egui::epaint::WHITE_UV,
        color: col,
    });
    let ring_base = mesh.vertices.len() as u32;
    for &p in ring_pts {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: p,
            uv: egui::epaint::WHITE_UV,
            color: col,
        });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.indices
            .extend_from_slice(&[c_idx, ring_base + i as u32, ring_base + j as u32]);
    }
}

#[inline]
fn add_solid_rect_quad(mesh: &mut egui::epaint::Mesh, rect: Rect, col: Color32) {
    if col == Color32::TRANSPARENT || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let b = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&[
        egui::epaint::Vertex {
            pos: rect.left_top(),
            uv: egui::epaint::WHITE_UV,
            color: col,
        },
        egui::epaint::Vertex {
            pos: rect.right_top(),
            uv: egui::epaint::WHITE_UV,
            color: col,
        },
        egui::epaint::Vertex {
            pos: rect.right_bottom(),
            uv: egui::epaint::WHITE_UV,
            color: col,
        },
        egui::epaint::Vertex {
            pos: rect.left_bottom(),
            uv: egui::epaint::WHITE_UV,
            color: col,
        },
    ]);
    mesh.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
}

#[inline]
fn add_corner_quadrant_fan(
    mesh: &mut egui::epaint::Mesh,
    corner_pts: &[Pos2],
    anchor: Pos2,
    col: Color32,
) {
    if col == Color32::TRANSPARENT || corner_pts.len() < 2 {
        return;
    }
    let a_idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: anchor,
        uv: egui::epaint::WHITE_UV,
        color: col,
    });
    let base = mesh.vertices.len() as u32;
    for &p in corner_pts {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: p,
            uv: egui::epaint::WHITE_UV,
            color: col,
        });
    }
    for i in 0..(corner_pts.len() - 1) {
        mesh.indices
            .extend_from_slice(&[a_idx, base + i as u32, base + i as u32 + 1]);
    }
}

/// Fills rounded-rect interior via 3 axis-aligned `WHITE_UV` quads + 4 localized corner fans
/// so software blitting uses `slice::fill` across >95% of panel interiors.
fn fill_rounded_interior_9slice(
    mesh: &mut egui::epaint::Mesh,
    fill_rect: Rect,
    ring_pts: &[Pos2],
    r_in: f32,
    col: Color32,
) {
    if col == Color32::TRANSPARENT || fill_rect.width() <= 0.0 || fill_rect.height() <= 0.0 {
        return;
    }
    if r_in <= 0.5 {
        add_solid_rect_quad(mesh, fill_rect, col);
        return;
    }
    if ring_pts.len() == 4 * CORNER_ARC_PTS
        && fill_rect.width() > 2.0 * r_in + 1.0
        && fill_rect.height() > 2.0 * r_in + 1.0
    {
        let left = fill_rect.left();
        let right = fill_rect.right();
        let top = fill_rect.top();
        let bottom = fill_rect.bottom();

        add_solid_rect_quad(
            mesh,
            Rect::from_min_max(Pos2::new(left + r_in, top), Pos2::new(right - r_in, top + r_in)),
            col,
        );
        add_solid_rect_quad(
            mesh,
            Rect::from_min_max(Pos2::new(left, top + r_in), Pos2::new(right, bottom - r_in)),
            col,
        );
        add_solid_rect_quad(
            mesh,
            Rect::from_min_max(
                Pos2::new(left + r_in, bottom - r_in),
                Pos2::new(right - r_in, bottom),
            ),
            col,
        );

        let tl_c = Pos2::new(left + r_in, top + r_in);
        let tr_c = Pos2::new(right - r_in, top + r_in);
        let br_c = Pos2::new(right - r_in, bottom - r_in);
        let bl_c = Pos2::new(left + r_in, bottom - r_in);
        add_corner_quadrant_fan(mesh, &ring_pts[0..CORNER_ARC_PTS], tl_c, col);
        add_corner_quadrant_fan(
            mesh,
            &ring_pts[CORNER_ARC_PTS..2 * CORNER_ARC_PTS],
            tr_c,
            col,
        );
        add_corner_quadrant_fan(
            mesh,
            &ring_pts[2 * CORNER_ARC_PTS..3 * CORNER_ARC_PTS],
            br_c,
            col,
        );
        add_corner_quadrant_fan(
            mesh,
            &ring_pts[3 * CORNER_ARC_PTS..4 * CORNER_ARC_PTS],
            bl_c,
            col,
        );
    } else {
        add_ring_center_fan(mesh, ring_pts, fill_rect.center(), col);
    }
}

/// Builds one physical-pixel-feathered rounded rectangle border+fill mesh (`1px` corner AA, crisp straight H/V edges).
fn build_feathered_rounded_box_mesh(
    snapped: Rect,
    rounding: f32,
    fill: Color32,
    border_color: Color32,
    border_px: f32,
    ppp: f32,
) -> egui::epaint::Mesh {
    let mut mesh = egui::epaint::Mesh::default();
    if snapped.width() <= 0.0 || snapped.height() <= 0.0 {
        return mesh;
    }
    let px = 1.0 / ppp.max(0.5);
    let half_px = 0.5 * px;
    let r = snap_scalar_to_px(
        rounding.clamp(0.0, 0.5 * snapped.width().min(snapped.height())),
        ppp,
    );

    if border_color == Color32::TRANSPARENT {
        if fill == Color32::TRANSPARENT {
            return mesh;
        }
        if r <= 0.5 * px {
            add_solid_rect_quad(&mut mesh, snapped, fill);
            return mesh;
        }
        let mut r0_pts = Vec::with_capacity(4 * CORNER_ARC_PTS);
        let mut r1_pts = Vec::with_capacity(4 * CORNER_ARC_PTS);
        sample_rounded_rect_ring(snapped.expand(half_px), r + half_px, 0.0, &mut r0_pts);
        let rect1 = if snapped.width() > px && snapped.height() > px {
            snapped.shrink(half_px)
        } else {
            snapped
        };
        let r1 = (r - half_px).max(0.0);
        sample_rounded_rect_ring(rect1, r1, 0.0, &mut r1_pts);
        if snapped.width() > 2.0 * r + px && snapped.height() > 2.0 * r + px {
            stitch_corner_arcs_quads(&mut mesh, &r0_pts, Color32::TRANSPARENT, &r1_pts, fill);
            fill_rounded_interior_9slice(&mut mesh, snapped, &r1_pts, r, fill);
        } else {
            stitch_closed_ring_quads(&mut mesh, &r0_pts, Color32::TRANSPARENT, &r1_pts, fill);
            fill_rounded_interior_9slice(&mut mesh, rect1, &r1_pts, r1, fill);
        }
        return mesh;
    }

    let bw = (border_px.max(1.0)) * px;
    let rect1 = if snapped.width() > px && snapped.height() > px {
        snapped.shrink(half_px)
    } else {
        snapped
    };
    let mut r0_pts = Vec::with_capacity(4 * CORNER_ARC_PTS);
    let mut r1_pts = Vec::with_capacity(4 * CORNER_ARC_PTS);
    let mut r2_pts = Vec::with_capacity(4 * CORNER_ARC_PTS);

    let straight_1px_fast = bw <= px * 1.05
        && r > px * 1.5
        && snapped.width() > 2.0 * r + 2.0 * px
        && snapped.height() > 2.0 * r + 2.0 * px;

    let r1b_pts = if bw > px * 1.05 {
        sample_rounded_rect_ring(snapped.expand(half_px), r + half_px, 0.0, &mut r0_pts);
        sample_rounded_rect_ring(rect1, (r - half_px).max(0.0), 0.0, &mut r1_pts);
        stitch_closed_ring_quads(&mut mesh, &r0_pts, Color32::TRANSPARENT, &r1_pts, border_color);
        let mut pts = Vec::with_capacity(4 * CORNER_ARC_PTS);
        let s1b = (bw - half_px).min(snapped.width().min(snapped.height()) * 0.45);
        sample_rounded_rect_ring(snapped.shrink(s1b), (r - s1b).max(0.0), 0.0, &mut pts);
        stitch_closed_ring_quads(&mut mesh, &r1_pts, border_color, &pts, border_color);
        pts
    } else {
        // Compensate 1.414px diagonal grid spacing at 45-degree corner midpoints so 1px strokes keep even optical density.
        let diag_pad = 0.42 * px;
        sample_rounded_rect_ring(snapped.expand(half_px), r + half_px, diag_pad, &mut r0_pts);
        sample_rounded_rect_ring(rect1, (r - half_px).max(0.0), diag_pad, &mut r1_pts);
        let mut r1_in_pts = Vec::with_capacity(4 * CORNER_ARC_PTS);
        sample_rounded_rect_ring(rect1, (r - half_px).max(0.0), -diag_pad, &mut r1_in_pts);
        if straight_1px_fast {
            stitch_corner_arcs_quads(&mut mesh, &r0_pts, Color32::TRANSPARENT, &r1_pts, border_color);
            stitch_corner_arcs_quads(&mut mesh, &r1_pts, border_color, &r1_in_pts, border_color);
            let x0 = snapped.left();
            let x1 = snapped.right();
            let y0 = snapped.top();
            let y1 = snapped.bottom();
            // 4 axis-aligned 1px border bars spanning between corner tangents
            add_solid_rect_quad(
                &mut mesh,
                Rect::from_min_max(Pos2::new(x0 + r, y0), Pos2::new(x1 - r, y0 + px)),
                border_color,
            );
            add_solid_rect_quad(
                &mut mesh,
                Rect::from_min_max(Pos2::new(x0 + r, y1 - px), Pos2::new(x1 - r, y1)),
                border_color,
            );
            add_solid_rect_quad(
                &mut mesh,
                Rect::from_min_max(Pos2::new(x0, y0 + r), Pos2::new(x0 + px, y1 - r)),
                border_color,
            );
            add_solid_rect_quad(
                &mut mesh,
                Rect::from_min_max(Pos2::new(x1 - px, y0 + r), Pos2::new(x1, y1 - r)),
                border_color,
            );
        } else {
            stitch_closed_ring_quads(&mut mesh, &r0_pts, Color32::TRANSPARENT, &r1_pts, border_color);
            stitch_closed_ring_quads(&mut mesh, &r1_pts, border_color, &r1_in_pts, border_color);
        }
        r1_in_pts
    };

    let inner_shrink = bw + half_px;
    if snapped.width() > inner_shrink * 2.0 && snapped.height() > inner_shrink * 2.0 {
        let rect2 = snapped.shrink(inner_shrink);
        let r2 = (r - inner_shrink).max(0.0);
        let inner_diag = if bw <= px * 1.05 { -0.42 * px } else { 0.0 };
        sample_rounded_rect_ring(rect2, r2, inner_diag, &mut r2_pts);
        if straight_1px_fast {
            stitch_corner_arcs_quads(&mut mesh, &r1b_pts, border_color, &r2_pts, fill);
            if fill != Color32::TRANSPARENT {
                let x0 = snapped.left();
                let x1 = snapped.right();
                let y0 = snapped.top();
                let y1 = snapped.bottom();
                add_solid_rect_quad(
                    &mut mesh,
                    Rect::from_min_max(Pos2::new(x0 + r, y0 + px), Pos2::new(x1 - r, y0 + r)),
                    fill,
                );
                add_solid_rect_quad(
                    &mut mesh,
                    Rect::from_min_max(Pos2::new(x0 + px, y0 + r), Pos2::new(x1 - px, y1 - r)),
                    fill,
                );
                add_solid_rect_quad(
                    &mut mesh,
                    Rect::from_min_max(Pos2::new(x0 + r, y1 - r), Pos2::new(x1 - r, y1 - px)),
                    fill,
                );
                let tl_c = Pos2::new(x0 + r, y0 + r);
                let tr_c = Pos2::new(x1 - r, y0 + r);
                let br_c = Pos2::new(x1 - r, y1 - r);
                let bl_c = Pos2::new(x0 + r, y1 - r);
                add_corner_quadrant_fan(&mut mesh, &r2_pts[0..CORNER_ARC_PTS], tl_c, fill);
                add_corner_quadrant_fan(
                    &mut mesh,
                    &r2_pts[CORNER_ARC_PTS..2 * CORNER_ARC_PTS],
                    tr_c,
                    fill,
                );
                add_corner_quadrant_fan(
                    &mut mesh,
                    &r2_pts[2 * CORNER_ARC_PTS..3 * CORNER_ARC_PTS],
                    br_c,
                    fill,
                );
                add_corner_quadrant_fan(
                    &mut mesh,
                    &r2_pts[3 * CORNER_ARC_PTS..4 * CORNER_ARC_PTS],
                    bl_c,
                    fill,
                );
            }
        } else {
            stitch_closed_ring_quads(&mut mesh, &r1b_pts, border_color, &r2_pts, fill);
            fill_rounded_interior_9slice(&mut mesh, rect2, &r2_pts, r2, fill);
        }
    } else {
        add_ring_center_fan(&mut mesh, &r1b_pts, snapped.center(), border_color);
    }
    mesh
}

fn build_rounded_rect_polygon(rect: Rect, radius: f32) -> Vec<Pos2> {
    let mut ring = Vec::with_capacity(4 * CORNER_ARC_PTS);
    push_rounded_rect_perimeter(rect, radius, &mut ring);
    ring.into_iter().map(|(p, _)| p).collect()
}

/// Builds origin-relative monochrome sweep rim mesh + inner polygon once per `(w, h, rounding, ppp, theme_key)` and translates on scroll frames.
fn build_cached_rim_geometry(
    w: f32,
    h: f32,
    rounding: f32,
    ppp: f32,
    pal: &ThemePalette,
) -> CachedRimMesh {
    let px = 1.0 / ppp.max(0.5);
    let half_px = 0.5 * px;
    let local_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w.max(1.0), h.max(1.0)));
    let fill_poly = build_rounded_rect_polygon(
        local_rect.shrink(px),
        (rounding - px).max(0.0),
    );
    let mut ring = Vec::with_capacity(4 * CORNER_ARC_PTS);
    push_rounded_rect_perimeter(local_rect, rounding, &mut ring);
    let n = ring.len();
    const BANDS: usize = 5;
    let offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -half_px, half_px];
    let alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8];

    let mut mesh = egui::epaint::Mesh::default();
    if n >= 8 {
        let lw = local_rect.width().max(1.0);
        let lh = local_rect.height().max(1.0);
        mesh.vertices.reserve(n * BANDS);
        mesh.indices.reserve(n * (BANDS - 1) * 6);
        for (p, norm) in &ring {
            let t = ((p.x / lw) * 0.65 + (p.y / lh) * 0.35).clamp(0.0, 1.0);
            let c = pal.sample_sweep(t);
            for (&off, &a) in offsets.iter().zip(alphas.iter()) {
                let color = if a == 0 {
                    Color32::TRANSPARENT
                } else {
                    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
                };
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: *p + *norm * off,
                    uv: egui::epaint::WHITE_UV,
                    color,
                });
            }
        }
        let stride = BANDS as u32;
        for i in 0..n {
            let j = (i + 1) % n;
            let b0 = (i as u32) * stride;
            let b1 = (j as u32) * stride;
            for band in 0..(stride - 1) {
                let i0 = b0 + band;
                let i1 = b0 + band + 1;
                let i2 = b1 + band;
                let i3 = b1 + band + 1;
                mesh.indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            }
        }
    }
    CachedRimMesh {
        w_q: (w * 2.0).round() as i32,
        h_q: (h * 2.0).round() as i32,
        r_q: (rounding * 10.0).round() as i32,
        ppp_q: (ppp * 100.0).round() as i32,
        theme_key: pal.theme_key,
        mesh,
        fill_poly,
    }
}

fn translate_cached_rim_for_rect(
    rect: Rect,
    rounding: f32,
    ppp: f32,
    pal: &ThemePalette,
    cache: &mut Option<CachedRimMesh>,
) -> (Vec<Pos2>, egui::epaint::Mesh) {
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    let w_q = (w * 2.0).round() as i32;
    let h_q = (h * 2.0).round() as i32;
    let r_q = (rounding * 10.0).round() as i32;
    let ppp_q = (ppp * 100.0).round() as i32;
    let hit = cache.as_ref().is_some_and(|c| {
        c.w_q == w_q
            && c.h_q == h_q
            && c.r_q == r_q
            && c.ppp_q == ppp_q
            && c.theme_key == pal.theme_key
    });
    if !hit {
        *cache = Some(build_cached_rim_geometry(w, h, rounding, ppp, pal));
    }
    let cached = cache.as_ref().unwrap();
    let offset = rect.min.to_vec2();
    let fill_poly = cached.fill_poly.iter().map(|&p| p + offset).collect();
    let mut mesh = cached.mesh.clone();
    for v in &mut mesh.vertices {
        v.pos += offset;
    }
    (fill_poly, mesh)
}

/// Diagonal within-family monochrome sweep rim bounded strictly inside `rect`.
#[allow(dead_code)]
fn paint_active_card_gradient_border(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    pal: &ThemePalette,
) {
    if rect.width() <= 4.0 || rect.height() <= 4.0 {
        return;
    }
    let mut ring = Vec::with_capacity(4 * CORNER_ARC_PTS);
    push_rounded_rect_perimeter(rect, rounding, &mut ring);
    let n = ring.len();
    if n < 8 {
        return;
    }

    const BANDS: usize = 5;
    let offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.4_f32, 0.4_f32];
    let alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8];

    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);

    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices.reserve(n * BANDS);
    mesh.indices.reserve(n * (BANDS - 1) * 6);

    for (p, norm) in &ring {
        let t = (((p.x - rect.left()) / w) * 0.65 + ((p.y - rect.top()) / h) * 0.35).clamp(0.0, 1.0);
        let c = pal.sample_sweep(t);
        for (&off, &a) in offsets.iter().zip(alphas.iter()) {
            let color = if a == 0 {
                Color32::TRANSPARENT
            } else {
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
            };
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *p + *norm * off,
                uv: egui::epaint::WHITE_UV,
                color,
            });
        }
    }

    let stride = BANDS as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        let b0 = (i as u32) * stride;
        let b1 = (j as u32) * stride;
        for band in 0..(stride - 1) {
            let i0 = b0 + band;
            let i1 = b0 + band + 1;
            let i2 = b1 + band;
            let i3 = b1 + band + 1;
            mesh.indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    painter.add(egui::Shape::mesh(mesh));
}

/// Paints an unbroken physical-pixel-feathered rounded rectangle border & optional fill, safe at 96 DPI and high DPI.
/// When `border_color == Color32::TRANSPARENT` and `fill == Color32::TRANSPARENT`, paints nothing (quiet `SubtleOutline`).
fn paint_crisp_bordered_box(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    fill: Color32,
    border_color: Color32,
    border_px: f32,
    ppp: f32,
) -> Rect {
    let snapped = snap_rect_to_pixels(rect, ppp);
    let mesh = build_feathered_rounded_box_mesh(snapped, rounding, fill, border_color, border_px, ppp);
    if !mesh.indices.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
    snapped
}

/// Container frame with a guaranteed continuous anti-aliased 1px outline at 96 DPI and high DPI.
fn draw_crisp_frame<R>(
    ui: &mut egui::Ui,
    fill: Color32,
    border_color: Color32,
    rounding: f32,
    inner_margin: Margin,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let frame_slot = ui.painter().add(egui::Shape::Noop);
    let padded = Margin {
        left: inner_margin.left + 1.0,
        right: inner_margin.right + 1.0,
        top: inner_margin.top + 1.0,
        bottom: inner_margin.bottom + 1.0,
    };
    let res = egui::Frame::none()
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .inner_margin(padded)
        .show(ui, add_contents);

    let ppp = ui.ctx().pixels_per_point();
    let snapped = snap_rect_to_pixels(res.response.rect, ppp);
    let mesh = build_feathered_rounded_box_mesh(snapped, rounding, fill, border_color, 1.0, ppp);
    if !mesh.indices.is_empty() {
        ui.painter().set(frame_slot, egui::Shape::mesh(mesh));
    }
    res
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonVariant {
    PrimaryOutline,
    SecondaryOutline,
    SubtleOutline,
}

#[inline]
fn snap_scalar_to_px(v: f32, ppp: f32) -> f32 {
    let p = ppp.max(0.5);
    (v * p).round() / p
}

#[inline]
fn font_family_tag(family: &FontFamily) -> u8 {
    match family {
        FontFamily::Proportional => 0,
        FontFamily::Monospace => 1,
        FontFamily::Name(_) => 2,
    }
}

fn cached_hx_metrics(ui: &egui::Ui, font_id: &FontId) -> Option<(f32, f32)> {
    static HX_CACHE: OnceLock<std::sync::Mutex<HashMap<(u32, u32, u8), (f32, f32)>>> =
        OnceLock::new();
    let key = (
        ui.ctx().pixels_per_point().to_bits(),
        font_id.size.to_bits(),
        font_family_tag(&font_id.family),
    );
    let cache = HX_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(&val) = guard.get(&key) {
            return Some(val);
        }
    }
    let ref_g = ui.fonts(|f| f.layout_no_wrap("Hx".to_owned(), font_id.clone(), Color32::WHITE));
    if ref_g.mesh_bounds.is_positive() {
        let val = (ref_g.mesh_bounds.center().y, ref_g.mesh_bounds.max.y);
        if let Ok(mut guard) = cache.lock() {
            guard.insert(key, val);
        }
        Some(val)
    } else {
        None
    }
}

/// Computes pixel-snapped top-left origin so glyph ink is optically centered vertically in `outer_rect`.
fn optical_galley_pos(
    ui: &egui::Ui,
    outer_rect: Rect,
    galley: &std::sync::Arc<egui::epaint::Galley>,
    align_right_x: Option<f32>,
    align_center_x: bool,
) -> Pos2 {
    let ppp = ui.ctx().pixels_per_point();
    let x = if let Some(right_x) = align_right_x {
        (right_x - galley.size().x).max(outer_rect.min.x)
    } else if align_center_x {
        outer_rect.center().x - galley.size().x * 0.5
    } else {
        outer_rect.min.x
    };
    let (ink_center_y, baseline_from_top) = if let Some(font_id) = galley
        .job
        .sections
        .first()
        .map(|s| &s.format.font_id)
    {
        if let Some(m) = cached_hx_metrics(ui, font_id) {
            m
        } else if galley.mesh_bounds.is_positive() {
            (galley.mesh_bounds.center().y, galley.mesh_bounds.max.y)
        } else {
            (galley.size().y * 0.5, galley.size().y * 0.8)
        }
    } else if galley.mesh_bounds.is_positive() {
        (galley.mesh_bounds.center().y, galley.mesh_bounds.max.y)
    } else {
        (galley.size().y * 0.5, galley.size().y * 0.8)
    };
    let _ = baseline_from_top;
    let y = outer_rect.center().y - ink_center_y;
    Pos2::new(snap_scalar_to_px(x, ppp), snap_scalar_to_px(y, ppp))
}

fn baseline_aligned_galley_pos(
    ui: &egui::Ui,
    outer_rect: Rect,
    galley: &std::sync::Arc<egui::epaint::Galley>,
    anchor_font: FontId,
) -> Pos2 {
    let ppp = ui.ctx().pixels_per_point();
    let probe = |fid: &FontId, fallback_h: f32| -> (f32, f32) {
        cached_hx_metrics(ui, fid).unwrap_or((fallback_h * 0.5, fallback_h * 0.8))
    };
    let (anchor_cy, anchor_base) = probe(&anchor_font, galley.size().y);
    let (_, self_base) = if let Some(fid) = galley.job.sections.first().map(|s| &s.format.font_id) {
        probe(fid, galley.size().y)
    } else {
        (galley.size().y * 0.5, galley.size().y * 0.8)
    };
    let baseline_y = outer_rect.center().y - anchor_cy + anchor_base;
    let y = baseline_y - self_base;
    Pos2::new(snap_scalar_to_px(outer_rect.min.x, ppp), snap_scalar_to_px(y, ppp))
}

fn draw_optical_label_in_rect(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    font_id: FontId,
    color: Color32,
    align_right: bool,
    align_center_x: bool,
) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font_id, color));
    let pos = optical_galley_pos(
        ui,
        rect,
        &galley,
        if align_right { Some(rect.max.x) } else { None },
        align_center_x,
    );
    let resp = ui.interact(
        rect,
        ui.make_persistent_id(("opt_lbl", rect.min.x.to_bits(), rect.min.y.to_bits(), text)),
        Sense::hover(),
    );
    ui.painter().galley(pos, galley, color);
    resp
}

/// Paints a within-family monochrome sweep across `full_track`, clipped to `fill_rect` (`WT_UI_LIBRARY §0.6`).
fn paint_horizontal_theme_sweep_bar(
    painter: &egui::Painter,
    full_track: Rect,
    fill_rect: Rect,
    rounding: f32,
    pal: &ThemePalette,
) {
    if fill_rect.width() <= 0.5 || fill_rect.height() <= 0.5 || full_track.width() <= 0.5 {
        return;
    }
    let r = rounding
        .min(fill_rect.height() * 0.5)
        .min(fill_rect.width() * 0.5)
        .max(0.0);
    let x0 = fill_rect.min.x;
    let x1 = fill_rect.max.x;
    let y0 = fill_rect.min.y;
    let y1 = fill_rect.max.y;

    let mut xs: Vec<f32> = Vec::with_capacity(14);
    xs.push(x0);
    if r > 1.5 {
        for k in 1..=3 {
            let a = (k as f32) / 4.0 * std::f32::consts::FRAC_PI_2;
            xs.push(x0 + r * (1.0 - a.cos()));
        }
    }
    for frac in [0.333_f32, 0.667_f32] {
        let stop_x = full_track.min.x + full_track.width() * frac;
        if stop_x > x0 + r + 0.5 && stop_x < x1 - r - 0.5 {
            xs.push(stop_x);
        }
    }
    if r > 1.5 {
        for k in 1..=3 {
            let a = (k as f32) / 4.0 * std::f32::consts::FRAC_PI_2;
            xs.push(x1 - r * (1.0 - a.sin()));
        }
    }
    xs.push(x1);
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    if xs.len() < 2 {
        let col = pal.sample_sweep((fill_rect.center().x - full_track.min.x) / full_track.width());
        painter.rect_filled(fill_rect, Rounding::same(r), col);
        return;
    }

    let mut mesh = egui::epaint::Mesh::default();
    let mut prev_top = 0u32;
    let mut prev_bot = 0u32;
    for (idx, &x) in xs.iter().enumerate() {
        let (ty, by) = if r > 0.0 && x < x0 + r {
            let dx = (x0 + r) - x;
            let dy = (r * r - dx * dx).max(0.0).sqrt();
            (y0 + r - dy, y1 - r + dy)
        } else if r > 0.0 && x > x1 - r {
            let dx = x - (x1 - r);
            let dy = (r * r - dx * dx).max(0.0).sqrt();
            (y0 + r - dy, y1 - r + dy)
        } else {
            (y0, y1)
        };
        let t = ((x - full_track.min.x) / full_track.width()).clamp(0.0, 1.0);
        let col = pal.sample_sweep(t);
        let vt = mesh.vertices.len() as u32;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(x, ty),
            uv: egui::epaint::WHITE_UV,
            color: col,
        });
        let vb = mesh.vertices.len() as u32;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(x, by),
            uv: egui::epaint::WHITE_UV,
            color: col,
        });
        if idx > 0 {
            mesh.indices.extend_from_slice(&[prev_top, vt, prev_bot, prev_bot, vt, vb]);
        }
        prev_top = vt;
        prev_bot = vb;
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Paints a smooth vertical fade-out gradient overlay at the bottom edge of a scrolling list.
fn paint_bottom_fade_overlay(painter: &egui::Painter, rect: Rect, surface_bg: Color32) {
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return;
    }
    let top_col = Color32::from_rgba_unmultiplied(surface_bg.r(), surface_bg.g(), surface_bg.b(), 0);
    let mid_col = Color32::from_rgba_unmultiplied(surface_bg.r(), surface_bg.g(), surface_bg.b(), 145);
    let bot_col = Color32::from_rgba_unmultiplied(surface_bg.r(), surface_bg.g(), surface_bg.b(), 238);
    let mid_y = rect.min.y + rect.height() * 0.55;
    let mut mesh = egui::epaint::Mesh::default();
    let add_row = |m: &mut egui::epaint::Mesh, y: f32, c: Color32| -> (u32, u32) {
        let l = m.vertices.len() as u32;
        m.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(rect.min.x, y),
            uv: egui::epaint::WHITE_UV,
            color: c,
        });
        let r = m.vertices.len() as u32;
        m.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(rect.max.x, y),
            uv: egui::epaint::WHITE_UV,
            color: c,
        });
        (l, r)
    };
    let (l0, r0) = add_row(&mut mesh, rect.min.y, top_col);
    let (l1, r1) = add_row(&mut mesh, mid_y, mid_col);
    let (l2, r2) = add_row(&mut mesh, rect.max.y, bot_col);
    mesh.indices.extend_from_slice(&[l0, r0, l1, l1, r0, r1, l1, r1, l2, l2, r1, r2]);
    painter.add(egui::Shape::mesh(mesh));
}

/// Resolves `(fill, border_col, text_color)` under WT_UI_LIBRARY §0 single-hue monochrome ladder.
fn resolve_button_colors(
    variant: ButtonVariant,
    enabled: bool,
    pressed: bool,
    hovered: bool,
    active_highlight: bool,
    pal: &ThemePalette,
) -> (Color32, Color32, Color32) {
    if !enabled {
        return match variant {
            ButtonVariant::SubtleOutline => (
                Color32::TRANSPARENT,
                Color32::TRANSPARENT,
                lerp_color(pal.bg, pal.muted, 0.55),
            ),
            ButtonVariant::PrimaryOutline | ButtonVariant::SecondaryOutline => {
                (Color32::TRANSPARENT, pal.border, pal.muted)
            }
        };
    }
    if pressed {
        let press_border = match variant {
            ButtonVariant::SubtleOutline => Color32::TRANSPARENT,
            ButtonVariant::PrimaryOutline | ButtonVariant::SecondaryOutline => pal.accent_border,
        };
        return (pal.widget_active, press_border, pal.text);
    }
    if hovered {
        return match variant {
            ButtonVariant::PrimaryOutline | ButtonVariant::SecondaryOutline => {
                (pal.widget_hover, pal.accent_border, pal.accent_line)
            }
            ButtonVariant::SubtleOutline => {
                (pal.widget_hover, Color32::TRANSPARENT, pal.accent_line)
            }
        };
    }
    if active_highlight {
        return (pal.accent_soft_fill, pal.accent_border, pal.accent_line);
    }
    match variant {
        ButtonVariant::PrimaryOutline => (pal.accent_soft_fill, pal.accent_border, pal.accent_line),
        ButtonVariant::SecondaryOutline => (Color32::TRANSPARENT, pal.border, pal.text),
        ButtonVariant::SubtleOutline => (Color32::TRANSPARENT, Color32::TRANSPARENT, pal.muted),
    }
}

/// Resolves `(fill, border_col, check_col)` for monochrome Checkbox (`WT_UI_LIBRARY §0 & §4 #6`).
fn resolve_checkbox_colors(
    checked: bool,
    hovered: bool,
    pal: &ThemePalette,
) -> (Color32, Color32, Color32) {
    let fill = if checked {
        pal.accent_soft_fill
    } else if hovered {
        pal.widget_hover
    } else {
        pal.input_bg
    };
    let border_col = if checked || hovered {
        pal.accent_border
    } else {
        pal.border
    };
    (fill, border_col, pal.accent_line)
}

/// Renders an outline or quiet row-level button enforcing the WT_UI_LIBRARY outline/quiet hierarchy.
#[allow(clippy::too_many_arguments)]
fn draw_z_button(
    ui: &mut egui::Ui,
    label: &str,
    min_size: Vec2,
    font_size: f32,
    rounding: f32,
    enabled: bool,
    active_highlight: bool,
    variant: ButtonVariant,
    pal: &ThemePalette,
) -> egui::Response {
    let h_pad = if variant == ButtonVariant::SubtleOutline && min_size.y <= 20.0 {
        3.0
    } else if variant == ButtonVariant::PrimaryOutline && min_size.y <= 24.0 && min_size.x <= 52.0 {
        6.0
    } else if min_size.y <= 24.0 {
        SPACE_8
    } else {
        SPACE_12
    };
    let font_id = FontId::new(font_size, FontFamily::Proportional);
    let measure_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id.clone(), pal.text));
    let desired_w = (measure_galley.size().x + h_pad * 2.0).max(min_size.x).round();
    let desired_h = min_size.y.round();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, desired_h), sense);

    let (fill, border_col, text_color) = resolve_button_colors(
        variant,
        enabled,
        resp.is_pointer_button_down_on(),
        resp.hovered(),
        active_highlight,
        pal,
    );

    let ppp = ui.ctx().pixels_per_point();
    paint_crisp_bordered_box(ui.painter(), rect, rounding, fill, border_col, 1.0, ppp);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let text_pos = optical_galley_pos(ui, rect, &galley, None, true);
    ui.painter().galley(text_pos, galley, text_color);
    resp
}

/// Recessed input box with crisp 1px border, dim placeholder hint hidden on focus or non-empty input, and full-strength entered text.
fn draw_crisp_text_input(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    text: &mut String,
    width: f32,
    hint: &str,
    well_bg: Color32,
    pal: &ThemePalette,
) -> egui::Response {
    let edit_id = ui.make_persistent_id(id_salt);
    let has_focus = ui.ctx().memory(|m| m.has_focus(edit_id));
    let border_col = if has_focus { pal.accent_line } else { pal.border };
    let show_hint = !has_focus && text.is_empty();
    let hint_rt = if show_hint {
        RichText::new(hint).size(12.0).color(pal.hint_text)
    } else {
        RichText::new("")
    };

    let inner = draw_crisp_frame(
        ui,
        well_bg,
        border_col,
        RADIUS_CONTROL,
        Margin::symmetric(SPACE_8, 3.0),
        |ui| {
            ui.set_min_height(18.0);
            ui.add(
                egui::TextEdit::singleline(text)
                    .id(edit_id)
                    .frame(false)
                    .desired_width((width - SPACE_16 - 2.0).max(80.0))
                    .font(FontId::new(12.0, FontFamily::Proportional))
                    .text_color(pal.text)
                    .hint_text(hint_rt),
            )
        },
    );
    if ui.rect_contains_pointer(inner.response.rect) || inner.inner.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        if inner.response.clicked() && !inner.inner.has_focus() {
            ui.ctx().memory_mut(|m| m.request_focus(edit_id));
        }
    }
    inner.inner
}

/// Crisp 1px-bordered toggle checkbox matching the monochrome theme ladder on one shared optical baseline.
fn draw_crisp_checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    label: &str,
    pal: &ThemePalette,
) -> bool {
    let font_id = FontId::new(12.0, FontFamily::Proportional);
    let text_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id.clone(), pal.text));
    let total_w = (16.0 + SPACE_8 + text_galley.size().x).ceil();
    let row_h = 22.0_f32.max(text_galley.size().y);
    let (hit_rect, resp) = ui.allocate_exact_size(Vec2::new(total_w, row_h), Sense::click());
    let ppp = ui.ctx().pixels_per_point();
    let (fill, border_col, check_col) = resolve_checkbox_colors(*checked, resp.hovered(), pal);
    let box_rect = Rect::from_center_size(
        Pos2::new(hit_rect.min.x + 8.0, hit_rect.center().y),
        Vec2::splat(16.0),
    );
    let snapped =
        paint_crisp_bordered_box(ui.painter(), box_rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
    if *checked {
        let c = snapped.center();
        let st = Stroke::new(1.75_f32, check_col);
        ui.painter().add(egui::Shape::line(
            vec![
                Pos2::new(c.x - 4.0, c.y + 0.5),
                Pos2::new(c.x - 1.0, c.y + 3.5),
                Pos2::new(c.x + 4.5, c.y - 3.0),
            ],
            st,
        ));
    }
    let lbl_col = if resp.hovered() { pal.accent_line } else { pal.text };
    let lbl_galley = if lbl_col == pal.text {
        text_galley
    } else {
        ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, lbl_col))
    };
    let label_rect = Rect::from_min_max(
        Pos2::new(hit_rect.min.x + 16.0 + SPACE_8, hit_rect.min.y),
        hit_rect.max,
    );
    let text_pos = optical_galley_pos(ui, label_rect, &lbl_galley, None, false);
    ui.painter().galley(text_pos, lbl_galley, lbl_col);
    if resp.clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// Vertical scroll area styled identically to z-controller, exposing viewport bounds for row virtualization.
fn draw_synced_scroll_area_viewport(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    max_height: Option<f32>,
    auto_shrink: [bool; 2],
    pal: &ThemePalette,
    rail_bg: Color32,
    body: impl FnOnce(&mut egui::Ui, Rect),
) {
    draw_synced_scroll_area_viewport_with_reset(
        ui,
        id_salt,
        max_height,
        auto_shrink,
        false,
        pal,
        rail_bg,
        body,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_synced_scroll_area_viewport_with_reset(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    max_height: Option<f32>,
    auto_shrink: [bool; 2],
    scroll_to_top: bool,
    pal: &ThemePalette,
    rail_bg: Color32,
    body: impl FnOnce(&mut egui::Ui, Rect),
) {
    ui.scope(|ui| {
        ui.spacing_mut().scroll.floating = false;
        ui.spacing_mut().scroll.foreground_color = false;
        ui.spacing_mut().scroll.bar_width = 6.0;
        ui.spacing_mut().scroll.bar_inner_margin = SPACE_8;
        ui.spacing_mut().scroll.bar_outer_margin = 0.0;
        ui.spacing_mut().scroll.handle_min_length = 24.0;
        {
            let v = ui.visuals_mut();
            v.extreme_bg_color = rail_bg;
            v.widgets.inactive.bg_fill = pal.scrollbar_thumb;
            v.widgets.hovered.bg_fill = pal.scrollbar_thumb_hover;
            v.widgets.active.bg_fill = pal.accent_line;
        }

        let mut sa = egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .auto_shrink(auto_shrink);
        if let Some(mh) = max_height {
            sa = sa.max_height(mh);
        }
        if scroll_to_top {
            sa = sa.vertical_scroll_offset(0.0);
        }
        sa.show_viewport(ui, |ui, viewport| {
            // Restore inner widget fills so buttons inside scrolled cards still paint with pal.widget.
            {
                let v = ui.visuals_mut();
                v.extreme_bg_color = pal.bg;
                v.widgets.inactive.bg_fill = pal.widget;
                v.widgets.hovered.bg_fill = pal.widget_hover;
                v.widgets.active.bg_fill = pal.widget_active;
            }
            body(ui, viewport);
        });
    });
}

fn draw_synced_scroll_area(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    max_height: Option<f32>,
    auto_shrink: [bool; 2],
    pal: &ThemePalette,
    rail_bg: Color32,
    body: impl FnOnce(&mut egui::Ui),
) {
    draw_synced_scroll_area_viewport(ui, id_salt, max_height, auto_shrink, pal, rail_bg, |ui, _| {
        body(ui);
    });
}

fn draw_status_glyph(painter: &egui::Painter, center: Pos2, kind: StatusKind, color: Color32) {
    match kind {
        StatusKind::Success => {
            painter.add(egui::Shape::line(
                vec![
                    Pos2::new(center.x - 3.0, center.y + 0.3),
                    Pos2::new(center.x - 0.8, center.y + 2.6),
                    Pos2::new(center.x + 3.2, center.y - 2.4),
                ],
                Stroke::new(1.5_f32, color),
            ));
        }
        StatusKind::Warning => {
            painter.line_segment(
                [
                    Pos2::new(center.x, center.y - 3.2),
                    Pos2::new(center.x, center.y + 0.6),
                ],
                Stroke::new(1.6_f32, color),
            );
            painter.circle_filled(Pos2::new(center.x, center.y + 2.8), 1.0, color);
        }
        StatusKind::Danger => {
            let st = Stroke::new(1.5_f32, color);
            painter.line_segment(
                [
                    Pos2::new(center.x - 2.5, center.y - 2.5),
                    Pos2::new(center.x + 2.5, center.y + 2.5),
                ],
                st,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 2.5, center.y - 2.5),
                    Pos2::new(center.x - 2.5, center.y + 2.5),
                ],
                st,
            );
        }
    }
}

fn status_chip_style(kind: StatusKind, pal: &ThemePalette) -> ChipStyle {
    match kind {
        StatusKind::Success => ChipStyle {
            fill: pal.pill_green_fill,
            stroke: pal.status_green,
            text: pal.status_green,
        },
        StatusKind::Warning => ChipStyle {
            fill: pal.pill_amber_fill,
            stroke: pal.status_amber,
            text: pal.status_amber,
        },
        StatusKind::Danger => ChipStyle {
            fill: pal.pill_red_fill,
            stroke: pal.status_red,
            text: pal.status_red,
        },
    }
}

/// Semantic §4B status pill ALWAYS carrying vector icon (`✓` / `!` / `✕`) + text label (`WT_UI_LIBRARY §0.8`).
fn paint_status_chip(
    ui: &mut egui::Ui,
    kind: StatusKind,
    label: &str,
    pal: &ThemePalette,
) -> egui::Response {
    let style = status_chip_style(kind, pal);
    let font_id = FontId::new(10.5, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, style.text));
    let icon_w = 7.0_f32;
    let icon_gap = 4.0_f32;
    let h_pad = 7.0_f32;
    let desired_w = (h_pad * 2.0 + icon_w + icon_gap + galley.size().x).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, BADGE_PILL_H), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(
        ui.painter(),
        rect,
        RADIUS_BADGE_PILL,
        style.fill,
        style.stroke,
        1.0,
        ppp,
    );
    let cx = snap_scalar_to_px(snapped.min.x + h_pad + icon_w * 0.5, ppp);
    let cy = snap_scalar_to_px(snapped.center().y, ppp);
    draw_status_glyph(ui.painter(), Pos2::new(cx, cy), kind, style.text);
    let label_box = Rect::from_min_max(
        Pos2::new(snapped.min.x + h_pad + icon_w + icon_gap, snapped.min.y),
        snapped.max,
    );
    let text_pos = optical_galley_pos(ui, label_box, &galley, None, false);
    ui.painter().galley(text_pos, galley, style.text);
    resp
}

fn paint_chip(ui: &mut egui::Ui, label: &str, style: ChipStyle, container_bg: Color32) -> egui::Response {
    let fill = if style.fill == Color32::TRANSPARENT {
        container_bg
    } else {
        style.fill
    };
    let font_id = FontId::new(10.5, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, style.text));
    let desired_w = (galley.size().x + 14.0).max(20.0).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, BADGE_PILL_H), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(
        ui.painter(),
        rect,
        RADIUS_BADGE_PILL,
        fill,
        style.stroke,
        1.0,
        ppp,
    );
    let text_pos = optical_galley_pos(ui, snapped, &galley, None, true);
    ui.painter().galley(text_pos, galley, style.text);
    resp
}

fn paint_zone_chip(
    ui: &mut egui::Ui,
    zone: ZoneKind,
    pal: &ThemePalette,
    container_bg: Color32,
) -> egui::Response {
    match zone {
        ZoneKind::Green => paint_status_chip(ui, StatusKind::Success, zone.short_label(), pal),
        ZoneKind::Red => paint_status_chip(ui, StatusKind::Danger, zone.short_label(), pal),
        ZoneKind::Body => paint_chip(ui, zone.short_label(), zone_chip_style(zone, pal), container_bg),
    }
}

fn zone_chip_style(zone: ZoneKind, pal: &ThemePalette) -> ChipStyle {
    match zone {
        ZoneKind::Green => status_chip_style(StatusKind::Success, pal),
        ZoneKind::Red => status_chip_style(StatusKind::Danger, pal),
        ZoneKind::Body => ChipStyle {
            fill: Color32::TRANSPARENT,
            stroke: pal.border,
            text: pal.muted,
        },
    }
}

fn qualifier_chip_style(pal: &ThemePalette) -> ChipStyle {
    ChipStyle {
        fill: pal.accent_soft_fill,
        stroke: pal.accent_border,
        text: pal.accent_pill_text,
    }
}

fn confidence_chip_style(conf: DependencyConfidence, pal: &ThemePalette) -> ChipStyle {
    match conf {
        DependencyConfidence::Proven => ChipStyle {
            fill: pal.accent_soft_fill,
            stroke: pal.accent_border,
            text: pal.accent_pill_text,
        },
        DependencyConfidence::Inferred => ChipStyle {
            fill: Color32::TRANSPARENT,
            stroke: pal.muted,
            text: pal.muted,
        },
    }
}

/// Truncates a string cleanly on a segment/word boundary ending in `"…"` so it never splits inside a filename or word at `max_w`.
fn truncate_word_boundary_px(
    ui: &egui::Ui,
    text: &str,
    font_id: &FontId,
    max_w: f32,
) -> (String, bool) {
    if max_w <= 8.0 || text.is_empty() {
        return (String::new(), !text.is_empty());
    }
    let full_w = ui.fonts(|f| {
        f.layout_no_wrap(text.to_owned(), font_id.clone(), Color32::WHITE)
            .size()
            .x
    });
    if full_w <= max_w {
        return (text.to_string(), false);
    }

    let mut best_end: Option<usize> = None;
    let mut prev_sep = true;
    for (idx, ch) in text.char_indices() {
        let is_sep = ch.is_whitespace() || matches!(ch, '\\' | '/' | '.' | ',' | ';' | ')' | '|');
        if is_sep && !prev_sep && idx > 0 {
            let trimmed = text[..idx].trim_end_matches(|c: char| {
                c.is_whitespace() || matches!(c, ',' | ';' | ':' | '+' | '(' | '[' | '-')
            });
            if !trimmed.is_empty() {
                let cand = format!("{trimmed}…");
                let w = ui.fonts(|f| {
                    f.layout_no_wrap(cand, font_id.clone(), Color32::WHITE)
                        .size()
                        .x
                });
                if w <= max_w {
                    best_end = Some(trimmed.len());
                } else {
                    break;
                }
            }
        }
        prev_sep = is_sep;
    }

    if let Some(end_idx) = best_end {
        return (format!("{}…", &text[..end_idx]), true);
    }

    // Fallback for a single unsegmented token wider than `max_w`.
    let mut char_end = 0usize;
    for (idx, ch) in text.char_indices() {
        let next_idx = idx + ch.len_utf8();
        let cand = format!("{}…", &text[..next_idx]);
        let w = ui.fonts(|f| {
            f.layout_no_wrap(cand, font_id.clone(), Color32::WHITE)
                .size()
                .x
        });
        if w <= max_w {
            char_end = next_idx;
        } else {
            break;
        }
    }
    if char_end == 0 {
        ("…".to_string(), true)
    } else {
        (format!("{}…", &text[..char_end]), true)
    }
}

fn draw_word_capped_text_cached(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
    max_w: f32,
    cache: &mut HashMap<(u64, u32, u16), (String, bool)>,
) -> egui::Response {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    font_id.family.hash(&mut hasher);
    let key = (
        hasher.finish(),
        (max_w.max(0.0) * 2.0).round() as u32,
        (font_id.size * 10.0).round() as u16,
    );
    let (display, truncated) = if let Some(hit) = cache.get(&key) {
        hit.clone()
    } else {
        if cache.len() >= 2048 {
            cache.clear();
        }
        let val = truncate_word_boundary_px(ui, text, &font_id, max_w);
        cache.insert(key, val.clone());
        val
    };
    let row_h = if ui.layout().main_dir().is_horizontal() && ui.max_rect().height() <= 32.0 {
        ui.max_rect().height()
    } else {
        0.0
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(display.clone(), font_id, color));
    let h = row_h.max(galley.size().y);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, h), Sense::hover());
    let pos = optical_galley_pos(ui, rect, &galley, None, false);
    ui.painter().galley(pos, galley, color);
    if truncated {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

#[allow(dead_code)]
fn draw_word_capped_text(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
    max_w: f32,
) -> egui::Response {
    let (display, truncated) = truncate_word_boundary_px(ui, text, &font_id, max_w);
    let row_h = if ui.layout().main_dir().is_horizontal() && ui.max_rect().height() <= 32.0 {
        ui.max_rect().height()
    } else {
        0.0
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(display, font_id, color));
    let h = row_h.max(galley.size().y);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, h), Sense::hover());
    let pos = optical_galley_pos(ui, rect, &galley, None, false);
    ui.painter().galley(pos, galley, color);
    if truncated {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

/// Renders single-line truncated text aligned to row optical center and shows a full-text tooltip on hover when truncated.
fn draw_truncated_text(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
    _strong: bool,
) -> egui::Response {
    let avail_w = ui.available_width().max(8.0);
    let (display, truncated) = truncate_word_boundary_px(ui, text, &font_id, avail_w);
    let row_h = if ui.layout().main_dir().is_horizontal() && ui.max_rect().height() <= 32.0 {
        ui.max_rect().height()
    } else {
        0.0
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(display, font_id, color));
    let h = row_h.max(galley.size().y);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, h), Sense::hover());
    let pos = optical_galley_pos(ui, rect, &galley, None, false);
    ui.painter().galley(pos, galley, color);
    if truncated {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

fn draw_baseline_paired_text(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
    anchor_font: FontId,
) -> egui::Response {
    let avail_w = ui.available_width().max(8.0);
    let (display, truncated) = truncate_word_boundary_px(ui, text, &font_id, avail_w);
    let row_h = if ui.layout().main_dir().is_horizontal() && ui.max_rect().height() <= 32.0 {
        ui.max_rect().height()
    } else {
        0.0
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(display, font_id, color));
    let h = row_h.max(galley.size().y);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, h), Sense::hover());
    let pos = baseline_aligned_galley_pos(ui, rect, &galley, anchor_font);
    ui.painter().galley(pos, galley, color);
    if truncated {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

/// Allocates one fixed-height horizontal row split into left content and a fixed right column so rows hug their content cleanly.
fn draw_split_row(
    ui: &mut egui::Ui,
    height: f32,
    right_width: f32,
    sense: Sense,
    left_fn: impl FnOnce(&mut egui::Ui),
    right_fn: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let avail_w = ui.available_width().max(right_width + 40.0);
    let (row_rect, resp) = ui.allocate_exact_size(Vec2::new(avail_w, height), sense);
    let right_rect = Rect::from_min_max(
        Pos2::new((row_rect.max.x - right_width).max(row_rect.min.x), row_rect.min.y),
        row_rect.max,
    );
    let left_rect = Rect::from_min_max(
        row_rect.min,
        Pos2::new((right_rect.min.x - SPACE_8).max(row_rect.min.x), row_rect.max.y),
    );

    let mut left_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(left_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    left_ui.set_clip_rect(
        left_rect
            .expand2(Vec2::new(1.0, 1.0))
            .intersect(ui.clip_rect()),
    );
    left_fn(&mut left_ui);

    let mut right_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(right_rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right_ui.set_clip_rect(
        right_rect
            .expand2(Vec2::new(1.0, 1.0))
            .intersect(ui.clip_rect()),
    );
    right_fn(&mut right_ui);

    resp
}

fn draw_right_aligned_size(
    ui: &mut egui::Ui,
    bytes: u64,
    size_px: f32,
    color: Color32,
    strong: bool,
) {
    let font_id = mono_font_for(ui, size_px, strong);
    let txt = format_size_2dec(bytes);
    let galley = ui.fonts(|f| f.layout_no_wrap(txt, font_id, color));
    let row_h = if ui.layout().main_dir().is_horizontal() && ui.max_rect().height() <= 32.0 {
        ui.max_rect().height().max(galley.size().y)
    } else {
        galley.size().y
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, row_h), Sense::hover());
    let pos = optical_galley_pos(ui, rect, &galley, Some(rect.max.x), false);
    ui.painter().galley(pos, galley, color);
    let _ = resp.on_hover_text(format_exact_bytes(bytes));
}

fn get_or_load_app_icon_tex(
    ctx: &egui::Context,
    cache: &mut HashMap<String, egui::TextureHandle>,
    app: &y_core::model::AttributedApp,
) -> Option<egui::TextureId> {
    if !cache.contains_key(&app.id) {
        let cached = app.icon_rgba.as_ref()?;
        let rgba = cached.decode_rgba()?;
        let img = egui::ColorImage::from_rgba_unmultiplied(
            [cached.width as usize, cached.height as usize],
            &rgba,
        );
        let handle = ctx.load_texture(
            format!("app_icon_{}", app.id),
            img,
            egui::TextureOptions::LINEAR,
        );
        cache.insert(app.id.clone(), handle);
    }
    cache.get(&app.id).map(|h| h.id())
}

/// Consistent neutral fallback tile for standalone folders, dot-folders, services, and iconless entries.
fn draw_fallback_app_icon(painter: &egui::Painter, rect: Rect, pal: &ThemePalette, ppp: f32) {
    let snapped = paint_crisp_bordered_box(
        painter,
        rect,
        RADIUS_PILL,
        pal.widget,
        pal.border,
        1.0,
        ppp,
    );
    let c = snapped.center();
    let st = Stroke::new(1.2_f32, pal.muted);
    let pts = [
        Pos2::new(c.x - 4.5, c.y - 3.5),
        Pos2::new(c.x - 1.0, c.y - 3.5),
        Pos2::new(c.x + 0.3, c.y - 2.0),
        Pos2::new(c.x + 4.5, c.y - 2.0),
        Pos2::new(c.x + 4.5, c.y + 3.5),
        Pos2::new(c.x - 4.5, c.y + 3.5),
    ];
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], st);
    }
    painter.line_segment([pts[5], pts[0]], st);
}

fn draw_chevron(ui: &mut egui::Ui, expanded: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    let c = rect.center();
    let pts = if expanded {
        vec![
            Pos2::new(c.x - 4.0, c.y - 2.0),
            Pos2::new(c.x + 4.0, c.y - 2.0),
            Pos2::new(c.x, c.y + 3.0),
        ]
    } else {
        vec![
            Pos2::new(c.x - 2.0, c.y - 4.0),
            Pos2::new(c.x + 3.0, c.y),
            Pos2::new(c.x - 2.0, c.y + 4.0),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

fn draw_theme_toggle_button(ui: &mut egui::Ui, mode: ThemeMode, pal: &ThemePalette) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HEADER_CONTROL_H), Sense::click());
    let hovered = resp.hovered();
    let pressed = resp.is_pointer_button_down_on();
    let (fill, border, fg) = if pressed {
        (pal.widget_active, pal.accent_border, pal.text)
    } else if hovered {
        (pal.widget_hover, pal.accent_border, pal.accent_line)
    } else {
        (Color32::TRANSPARENT, pal.border, pal.text)
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, 6.0, fill, border, 1.0, ppp);
    let c = snapped.center();
    match mode {
        ThemeMode::Dark => {
            // Sun glyph synced with SecondaryOutline (neutral text at rest, accent on hover)
            ui.painter()
                .circle_stroke(c, 3.2, Stroke::new(1.3_f32, fg));
            for i in 0..8 {
                let a = (i as f32) * std::f32::consts::FRAC_PI_4;
                let dir = Vec2::new(a.cos(), a.sin());
                ui.painter().line_segment(
                    [c + dir * 5.0, c + dir * 7.2],
                    Stroke::new(1.25_f32, fg),
                );
            }
        }
        ThemeMode::Light => {
            // Crescent moon glyph synced with SecondaryOutline (cutout matches canvas or interaction fill)
            let cutout = if pressed || hovered { fill } else { pal.bg };
            ui.painter()
                .circle_filled(c + Vec2::new(-0.5, 0.2), 5.3, fg);
            ui.painter()
                .circle_filled(c + Vec2::new(2.2, -1.9), 4.4, cutout);
        }
    }
    let clicked = resp.clicked();
    let tip = match mode {
        ThemeMode::Dark => "Switch to Light mode",
        ThemeMode::Light => "Switch to Dark mode",
    };
    let _ = resp.on_hover_text(tip);
    clicked
}

fn draw_sort_arrow(painter: &egui::Painter, center: Pos2, desc: bool, color: Color32) {
    let st = Stroke::new(1.4_f32, color);
    let top = Pos2::new(center.x, center.y - 4.0);
    let bot = Pos2::new(center.x, center.y + 4.0);
    painter.line_segment([top, bot], st);
    if desc {
        painter.line_segment([bot, Pos2::new(center.x - 3.0, center.y + 1.0)], st);
        painter.line_segment([bot, Pos2::new(center.x + 3.0, center.y + 1.0)], st);
    } else {
        painter.line_segment([top, Pos2::new(center.x - 3.0, center.y - 1.0)], st);
        painter.line_segment([top, Pos2::new(center.x + 3.0, center.y - 1.0)], st);
    }
}

#[derive(Debug, Clone, Copy)]
struct ColSpan {
    min_x: f32,
    max_x: f32,
}

impl ColSpan {
    fn new(min_x: f32, width: f32) -> Self {
        Self {
            min_x,
            max_x: min_x + width.max(0.0),
        }
    }

    fn rect(self, y0: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(self.min_x, y0), Pos2::new(self.max_x, y1))
    }
}

struct MainTableColumns {
    chevron: ColSpan,
    name: ColSpan,
    publisher: ColSpan,
    category: ColSpan,
    delta: ColSpan,
    size: ColSpan,
}

impl MainTableColumns {
    fn compute(origin_x: f32, row_w: f32, per_drive_mode: bool) -> Self {
        let gap = SPACE_12;
        let chev_w = 16.0;
        let chev_gap = SPACE_8;
        let pub_w = 148.0;
        let cat_w = 122.0;
        let delta_w = DELTA_COL_W;
        let size_w = if per_drive_mode {
            PER_DRIVE_SIZE_COL_W
        } else {
            SIZE_COL_W
        };
        let fixed_w = chev_w + chev_gap + pub_w + cat_w + delta_w + size_w + gap * 4.0;
        let name_w = (row_w - fixed_w).max(120.0);

        let chevron = ColSpan::new(origin_x, chev_w);
        let name = ColSpan::new(chevron.max_x + chev_gap, name_w);
        let publisher = ColSpan::new(name.max_x + gap, pub_w);
        let category = ColSpan::new(publisher.max_x + gap, cat_w);
        let delta = ColSpan::new(category.max_x + gap, delta_w);
        let size = ColSpan::new(delta.max_x + gap, (origin_x + row_w - (delta.max_x + gap)).max(size_w));
        Self {
            chevron,
            name,
            publisher,
            category,
            delta,
            size,
        }
    }
}

struct LocationsTableColumns {
    zone: ColSpan,
    path: ColSpan,
    basis: ColSpan,
    actions: ColSpan,
    size: ColSpan,
}

impl LocationsTableColumns {
    fn compute(origin_x: f32, sub_w: f32) -> Self {
        let gap = SPACE_12;
        let zone_w = 78.0;
        let basis_w = 138.0;
        let actions_w = 144.0;
        let size_w = SIZE_COL_W;
        let fixed_w = zone_w + basis_w + actions_w + size_w + gap * 3.0 + SPACE_8;
        let path_w = (sub_w - fixed_w).max(120.0);

        let zone = ColSpan::new(origin_x, zone_w);
        let path = ColSpan::new(zone.max_x + gap, path_w);
        let basis = ColSpan::new(path.max_x + gap, basis_w);
        let actions = ColSpan::new(basis.max_x + gap, actions_w);
        let size = ColSpan::new(actions.max_x + SPACE_8, (origin_x + sub_w - (actions.max_x + SPACE_8)).max(size_w));
        Self {
            zone,
            path,
            basis,
            actions,
            size,
        }
    }
}

struct DependenciesTableColumns {
    name: ColSpan,
    kind: ColSpan,
    confidence: ColSpan,
    used_by: ColSpan,
    own_size: ColSpan,
}

impl DependenciesTableColumns {
    fn compute(origin_x: f32, sub_w: f32) -> Self {
        let gap = SPACE_12;
        let kind_w = 148.0;
        let conf_w = 156.0;
        let used_w = 112.0;
        let size_w = SIZE_COL_W;
        let fixed_w = kind_w + conf_w + used_w + size_w + gap * 4.0;
        let name_w = (sub_w - fixed_w).max(120.0);

        let name = ColSpan::new(origin_x, name_w);
        let kind = ColSpan::new(name.max_x + gap, kind_w);
        let confidence = ColSpan::new(kind.max_x + gap, conf_w);
        let used_by = ColSpan::new(confidence.max_x + gap, used_w);
        let own_size = ColSpan::new(used_by.max_x + gap, (origin_x + sub_w - (used_by.max_x + gap)).max(size_w));
        Self {
            name,
            kind,
            confidence,
            used_by,
            own_size,
        }
    }
}

fn table_cell_ui(ui: &mut egui::Ui, cell_rect: Rect, layout: Layout) -> egui::Ui {
    let mut cell_ui = ui.new_child(UiBuilder::new().max_rect(cell_rect).layout(layout));
    cell_ui.set_clip_rect(
        cell_rect
            .expand2(Vec2::new(0.0, 1.0))
            .intersect(ui.clip_rect()),
    );
    cell_ui
}

pub fn ledger_page_bounds(
    total_items: usize,
    page_idx: usize,
    page_size: usize,
) -> (usize, usize, usize, usize) {
    let ps = page_size.max(1);
    let total_pages = total_items.div_ceil(ps).max(1);
    let clamped_page = page_idx.min(total_pages - 1);
    let start = (clamped_page * ps).min(total_items);
    let end = (start + ps).min(total_items);
    (clamped_page, total_pages, start, end)
}

fn draw_sortable_col_header(
    ui: &mut egui::Ui,
    cell_rect: Rect,
    id_salt: &str,
    label: &str,
    mode: AppSortMode,
    active_mode: AppSortMode,
    active_dir: SortDirection,
    right_aligned: bool,
    pal: &ThemePalette,
) -> bool {
    let is_active = mode == active_mode;
    let id = ui.make_persistent_id(("table_hdr", id_salt));
    let resp = ui.interact(cell_rect, id, Sense::click());
    let hovered = resp.hovered();
    let ppp = ui.ctx().pixels_per_point();

    if hovered {
        let mut hover_rect = snap_rect_to_pixels(cell_rect, ppp);
        hover_rect.min.y = hover_rect.min.y.max(cell_rect.min.y);
        hover_rect.max.y = hover_rect.max.y.min(cell_rect.max.y);
        paint_crisp_bordered_box(
            ui.painter(),
            hover_rect,
            RADIUS_TABLE_CELL,
            pal.widget_hover,
            Color32::TRANSPARENT,
            0.0,
            ppp,
        );
    }

    let fg = if resp.is_pointer_button_down_on() {
        pal.accent_line
    } else if is_active || hovered {
        pal.text
    } else {
        pal.muted
    };

    let font_id = FontId::new(11.0, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, fg));
    let arrow_w = if is_active { 12.0 } else { 0.0 };
    let total_w = galley.size().x + arrow_w;
    let start_x = if right_aligned {
        (cell_rect.max.x - total_w).max(cell_rect.min.x)
    } else {
        cell_rect.min.x
    };
    let align_rect = Rect::from_min_max(Pos2::new(start_x, cell_rect.min.y), cell_rect.max);
    let text_pos = optical_galley_pos(ui, align_rect, &galley, None, false);
    ui.painter().galley(text_pos, galley, fg);
    if is_active {
        let arrow_x = snap_scalar_to_px(
            (start_x + total_w - 4.5).clamp(cell_rect.min.x + 4.5, cell_rect.max.x - 4.5),
            ppp,
        );
        let arrow_c = Pos2::new(arrow_x, snap_scalar_to_px(cell_rect.center().y, ppp));
        draw_sort_arrow(ui.painter(), arrow_c, active_dir == SortDirection::Desc, pal.accent_line);
    }

    let tip = match mode {
        AppSortMode::Size => "Sort by total storage footprint (click active column to flip desc/asc)",
        AppSortMode::Name => "Sort alphabetically by app name (click active column to flip A-Z / Z-A)",
        AppSortMode::Publisher => {
            "Sort alphabetically by publisher (click active column to flip A-Z / Z-A; empty publishers grouped separately)"
        }
        AppSortMode::Category => {
            "Sort by app category (click active column to flip asc/desc; ties ordered by size)"
        }
        AppSortMode::Growth => {
            "Sort by growth delta vs previous scan (click active column to flip desc/asc; no-baseline apps follow at bottom)"
        }
    };
    let clicked = resp.clicked();
    let _ = resp.on_hover_text(tip);
    clicked
}

fn draw_static_col_header(
    ui: &mut egui::Ui,
    cell_rect: Rect,
    label: &str,
    font_size: f32,
    right_aligned: bool,
    pal: &ThemePalette,
) {
    let font_id = FontId::new(font_size, FontFamily::Proportional);
    let fg = pal.subtle_text;
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, fg));
    let text_pos = optical_galley_pos(
        ui,
        cell_rect,
        &galley,
        if right_aligned { Some(cell_rect.max.x) } else { None },
        false,
    );
    ui.painter().galley(text_pos, galley, fg);
}

/// Facet filter dropdown trigger: strictly neutral (`SecondaryOutline`) at rest, accent ONLY when open, active, or hovered.
fn draw_facet_selector_button(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    label: &str,
    is_open: bool,
    is_non_default: bool,
    pal: &ThemePalette,
) -> egui::Response {
    const BTN_H: f32 = 24.0;
    let font_id = FontId::new(11.5, FontFamily::Proportional);
    let measure_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id.clone(), pal.text));
    let chevron_w = 10.0;
    let desired_w = (measure_galley.size().x + SPACE_8 * 2.0 + 4.0 + chevron_w).round();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(desired_w, BTN_H), Sense::hover());
    let resp = ui.interact(rect, ui.make_persistent_id(("facet_trigger", id_salt)), Sense::click());
    let (fill, border_col, text_color) = resolve_button_colors(
        ButtonVariant::SecondaryOutline,
        true,
        resp.is_pointer_button_down_on(),
        resp.hovered(),
        is_open || is_non_default,
        pal,
    );
    let ppp = ui.ctx().pixels_per_point();
    let snapped =
        paint_crisp_bordered_box(ui.painter(), rect, RADIUS_CONTROL, fill, border_col, 1.0, ppp);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let label_box = Rect::from_min_max(
        Pos2::new(snapped.min.x + SPACE_8, snapped.min.y),
        Pos2::new((snapped.max.x - chevron_w - SPACE_8).max(snapped.min.x + SPACE_8), snapped.max.y),
    );
    let text_pos = optical_galley_pos(ui, label_box, &galley, None, false);
    ui.painter().galley(text_pos, galley, text_color);

    let cx = snap_scalar_to_px(snapped.max.x - SPACE_8 - 3.5, ppp);
    let cy = snap_scalar_to_px(snapped.center().y, ppp);
    let st = Stroke::new(1.3_f32, text_color);
    ui.painter()
        .line_segment([Pos2::new(cx - 3.0, cy - 1.5), Pos2::new(cx, cy + 2.0)], st);
    ui.painter()
        .line_segment([Pos2::new(cx, cy + 2.0), Pos2::new(cx + 3.0, cy - 1.5)], st);
    resp
}

fn draw_facet_popup_row(
    ui: &mut egui::Ui,
    row_w: f32,
    left_label: &str,
    right_meta: Option<&str>,
    selected: bool,
    pal: &ThemePalette,
) -> bool {
    const ROW_H: f32 = 24.0;
    let slot = ui.painter().add(egui::Shape::Noop);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(row_w, ROW_H), Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.painter()
            .set(slot, egui::Shape::rect_filled(rect, Rounding::same(RADIUS_PILL), pal.widget_hover));
    } else if selected {
        ui.painter()
            .set(slot, egui::Shape::rect_filled(rect, Rounding::same(RADIUS_PILL), pal.accent_soft_fill));
    }

    let fg = if selected {
        pal.accent_line
    } else if hovered {
        pal.text
    } else {
        pal.text
    };
    let ppp = ui.ctx().pixels_per_point();
    let pad_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + SPACE_8, rect.min.y),
        Pos2::new((rect.max.x - SPACE_8).max(rect.min.x + SPACE_8), rect.max.y),
    );
    let meta_w = if let Some(meta) = right_meta {
        let mg = ui.fonts(|f| {
            f.layout_no_wrap(
                meta.to_owned(),
                FontId::new(10.5, FontFamily::Monospace),
                pal.muted,
            )
        });
        let mp = optical_galley_pos(ui, pad_rect, &mg, Some(pad_rect.max.x), false);
        let w = mg.size().x + SPACE_8;
        ui.painter().galley(mp, mg, pal.muted);
        w
    } else {
        0.0
    };

    let avail_lbl_w = (pad_rect.width() - meta_w).max(40.0);
    let lbl_font = FontId::new(11.5, FontFamily::Proportional);
    let (display_lbl, _) = truncate_word_boundary_px(ui, left_label, &lbl_font, avail_lbl_w);
    let lg = ui.fonts(|f| f.layout_no_wrap(display_lbl, lbl_font, fg));
    let lp = optical_galley_pos(ui, pad_rect, &lg, None, false);
    ui.painter().galley(lp, lg, fg);
    let _ = snap_scalar_to_px(rect.min.y, ppp);
    resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn draw_facet_popup_overlay(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    facet_key: &'static str,
    trigger_rect: Rect,
    pal: &ThemePalette,
    emit: &mut dyn FnMut(UiAction),
) {
    let screen = ctx.screen_rect();
    let popup_w = match facet_key {
        "publisher" => 360.0_f32,
        "category" => 260.0_f32,
        "deps" | "zone" => 256.0_f32,
        _ => 220.0_f32,
    };
    let pos_x = trigger_rect
        .left()
        .clamp(screen.left() + SPACE_16, (screen.right() - SPACE_16 - popup_w).max(screen.left() + SPACE_16));
    let pos_y = (trigger_rect.bottom() + SPACE_4).min((screen.bottom() - 120.0).max(screen.top() + SPACE_16));

    let area_resp = egui::Area::new(egui::Id::new(("facet_popup_overlay", facet_key)))
        .order(Order::Foreground)
        .fixed_pos(Pos2::new(pos_x, pos_y))
        .show(ctx, |ui| {
            draw_crisp_frame(
                ui,
                pal.panel,
                pal.accent_line,
                RADIUS_PANEL,
                Margin::same(SPACE_8),
                |ui| {
                    let inner_w = popup_w - SPACE_16 - 2.0;
                    ui.set_width(inner_w);
                    let mut picked = false;
                    draw_synced_scroll_area(
                        ui,
                        ("facet_popup_scroll", facet_key),
                        Some(260.0),
                        [false, true],
                        pal,
                        pal.panel,
                        |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let row_w = ui.available_width().max(120.0);
                            match facet_key {
                                "publisher" => {
                                    let is_all = ui_state.facets.publisher.is_none();
                                    if draw_facet_popup_row(ui, row_w, "All publishers", None, is_all, pal) {
                                        ui_state.facets.publisher = None;
                                        picked = true;
                                    }
                                    for (key, lbl, cnt, bytes) in publisher_facet_options(&rep.apps) {
                                        let sel = ui_state
                                            .facets
                                            .publisher
                                            .as_deref()
                                            .is_some_and(|p| p.eq_ignore_ascii_case(&key));
                                        let meta = format!("{cnt} · {}", format_size_2dec(bytes));
                                        if draw_facet_popup_row(ui, row_w, &lbl, Some(&meta), sel, pal) {
                                            ui_state.facets.publisher = Some(key);
                                            picked = true;
                                        }
                                    }
                                }
                                "category" => {
                                    let is_all = ui_state.facets.category.is_none();
                                    if draw_facet_popup_row(ui, row_w, "All categories", None, is_all, pal) {
                                        ui_state.facets.category = None;
                                        picked = true;
                                    }
                                    for (cat, cnt) in category_facet_options(&rep.apps) {
                                        let sel = ui_state.facets.category == Some(cat);
                                        let meta = format!("{cnt}");
                                        if draw_facet_popup_row(ui, row_w, cat.label(), Some(&meta), sel, pal) {
                                            ui_state.facets.category = Some(cat);
                                            picked = true;
                                        }
                                    }
                                }
                                "drive" => {
                                    let is_all = ui_state.facets.drive.is_none();
                                    if draw_facet_popup_row(ui, row_w, "All drives", None, is_all, pal) {
                                        ui_state.facets.drive = None;
                                        picked = true;
                                    }
                                    for drv in &rep.drives {
                                        let dl = drv.letter.to_ascii_uppercase();
                                        let sel = ui_state
                                            .facets
                                            .drive
                                            .is_some_and(|d| d.to_ascii_uppercase() == dl);
                                        let lbl = format!("{dl}:\\");
                                        let (cnt, sum_b): (usize, u64) = rep
                                            .apps
                                            .iter()
                                            .map(|a| a.bytes_on_drive(dl))
                                            .filter(|&b| b > 0)
                                            .fold((0, 0), |(c, s), b| (c + 1, s.saturating_add(b)));
                                        let meta = if !drv.full_walk_enabled {
                                            "Not scanned".to_string()
                                        } else {
                                            format!("{cnt} · {}", format_size_2dec(sum_b))
                                        };
                                        if draw_facet_popup_row(ui, row_w, &lbl, Some(&meta), sel, pal) {
                                            ui_state.facets.drive = Some(dl);
                                            picked = true;
                                        }
                                    }
                                }
                                "size" => {
                                    for band in SizeBandFilter::ALL_OPTIONS {
                                        let sel = ui_state.facets.size_band == band;
                                        if draw_facet_popup_row(ui, row_w, band.label(), None, sel, pal) {
                                            ui_state.facets.size_band = band;
                                            picked = true;
                                        }
                                    }
                                }
                                "deps" => {
                                    for ds in DepStateFilter::ALL_OPTIONS {
                                        let sel = ui_state.facets.dep_state == ds;
                                        if draw_facet_popup_row(ui, row_w, ds.label(), None, sel, pal) {
                                            ui_state.facets.dep_state = ds;
                                            picked = true;
                                        }
                                    }
                                }
                                "zone" => {
                                    for fz in FootprintZoneFilter::ALL_OPTIONS {
                                        let sel = ui_state.facets.footprint_zone == fz;
                                        if draw_facet_popup_row(ui, row_w, fz.label(), None, sel, pal) {
                                            ui_state.facets.footprint_zone = fz;
                                            picked = true;
                                        }
                                    }
                                }
                                "growth" => {
                                    for gf in GrowthFacetFilter::ALL_OPTIONS {
                                        let sel = ui_state.facets.growth == gf;
                                        if draw_facet_popup_row(ui, row_w, gf.label(), None, sel, pal) {
                                            ui_state.facets.growth = gf;
                                            picked = true;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        },
                    );
                    picked
                },
            )
            .inner
        });

    if area_resp.inner {
        ui_state.open_facet_popup = None;
        ui_state.ledger_page = 0;
        ui_state.ledger_scroll_to_top = true;
        emit(UiAction::UpdateListView {
            sort_mode: ui_state.sort_mode,
            sort_direction: ui_state.sort_direction,
            filter_chip: ui_state.filter_chip,
            search_query: ui_state.search_query.clone(),
            facets: ui_state.facets.clone(),
        });
    } else if ctx.input(|i| i.pointer.primary_clicked()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !area_resp.response.rect.contains(pos) && !trigger_rect.contains(pos) {
                ui_state.open_facet_popup = None;
            }
        }
    }
}

fn draw_filter_radio_chip(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    pal: &ThemePalette,
) -> bool {
    let font_id = FontId::new(11.5, FontFamily::Proportional);
    let text_color = if selected { pal.text } else { pal.muted };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let desired_w = (galley.size().x + SPACE_16).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, 22.0), Sense::click());
    let (fill, border_col, draw_fg) = if resp.is_pointer_button_down_on() {
        (pal.widget_active, pal.accent_border, pal.text)
    } else if resp.hovered() {
        (
            pal.widget_hover,
            pal.accent_border,
            if selected { pal.text } else { pal.accent_line },
        )
    } else if selected {
        (pal.accent_soft_fill, pal.accent_border, pal.text)
    } else {
        (pal.widget, pal.border, pal.muted)
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
    let text_pos = optical_galley_pos(ui, snapped, &galley, None, true);
    ui.painter().galley(text_pos, galley, draw_fg);
    resp.clicked()
}

fn ensure_report_and_filter_caches(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    pal: &ThemePalette,
    sys_drive_letter: char,
    theme_key: u32,
) {
    let ppp = ctx.pixels_per_point();
    if ui_state.cached_measure_ppp != Some(ppp) {
        ui_state.truncate_cache.clear();
        if let Some(cr) = ui_state.cached_report.as_mut() {
            for a in &mut cr.apps {
                a.name_w_px = None;
                a.pub_w_px = None;
            }
        }
        ui_state.cached_measure_ppp = Some(ppp);
    }

    if ui_state.icon_cache_scan_ts != Some(rep.scan_timestamp_unix) {
        ui_state.icon_textures.clear();
        ui_state.icon_cache_scan_ts = Some(rep.scan_timestamp_unix);
        ui_state.expanded_row_heights.clear();
        ui_state.expanded_family_heights.clear();
        ui_state.truncate_cache.clear();
        ui_state.unattrib_exists_cache.clear();
    }

    let report_stale = ui_state.cached_report.as_ref().is_none_or(|c| {
        c.scan_ts != rep.scan_timestamp_unix
            || c.theme_key != theme_key
            || c.apps_count != rep.apps.len()
    });
    if report_stale {
        let mut max_unfiltered_bytes = 0u64;
        let mut total_attributed_bytes = 0u64;
        let mut apps = Vec::with_capacity(rep.apps.len());

        for app in &rep.apps {
            max_unfiltered_bytes = max_unfiltered_bytes.max(app.total_bytes);
            total_attributed_bytes += app.total_bytes;
            apps.push(CachedAppMeta {
                total_str: format_size_2dec(app.total_bytes),
                total_tip: format_exact_bytes(app.total_bytes),
                delta_display: format_delta_display(app.growth, pal),
                name_w_px: None,
                pub_w_px: None,
            });
        }
        for (a, app) in apps.iter_mut().zip(rep.apps.iter()) {
            let share_pct = if total_attributed_bytes > 0 {
                (app.total_bytes as f64 / total_attributed_bytes as f64) * 100.0
            } else {
                0.0
            };
            a.total_tip = format!("{} · {:.2}% share", a.total_tip, share_pct);
        }

        let rev_counts: HashMap<String, usize> = rep
            .reverse_dependencies
            .iter()
            .map(|r| (r.target_id.clone(), r.dependent_count))
            .collect();
        let mut rev_by_app_token: HashMap<String, (String, usize)> =
            HashMap::with_capacity(rep.reverse_dependencies.len());
        for r in &rep.reverse_dependencies {
            if r.dependent_count > 0 {
                let tok = normalize_app_token(&r.target_name);
                if !tok.is_empty() {
                    rev_by_app_token
                        .entry(tok)
                        .or_insert_with(|| (r.target_id.clone(), r.dependent_count));
                }
            }
        }
        let catalog = y_core::dependencies::DependencyCatalog::load_from_disk_or_embedded();
        let mut rev_by_app_id: HashMap<String, (Option<String>, usize)> = HashMap::new();
        for app in &rep.apps {
            let dn = app.display_name.to_ascii_lowercase();
            let tok = normalize_app_token(&app.display_name);
            let mut matched_target: Option<(&y_core::dependencies::CatalogTarget, usize)> = None;
            for target in &catalog.targets {
                let hits = target.registry_matches.iter().any(|m| {
                    y_core::dependencies::contains_token_boundary(&dn, &m.to_ascii_lowercase())
                });
                if hits {
                    let cnt = rev_counts.get(&target.id).copied().unwrap_or(0);
                    if matched_target.is_none_or(|(prev_t, prev_c)| {
                        cnt > prev_c
                            || (cnt == prev_c && prev_t.id == "dotnet-runtime" && target.id != "dotnet-runtime")
                    }) {
                        matched_target = Some((target, cnt));
                    }
                }
            }
            if let Some((t, cnt)) = matched_target {
                rev_by_app_id.insert(app.id.clone(), (Some(t.id.clone()), cnt));
            } else if let Some((tid, cnt)) = rev_by_app_token.get(&tok) {
                rev_by_app_id.insert(app.id.clone(), (Some(tid.clone()), *cnt));
            } else if app.no_touch == Some(y_core::model::NoTouchCategory::SystemRuntime) {
                rev_by_app_id.insert(app.id.clone(), (None, 0));
            }
        }
        let shared_app_ids: HashSet<String> = rev_by_app_id
            .iter()
            .filter(|(_, (_, cnt))| *cnt > 0)
            .map(|(k, _)| k.clone())
            .collect();
        let has_growth = has_growth_baseline(rep);
        let sys_reconcile = rep
            .windows_system
            .iter()
            .map(|sys| {
                let apps_d = rep.total_apps_bytes_on_drive(sys.drive_letter);
                let cache_d = rep.total_shared_caches_on_drive(sys.drive_letter);
                let yf_d = rep.total_your_files_on_drive(sys.drive_letter);
                let un_d = rep.total_unattributed_on_drive(sys.drive_letter);
                let sum_all = (apps_d + cache_d + yf_d + un_d + sys.total_bytes + sys.drive_free_bytes) as i128
                    - sys.hardlink_overlap_bytes as i128;
                let diff = sum_all - sys.drive_total_bytes as i128;
                let hl_part = if sys.hardlink_overlap_bytes > 0 {
                    format!(" - Hardlinks {}", format_size_2dec(sys.hardlink_overlap_bytes))
                } else {
                    String::new()
                };
                let summary = format!(
                    "{}:\\ reconciliation · Apps {} + Caches {} + Your files {} + Unattributed {} + Win/sys {}{} + Free {} = {}",
                    sys.drive_letter,
                    format_size_2dec(apps_d),
                    format_size_2dec(cache_d),
                    format_size_2dec(yf_d),
                    format_size_2dec(un_d),
                    format_size_2dec(sys.total_bytes),
                    hl_part,
                    format_size_2dec(sys.drive_free_bytes),
                    format_size_2dec(sys.drive_total_bytes),
                );
                let exact = format!(
                    "{}:\\ exact: {} + {} + {} + {} + {}{} + {} = {} (diff {} B)",
                    sys.drive_letter,
                    format_exact_bytes(apps_d),
                    format_exact_bytes(cache_d),
                    format_exact_bytes(yf_d),
                    format_exact_bytes(un_d),
                    format_exact_bytes(sys.total_bytes),
                    if sys.hardlink_overlap_bytes > 0 {
                        format!(" - {} hardlinks", format_exact_bytes(sys.hardlink_overlap_bytes))
                    } else {
                        String::new()
                    },
                    format_exact_bytes(sys.drive_free_bytes),
                    format_exact_bytes(sys.drive_total_bytes),
                    diff,
                );
                (sys.drive_letter, summary, exact, diff)
            })
            .collect();

        let ledger_entries = derive_app_families(&rep.apps);
        let max_entry_bytes = ledger_entries
            .iter()
            .map(|e| e.effective_bytes(&rep.apps, None))
            .max()
            .unwrap_or(max_unfiltered_bytes);

        ui_state.cached_report = Some(CachedReportData {
            scan_ts: rep.scan_timestamp_unix,
            theme_key,
            apps_count: rep.apps.len(),
            apps,
            ledger_entries,
            max_unfiltered_bytes: max_entry_bytes,
            total_attributed_bytes,
            attributed_label: format!("{} attributed", format_size_2dec(total_attributed_bytes)),
            attributed_tip: format!(
                "Total attributed app bytes across all drives: {}",
                format_exact_bytes(total_attributed_bytes)
            ),
            rev_counts,
            rev_by_app_token,
            rev_by_app_id,
            shared_app_ids,
            has_growth_baseline: has_growth,
            sys_reconcile,
        });
    }

    sanitize_facets_for_report(&mut ui_state.facets, rep);

    let filter_stale = ui_state.cached_filtered.as_ref().is_none_or(|f| {
        f.scan_ts != rep.scan_timestamp_unix
            || f.sort_mode != ui_state.sort_mode
            || f.sort_direction != ui_state.sort_direction
            || f.filter_chip != ui_state.filter_chip
            || f.search_query != ui_state.search_query
            || f.facets != ui_state.facets
            || f.sys_drive != sys_drive_letter
    });
    if filter_stale {
        if ui_state.cached_filtered.is_some() {
            ui_state.ledger_page = 0;
            ui_state.ledger_scroll_to_top = true;
        }
        let cr = ui_state.cached_report.as_ref().unwrap();
        let visible_entries = filter_and_sort_ledger_entries(
            &cr.ledger_entries,
            &rep.apps,
            ui_state.sort_mode,
            ui_state.sort_direction,
            ui_state.filter_chip,
            &ui_state.search_query,
            &ui_state.facets,
            &cr.shared_app_ids,
            sys_drive_letter,
        );
        let mut seen_app = vec![false; rep.apps.len()];
        let mut visible_indices = Vec::new();
        for &ei in &visible_entries {
            match &cr.ledger_entries[ei] {
                LedgerEntry::Unit(idx) => {
                    if !seen_app[*idx] {
                        seen_app[*idx] = true;
                        visible_indices.push(*idx);
                    }
                }
                LedgerEntry::Family(fam) => {
                    for &m_idx in &fam.member_indices {
                        if !seen_app[m_idx] {
                            seen_app[m_idx] = true;
                            visible_indices.push(m_idx);
                        }
                    }
                }
            }
        }
        ui_state.cached_filtered = Some(CachedFilteredList {
            scan_ts: rep.scan_timestamp_unix,
            sort_mode: ui_state.sort_mode,
            sort_direction: ui_state.sort_direction,
            filter_chip: ui_state.filter_chip,
            search_query: ui_state.search_query.clone(),
            facets: ui_state.facets.clone(),
            sys_drive: sys_drive_letter,
            visible_entries,
            visible_indices,
        });
    }
}

fn app_has_shared_runtime_banner(app: &y_core::model::AttributedApp) -> bool {
    let dn = app.display_name.to_ascii_lowercase();
    app.no_touch == Some(y_core::model::NoTouchCategory::SystemRuntime)
        || dn.contains("visual c++")
        || dn.contains("windows desktop runtime")
        || dn.contains(".net runtime")
        || dn.contains(".net sdk")
        || dn.contains("webview2")
        || dn.contains("openal")
        || dn.contains("physx")
        || dn.contains("python 3.")
}

fn estimate_expanded_row_height(app: &y_core::model::AttributedApp) -> f32 {
    let n_locs = app.items.len();
    let locs_h = (n_locs as f32) * (SUB_LINE1_H + 2.0) + (n_locs.saturating_sub(1) as f32) * 1.0;
    let deps_h = if app.dependencies.is_empty() {
        SUB_LINE1_H
    } else {
        let n_deps = app.dependencies.len();
        SUB_HEADER_H
            + (n_deps as f32) * ((SUB_LINE1_H + 2.0) + (SUB_LINE2_H + 2.0))
            + (n_deps.saturating_sub(1) as f32) * 2.0
    };
    let shared_banner_h = if app_has_shared_runtime_banner(app) {
        22.0 + SPACE_4
    } else {
        0.0
    };
    let carved_h = if app.carved_out_apps.is_empty() {
        0.0
    } else {
        (app.carved_out_apps.len() as f32) * (18.0 + SPACE_4)
    };
    3.0 + COLLAPSED_ROW_OUTER_H
        + SPACE_4
        + 18.0
        + carved_h
        + shared_banner_h
        + SPACE_4
        + 18.0
        + SUB_HEADER_H
        + locs_h
        + (SPACE_8 * 2.0)
        + 18.0
        + deps_h
        + (SPACE_8 + 3.0)
}

fn format_pod_short_time(last_scanned_display: Option<&str>, _compact: bool) -> String {
    let Some(ts) = last_scanned_display.map(str::trim).filter(|s| !s.is_empty()) else {
        return "Never scanned".into();
    };
    let hhmm = ts
        .split_whitespace()
        .nth(1)
        .and_then(|t| t.get(..5))
        .unwrap_or(ts);
    format!("Scanned {hhmm}")
}

fn draw_compact_pod_checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    label: &str,
    font_size: f32,
    row_h: f32,
    muted_look: bool,
    hover_tip: &str,
    pal: &ThemePalette,
) -> bool {
    let font_id = FontId::new(font_size, FontFamily::Proportional);
    let measure_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id.clone(), pal.text));
    let total_w = (14.0 + SPACE_4 + measure_galley.size().x).ceil();
    let alloc_h = row_h.max(16.0);
    let (hit_rect, resp) = ui.allocate_exact_size(Vec2::new(total_w, alloc_h), Sense::click());
    let ppp = ui.ctx().pixels_per_point();
    let (fill, border_col, check_col) =
        resolve_checkbox_colors(*checked, resp.hovered(), pal);
    let box_rect = Rect::from_center_size(
        Pos2::new(hit_rect.min.x + 7.0, hit_rect.center().y),
        Vec2::splat(14.0),
    );
    let snapped =
        paint_crisp_bordered_box(ui.painter(), box_rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
    if *checked {
        let c = snapped.center();
        let st = Stroke::new(1.6_f32, check_col);
        ui.painter().add(egui::Shape::line(
            vec![
                Pos2::new(c.x - 3.5, c.y + 0.5),
                Pos2::new(c.x - 0.8, c.y + 3.0),
                Pos2::new(c.x + 3.8, c.y - 2.6),
            ],
            st,
        ));
    }
    let lbl_col = if resp.hovered() {
        pal.accent_line
    } else if muted_look {
        pal.muted
    } else {
        pal.text
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, lbl_col));
    let label_rect = Rect::from_min_max(
        Pos2::new(hit_rect.min.x + 14.0 + SPACE_4, hit_rect.min.y),
        hit_rect.max,
    );
    let text_pos = optical_galley_pos(ui, label_rect, &galley, None, false);
    ui.painter().galley(text_pos, galley, lbl_col);
    let clicked = resp.clicked();
    if !hover_tip.is_empty() {
        let _ = resp.on_hover_text(hover_tip);
    }
    if clicked {
        *checked = !*checked;
        true
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_drive_pods_grid(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    config: &AppConfig,
    rep: &ScanReport,
    scanning: bool,
    rescanning_drives: &BTreeSet<char>,
    pal: &ThemePalette,
    emit: &mut dyn FnMut(UiAction),
) -> Option<(char, Rect)> {
    if rep.drives.is_empty() {
        return None;
    }
    const MIN_POD_W: f32 = 180.0;
    let n = rep.drives.len();
    let avail_w = ui.available_width().max(MIN_POD_W);
    let fit_cols = ((avail_w + SPACE_8) / (MIN_POD_W + SPACE_8))
        .floor()
        .max(1.0) as usize;
    // DESIGN.md §11: 1..=4 in 1 row, 5..=8 in 2 balanced rows (max_cols = 4 -> 3+2 on 5), 9+ in 5-col wrap.
    let max_cols = if n <= 4 {
        fit_cols.min(4)
    } else if n <= 8 {
        fit_cols.min(4)
    } else {
        fit_cols.min(5)
    }
    .max(1);
    let num_rows = n.div_ceil(max_cols);
    let base_per_row = n / num_rows;
    let extra = n % num_rows;

    let ppp = ui.ctx().pixels_per_point();
    let mut overlay_anchor: Option<(char, Rect)> = None;
    let mut drv_offset = 0usize;

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = SPACE_8;
        for row_idx in 0..num_rows {
            let cols_in_row = base_per_row + usize::from(row_idx < extra);
            let pod_col_gap = if cols_in_row >= 5 { SPACE_4 } else { SPACE_8 };
            let tile_w =
                ((avail_w - pod_col_gap * ((cols_in_row - 1) as f32)) / (cols_in_row as f32))
                    .max(140.0);
            let compact = tile_w < 220.0;
            let pod_h_margin = if compact { SPACE_8 } else { SPACE_12 };

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = pod_col_gap;
                for local_col in 0..cols_in_row {
                    let drv_idx = drv_offset + local_col;
                    let drv = &rep.drives[drv_idx];
                    let dl = drv.letter.to_ascii_uppercase();
                    let mut included = config.is_drive_enabled(dl);
                    let busy_this_drive =
                        scanning || (included && rescanning_drives.contains(&dl));
                    let exclusions = config.exclusions_for_drive(dl);
                    let excl_count = exclusions.len();
                    let ratio = if drv.total_bytes > 0 {
                        (drv.used_bytes as f64 / drv.total_bytes as f64).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let pod_fill = if included {
                        pal.panel
                    } else {
                        lerp_color(pal.bg, pal.panel, 0.55)
                    };
                    let pod_border = if ui_state.open_pod_scope_drive == Some(dl) {
                        pal.accent_line
                    } else if included {
                        pal.border
                    } else {
                        lerp_color(pal.bg, pal.border, 0.65)
                    };
                    let primary_col = if included { pal.text } else { pal.muted };
                    let cap_font_sz = if compact { 10.5 } else { 11.0 };

                    let pod_frame = draw_crisp_frame(
                        ui,
                        pod_fill,
                        pod_border,
                        RADIUS_PANEL,
                        Margin::symmetric(pod_h_margin, SPACE_8),
                        |ui| {
                            let inner_w = (tile_w - pod_h_margin * 2.0 - 2.0).max(112.0);
                            ui.set_width(inner_w);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;

                                // Row 1: Drive identity ("C:\") on left, "<used> of <total>" on right
                                let (row1_rect, row1_resp) =
                                    ui.allocate_exact_size(Vec2::new(inner_w, 18.0), Sense::hover());
                                let (drv_font, _) = prop_strong_font(ui_state.has_prop_strong, 13.0);
                                let drv_galley = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        format!("{dl}:\\"),
                                        drv_font,
                                        primary_col,
                                    )
                                });
                                let drv_pos =
                                    optical_galley_pos(ui, row1_rect, &drv_galley, None, false);
                                ui.painter().galley(drv_pos, drv_galley, primary_col);

                                let mono_pod = mono_font_for(ui, cap_font_sz, false);
                                let mut cap_job = egui::text::LayoutJob::default();
                                cap_job.append(
                                    &format_size_2dec(drv.used_bytes),
                                    0.0,
                                    egui::TextFormat {
                                        font_id: mono_pod.clone(),
                                        color: primary_col,
                                        ..Default::default()
                                    },
                                );
                                cap_job.append(
                                    " of ",
                                    0.0,
                                    egui::TextFormat {
                                        font_id: mono_pod.clone(),
                                        color: pal.muted,
                                        ..Default::default()
                                    },
                                );
                                cap_job.append(
                                    &format_size_2dec(drv.total_bytes),
                                    0.0,
                                    egui::TextFormat {
                                        font_id: mono_pod.clone(),
                                        color: primary_col,
                                        ..Default::default()
                                    },
                                );
                                let cap_galley = ui.fonts(|f| f.layout_job(cap_job));
                                let cap_pos = optical_galley_pos(
                                    ui,
                                    row1_rect,
                                    &cap_galley,
                                    Some(row1_rect.max.x),
                                    false,
                                );
                                ui.painter().galley(cap_pos, cap_galley, primary_col);
                                let _ = row1_resp.on_hover_text(format!(
                                    "Used: {} / Total: {}",
                                    format_exact_bytes(drv.used_bytes),
                                    format_exact_bytes(drv.total_bytes)
                                ));

                                ui.add_space(SPACE_4);

                                // Row 2: Scan status / time on left + "<free> free" on right
                                let (row2_rect, row2_resp) =
                                    ui.allocate_exact_size(Vec2::new(inner_w, 16.0), Sense::hover());
                                let (status_txt, status_col) = if !included {
                                    ("Not scanned".to_string(), pal.muted)
                                } else if drv.total_bytes == 0 {
                                    ("✕ Read error".to_string(), pal.status_red)
                                } else if busy_this_drive {
                                    ("Scanning…".to_string(), pal.accent_line)
                                } else {
                                    (
                                        format_pod_short_time(
                                            drv.last_scanned_display.as_deref(),
                                            compact,
                                        ),
                                        pal.muted,
                                    )
                                };
                                let st_galley = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        status_txt,
                                        FontId::new(10.5, FontFamily::Proportional),
                                        status_col,
                                    )
                                });
                                let st_pos =
                                    optical_galley_pos(ui, row2_rect, &st_galley, None, false);
                                ui.painter().galley(st_pos, st_galley, status_col);

                                let free_lbl = format!("{} free", format_size_2dec(drv.free_bytes));
                                let free_galley =
                                    ui.fonts(|f| f.layout_no_wrap(free_lbl, mono_pod, pal.muted));
                                let free_pos = optical_galley_pos(
                                    ui,
                                    row2_rect,
                                    &free_galley,
                                    Some(row2_rect.max.x),
                                    false,
                                );
                                ui.painter().galley(free_pos, free_galley, pal.muted);
                                let ts_tip = drv
                                    .last_scanned_display
                                    .as_deref()
                                    .unwrap_or("Not yet scanned");
                                let _ = row2_resp.on_hover_text(format!(
                                    "Free: {} ({:.1}% free)\nLast scanned: {ts_tip}",
                                    format_exact_bytes(drv.free_bytes),
                                    (1.0 - ratio) * 100.0
                                ));

                                ui.add_space(SPACE_4);

                                // Row 3: Slim 2px flush proportional bar (greyed when excluded, WT gradient when included)
                                let bar_w = inner_w.floor().max(80.0);
                                let (bar_rect, bar_resp) = ui.allocate_exact_size(
                                    Vec2::new(bar_w, PROP_BAR_H),
                                    Sense::hover(),
                                );
                                let snapped_track = snap_rect_to_pixels(bar_rect, ppp);
                                let track_col = lerp_color(pod_fill, pal.border, 0.65);
                                ui.painter()
                                    .rect_filled(snapped_track, Rounding::same(1.0), track_col);
                                let track_px = (snapped_track.width() * ppp).round();
                                let fill_px = if drv.total_bytes == 0 || drv.used_bytes == 0 {
                                    0.0
                                } else {
                                    ((track_px as f64) * ratio)
                                        .round()
                                        .clamp(2.0, track_px as f64) as f32
                                };
                                if drv.total_bytes == 0 && snapped_track.width() > 2.0 {
                                    ui.painter().rect_filled(
                                        snapped_track,
                                        Rounding::same(1.0),
                                        pal.status_red,
                                    );
                                } else if fill_px > 0.0 && snapped_track.height() > 0.0 {
                                    let fill_rect = Rect::from_min_size(
                                        snapped_track.min,
                                        Vec2::new(
                                            fill_px / ppp.max(0.5),
                                            snapped_track.height(),
                                        ),
                                    );
                                    if included {
                                        paint_horizontal_theme_sweep_bar(
                                            ui.painter(),
                                            snapped_track,
                                            fill_rect,
                                            1.0,
                                            pal,
                                        );
                                    } else {
                                        let grey_col = lerp_color(pod_fill, pal.muted, 0.35);
                                        ui.painter().rect_filled(
                                            fill_rect,
                                            Rounding::same(1.0),
                                            grey_col,
                                        );
                                    }
                                }
                                let _ = bar_resp.on_hover_text(format!(
                                    "{:.2}% used · {}",
                                    ratio * 100.0,
                                    if included {
                                        "Included in scan scope. Excluded drives are not sized at all, and apps solely on excluded drives are omitted."
                                    } else {
                                        "Not scanned — excluded drives are not sized at all (including registered InstallLocation folders), and apps solely on them are omitted."
                                    }
                                ));

                                ui.add_space(SPACE_4);

                                // Row 4: In-place Scope controls (Include checkbox + Exclusions overlay) & per-drive Rescan
                                let (ctrl_rect, _) =
                                    ui.allocate_exact_size(Vec2::new(inner_w, 24.0), Sense::hover());
                                let rescan_btn_min_w = if compact { 48.0 } else { 64.0 };
                                let rescan_w = if included {
                                    if busy_this_drive {
                                        76.0
                                    } else if compact {
                                        50.0
                                    } else {
                                        64.0
                                    }
                                } else {
                                    0.0
                                };
                                let ctrl_gap = if compact { 2.0 } else { SPACE_4 };
                                let left_ctrl_w = if rescan_w > 0.0 {
                                    (inner_w - rescan_w - ctrl_gap).max(80.0)
                                } else {
                                    inner_w
                                };
                                let mut lc_ui = table_cell_ui(
                                    ui,
                                    Rect::from_min_size(ctrl_rect.min, Vec2::new(left_ctrl_w, 24.0)),
                                    Layout::left_to_right(Align::Center),
                                );
                                lc_ui.spacing_mut().item_spacing.x = if compact { 2.0 } else { SPACE_4 };

                                let was_excluded = !included;
                                let ctrl_font_sz = if compact { 10.5 } else { 11.0 };
                                let include_tip = if included {
                                    format!(
                                        "Include {dl}:\\ in scan scope. Excluded drives are not walked or sized at all (including registered InstallLocation folders), and apps solely on excluded drives are omitted."
                                    )
                                } else {
                                    format!(
                                        "Excluded from scan scope — {dl}:\\ is not sized at all (including registered InstallLocation folders), and apps solely on it are omitted."
                                    )
                                };
                                if draw_compact_pod_checkbox(
                                    &mut lc_ui,
                                    &mut included,
                                    "Include",
                                    ctrl_font_sz,
                                    24.0,
                                    was_excluded,
                                    &include_tip,
                                    pal,
                                ) {
                                    let mut next_cfg = config.clone();
                                    if let Some(t) = next_cfg
                                        .targets
                                        .iter_mut()
                                        .find(|t| t.letter.to_ascii_uppercase() == dl)
                                    {
                                        t.enabled = included;
                                    }
                                    ui_state.ledger_page = 0;
                                    ui_state.ledger_scroll_to_top = true;
                                    emit(UiAction::SaveConfig(next_cfg));
                                }

                                let excl_lbl = if excl_count == 0 {
                                    "Exclusions".to_string()
                                } else {
                                    format!("Exclusions ({excl_count})")
                                };
                                let is_excl_open = ui_state.open_pod_scope_drive == Some(dl);
                                let excl_resp = draw_z_button(
                                    &mut lc_ui,
                                    &excl_lbl,
                                    Vec2::new(0.0, 20.0),
                                    ctrl_font_sz,
                                    RADIUS_PILL,
                                    true,
                                    is_excl_open,
                                    ButtonVariant::SubtleOutline,
                                    pal,
                                );
                                if excl_resp
                                    .on_hover_text(format!("Manage folder exclusions on {dl}:\\"))
                                    .clicked()
                                {
                                    ui_state.open_pod_scope_drive =
                                        if is_excl_open { None } else { Some(dl) };
                                }

                                if included {
                                    let mut rc_ui = table_cell_ui(
                                        ui,
                                        Rect::from_min_max(
                                            Pos2::new(ctrl_rect.max.x - rescan_w, ctrl_rect.min.y),
                                            ctrl_rect.max,
                                        ),
                                        Layout::right_to_left(Align::Center),
                                    );
                                    let r_lbl = if busy_this_drive {
                                        "Scanning…"
                                    } else {
                                        "Rescan"
                                    };
                                    let r_resp = draw_z_button(
                                        &mut rc_ui,
                                        r_lbl,
                                        Vec2::new(rescan_btn_min_w, 24.0),
                                        if compact { 11.0 } else { 11.5 },
                                        RADIUS_CONTROL,
                                        !busy_this_drive,
                                        false,
                                        ButtonVariant::PrimaryOutline,
                                        pal,
                                    );
                                    if r_resp
                                        .on_hover_text(format!(
                                            "Rescan only {dl}:\\ and merge into current report"
                                        ))
                                        .clicked()
                                    {
                                        ui_state.ledger_page = 0;
                                        ui_state.ledger_scroll_to_top = true;
                                        emit(UiAction::StartDriveRescan(dl));
                                    }
                                }
                            });
                        },
                    );
                    if drv_idx == 0 {
                        ui_state.first_drive_pod_rect = Some(pod_frame.response.rect);
                    }
                    if ui_state.open_pod_scope_drive == Some(dl) {
                        overlay_anchor = Some((dl, pod_frame.response.rect));
                    }
                }
            });
            drv_offset += cols_in_row;
        }
    });

    overlay_anchor
}

#[allow(clippy::too_many_arguments)]
fn draw_pod_scope_overlay(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    config: &AppConfig,
    drive_letter: char,
    anchor_rect: Rect,
    system_windows_dir: &str,
    pal: &ThemePalette,
    emit: &mut dyn FnMut(UiAction),
) {
    let dl = drive_letter.to_ascii_uppercase();
    let Some(target) = config
        .targets
        .iter()
        .find(|t| t.letter.to_ascii_uppercase() == dl)
        .cloned()
    else {
        ui_state.open_pod_scope_drive = None;
        return;
    };

    let sys_win_key = normalize_path_key(system_windows_dir);
    let is_sys = dl == 'C' || target.exclusions.iter().any(|e| normalize_path_key(e) == sys_win_key);
    let screen = ctx.screen_rect();
    let popup_w = 376.0_f32;
    let pos_x = anchor_rect.left().clamp(
        screen.left() + SPACE_16,
        (screen.right() - SPACE_16 - popup_w).max(screen.left() + SPACE_16),
    );
    let pos_y = (anchor_rect.bottom() + SPACE_4)
        .min((screen.bottom() - 200.0).max(screen.top() + SPACE_16));

    let mut draft = config.clone();
    let mut changed = false;
    let mut close_clicked = false;

    let area_resp = egui::Area::new(egui::Id::new(("pod_scope_overlay", dl)))
        .order(Order::Foreground)
        .fixed_pos(Pos2::new(pos_x, pos_y))
        .show(ctx, |ui| {
            draw_crisp_frame(
                ui,
                pal.panel,
                pal.accent_line,
                RADIUS_PANEL,
                Margin::same(SPACE_12),
                |ui| {
                    let inner_w = popup_w - SPACE_16 - SPACE_8 - 2.0;
                    ui.set_width(inner_w);
                    ui.spacing_mut().item_spacing.y = SPACE_8;

                    draw_split_row(
                        ui,
                        24.0,
                        60.0,
                        Sense::hover(),
                        |ui| {
                            let (title_font, fb) = prop_strong_font(ui_state.has_prop_strong, 13.0);
                            let mut rt = RichText::new(format!("Drive {dl}:\\ exclusions"))
                                .font(title_font)
                                .color(pal.text);
                            if fb {
                                rt = rt.strong();
                            }
                            ui.label(rt);
                        },
                        |ui| {
                            if draw_z_button(
                                ui,
                                "Close",
                                Vec2::new(56.0, 24.0),
                                11.5,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::SecondaryOutline,
                                pal,
                            )
                            .clicked()
                            {
                                close_clicked = true;
                            }
                        },
                    );

                    ui.label(
                        RichText::new(
                            "Excluded folders on this drive are not walked or sized, and content under them is not attributed to apps or Unattributed. Locked system folders (such as C:\\Windows) remain excluded and cannot be removed.",
                        )
                        .size(11.0)
                        .color(pal.muted),
                    );

                    if target.exclusions.is_empty() {
                        ui.label(
                            RichText::new("No folder exclusions on this drive.")
                                .size(11.5)
                                .color(pal.muted),
                        );
                    } else {
                        draw_synced_scroll_area(
                            ui,
                            ("pod_excl_scroll", dl),
                            Some(140.0),
                            [false, true],
                            pal,
                            pal.panel,
                            |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                if let Some(dt) = draft
                                    .targets
                                    .iter_mut()
                                    .find(|t| t.letter.to_ascii_uppercase() == dl)
                                {
                                    let mut remove_idx: Option<usize> = None;
                                    for (idx, ex) in dt.exclusions.iter().enumerate() {
                                        let is_locked = is_sys && normalize_path_key(ex) == sys_win_key;
                                        draw_split_row(
                                            ui,
                                            22.0,
                                            72.0,
                                            Sense::hover(),
                                            |ui| {
                                                draw_truncated_text(
                                                    ui,
                                                    ex,
                                                    FontId::new(11.5, FontFamily::Monospace),
                                                    pal.text,
                                                    false,
                                                );
                                            },
                                            |ui| {
                                                if is_locked {
                                                    paint_status_chip(
                                                        ui,
                                                        StatusKind::Danger,
                                                        "LOCKED",
                                                        pal,
                                                    );
                                                } else if draw_z_button(
                                                    ui,
                                                    "Remove",
                                                    Vec2::new(56.0, 20.0),
                                                    10.5,
                                                    RADIUS_PILL,
                                                    true,
                                                    false,
                                                    ButtonVariant::SubtleOutline,
                                                    pal,
                                                )
                                                .clicked()
                                                {
                                                    remove_idx = Some(idx);
                                                }
                                            },
                                        );
                                    }
                                    if let Some(idx) = remove_idx {
                                        dt.exclusions.remove(idx);
                                        changed = true;
                                    }
                                }
                            },
                        );
                    }

                    let input = ui_state.new_exclusion_inputs.entry(dl).or_default();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_8;
                        draw_crisp_text_input(
                            ui,
                            ("pod_exclusion_input", dl),
                            input,
                            (inner_w - 76.0).max(160.0),
                            &format!("{dl}:\\FolderToExclude"),
                            pal.input_bg,
                            pal,
                        );
                        if draw_z_button(
                            ui,
                            "Add",
                            Vec2::new(64.0, HEADER_CONTROL_H),
                            11.5,
                            RADIUS_CONTROL,
                            true,
                            false,
                            ButtonVariant::SecondaryOutline,
                            pal,
                        )
                        .clicked()
                        {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                if let Some(dt) = draft
                                    .targets
                                    .iter_mut()
                                    .find(|t| t.letter.to_ascii_uppercase() == dl)
                                {
                                    if !dt
                                        .exclusions
                                        .iter()
                                        .any(|e| normalize_path_key(e) == normalize_path_key(&trimmed))
                                    {
                                        dt.exclusions.push(trimmed);
                                        input.clear();
                                        changed = true;
                                    }
                                }
                            }
                        }
                    });
                },
            );
        });

    if changed {
        ui_state.ledger_page = 0;
        ui_state.ledger_scroll_to_top = true;
        emit(UiAction::SaveConfig(draft));
    }
    if close_clicked {
        ui_state.open_pod_scope_drive = None;
    } else if ctx.input(|i| i.pointer.primary_clicked()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !area_resp.response.rect.contains(pos) && !anchor_rect.contains(pos) {
                ui_state.open_pod_scope_drive = None;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_unattributed_facet_popup_overlay(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    facet_key: &'static str,
    trigger_rect: Rect,
    pal: &ThemePalette,
) {
    let screen = ctx.screen_rect();
    let popup_w = match facet_key {
        "unattrib_zone" => 236.0_f32,
        _ => 216.0_f32,
    };
    let pos_x = trigger_rect.left().clamp(
        screen.left() + SPACE_16,
        (screen.right() - SPACE_16 - popup_w).max(screen.left() + SPACE_16),
    );
    let room_below = screen.bottom() - trigger_rect.bottom() - SPACE_8;
    let max_popup_h = 240.0_f32;
    let pos_y = if room_below >= 156.0 {
        trigger_rect.bottom() + SPACE_4
    } else {
        (trigger_rect.top() - SPACE_4 - max_popup_h.min(200.0)).max(screen.top() + SPACE_16)
    };

    let mut picked = false;
    let area_resp = egui::Area::new(egui::Id::new(("unattrib_facet_popup", facet_key)))
        .order(Order::Foreground)
        .fixed_pos(Pos2::new(pos_x, pos_y))
        .show(ctx, |ui| {
            draw_crisp_frame(
                ui,
                pal.panel,
                pal.accent_line,
                RADIUS_PANEL,
                Margin::same(SPACE_8),
                |ui| {
                    let inner_w = popup_w - SPACE_16 - 2.0;
                    ui.set_width(inner_w);
                    draw_synced_scroll_area(
                        ui,
                        ("unattrib_facet_scroll", facet_key),
                        Some(max_popup_h),
                        [false, true],
                        pal,
                        pal.panel,
                        |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let row_w = ui.available_width().max(120.0);
                            match facet_key {
                                "unattrib_drive" => {
                                    let is_all = ui_state.unattrib_facets.drive.is_none();
                                    let all_cnt = format!("{}", rep.unattributed.len());
                                    if draw_facet_popup_row(
                                        ui,
                                        row_w,
                                        "All drives",
                                        Some(&all_cnt),
                                        is_all,
                                        pal,
                                    ) {
                                        ui_state.unattrib_facets.drive = None;
                                        picked = true;
                                    }
                                    for drv in &rep.drives {
                                        let dl = drv.letter.to_ascii_uppercase();
                                        let cnt = rep
                                            .unattributed
                                            .iter()
                                            .filter(|it| it.drive_letter.to_ascii_uppercase() == dl)
                                            .count();
                                        let sel = ui_state
                                            .unattrib_facets
                                            .drive
                                            .is_some_and(|d| d.to_ascii_uppercase() == dl);
                                        let meta = format!("{cnt}");
                                        if draw_facet_popup_row(
                                            ui,
                                            row_w,
                                            &format!("{dl}:\\"),
                                            Some(&meta),
                                            sel,
                                            pal,
                                        ) {
                                            ui_state.unattrib_facets.drive = Some(dl);
                                            picked = true;
                                        }
                                    }
                                }
                                "unattrib_size" => {
                                    for band in SizeBandFilter::ALL_OPTIONS {
                                        let cnt = if band == SizeBandFilter::All {
                                            rep.unattributed.len()
                                        } else {
                                            rep.unattributed
                                                .iter()
                                                .filter(|it| band.matches_bytes(it.bytes))
                                                .count()
                                        };
                                        let sel = ui_state.unattrib_facets.size_band == band;
                                        let meta = format!("{cnt}");
                                        if draw_facet_popup_row(
                                            ui,
                                            row_w,
                                            band.label(),
                                            Some(&meta),
                                            sel,
                                            pal,
                                        ) {
                                            ui_state.unattrib_facets.size_band = band;
                                            picked = true;
                                        }
                                    }
                                }
                                "unattrib_zone" => {
                                    for zone in UnattributedLocationZone::ALL_OPTIONS {
                                        let cnt = if zone == UnattributedLocationZone::All {
                                            rep.unattributed.len()
                                        } else {
                                            rep.unattributed
                                                .iter()
                                                .filter(|it| {
                                                    UnattributedLocationZone::classify_path(&it.path)
                                                        == zone
                                                })
                                                .count()
                                        };
                                        let sel = ui_state.unattrib_facets.location_zone == zone;
                                        let meta = format!("{cnt}");
                                        if draw_facet_popup_row(
                                            ui,
                                            row_w,
                                            zone.label(),
                                            Some(&meta),
                                            sel,
                                            pal,
                                        ) {
                                            ui_state.unattrib_facets.location_zone = zone;
                                            picked = true;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        },
                    );
                },
            );
        });

    if picked {
        ui_state.open_unattrib_facet_popup = None;
    } else if ctx.input(|i| i.pointer.primary_clicked()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !area_resp.response.rect.contains(pos) && !trigger_rect.contains(pos) {
                ui_state.open_unattrib_facet_popup = None;
            }
        }
    }
}

pub fn draw_app_ui(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    config: &AppConfig,
    report: Option<&ScanReport>,
    scanning: bool,
    progress: Option<&ScanProgressSnapshot>,
    system_windows_dir: &str,
    emit: &mut dyn FnMut(UiAction),
) {
    let empty_busy = BTreeSet::new();
    draw_app_ui_with_drive_busy(
        ctx,
        ui_state,
        config,
        report,
        scanning,
        &empty_busy,
        progress,
        system_windows_dir,
        emit,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_app_ui_with_drive_busy(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    config: &AppConfig,
    report: Option<&ScanReport>,
    scanning: bool,
    rescanning_drives: &BTreeSet<char>,
    progress: Option<&ScanProgressSnapshot>,
    system_windows_dir: &str,
    emit: &mut dyn FnMut(UiAction),
) {
    let mode = config.theme_mode;
    let theme_key = compute_theme_key(mode, config.theme_preset, config.theme_custom_rgb);
    if ui_state.applied_theme_key != Some(theme_key) {
        apply_theme_visuals_with(ctx, mode, config.theme_preset, config.theme_custom_rgb);
        ui_state.applied_theme_key = Some(theme_key);
        ui_state.has_prop_strong = ctx.fonts(|f| {
            f.families()
                .iter()
                .any(|fam| *fam == FontFamily::Name("ProportionalStrong".into()))
        });
    }
    let pal = ThemePalette::from_config(mode, config.theme_preset, config.theme_custom_rgb);
    ui_state.first_unattrib_web_search_rect = None;
    ui_state.unattrib_mid_row_rect = None;
    ui_state.first_drive_pod_rect = None;

    let sys_drive_letter = report
        .and_then(|r| {
            r.drives
                .iter()
                .find(|d| d.is_system_drive)
                .map(|d| d.letter.to_ascii_uppercase())
        })
        .unwrap_or('C');
    if let Some(rep) = report {
        ensure_report_and_filter_caches(
            ctx,
            ui_state,
            rep,
            &pal,
            sys_drive_letter,
            pal.theme_key,
        );
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        ui_state.open_reverse_dep_id = None;
        ui_state.open_facet_popup = None;
        ui_state.open_unattrib_facet_popup = None;
        ui_state.open_pod_scope_drive = None;
    }
    let mut active_facet_anchor: Option<(&'static str, Rect)> = None;
    let mut active_unattrib_facet_anchor: Option<(&'static str, Rect)> = None;
    let mut active_pod_overlay: Option<(char, Rect)> = None;
    let any_scan_active = scanning || !rescanning_drives.is_empty();
    let scope_stale = report.is_some_and(|r| r.is_scope_stale(config));

    // Masthead row ONLY (title + live status + [ThemeToggle] [Rescan / Rescan needed] [Settings])
    // Keeping pods/search/filter inside CentralPanel below lets the raised ledger cover them up to here (FIX 4).
    egui::TopBottomPanel::top("masthead_panel")
        .show_separator_line(false)
        .frame(
            egui::Frame::none()
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .inner_margin(Margin {
                    left: SPACE_16,
                    right: SPACE_16,
                    top: SPACE_12,
                    bottom: SPACE_8,
                }),
        )
        .show(ctx, |ui| {
            draw_split_row(
                ui,
                HEADER_CONTROL_H,
                276.0,
                Sense::hover(),
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_8;
                    let last_scan_str = report
                        .map(|r| r.scan_timestamp_display.as_str())
                        .unwrap_or("none");
                    let (status, status_color, caption_tip) = if any_scan_active {
                        let s = if let Some(p) = progress {
                            format!(
                                "Read-only · {} · {} dirs · {}",
                                p.phase,
                                p.dirs_walked,
                                format_size_2dec(p.bytes_counted)
                            )
                        } else {
                            "Read-only · Scanning...".to_string()
                        };
                        (s, pal.accent_line, None)
                    } else if scope_stale {
                        (
                            "Read-only · ! Scope changed — rescan needed".to_string(),
                            pal.status_amber,
                            Some(format!(
                                "Last scan: {last_scan_str}\nScan scope, exclusions, or zone rules changed — click Rescan needed to refresh."
                            )),
                        )
                    } else if let Some(rep) = report {
                        (
                            format!(
                                "Read-only · Last scan: {} · {:.2}s",
                                rep.scan_timestamp_display,
                                rep.duration_ms as f64 / 1000.0
                            ),
                            pal.muted,
                            None,
                        )
                    } else {
                        (
                            "Read-only · No cached scan — click Rescan".to_string(),
                            pal.muted,
                            None,
                        )
                    };
                    if let Some(cap_resp) = draw_wt_header_lockup(
                        ui,
                        &mut ui_state.wt_logo_tex,
                        "y-storage",
                        &status,
                        status_color,
                        &pal,
                    ) {
                        if let Some(tip) = caption_tip {
                            let _ = cap_resp.on_hover_text(tip);
                        }
                    }
                },
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_8;
                    let settings_lbl = match ui_state.active_view {
                        ActiveView::Settings | ActiveView::Dependencies => "Back to Apps",
                        ActiveView::Main => "Settings",
                    };
                    if draw_z_button(
                        ui,
                        settings_lbl,
                        Vec2::new(88.0, HEADER_CONTROL_H),
                        12.0,
                        RADIUS_CONTROL,
                        true,
                        false,
                        ButtonVariant::SecondaryOutline,
                        &pal,
                    )
                    .clicked()
                    {
                        ui_state.open_reverse_dep_id = None;
                        ui_state.open_pod_scope_drive = None;
                        ui_state.open_facet_popup = None;
                        ui_state.open_unattrib_facet_popup = None;
                        ui_state.active_view = match ui_state.active_view {
                            ActiveView::Main => ActiveView::Settings,
                            ActiveView::Settings | ActiveView::Dependencies => ActiveView::Main,
                        };
                    }

                    let rescan_lbl = if scanning {
                        "Scanning..."
                    } else if scope_stale {
                        "Rescan needed"
                    } else {
                        "Rescan"
                    };
                    let rescan_min_w = if scope_stale { 116.0 } else { 76.0 };
                    let rescan_resp = draw_z_button(
                        ui,
                        rescan_lbl,
                        Vec2::new(rescan_min_w, HEADER_CONTROL_H),
                        12.0,
                        RADIUS_CONTROL,
                        !any_scan_active,
                        false,
                        ButtonVariant::PrimaryOutline,
                        &pal,
                    );
                    let rescan_tip = if scope_stale {
                        format!(
                            "Last scan: {}\nScan scope, exclusions, or zone rules changed — click to rescan enabled drives",
                            report.map_or("none", |r| r.scan_timestamp_display.as_str())
                        )
                    } else {
                        "Rescan all enabled drives and registry".to_string()
                    };
                    if rescan_resp.on_hover_text(rescan_tip).clicked() {
                        ui_state.ledger_page = 0;
                        ui_state.ledger_scroll_to_top = true;
                        emit(UiAction::StartRescan);
                    }

                    if draw_theme_toggle_button(ui, mode, &pal) {
                        let next = match mode {
                            ThemeMode::Dark => ThemeMode::Light,
                            ThemeMode::Light => ThemeMode::Dark,
                        };
                        emit(UiAction::ToggleTheme(next));
                    }
                },
            );
        });

    match ui_state.active_view {
        ActiveView::Main => {
            // Collapsible & drag-resizable Pinned Bottom Panel rendered BEFORE CentralPanel so it can rise
            // up to just below masthead_panel (`max_ledger_h = client_height - masthead_height - 4.0`).
            if let Some(rep) = report {
                let sys_lines = rep.windows_system.len();
                let reconcile_reserve_h = if sys_lines > 0 {
                    SPACE_4 + (sys_lines as f32) * 24.0
                } else {
                    0.0
                };
                let max_ledger_h = (ctx.available_rect().height() - 4.0).max(156.0);
                let min_ledger_h = (132.0 + reconcile_reserve_h).min(max_ledger_h);
                if let Some(h) = ui_state.ledger_height_px {
                    ui_state.ledger_height_px = Some(h.clamp(min_ledger_h, max_ledger_h));
                }
                let mut panel = egui::TopBottomPanel::bottom("pinned_groups_panel")
                    .show_separator_line(false)
                    .resizable(false)
                    .frame(
                        egui::Frame::none()
                            .fill(pal.panel_alt)
                            .stroke(Stroke::NONE)
                            .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                    );
                if !ui_state.ledger_collapsed {
                    if let Some(h) = ui_state.ledger_height_px {
                        panel = panel.exact_height(h.clamp(min_ledger_h, max_ledger_h));
                    } else if rep.windows_system.len() > 1 {
                        // Never allow default expansion to overflow the smallest supported window height.
                        panel = panel.max_height(max_ledger_h);
                    }
                }
                panel.show(ctx, |ui| {
                    let sr = ctx.screen_rect();
                    let top_y = (ui.max_rect().top() - SPACE_8).round();
                    let panel_outer_h = ui.max_rect().height() + SPACE_16;
                    let mut grip_active = false;
                    if !ui_state.ledger_collapsed {
                        let grip_rect = Rect::from_min_max(
                            Pos2::new(sr.left(), top_y - 2.0),
                            Pos2::new(sr.right(), top_y + 4.0),
                        );
                        let grip_resp = ui.interact(
                            grip_rect,
                            ui.make_persistent_id("pinned_ledger_top_resize_grip"),
                            Sense::click_and_drag(),
                        );
                        grip_active = grip_resp.hovered() || grip_resp.dragged();
                        if grip_active {
                            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        }
                        if grip_resp.drag_started() {
                            ui_state.ledger_drag_start_h =
                                Some(ui_state.ledger_height_px.unwrap_or(panel_outer_h));
                        }
                        if grip_resp.dragged() {
                            let cur = ui_state
                                .ledger_height_px
                                .unwrap_or_else(|| ui_state.ledger_drag_start_h.unwrap_or(panel_outer_h));
                            let next_h =
                                (cur - grip_resp.drag_delta().y).clamp(min_ledger_h, max_ledger_h);
                            ui_state.ledger_height_px = Some(next_h);
                        }
                        if grip_resp.drag_stopped() {
                            ui_state.ledger_drag_start_h = None;
                            emit(UiAction::UpdateLedgerLayout {
                                collapsed: ui_state.ledger_collapsed,
                                height_px: ui_state.ledger_height_px,
                            });
                        }
                    }
                    paint_solid_px_rect(
                        ui.painter(),
                        Rect::from_min_size(Pos2::new(sr.left(), top_y), Vec2::new(sr.width(), 1.0)),
                        if grip_active { pal.accent_line } else { pal.border },
                    );
                    if !ui_state.ledger_collapsed {
                        let grip_pill = Rect::from_center_size(
                            Pos2::new(sr.center().x, top_y + 2.0),
                            Vec2::new(36.0, 2.0),
                        );
                        ui.painter().rect_filled(
                            grip_pill,
                            Rounding::same(1.0),
                            if grip_active {
                                pal.accent_line
                            } else {
                                lerp_color(pal.border, pal.muted, 0.35)
                            },
                        );
                    }
                    let body_budget_h = if !ui_state.ledger_collapsed {
                        ui_state.ledger_height_px.map(|h| {
                            (h - (SPACE_16 + 22.0 + reconcile_reserve_h + SPACE_4)).max(56.0)
                        })
                    } else {
                        None
                    };
                    active_unattrib_facet_anchor =
                        draw_pinned_bottom_groups(ui, ui_state, rep, &pal, body_budget_h, emit);
                });
            }

            let rem_h = ctx.available_rect().height();
            let v_pad = if rem_h < TABLE_HEADER_H + SPACE_16 + 8.0 {
                0.0
            } else {
                SPACE_4
            };
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(Margin::symmetric(SPACE_16, v_pad)),
                )
                .show(ctx, |ui| {
                    if ui.available_height() <= 12.0 {
                        return;
                    }
                    ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));

                    if let Some(rep) = report {
                        active_pod_overlay = draw_drive_pods_grid(
                            ui,
                            ui_state,
                            config,
                            rep,
                            scanning,
                            rescanning_drives,
                            &pal,
                            emit,
                        );
                    }

                    if ui.available_height() >= HEADER_CONTROL_H {
                        let total_apps_count = report.map_or(0, |r| r.apps.len());
                        let shown_count = ui_state
                            .cached_filtered
                            .as_ref()
                            .map_or(0, |cf| cf.visible_indices.len());
                        let mut list_view_dirty = false;

                        ui.add_space(SPACE_8);
                        // Search input + Clear + Reset filters
                        let mut search_dirty = false;
                        let any_filter_non_default = !ui_state.search_query.trim().is_empty()
                            || ui_state.filter_chip != AppFilterChip::All
                            || !ui_state.facets.is_default();
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), HEADER_CONTROL_H),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new("Filter:").size(12.0).color(pal.muted))
                                        .selectable(false),
                                );
                                let prev_q = ui_state.search_query.clone();
                                draw_crisp_text_input(
                                    ui,
                                    "main_app_filter",
                                    &mut ui_state.search_query,
                                    272.0,
                                    "Search app, publisher, category, or path...",
                                    pal.header_input_bg,
                                    &pal,
                                );
                                if ui_state.search_query != prev_q {
                                    search_dirty = true;
                                }
                                if !ui_state.search_query.is_empty()
                                    && draw_z_button(
                                        ui,
                                        "Clear",
                                        Vec2::new(56.0, HEADER_CONTROL_H),
                                        12.0,
                                        6.0,
                                        true,
                                        false,
                                        ButtonVariant::SubtleOutline,
                                        &pal,
                                    )
                                    .clicked()
                                {
                                    ui_state.search_query.clear();
                                    search_dirty = true;
                                }
                                if any_filter_non_default
                                    && draw_z_button(
                                        ui,
                                        "Reset filters",
                                        Vec2::new(88.0, HEADER_CONTROL_H),
                                        12.0,
                                        RADIUS_CONTROL,
                                        true,
                                        false,
                                        ButtonVariant::SubtleOutline,
                                        &pal,
                                    )
                                    .clicked()
                                {
                                    ui_state.search_query.clear();
                                    ui_state.filter_chip = AppFilterChip::All;
                                    ui_state.facets = FacetFilters::default();
                                    ui_state.open_facet_popup = None;
                                    list_view_dirty = true;
                                }
                            },
                        );
                        if search_dirty {
                            list_view_dirty = true;
                        }

                        if ui.available_height() >= 26.0 {
                            ui.add_space(SPACE_4);
                            let has_c_label = format!("Has {sys_drive_letter}: footprint");
                            let chips = [
                                (AppFilterChip::All, "All"),
                                (AppFilterChip::Grew, "Grew"),
                                (AppFilterChip::NoTouch, "NO-TOUCH"),
                                (AppFilterChip::Standalone, "Standalone"),
                                (AppFilterChip::HasSystemDriveFootprint, has_c_label.as_str()),
                            ];
                            draw_split_row(
                                ui,
                                22.0,
                                156.0,
                                Sense::hover(),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = SPACE_8;
                                    for &(chip, lbl) in &chips {
                                        let selected = ui_state.filter_chip == chip;
                                        if draw_filter_radio_chip(ui, lbl, selected, &pal)
                                            && !selected
                                        {
                                            ui_state.filter_chip = chip;
                                            list_view_dirty = true;
                                        }
                                    }
                                    if let Some(rep_ref) = report {
                                        let per_drive_mode = active_per_drive_mode(
                                            ui_state.filter_chip,
                                            &ui_state.facets,
                                            sys_drive_letter,
                                        );
                                        let filtered_active = ui_state.filter_chip
                                            != AppFilterChip::All
                                            || !ui_state.search_query.trim().is_empty()
                                            || !ui_state.facets.is_default();
                                        let count_txt = if let Some(dl) = per_drive_mode {
                                            let drv_sum: u64 = ui_state
                                                .cached_filtered
                                                .as_ref()
                                                .map(|cf| {
                                                    cf.visible_indices
                                                        .iter()
                                                        .map(|&idx| rep_ref.apps[idx].bytes_on_drive(dl))
                                                        .sum()
                                                })
                                                .unwrap_or(0);
                                            format!(
                                                "{shown_count} apps · {} on {dl}:",
                                                format_size_2dec(drv_sum)
                                            )
                                        } else {
                                            let shown_entries_count = ui_state
                                                .cached_filtered
                                                .as_ref()
                                                .map_or(shown_count, |cf| cf.visible_entries.len());
                                            let total_entries_count = ui_state
                                                .cached_report
                                                .as_ref()
                                                .map_or(total_apps_count, |cr| cr.ledger_entries.len());
                                            if shown_entries_count != shown_count
                                                || total_entries_count != total_apps_count
                                            {
                                                format!(
                                                    "{shown_entries_count} rows · {shown_count} of {total_apps_count} apps"
                                                )
                                            } else {
                                                format!("{shown_count} of {total_apps_count} apps")
                                            }
                                        };
                                        let count_rt = RichText::new(count_txt).size(11.5).color(
                                            if filtered_active { pal.text } else { pal.muted },
                                        );
                                        ui.add(egui::Label::new(count_rt).selectable(false));
                                    }
                                },
                                |ui| {
                                    if let Some(cr) = &ui_state.cached_report {
                                        let sum_font = mono_font_for(ui, 11.5, false);
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&cr.attributed_label)
                                                    .font(sum_font)
                                                    .color(pal.muted),
                                            )
                                            .selectable(false),
                                        )
                                        .on_hover_text(&cr.attributed_tip);
                                    }
                                },
                            );
                        }

                        if report.is_some() && ui.available_height() >= 28.0 {
                            ui.add_space(SPACE_4);
                            let has_growth = ui_state
                                .cached_report
                                .as_ref()
                                .is_some_and(|cr| cr.has_growth_baseline);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);

                                let pub_lbl = match ui_state.facets.publisher.as_deref() {
                                    None => "Publisher: All".to_string(),
                                    Some(k) if k == NO_PUBLISHER_FACET_KEY => {
                                        "Publisher: (no publisher)".to_string()
                                    }
                                    Some(k) => {
                                        let short: String = k.chars().take(24).collect();
                                        if k.chars().count() > 24 {
                                            format!("Publisher: {short}…")
                                        } else {
                                            format!("Publisher: {k}")
                                        }
                                    }
                                };
                                let pub_open = ui_state.open_facet_popup == Some("publisher");
                                let pub_resp = draw_facet_selector_button(
                                    ui,
                                    "publisher",
                                    &pub_lbl,
                                    pub_open,
                                    ui_state.facets.publisher.is_some(),
                                    &pal,
                                );
                                if pub_resp.clicked() {
                                    ui_state.open_facet_popup =
                                        if pub_open { None } else { Some("publisher") };
                                }
                                if ui_state.open_facet_popup == Some("publisher") {
                                    active_facet_anchor = Some(("publisher", pub_resp.rect));
                                }

                                let cat_lbl = match ui_state.facets.category {
                                    None => "Category: All".to_string(),
                                    Some(c) => format!("Category: {}", c.label()),
                                };
                                let cat_open = ui_state.open_facet_popup == Some("category");
                                let cat_resp = draw_facet_selector_button(
                                    ui,
                                    "category",
                                    &cat_lbl,
                                    cat_open,
                                    ui_state.facets.category.is_some(),
                                    &pal,
                                );
                                if cat_resp.clicked() {
                                    ui_state.open_facet_popup =
                                        if cat_open { None } else { Some("category") };
                                }
                                if ui_state.open_facet_popup == Some("category") {
                                    active_facet_anchor = Some(("category", cat_resp.rect));
                                }

                                let drv_lbl = match ui_state.facets.drive {
                                    None => "Drive: All".to_string(),
                                    Some(d) => format!("Drive: {}:\\", d.to_ascii_uppercase()),
                                };
                                let drv_open = ui_state.open_facet_popup == Some("drive");
                                let drv_resp = draw_facet_selector_button(
                                    ui,
                                    "drive",
                                    &drv_lbl,
                                    drv_open,
                                    ui_state.facets.drive.is_some(),
                                    &pal,
                                );
                                if drv_resp.clicked() {
                                    ui_state.open_facet_popup =
                                        if drv_open { None } else { Some("drive") };
                                }
                                if ui_state.open_facet_popup == Some("drive") {
                                    active_facet_anchor = Some(("drive", drv_resp.rect));
                                }

                                let size_lbl = if ui_state.facets.size_band == SizeBandFilter::All {
                                    "Size: All".to_string()
                                } else {
                                    format!("Size: {}", ui_state.facets.size_band.label())
                                };
                                let size_open = ui_state.open_facet_popup == Some("size");
                                let size_resp = draw_facet_selector_button(
                                    ui,
                                    "size",
                                    &size_lbl,
                                    size_open,
                                    ui_state.facets.size_band != SizeBandFilter::All,
                                    &pal,
                                );
                                if size_resp.clicked() {
                                    ui_state.open_facet_popup =
                                        if size_open { None } else { Some("size") };
                                }
                                if ui_state.open_facet_popup == Some("size") {
                                    active_facet_anchor = Some(("size", size_resp.rect));
                                }

                                let dep_lbl = if ui_state.facets.dep_state == DepStateFilter::All {
                                    "Deps: All".to_string()
                                } else {
                                    format!("Deps: {}", ui_state.facets.dep_state.label())
                                };
                                let dep_open = ui_state.open_facet_popup == Some("deps");
                                let dep_resp = draw_facet_selector_button(
                                    ui,
                                    "deps",
                                    &dep_lbl,
                                    dep_open,
                                    ui_state.facets.dep_state != DepStateFilter::All,
                                    &pal,
                                );
                                if dep_resp.clicked() {
                                    ui_state.open_facet_popup =
                                        if dep_open { None } else { Some("deps") };
                                }
                                if ui_state.open_facet_popup == Some("deps") {
                                    active_facet_anchor = Some(("deps", dep_resp.rect));
                                }

                                let zone_lbl =
                                    if ui_state.facets.footprint_zone == FootprintZoneFilter::All {
                                        "Zone: All".to_string()
                                    } else {
                                        format!("Zone: {}", ui_state.facets.footprint_zone.label())
                                    };
                                let zone_open = ui_state.open_facet_popup == Some("zone");
                                let zone_resp = draw_facet_selector_button(
                                    ui,
                                    "zone",
                                    &zone_lbl,
                                    zone_open,
                                    ui_state.facets.footprint_zone != FootprintZoneFilter::All,
                                    &pal,
                                );
                                if zone_resp.clicked() {
                                    ui_state.open_facet_popup =
                                        if zone_open { None } else { Some("zone") };
                                }
                                if ui_state.open_facet_popup == Some("zone") {
                                    active_facet_anchor = Some(("zone", zone_resp.rect));
                                }

                                if has_growth {
                                    let gr_lbl = if ui_state.facets.growth == GrowthFacetFilter::All
                                    {
                                        "Growth: All".to_string()
                                    } else {
                                        format!("Growth: {}", ui_state.facets.growth.label())
                                    };
                                    let gr_open = ui_state.open_facet_popup == Some("growth");
                                    let gr_resp = draw_facet_selector_button(
                                        ui,
                                        "growth",
                                        &gr_lbl,
                                        gr_open,
                                        ui_state.facets.growth != GrowthFacetFilter::All,
                                        &pal,
                                    );
                                    if gr_resp.clicked() {
                                        ui_state.open_facet_popup =
                                            if gr_open { None } else { Some("growth") };
                                    }
                                    if ui_state.open_facet_popup == Some("growth") {
                                        active_facet_anchor = Some(("growth", gr_resp.rect));
                                    }
                                }
                            });
                        }

                        if list_view_dirty {
                            ui_state.ledger_page = 0;
                            ui_state.ledger_scroll_to_top = true;
                            if let Some(rep) = report {
                                ensure_report_and_filter_caches(
                                    ctx,
                                    ui_state,
                                    rep,
                                    &pal,
                                    sys_drive_letter,
                                    pal.theme_key,
                                );
                            }
                            emit(UiAction::UpdateListView {
                                sort_mode: ui_state.sort_mode,
                                sort_direction: ui_state.sort_direction,
                                filter_chip: ui_state.filter_chip,
                                search_query: ui_state.search_query.clone(),
                                facets: ui_state.facets.clone(),
                            });
                        }
                    }

                    if let Some(rep) = report {
                        for w in &rep.warnings {
                            ui.add_space(SPACE_4);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_4;
                                paint_status_chip(ui, StatusKind::Danger, "Warning", &pal);
                                draw_truncated_text(
                                    ui,
                                    w,
                                    FontId::new(11.0, FontFamily::Proportional),
                                    pal.status_red,
                                    false,
                                );
                            });
                        }
                    }

                    ui.add_space(SPACE_4);
                    if ui.available_height() >= TABLE_HEADER_H + 6.0 {
                        draw_main_app_list(ui, ui_state, report, &pal, emit);
                    }
                });

            if let (Some(rep), Some(target_id)) = (report, ui_state.open_reverse_dep_id.clone()) {
                draw_reverse_dependency_overlay(ctx, ui_state, rep, &target_id, &pal);
            }
            if let (Some(rep), Some((facet_key, trigger_rect))) = (report, active_facet_anchor) {
                draw_facet_popup_overlay(ctx, ui_state, rep, facet_key, trigger_rect, &pal, emit);
            } else {
                ui_state.open_facet_popup = None;
            }
            if let (Some(rep), Some((facet_key, trigger_rect))) =
                (report, active_unattrib_facet_anchor)
            {
                draw_unattributed_facet_popup_overlay(
                    ctx,
                    ui_state,
                    rep,
                    facet_key,
                    trigger_rect,
                    &pal,
                );
            } else {
                ui_state.open_unattrib_facet_popup = None;
            }
            if let (Some(_rep), Some((pod_dl, pod_rect))) = (report, active_pod_overlay) {
                draw_pod_scope_overlay(
                    ctx,
                    ui_state,
                    config,
                    pod_dl,
                    pod_rect,
                    system_windows_dir,
                    &pal,
                    emit,
                );
            }
        }
        ActiveView::Dependencies => {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                )
                .show(ctx, |ui| {
                    if let Some(rep) = report {
                        active_pod_overlay = draw_drive_pods_grid(
                            ui,
                            ui_state,
                            config,
                            rep,
                            scanning,
                            rescanning_drives,
                            &pal,
                            emit,
                        );
                        ui.add_space(SPACE_8);
                    }
                    draw_dependencies_view(ui, ui_state, report, &pal);
                });
            if let (Some(_rep), Some((pod_dl, pod_rect))) = (report, active_pod_overlay) {
                draw_pod_scope_overlay(
                    ctx,
                    ui_state,
                    config,
                    pod_dl,
                    pod_rect,
                    system_windows_dir,
                    &pal,
                    emit,
                );
            }
        }
        ActiveView::Settings => {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                )
                .show(ctx, |ui| {
                    draw_settings_view(ui, ui_state, config, report, system_windows_dir, &pal, emit);
                });
        }
    }
}

fn draw_main_app_list(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    report: Option<&ScanReport>,
    pal: &ThemePalette,
    emit: &mut dyn FnMut(UiAction),
) {
    let Some(rep) = report else {
        draw_crisp_frame(
            ui,
            pal.panel,
            pal.border,
            RADIUS_PANEL,
            Margin::same(SPACE_16),
            |ui| {
                ui.set_width((ui.available_width() - SCROLL_GUTTER_W).max(200.0));
                ui.label(
                    RichText::new("No cached scan on disk — click Rescan to inventory installed apps and drive usage.")
                        .size(13.0)
                        .color(pal.muted),
                );
            },
        );
        return;
    };

    ui_state.first_row_size_rect = None;
    let has_prop_strong = ui_state.has_prop_strong;
    let sys_drive_letter = rep
        .drives
        .iter()
        .find(|d| d.is_system_drive)
        .map_or('C', |d| d.letter.to_ascii_uppercase());
    let per_drive_mode = active_per_drive_mode(
        ui_state.filter_chip,
        &ui_state.facets,
        sys_drive_letter,
    );
    let mut cached_report = ui_state.cached_report.take();
    let ledger_entries = cached_report
        .as_mut()
        .map(|cr| std::mem::take(&mut cr.ledger_entries))
        .unwrap_or_default();
    let visible_entries: Vec<usize> = ui_state
        .cached_filtered
        .as_ref()
        .map(|f| f.visible_entries.clone())
        .unwrap_or_default();
    let visible_apps_count = ui_state
        .cached_filtered
        .as_ref()
        .map_or(0, |f| f.visible_indices.len());
    let max_unfiltered_bytes = cached_report
        .as_ref()
        .map_or_else(|| rep.apps.iter().map(|a| a.total_bytes).max().unwrap_or(0), |c| c.max_unfiltered_bytes);
    let max_bar_bytes = per_drive_mode.map_or(max_unfiltered_bytes, |dl| {
        ledger_entries
            .iter()
            .map(|e| e.effective_bytes(&rep.apps, Some(dl)))
            .max()
            .unwrap_or(0)
    });
    let total_attributed_bytes = cached_report
        .as_ref()
        .map_or_else(|| rep.apps.iter().map(|a| a.total_bytes).sum(), |c| c.total_attributed_bytes);
    let total_mode_bytes = per_drive_mode.map_or(total_attributed_bytes, |dl| {
        rep.total_apps_bytes_on_drive(dl)
    });
    let total_filtered = visible_entries.len();
    let (page_idx, total_pages, page_start, page_end) =
        ledger_page_bounds(total_filtered, ui_state.ledger_page, LEDGER_PAGE_SIZE);
    ui_state.ledger_page = page_idx;
    let page_entry_indices = &visible_entries[page_start..page_end];
    let page_shown = page_entry_indices.len();

    let avail_w = ui.available_width();
    let avail_h = ui.available_height();
    if avail_h < TABLE_HEADER_H + 6.0 {
        if let Some(cr) = cached_report.as_mut() {
            cr.ledger_entries = ledger_entries;
        }
        ui_state.cached_report = cached_report;
        return;
    }
    let panel_left = ui.cursor().min.x;
    let panel_w = (avail_w - SCROLL_GUTTER_W).max(320.0);
    let ppp = ui.ctx().pixels_per_point();
    let inset = 1.0 / ppp.max(0.5);

    let outer_border_slot = ui.painter().add(egui::Shape::Noop);
    let header_bg_slot = ui.painter().add(egui::Shape::Noop);

    let panel_top_y = ui.cursor().min.y;
    let prev_item_spacing_y = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(inset);

    let hdr_content_x0 = panel_left + inset + SPACE_12;
    let hdr_content_w = (panel_w - (inset * 2.0 + SPACE_12 * 2.0)).max(200.0);
    let cols = MainTableColumns::compute(hdr_content_x0, hdr_content_w, per_drive_mode.is_some());

    // Sticky column-header strip fixed outside ScrollArea; clicking sortable columns changes sort mode/direction.
    let (hdr_row_rect, _) =
        ui.allocate_exact_size(Vec2::new(panel_w, TABLE_HEADER_H), Sense::hover());
    let hdr_y0 = hdr_row_rect.min.y;
    let hdr_y1 = hdr_row_rect.max.y;

    let name_hdr_rect = cols.name.rect(hdr_y0, hdr_y1);
    let pub_hdr_rect = cols.publisher.rect(hdr_y0, hdr_y1);
    let cat_hdr_rect = cols.category.rect(hdr_y0, hdr_y1);
    let delta_hdr_rect = cols.delta.rect(hdr_y0, hdr_y1);
    let size_hdr_rect = cols.size.rect(hdr_y0, hdr_y1);
    ui_state.header_rects = [
        Some((AppSortMode::Name, name_hdr_rect)),
        Some((AppSortMode::Publisher, pub_hdr_rect)),
        Some((AppSortMode::Category, cat_hdr_rect)),
        Some((AppSortMode::Growth, delta_hdr_rect)),
        Some((AppSortMode::Size, size_hdr_rect)),
    ];

    let mut sort_clicked: Option<AppSortMode> = None;
    if draw_sortable_col_header(
        ui,
        name_hdr_rect,
        "main_name",
        "App name",
        AppSortMode::Name,
        ui_state.sort_mode,
        ui_state.sort_direction,
        false,
        pal,
    ) {
        sort_clicked = Some(AppSortMode::Name);
    }
    if draw_sortable_col_header(
        ui,
        pub_hdr_rect,
        "main_pub",
        "Publisher",
        AppSortMode::Publisher,
        ui_state.sort_mode,
        ui_state.sort_direction,
        false,
        pal,
    ) {
        sort_clicked = Some(AppSortMode::Publisher);
    }
    if draw_sortable_col_header(
        ui,
        cat_hdr_rect,
        "main_cat",
        "Category",
        AppSortMode::Category,
        ui_state.sort_mode,
        ui_state.sort_direction,
        false,
        pal,
    ) {
        sort_clicked = Some(AppSortMode::Category);
    }
    if draw_sortable_col_header(
        ui,
        delta_hdr_rect,
        "main_delta",
        "Growth delta",
        AppSortMode::Growth,
        ui_state.sort_mode,
        ui_state.sort_direction,
        true,
        pal,
    ) {
        sort_clicked = Some(AppSortMode::Growth);
    }
    let size_col_hdr = match per_drive_mode {
        Some(dl) => format!("Size on {dl}:"),
        None => "Total size".to_string(),
    };
    if draw_sortable_col_header(
        ui,
        size_hdr_rect,
        "main_size",
        &size_col_hdr,
        AppSortMode::Size,
        ui_state.sort_mode,
        ui_state.sort_direction,
        true,
        pal,
    ) {
        sort_clicked = Some(AppSortMode::Size);
    }
    if let Some(mode) = sort_clicked {
        let (nm, nd) = mode.click_transition(ui_state.sort_mode, ui_state.sort_direction);
        ui_state.sort_mode = nm;
        ui_state.sort_direction = nd;
        ui_state.ledger_page = 0;
        ui_state.ledger_scroll_to_top = true;
        ui_state.cached_filtered = None;
        ui.ctx().request_repaint();
        emit(UiAction::UpdateListView {
            sort_mode: ui_state.sort_mode,
            sort_direction: ui_state.sort_direction,
            filter_chip: ui_state.filter_chip,
            search_query: ui_state.search_query.clone(),
            facets: ui_state.facets.clone(),
        });
    }

    let hdr_div_rect = Rect::from_min_size(
        Pos2::new(panel_left + inset, hdr_y1),
        Vec2::new((panel_w - inset * 2.0).max(0.0), 1.0),
    );
    paint_solid_px_rect(ui.painter(), hdr_div_rect, pal.border);
    ui.advance_cursor_after_rect(hdr_div_rect);

    let has_visible_family = visible_entries
        .iter()
        .any(|&ei| ledger_entries[ei].is_family());

    // Precompute cumulative row y-offsets across current page top-level entries (`<= 50`) for exact O(log N) viewport virtualization.
    let mut row_tops = Vec::with_capacity(page_shown + 1);
    row_tops.push(0.0_f32);
    for &entry_idx in page_entry_indices {
        let h = match &ledger_entries[entry_idx] {
            LedgerEntry::Unit(app_idx) => {
                let app = &rep.apps[*app_idx];
                if ui_state.expanded_apps.contains(&app.id) {
                    ui_state
                        .expanded_row_heights
                        .get(&app.id)
                        .copied()
                        .unwrap_or_else(|| estimate_expanded_row_height(app))
                } else {
                    COLLAPSED_ROW_OUTER_H
                }
            }
            LedgerEntry::Family(fam) => {
                if ui_state.expanded_families.contains(&fam.id) {
                    ui_state
                        .expanded_family_heights
                        .get(&fam.id)
                        .copied()
                        .unwrap_or_else(|| {
                            let members_h: f32 = fam
                                .member_indices
                                .iter()
                                .map(|&mi| {
                                    let m = &rep.apps[mi];
                                    if ui_state.expanded_apps.contains(&m.id) {
                                        ui_state
                                            .expanded_row_heights
                                            .get(&m.id)
                                            .copied()
                                            .unwrap_or_else(|| estimate_expanded_row_height(m))
                                    } else {
                                        COLLAPSED_ROW_OUTER_H
                                    }
                                })
                                .sum();
                            COLLAPSED_ROW_OUTER_H + 22.0 + members_h + SPACE_8
                        })
                } else {
                    COLLAPSED_ROW_OUTER_H
                }
            }
        };
        let next = row_tops.last().copied().unwrap_or(0.0) + h;
        row_tops.push(next);
    }
    let total_virtual_h = row_tops.last().copied().unwrap_or(0.0);

    let footer_reserve = LEDGER_FOOTER_H + 1.0;
    let body_max_h = (avail_h - (TABLE_HEADER_H + 1.0 + footer_reserve + inset * 2.0 + 2.0)).max(0.0);
    let scroll_to_top = std::mem::take(&mut ui_state.ledger_scroll_to_top);
    let mut unit_mut = UnitRowMut {
        cached_report: &mut cached_report,
        expanded_apps: &mut ui_state.expanded_apps,
        expanded_families: &mut ui_state.expanded_families,
        expanded_row_heights: &mut ui_state.expanded_row_heights,
        expanded_family_heights: &mut ui_state.expanded_family_heights,
        icon_textures: &mut ui_state.icon_textures,
        open_reverse_dep_id: &mut ui_state.open_reverse_dep_id,
        cached_rim: &mut ui_state.cached_rim,
        truncate_cache: &mut ui_state.truncate_cache,
        path_exists_cache: &mut ui_state.unattrib_exists_cache,
        first_row_size_rect: &mut ui_state.first_row_size_rect,
    };
    let unit_ctx = UnitRowCtx {
        rep,
        pal,
        cols: &cols,
        panel_left,
        panel_w,
        inset,
        ppp,
        has_prop_strong,
        per_drive_mode,
        max_bar_bytes,
        total_mode_bytes,
        total_attributed_bytes,
    };
    let mut show_bottom_fade = false;

    if body_max_h >= 6.0 {
        draw_synced_scroll_area_viewport_with_reset(
            ui,
            "main_app_scroll",
            Some(body_max_h),
            [false, true],
            scroll_to_top,
            pal,
            pal.bg,
            |ui, viewport| {
                ui.set_width(panel_w);
                ui.spacing_mut().item_spacing.y = 0.0;
                show_bottom_fade = page_shown > 0 && viewport.max.y + 1.5 < total_virtual_h;

                let body_clip = Rect::from_min_max(
                    Pos2::new(panel_left + inset, ui.clip_rect().min.y),
                    Pos2::new(panel_left + panel_w - inset, ui.clip_rect().max.y),
                )
                .intersect(ui.clip_rect());
                ui.set_clip_rect(body_clip);

                if page_shown == 0 {
                    egui::Frame::none()
                        .inner_margin(Margin::symmetric(SPACE_12 + inset, SPACE_12))
                        .show(ui, |ui| {
                            ui.set_width((panel_w - (SPACE_12 + inset) * 2.0).max(160.0));
                            ui.label(
                                RichText::new("No matching apps for the active filter/search.")
                                    .size(12.5)
                                    .color(pal.muted),
                            );
                        });
                    return;
                }

                let vis_y0 = (viewport.min.y - COLLAPSED_ROW_OUTER_H * 2.0).max(0.0);
                let vis_y1 = (viewport.max.y + COLLAPSED_ROW_OUTER_H * 2.0).min(total_virtual_h);
                let first_vis = row_tops[1..]
                    .partition_point(|&bot| bot < vis_y0)
                    .min(page_shown.saturating_sub(1));
                let end_vis = row_tops[first_vis + 1..]
                    .partition_point(|&bot| bot < vis_y1)
                    .saturating_add(first_vis + 1)
                    .min(page_shown);

                let top_skip = row_tops[first_vis];
                if top_skip > 0.0 {
                    ui.allocate_exact_size(Vec2::new(panel_w, top_skip), Sense::hover());
                }

                for row_idx in first_vis..end_vis {
                    let entry_idx = page_entry_indices[row_idx];
                    let curr_expanded = match &ledger_entries[entry_idx] {
                        LedgerEntry::Unit(app_idx) => {
                            let _ = draw_unit_ledger_row(
                                ui,
                                &unit_ctx,
                                &mut unit_mut,
                                *app_idx,
                                0.0,
                                pal.panel,
                                row_idx == 0,
                                emit,
                            );
                            unit_mut.expanded_apps.contains(&rep.apps[*app_idx].id)
                        }
                        LedgerEntry::Family(fam) => {
                            draw_family_ledger_row(
                                ui,
                                &unit_ctx,
                                &mut unit_mut,
                                fam,
                                row_idx == 0,
                                emit,
                            );
                            unit_mut.expanded_families.contains(&fam.id)
                        }
                    };

                    let next_expanded = page_entry_indices.get(row_idx + 1).is_some_and(|&next_ei| {
                        match &ledger_entries[next_ei] {
                            LedgerEntry::Unit(ni) => {
                                unit_mut.expanded_apps.contains(&rep.apps[*ni].id)
                            }
                            LedgerEntry::Family(nf) => unit_mut.expanded_families.contains(&nf.id),
                        }
                    });
                    if !curr_expanded && row_idx + 1 < page_shown && !next_expanded {
                        let sep_y = (ui.cursor().min.y - 1.0).round();
                        let sep = Rect::from_min_size(
                            Pos2::new(panel_left + inset, sep_y),
                            Vec2::new((panel_w - inset * 2.0).max(0.0), 1.0),
                        );
                        paint_solid_px_rect(ui.painter(), sep, pal.border);
                    }
                }

                let bot_skip = (total_virtual_h - row_tops[end_vis]).max(0.0);
                if bot_skip > 0.0 {
                    ui.allocate_exact_size(Vec2::new(panel_w, bot_skip), Sense::hover());
                }
            },
        );
    }
    if let Some(cr) = cached_report.as_mut() {
        cr.ledger_entries = ledger_entries;
    }
    ui_state.cached_report = cached_report;

    let footer_div_y = ui.cursor().min.y;
    let footer_div_rect = Rect::from_min_size(
        Pos2::new(panel_left + inset, footer_div_y),
        Vec2::new((panel_w - inset * 2.0).max(0.0), 1.0),
    );
    paint_solid_px_rect(ui.painter(), footer_div_rect, pal.border);
    ui.advance_cursor_after_rect(footer_div_rect);

    let (footer_row_rect, _) =
        ui.allocate_exact_size(Vec2::new(panel_w, LEDGER_FOOTER_H), Sense::hover());
    let footer_inner_rect = Rect::from_min_max(
        Pos2::new(panel_left + inset + SPACE_12, footer_row_rect.min.y),
        Pos2::new(
            (panel_left + panel_w - inset - SPACE_12).max(panel_left + inset + SPACE_12 + 120.0),
            footer_row_rect.max.y,
        ),
    );
    {
        let mut f_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(footer_inner_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let range_txt = if total_filtered == 0 {
            "Showing 0 of 0".to_string()
        } else if has_visible_family {
            format!(
                "Showing {}–{} of {} entries ({} apps)",
                page_start + 1,
                page_end,
                total_filtered,
                visible_apps_count
            )
        } else {
            format!("Showing {}–{} of {}", page_start + 1, page_end, total_filtered)
        };
        let pager_w = 240.0_f32;
        let left_w = (footer_inner_rect.width() - pager_w - SPACE_8).max(80.0);
        let left_rect = Rect::from_min_size(
            footer_inner_rect.min,
            Vec2::new(left_w, footer_inner_rect.height()),
        );
        draw_optical_label_in_rect(
            &mut f_ui,
            left_rect,
            &range_txt,
            FontId::new(11.5, FontFamily::Proportional),
            pal.muted,
            false,
            false,
        );

        let right_rect = Rect::from_min_max(
            Pos2::new(left_rect.max.x + SPACE_8, footer_inner_rect.min.y),
            footer_inner_rect.max,
        );
        let mut r_ui = f_ui.new_child(
            UiBuilder::new()
                .max_rect(right_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        r_ui.spacing_mut().item_spacing.x = SPACE_8;

        let can_next = page_idx + 1 < total_pages;
        if draw_z_button(
            &mut r_ui,
            "Next",
            Vec2::new(62.0, 24.0),
            11.5,
            RADIUS_CONTROL,
            can_next,
            false,
            ButtonVariant::SecondaryOutline,
            pal,
        )
        .clicked()
            && can_next
        {
            ui_state.ledger_page = (page_idx + 1).min(total_pages - 1);
            ui_state.ledger_scroll_to_top = true;
            ui.ctx().request_repaint();
        }

        let page_lbl = format!("Page {} of {}", page_idx + 1, total_pages);
        let page_font = FontId::new(11.5, FontFamily::Proportional);
        let page_w = r_ui
            .fonts(|f| f.layout_no_wrap(page_lbl.clone(), page_font.clone(), pal.text).size().x)
            .ceil()
            + 4.0;
        let (lbl_rect, _) = r_ui.allocate_exact_size(Vec2::new(page_w, 24.0), Sense::hover());
        draw_optical_label_in_rect(&mut r_ui, lbl_rect, &page_lbl, page_font, pal.text, false, true);

        let can_prev = page_idx > 0;
        if draw_z_button(
            &mut r_ui,
            "Previous",
            Vec2::new(76.0, 24.0),
            11.5,
            RADIUS_CONTROL,
            can_prev,
            false,
            ButtonVariant::SecondaryOutline,
            pal,
        )
        .clicked()
            && can_prev
        {
            ui_state.ledger_page = page_idx.saturating_sub(1);
            ui_state.ledger_scroll_to_top = true;
            ui.ctx().request_repaint();
        }
    }

    ui.add_space(inset);
    ui.spacing_mut().item_spacing.y = prev_item_spacing_y;
    let panel_bot_y = ui.cursor().min.y;
    let raw_panel_rect = Rect::from_min_max(
        Pos2::new(panel_left, panel_top_y),
        Pos2::new(panel_left + panel_w, panel_bot_y),
    );
    let snapped_panel = snap_rect_to_pixels(raw_panel_rect, ppp);
    let panel_mesh =
        build_feathered_rounded_box_mesh(snapped_panel, RADIUS_PANEL, pal.panel, pal.border, 1.0, ppp);
    if !panel_mesh.indices.is_empty() {
        ui.painter().set(outer_border_slot, egui::Shape::mesh(panel_mesh));
    }
    let inner_panel = snapped_panel.shrink(inset);
    if inner_panel.width() > 0.0 && inner_panel.height() > 0.0 {
        let inner_r = (RADIUS_PANEL - inset).max(0.0);
        let hdr_fill_rect = Rect::from_min_max(
            inner_panel.min,
            Pos2::new(inner_panel.max.x, hdr_y1.min(inner_panel.max.y)),
        );
        if hdr_fill_rect.height() > 0.0 {
            ui.painter().set(
                header_bg_slot,
                egui::Shape::rect_filled(
                    hdr_fill_rect,
                    Rounding {
                        nw: inner_r,
                        ne: inner_r,
                        sw: 0.0,
                        se: 0.0,
                    },
                    pal.panel_alt,
                ),
            );
        }
        let fade_bottom = footer_div_y.min(inner_panel.max.y);
        if show_bottom_fade && fade_bottom > hdr_y1 + 30.0 {
            let fade_rect = Rect::from_min_max(
                Pos2::new(
                    inner_panel.min.x,
                    (fade_bottom - 22.0).max(hdr_y1 + 1.0),
                ),
                Pos2::new(
                    (inner_panel.max.x - 10.0).max(inner_panel.min.x),
                    fade_bottom,
                ),
            );
            paint_bottom_fade_overlay(ui.painter(), fade_rect, pal.panel);
        }
    }
}

struct UnitRowCtx<'a> {
    rep: &'a ScanReport,
    pal: &'a ThemePalette,
    cols: &'a MainTableColumns,
    panel_left: f32,
    panel_w: f32,
    inset: f32,
    ppp: f32,
    has_prop_strong: bool,
    per_drive_mode: Option<char>,
    max_bar_bytes: u64,
    total_mode_bytes: u64,
    total_attributed_bytes: u64,
}

struct UnitRowMut<'a> {
    cached_report: &'a mut Option<CachedReportData>,
    expanded_apps: &'a mut HashSet<String>,
    expanded_families: &'a mut HashSet<String>,
    expanded_row_heights: &'a mut HashMap<String, f32>,
    expanded_family_heights: &'a mut HashMap<String, f32>,
    icon_textures: &'a mut HashMap<String, egui::TextureHandle>,
    open_reverse_dep_id: &'a mut Option<String>,
    cached_rim: &'a mut Option<CachedRimMesh>,
    truncate_cache: &'a mut HashMap<(u64, u32, u16), (String, bool)>,
    path_exists_cache: &'a mut HashMap<String, bool>,
    first_row_size_rect: &'a mut Option<Rect>,
}

#[allow(clippy::too_many_arguments)]
fn draw_unit_ledger_row(
    ui: &mut egui::Ui,
    ctx: &UnitRowCtx<'_>,
    st: &mut UnitRowMut<'_>,
    app_idx: usize,
    indent_x: f32,
    base_bg: Color32,
    record_first_size_rect: bool,
    emit: &mut dyn FnMut(UiAction),
) -> bool {
    let app = &ctx.rep.apps[app_idx];
    let pal = ctx.pal;
    let cols = ctx.cols;
    let panel_left = ctx.panel_left;
    let panel_w = ctx.panel_w;
    let inset = ctx.inset;
    let expanded = st.expanded_apps.contains(&app.id);
    let row_slot = ui.painter().add(egui::Shape::Noop);
    let mut row_clicked_out = false;

    let chev_min_x = cols.chevron.min_x + indent_x;
    let chev_max_x = cols.chevron.max_x + indent_x;
    let name_min_x = (cols.name.min_x + indent_x).min(cols.name.max_x - 80.0);

    let row_outer_rect = ui.allocate_ui_with_layout(
        Vec2::new(panel_w, 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if expanded {
                ui.add_space(3.0);
            }
            let (sum_rect, _) = ui.allocate_exact_size(
                Vec2::new(panel_w, APP_ROW_H + SPACE_4 * 2.0),
                Sense::hover(),
            );
            let y0 = sum_rect.min.y + SPACE_4;
            let y1 = sum_rect.max.y - SPACE_4;

            // 1. Chevron
            {
                let mut c_ui = table_cell_ui(
                    ui,
                    Rect::from_min_max(Pos2::new(chev_min_x, y0), Pos2::new(chev_max_x, y1)),
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                );
                draw_chevron(
                    &mut c_ui,
                    expanded,
                    if expanded { pal.accent_line } else { pal.muted },
                );
            }

            // 2. App name (18x18 real icon or fallback + Strong app name / drawer title + NO-TOUCH/LOCKED chip)
            let mut truncated_name_tip: Option<String> = None;
            {
                let name_rect =
                    Rect::from_min_max(Pos2::new(name_min_x, y0), Pos2::new(cols.name.max_x, y1));
                let icon_rect = Rect::from_center_size(
                    Pos2::new(
                        (name_rect.min.x + APP_ICON_SIZE * 0.5).round(),
                        name_rect.center().y.round(),
                    ),
                    Vec2::splat(APP_ICON_SIZE),
                );
                if let Some(tex_id) = get_or_load_app_icon_tex(ui.ctx(), st.icon_textures, app) {
                    ui.painter().image(
                        tex_id,
                        icon_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    draw_fallback_app_icon(ui.painter(), icon_rect, pal, ctx.ppp);
                }

                let text_rect = Rect::from_min_max(
                    Pos2::new(
                        (icon_rect.max.x + SPACE_8).min(name_rect.max.x),
                        name_rect.min.y,
                    ),
                    name_rect.max,
                );
                let (name_font, fallback_strong) = prop_strong_font(ctx.has_prop_strong, 13.0);
                let mut n_ui = table_cell_ui(ui, text_rect, Layout::left_to_right(Align::Center));
                n_ui.spacing_mut().item_spacing.x = SPACE_8;
                let measured_name_w = if let Some(w) = st
                    .cached_report
                    .as_ref()
                    .and_then(|cr| cr.apps[app_idx].name_w_px)
                {
                    w
                } else {
                    let w = n_ui.fonts(|f| {
                        f.layout_no_wrap(app.display_name.clone(), name_font.clone(), pal.text)
                            .size()
                            .x
                    });
                    if let Some(cr) = st.cached_report.as_mut() {
                        cr.apps[app_idx].name_w_px = Some(w);
                    }
                    w
                };
                if let Some(nt) = app.no_touch {
                    let chip_lbl = nt.label();
                    let chip_text_w = n_ui.fonts(|f| {
                        f.layout_no_wrap(
                            chip_lbl.to_owned(),
                            FontId::new(10.5, FontFamily::Proportional),
                            pal.no_touch_outline,
                        )
                        .size()
                        .x
                    });
                    let chip_reserve = chip_text_w + 25.0 + SPACE_8;
                    let max_name_w = (text_rect.width() - chip_reserve).max(40.0);
                    if measured_name_w > max_name_w + 0.5 {
                        truncated_name_tip = Some(app.display_name.clone());
                    }
                    let text_alloc_w = measured_name_w.min(max_name_w);
                    let (t_rect, _) =
                        n_ui.allocate_exact_size(Vec2::new(text_alloc_w, y1 - y0), Sense::hover());
                    let mut t_ui = table_cell_ui(
                        &mut n_ui,
                        t_rect,
                        Layout::left_to_right(Align::Center),
                    );
                    let mut rt = RichText::new(&app.display_name)
                        .font(name_font)
                        .color(pal.text);
                    if fallback_strong {
                        rt = rt.strong();
                    }
                    t_ui.add(egui::Label::new(rt).truncate().selectable(false));
                    paint_status_chip(&mut n_ui, StatusKind::Danger, chip_lbl, pal);
                } else {
                    if measured_name_w > text_rect.width() + 0.5 {
                        truncated_name_tip = Some(app.display_name.clone());
                    }
                    let mut rt = RichText::new(&app.display_name)
                        .font(name_font)
                        .color(pal.text);
                    if fallback_strong {
                        rt = rt.strong();
                    }
                    n_ui.add(egui::Label::new(rt).truncate().selectable(false));
                }
            }

            // 3. Publisher
            let mut truncated_pub_tip: Option<String> = None;
            if let Some(first_pub) = app.publishers.first() {
                let pub_rect = cols.publisher.rect(y0, y1);
                let pub_font = FontId::new(11.0, FontFamily::Proportional);
                let pub_w = if let Some(w) = st
                    .cached_report
                    .as_ref()
                    .and_then(|cr| cr.apps[app_idx].pub_w_px)
                {
                    w
                } else {
                    let w = ui.fonts(|f| {
                        f.layout_no_wrap(first_pub.clone(), pub_font.clone(), pal.subtle_text)
                            .size()
                            .x
                    });
                    if let Some(cr) = st.cached_report.as_mut() {
                        cr.apps[app_idx].pub_w_px = Some(w);
                    }
                    w
                };
                if pub_w > pub_rect.width() + 0.5 {
                    truncated_pub_tip = Some(first_pub.clone());
                }
                let mut p_ui =
                    table_cell_ui(ui, pub_rect, Layout::left_to_right(Align::Center));
                p_ui.add(
                    egui::Label::new(
                        RichText::new(first_pub)
                            .font(pub_font)
                            .color(pal.subtle_text),
                    )
                    .truncate()
                    .selectable(false),
                );
            }

            // 4. Category
            {
                let cat_rect = cols.category.rect(y0, y1);
                let cat_font = FontId::new(11.0, FontFamily::Proportional);
                let cat_col = if app.category == y_core::model::AppCategory::Uncategorized {
                    pal.subtle_text
                } else {
                    pal.muted
                };
                let (cat_text, _) =
                    truncate_word_boundary_px(ui, app.category.label(), &cat_font, cat_rect.width());
                draw_optical_label_in_rect(
                    ui,
                    cat_rect,
                    &cat_text,
                    cat_font,
                    cat_col,
                    false,
                    false,
                );
            }

            // 5. Growth delta
            let delta_info = if let Some(dl) = ctx.per_drive_mode {
                format_delta_display(app.growth_for_drive(dl), pal)
            } else {
                st.cached_report
                    .as_ref()
                    .map(|cr| cr.apps[app_idx].delta_display.clone())
                    .unwrap_or_else(|| format_delta_display(app.growth, pal))
            };
            let delta_rect = cols.delta.rect(y0, y1);
            match delta_info.kind {
                DeltaCellKind::NewPill => {
                    let mut d_ui =
                        table_cell_ui(ui, delta_rect, Layout::right_to_left(Align::Center));
                    let resp = paint_status_chip(&mut d_ui, StatusKind::Success, "new", pal);
                    if let Some(tip) = delta_info.tip.as_deref() {
                        let _ = resp.on_hover_text(tip);
                    }
                }
                DeltaCellKind::SignedValue { ref text, color } => {
                    let mut d_ui =
                        table_cell_ui(ui, delta_rect, Layout::right_to_left(Align::Center));
                    let d_font = mono_font_for(&d_ui, 11.5, false);
                    let resp = d_ui.add(
                        egui::Label::new(RichText::new(text).font(d_font).color(color))
                            .selectable(false),
                    );
                    if let Some(tip) = delta_info.tip.as_deref() {
                        let _ = resp.on_hover_text(tip);
                    }
                }
                DeltaCellKind::Blank => {
                    if let Some(tip) = delta_info.tip.as_deref() {
                        let hover_id = ui.make_persistent_id(("delta_floor_tip", app_idx));
                        let _ = ui
                            .interact(delta_rect, hover_id, Sense::hover())
                            .on_hover_text(tip);
                    }
                }
            }

            // 6. Primary Size column
            let display_bytes = ctx
                .per_drive_mode
                .map_or(app.total_bytes, |dl| app.bytes_on_drive(dl));
            let size_cell_rect = cols.size.rect(y0, y1);
            if record_first_size_rect {
                *st.first_row_size_rect = Some(size_cell_rect);
            }
            {
                let text_rect = Rect::from_min_max(
                    size_cell_rect.min,
                    Pos2::new(size_cell_rect.max.x, (y1 - 4.0).max(y0 + 14.0)),
                );
                let mut s_ui =
                    table_cell_ui(ui, text_rect, Layout::right_to_left(Align::Center));
                s_ui.spacing_mut().item_spacing.x = 6.0;
                let s_font = mono_font_for(&s_ui, 13.0, false);
                if ctx.per_drive_mode.is_none() {
                    let total_label = st
                        .cached_report
                        .as_ref()
                        .map(|cr| cr.apps[app_idx].total_str.as_str())
                        .unwrap_or("");
                    let lbl_txt = if total_label.is_empty() {
                        format_size_2dec(display_bytes)
                    } else {
                        total_label.to_string()
                    };
                    s_ui.add(
                        egui::Label::new(RichText::new(lbl_txt).font(s_font).color(pal.text))
                            .selectable(false),
                    );
                } else {
                    if app.total_bytes != display_bytes {
                        let comp_font = mono_font_for(&s_ui, 10.5, false);
                        s_ui.add(
                            egui::Label::new(
                                RichText::new(format!("of {}", format_size_2dec(app.total_bytes)))
                                    .font(comp_font)
                                    .color(pal.muted),
                            )
                            .selectable(false),
                        );
                    }
                    s_ui.add(
                        egui::Label::new(
                            RichText::new(format_size_2dec(display_bytes))
                                .font(s_font)
                                .color(pal.text),
                        )
                        .selectable(false),
                    );
                }

                let row_surface = if expanded { pal.panel_alt } else { base_bg };
                let track_rect = Rect::from_min_max(
                    Pos2::new(
                        (size_cell_rect.max.x - PROP_BAR_W).max(size_cell_rect.min.x),
                        y1 - PROP_BAR_H - 1.0,
                    ),
                    Pos2::new(size_cell_rect.max.x, y1 - 1.0),
                );
                let track_col = lerp_color(row_surface, pal.border, 0.55);
                ui.painter()
                    .rect_filled(track_rect, Rounding::ZERO, track_col);

                let ratio = if ctx.max_bar_bytes > 0 {
                    (display_bytes as f64 / ctx.max_bar_bytes as f64).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let fill_w = if display_bytes == 0 {
                    0.0
                } else {
                    (track_rect.width() * ratio as f32).clamp(1.0, track_rect.width())
                };
                if fill_w > 0.0 {
                    let fill_rect = Rect::from_min_size(
                        track_rect.min,
                        Vec2::new(fill_w, track_rect.height()),
                    );
                    paint_horizontal_theme_sweep_bar(
                        ui.painter(),
                        track_rect,
                        fill_rect,
                        1.0,
                        pal,
                    );
                }
            }

            let sum_click_rect = Rect::from_min_max(
                Pos2::new(panel_left + inset + indent_x * 0.5, sum_rect.min.y),
                Pos2::new(panel_left + panel_w - inset, sum_rect.max.y),
            );
            let sum_resp = ui.interact(
                sum_click_rect,
                ui.make_persistent_id(("app_row_toggle", &app.id, indent_x > 0.0)),
                Sense::click(),
            );
            let mut cell_tooltip: Option<String> = None;
            if let Some(pos) = sum_resp.hover_pos() {
                if size_cell_rect.contains(pos) {
                    if let Some(dl) = ctx.per_drive_mode {
                        let share_pct = if ctx.total_mode_bytes > 0 {
                            (display_bytes as f64 / ctx.total_mode_bytes as f64) * 100.0
                        } else {
                            0.0
                        };
                        cell_tooltip = Some(format!(
                            "On {dl}:\\: {} ({:.2}% share) · Global total: {}",
                            format_exact_bytes(display_bytes),
                            share_pct,
                            format_exact_bytes(app.total_bytes)
                        ));
                    } else {
                        cell_tooltip = st
                            .cached_report
                            .as_ref()
                            .map(|cr| cr.apps[app_idx].total_tip.clone())
                            .or_else(|| {
                                let share_pct = if ctx.total_attributed_bytes > 0 {
                                    (app.total_bytes as f64 / ctx.total_attributed_bytes as f64)
                                        * 100.0
                                } else {
                                    0.0
                                };
                                Some(format!(
                                    "{} · {:.2}% share",
                                    format_exact_bytes(app.total_bytes),
                                    share_pct
                                ))
                            });
                    }
                } else if cols.delta.rect(y0, y1).contains(pos) {
                    cell_tooltip = delta_info.tip.clone();
                } else if cols.category.rect(y0, y1).contains(pos) {
                    let basis_lbl = if app.category_basis.is_empty() {
                        "default".to_string()
                    } else {
                        app.category_basis.clone()
                    };
                    cell_tooltip = Some(format!("{} — {}", app.category.label(), basis_lbl));
                } else if cols.publisher.rect(y0, y1).contains(pos) {
                    cell_tooltip = truncated_pub_tip;
                } else if Rect::from_min_max(Pos2::new(name_min_x, y0), Pos2::new(cols.name.max_x, y1))
                    .contains(pos)
                {
                    cell_tooltip = truncated_name_tip;
                }
            }
            let row_hovered = sum_resp.hovered();
            let row_clicked = sum_resp.clicked();
            if let Some(tip) = cell_tooltip {
                let _ = sum_resp.on_hover_text(tip);
            }
            if row_clicked {
                row_clicked_out = true;
                if expanded {
                    st.expanded_apps.remove(&app.id);
                } else {
                    st.expanded_apps.insert(app.id.clone());
                }
            }

            if expanded {
                let sub_x0 = name_min_x;
                let sub_w = (cols.size.max_x - sub_x0).max(200.0);
                let loc_cols = LocationsTableColumns::compute(sub_x0, sub_w);
                let dep_cols = DependenciesTableColumns::compute(sub_x0, sub_w);

                ui.add_space(SPACE_4);
                let (cat_row_rect, _) =
                    ui.allocate_exact_size(Vec2::new(panel_w, 18.0), Sense::hover());
                let mut cat_ui = table_cell_ui(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(sub_x0, cat_row_rect.min.y),
                        Pos2::new(cols.size.max_x, cat_row_rect.max.y),
                    ),
                    Layout::left_to_right(Align::Center),
                );
                cat_ui.spacing_mut().item_spacing.x = SPACE_8;
                draw_truncated_text(
                    &mut cat_ui,
                    "Category",
                    FontId::new(10.5, FontFamily::Proportional),
                    pal.muted,
                    false,
                );
                paint_chip(
                    &mut cat_ui,
                    app.category.label(),
                    ChipStyle {
                        fill: Color32::TRANSPARENT,
                        stroke: pal.border,
                        text: pal.text,
                    },
                    pal.panel_alt,
                );
                if !app.bases.is_empty()
                    && app.bases.iter().all(|b| {
                        matches!(
                            b,
                            AttributionBasis::ContainerRule | AttributionBasis::ExecutableScan
                        )
                    })
                {
                    paint_chip(
                        &mut cat_ui,
                        "inferred",
                        confidence_chip_style(DependencyConfidence::Inferred, pal),
                        pal.panel_alt,
                    );
                }
                let rev_info = st
                    .cached_report
                    .as_ref()
                    .and_then(|cr| cr.rev_by_app_id.get(&app.id).cloned())
                    .or_else(|| {
                        st.cached_report
                            .as_ref()
                            .and_then(|cr| {
                                cr.rev_by_app_token
                                    .get(&normalize_app_token(&app.display_name))
                                    .cloned()
                            })
                            .map(|(id, c)| (Some(id), c))
                    });
                let rev_match = rev_info
                    .as_ref()
                    .and_then(|(opt_id, c)| opt_id.as_ref().map(|id| (id.clone(), *c)));
                let mut used_btn_w = 0.0_f32;
                if let Some((ref rev_id, rev_cnt)) = rev_match {
                    if rev_cnt > 0 {
                        let lbl = if rev_cnt == 1 {
                            "used by 1 app >".to_string()
                        } else {
                            format!("used by {rev_cnt} apps >")
                        };
                        let is_open =
                            st.open_reverse_dep_id.as_deref() == Some(rev_id.as_str());
                        let btn_resp = draw_z_button(
                            &mut cat_ui,
                            &lbl,
                            Vec2::new(0.0, 18.0),
                            10.5,
                            4.0,
                            true,
                            is_open,
                            ButtonVariant::SubtleOutline,
                            pal,
                        );
                        used_btn_w = btn_resp.rect.width() + SPACE_8;
                        if btn_resp
                            .on_hover_text("Show apps depending on this component")
                            .clicked()
                        {
                            *st.open_reverse_dep_id = if is_open {
                                None
                            } else {
                                Some(rev_id.clone())
                            };
                        }
                    }
                }
                let basis_txt = if app.category_basis.is_empty() {
                    "default"
                } else {
                    app.category_basis.as_str()
                };
                let cat_basis_max_w = (sub_w - 190.0 - used_btn_w).max(80.0);
                draw_word_capped_text_cached(
                    &mut cat_ui,
                    basis_txt,
                    FontId::new(10.5, FontFamily::Proportional),
                    pal.subtle_text,
                    cat_basis_max_w,
                    st.truncate_cache,
                );

                for child_name in &app.carved_out_apps {
                    ui.add_space(SPACE_4);
                    let (carve_rect, _) =
                        ui.allocate_exact_size(Vec2::new(panel_w, 18.0), Sense::hover());
                    let carve_cap_w = (cols.size.max_x - sub_x0).max(160.0);
                    let mut carve_ui = table_cell_ui(
                        ui,
                        Rect::from_min_max(
                            Pos2::new(sub_x0, carve_rect.min.y),
                            Pos2::new(cols.size.max_x, carve_rect.max.y),
                        ),
                        Layout::left_to_right(Align::Center),
                    );
                    let note = format!("contains {child_name} — counted under {child_name}");
                    draw_word_capped_text_cached(
                        &mut carve_ui,
                        &note,
                        FontId::new(11.5, FontFamily::Proportional),
                        pal.muted,
                        carve_cap_w,
                        st.truncate_cache,
                    );
                }

                if let Some((opt_rev_id, rev_cnt)) = rev_info {
                    ui.add_space(SPACE_4);
                    let (rev_row_rect, _) =
                        ui.allocate_exact_size(Vec2::new(panel_w, 22.0), Sense::hover());
                    let mut rev_ui = table_cell_ui(
                        ui,
                        Rect::from_min_max(
                            Pos2::new(sub_x0, rev_row_rect.min.y),
                            Pos2::new(cols.size.max_x, rev_row_rect.max.y),
                        ),
                        Layout::left_to_right(Align::Center),
                    );
                    rev_ui.spacing_mut().item_spacing.x = SPACE_8;
                    let summary_lbl = match rev_cnt {
                        0 => "Used by 0 installed apps on this scan (shared runtime)".to_string(),
                        1 => "Used by 1 app".to_string(),
                        n => format!("Used by {n} apps"),
                    };
                    draw_truncated_text(
                        &mut rev_ui,
                        &summary_lbl,
                        FontId::new(11.5, FontFamily::Proportional),
                        if rev_cnt > 0 { pal.text } else { pal.muted },
                        false,
                    );
                    if rev_cnt > 0 {
                        if let Some(rev_id) = opt_rev_id {
                            let is_open =
                                st.open_reverse_dep_id.as_deref() == Some(rev_id.as_str());
                            let inspect_btn = draw_z_button(
                                &mut rev_ui,
                                "Inspect consumers >",
                                Vec2::new(0.0, 20.0),
                                11.5,
                                RADIUS_PILL,
                                true,
                                is_open,
                                ButtonVariant::SecondaryOutline,
                                pal,
                            );
                            if inspect_btn
                                .on_hover_text("Open full list of apps using this shared runtime")
                                .clicked()
                            {
                                *st.open_reverse_dep_id = if is_open {
                                    None
                                } else {
                                    Some(rev_id)
                                };
                            }
                        }
                    }
                }

                // Sub-table 1: Locations (Zone · Path · Basis · Size)
                ui.add_space(SPACE_4);
                let (loc_title_rect, _) =
                    ui.allocate_exact_size(Vec2::new(panel_w, 18.0), Sense::hover());
                let mut lt_ui = table_cell_ui(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(sub_x0, loc_title_rect.min.y),
                        Pos2::new(cols.size.max_x, loc_title_rect.max.y),
                    ),
                    Layout::left_to_right(Align::Center),
                );
                lt_ui.label(
                    RichText::new(format!("Locations ({})", app.items.len()))
                        .size(11.0)
                        .color(pal.muted),
                );

                let (loc_hdr_rect, _) =
                    ui.allocate_exact_size(Vec2::new(panel_w, SUB_HEADER_H), Sense::hover());
                let ly0 = loc_hdr_rect.min.y;
                let ly1 = loc_hdr_rect.max.y;
                draw_static_col_header(ui, loc_cols.zone.rect(ly0, ly1), "Zone", 10.5, false, pal);
                draw_static_col_header(ui, loc_cols.path.rect(ly0, ly1), "Path", 10.5, false, pal);
                draw_static_col_header(ui, loc_cols.basis.rect(ly0, ly1), "Basis", 10.5, false, pal);
                draw_static_col_header(ui, loc_cols.size.rect(ly0, ly1), "Size", 10.5, true, pal);
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::new(sub_x0, ly1 - 1.0), Vec2::new(sub_w, 1.0)),
                    Rounding::ZERO,
                    pal.border,
                );

                for (loc_i, item) in app.items.iter().enumerate() {
                    if loc_i > 0 {
                        let div_y = ui.cursor().min.y;
                        ui.painter().rect_filled(
                            Rect::from_min_size(Pos2::new(sub_x0, div_y), Vec2::new(sub_w, 1.0)),
                            Rounding::ZERO,
                            pal.border,
                        );
                        ui.add_space(1.0);
                    }
                    let loc_hover_slot = ui.painter().add(egui::Shape::Noop);
                    let (l_rect, _) =
                        ui.allocate_exact_size(Vec2::new(panel_w, SUB_LINE1_H + 2.0), Sense::hover());
                    let ry0 = l_rect.min.y;
                    let ry1 = l_rect.max.y;
                    let sub_row_rect = Rect::from_min_max(
                        Pos2::new(sub_x0 - SPACE_4, ry0),
                        Pos2::new(cols.size.max_x + SPACE_4, ry1),
                    );
                    if ui.rect_contains_pointer(sub_row_rect) {
                        ui.painter().set(
                            loc_hover_slot,
                            egui::Shape::rect_filled(sub_row_rect, Rounding::ZERO, pal.widget_hover),
                        );
                    }

                    if let Some(zone) = item.zone {
                        let mut z_ui = table_cell_ui(
                            ui,
                            loc_cols.zone.rect(ry0, ry1),
                            Layout::left_to_right(Align::Center),
                        );
                        paint_zone_chip(&mut z_ui, zone, pal, pal.panel_alt);
                    }

                    {
                        let p_rect = loc_cols.path.rect(ry0, ry1);
                        let path_font = FontId::new(11.5, FontFamily::Monospace);
                        if let Some(qual) = item.qualifier {
                            let q_lbl = match qual {
                                LocationQualifier::RoamingConfig => "Roaming config",
                                LocationQualifier::MachineShared => "ProgramData · shared",
                                LocationQualifier::InstallLooseFiles => {
                                    "loose files in the install folder"
                                }
                                LocationQualifier::LooseFiles => "loose files",
                            };
                            let mut p_ui = table_cell_ui(
                                ui,
                                p_rect,
                                Layout::left_to_right(Align::Center),
                            );
                            p_ui.spacing_mut().item_spacing.x = SPACE_8;
                            let q_text_w = p_ui.fonts(|f| {
                                f.layout_no_wrap(
                                    q_lbl.to_owned(),
                                    FontId::new(10.5, FontFamily::Proportional),
                                    pal.accent_pill_text,
                                )
                                .size()
                                .x
                            });
                            let q_reserve = q_text_w + 14.0 + SPACE_8;
                            let max_p_w = (p_rect.width() - q_reserve).max(80.0);
                            let measured_p_w = p_ui.fonts(|f| {
                                f.layout_no_wrap(item.path.clone(), path_font.clone(), pal.text)
                                    .size()
                                    .x
                            });
                            let (t_rect, _) = p_ui.allocate_exact_size(
                                Vec2::new(measured_p_w.min(max_p_w), ry1 - ry0),
                                Sense::hover(),
                            );
                            let mut pt_ui = table_cell_ui(
                                &mut p_ui,
                                t_rect,
                                Layout::left_to_right(Align::Center),
                            );
                            draw_word_capped_text_cached(
                                &mut pt_ui,
                                &item.path,
                                path_font,
                                pal.text,
                                max_p_w,
                                st.truncate_cache,
                            );
                            paint_chip(&mut p_ui, q_lbl, qualifier_chip_style(pal), pal.panel_alt);
                        } else {
                            let mut p_ui = table_cell_ui(
                                ui,
                                p_rect,
                                Layout::left_to_right(Align::Center),
                            );
                            draw_word_capped_text_cached(
                                &mut p_ui,
                                &item.path,
                                path_font,
                                pal.text,
                                p_rect.width(),
                                st.truncate_cache,
                            );
                        }
                    }

                    {
                        let b_rect = loc_cols.basis.rect(ry0, ry1);
                        let mut b_ui =
                            table_cell_ui(ui, b_rect, Layout::left_to_right(Align::Center));
                        let basis_str = item.basis_label();
                        draw_word_capped_text_cached(
                            &mut b_ui,
                            basis_str,
                            FontId::new(10.5, FontFamily::Proportional),
                            pal.subtle_text,
                            b_rect.width(),
                            st.truncate_cache,
                        );
                    }

                    {
                        let mut a_ui = table_cell_ui(
                            ui,
                            loc_cols.actions.rect(ry0, ry1),
                            Layout::right_to_left(Align::Center),
                        );
                        a_ui.spacing_mut().item_spacing.x = SPACE_4;
                        let (search_query, search_url) =
                            crate::unattributed_assist::build_unattributed_search_url(&item.path);
                        let web_resp = draw_z_button(
                            &mut a_ui,
                            "Web search",
                            Vec2::new(0.0, 18.0),
                            10.5,
                            RADIUS_PILL,
                            true,
                            false,
                            ButtonVariant::SubtleOutline,
                            pal,
                        );
                        if web_resp.clicked() {
                            emit(UiAction::OpenWebSearch(search_url));
                        }
                        let _ = web_resp
                            .on_hover_text(format!("Web search (Google): {search_query}"));

                        let exists = *st
                            .path_exists_cache
                            .entry(item.path.clone())
                            .or_insert_with(|| std::path::Path::new(&item.path).exists());
                        let loc_resp = draw_z_button(
                            &mut a_ui,
                            "Locate",
                            Vec2::new(0.0, 18.0),
                            10.5,
                            RADIUS_PILL,
                            exists,
                            false,
                            ButtonVariant::SubtleOutline,
                            pal,
                        );
                        if exists {
                            if loc_resp.clicked() {
                                emit(UiAction::LocateInExplorer(item.path.clone()));
                            }
                            let _ = loc_resp.on_hover_text(
                                crate::unattributed_assist::build_explorer_select_command_line(
                                    &item.path,
                                ),
                            );
                        } else {
                            let _ = loc_resp.on_hover_text("no longer on disk");
                        }
                    }

                    {
                        let mut s_ui = table_cell_ui(
                            ui,
                            loc_cols.size.rect(ry0, ry1),
                            Layout::right_to_left(Align::Center),
                        );
                        draw_right_aligned_size(&mut s_ui, item.bytes, 11.5, pal.text, false);
                    }
                }

                ui.add_space(SPACE_8);
                let sec_div_y = ui.cursor().min.y;
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::new(sub_x0, sec_div_y), Vec2::new(sub_w, 1.0)),
                    Rounding::ZERO,
                    pal.border,
                );
                ui.add_space(SPACE_8);

                // Sub-table 2: Dependencies
                let (dep_title_rect, _) =
                    ui.allocate_exact_size(Vec2::new(panel_w, 18.0), Sense::hover());
                let mut dt_ui = table_cell_ui(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(sub_x0, dep_title_rect.min.y),
                        Pos2::new(cols.size.max_x, dep_title_rect.max.y),
                    ),
                    Layout::left_to_right(Align::Center),
                );
                dt_ui.spacing_mut().item_spacing.x = SPACE_8;
                dt_ui.label(RichText::new("Dependencies").size(11.0).color(pal.muted));
                dt_ui.label(
                    RichText::new(format!(
                        "{} · shared sizes excluded from app total",
                        app.dependencies.len()
                    ))
                    .size(10.5)
                    .color(pal.subtle_text),
                );

                if app.dependencies.is_empty() {
                    let (empty_r, _) =
                        ui.allocate_exact_size(Vec2::new(panel_w, SUB_LINE1_H), Sense::hover());
                    let mut e_ui = table_cell_ui(
                        ui,
                        Rect::from_min_max(
                            Pos2::new(sub_x0, empty_r.min.y),
                            Pos2::new(cols.size.max_x, empty_r.max.y),
                        ),
                        Layout::left_to_right(Align::Center),
                    );
                    e_ui.label(
                        RichText::new(NO_KNOWN_DEPENDENCIES_LABEL)
                            .size(11.5)
                            .color(pal.subtle_text),
                    );
                } else {
                    let (dep_hdr_rect, _) =
                        ui.allocate_exact_size(Vec2::new(panel_w, SUB_HEADER_H), Sense::hover());
                    let dy0 = dep_hdr_rect.min.y;
                    let dy1 = dep_hdr_rect.max.y;
                    draw_static_col_header(
                        ui,
                        dep_cols.name.rect(dy0, dy1),
                        "Dependency name",
                        10.5,
                        false,
                        pal,
                    );
                    draw_static_col_header(
                        ui,
                        dep_cols.kind.rect(dy0, dy1),
                        "Kind",
                        10.5,
                        false,
                        pal,
                    );
                    draw_static_col_header(
                        ui,
                        dep_cols.confidence.rect(dy0, dy1),
                        "Confidence",
                        10.5,
                        false,
                        pal,
                    );
                    draw_static_col_header(
                        ui,
                        dep_cols.used_by.rect(dy0, dy1),
                        "Used by",
                        10.5,
                        false,
                        pal,
                    );
                    draw_static_col_header(
                        ui,
                        dep_cols.own_size.rect(dy0, dy1),
                        "Own size",
                        10.5,
                        true,
                        pal,
                    );
                    paint_solid_px_rect(
                        ui.painter(),
                        Rect::from_min_size(Pos2::new(sub_x0, dy1 - 1.0), Vec2::new(sub_w, 1.0)),
                        pal.border,
                    );

                    for (dep_i, dep) in app.dependencies.iter().enumerate() {
                        if dep_i > 0 {
                            let div_y = ui.cursor().min.y;
                            paint_solid_px_rect(
                                ui.painter(),
                                Rect::from_min_size(
                                    Pos2::new(sub_x0, div_y),
                                    Vec2::new(sub_w, 1.0),
                                ),
                                pal.border,
                            );
                            ui.add_space(2.0);
                        }
                        let rev_n = st
                            .cached_report
                            .as_ref()
                            .and_then(|cr| cr.rev_counts.get(&dep.target_id).copied())
                            .unwrap_or(0);
                        let is_open_rev =
                            st.open_reverse_dep_id.as_deref() == Some(dep.target_id.as_str());

                        let (d_rect, _) = ui.allocate_exact_size(
                            Vec2::new(panel_w, SUB_LINE1_H + 2.0),
                            Sense::hover(),
                        );
                        let r1y0 = d_rect.min.y;
                        let r1y1 = d_rect.max.y;

                        {
                            let dn_rect = dep_cols.name.rect(r1y0, r1y1);
                            let mut dn_ui = table_cell_ui(
                                ui,
                                dn_rect,
                                Layout::left_to_right(Align::Center),
                            );
                            draw_word_capped_text_cached(
                                &mut dn_ui,
                                &dep.target_name,
                                FontId::new(12.0, FontFamily::Proportional),
                                pal.text,
                                dn_rect.width(),
                                st.truncate_cache,
                            );
                        }

                        {
                            let dk_rect = dep_cols.kind.rect(r1y0, r1y1);
                            let mut dk_ui = table_cell_ui(
                                ui,
                                dk_rect,
                                Layout::left_to_right(Align::Center),
                            );
                            draw_word_capped_text_cached(
                                &mut dk_ui,
                                dep.kind.label(),
                                FontId::new(11.0, FontFamily::Proportional),
                                pal.subtle_text,
                                dk_rect.width(),
                                st.truncate_cache,
                            );
                        }

                        {
                            let mut dc_ui = table_cell_ui(
                                ui,
                                dep_cols.confidence.rect(r1y0, r1y1),
                                Layout::left_to_right(Align::Center),
                            );
                            dc_ui.spacing_mut().item_spacing.x = SPACE_4;
                            paint_chip(
                                &mut dc_ui,
                                dep.confidence.label(),
                                confidence_chip_style(dep.confidence, pal),
                                pal.panel_alt,
                            );
                            if dep.shared {
                                paint_chip(
                                    &mut dc_ui,
                                    "shared",
                                    ChipStyle {
                                        fill: Color32::TRANSPARENT,
                                        stroke: pal.border,
                                        text: pal.muted,
                                    },
                                    pal.panel_alt,
                                );
                            }
                            if !dep.present {
                                paint_status_chip(&mut dc_ui, StatusKind::Danger, "missing", pal);
                            }
                        }

                        {
                            let mut du_ui = table_cell_ui(
                                ui,
                                dep_cols.used_by.rect(r1y0, r1y1),
                                Layout::left_to_right(Align::Center),
                            );
                            if rev_n > 0 {
                                let lbl = if rev_n == 1 {
                                    "used by 1 app >".to_string()
                                } else {
                                    format!("used by {rev_n} apps >")
                                };
                                if draw_z_button(
                                    &mut du_ui,
                                    &lbl,
                                    Vec2::new(0.0, 18.0),
                                    10.5,
                                    4.0,
                                    true,
                                    is_open_rev,
                                    ButtonVariant::SubtleOutline,
                                    pal,
                                )
                                .on_hover_text(
                                    "Open reverse dependency list (\"What else uses this?\")",
                                )
                                .clicked()
                                {
                                    *st.open_reverse_dep_id = if is_open_rev {
                                        None
                                    } else {
                                        Some(dep.target_id.clone())
                                    };
                                }
                            } else {
                                du_ui.label(
                                    RichText::new("—").size(11.0).color(pal.subtle_text),
                                );
                            }
                        }

                        {
                            let mut ds_ui = table_cell_ui(
                                ui,
                                dep_cols.own_size.rect(r1y0, r1y1),
                                Layout::right_to_left(Align::Center),
                            );
                            if let Some(own) = dep.own_size_bytes {
                                draw_right_aligned_size(&mut ds_ui, own, 11.5, pal.muted, false);
                            } else {
                                ds_ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new("—")
                                                .size(11.5)
                                                .color(pal.subtle_text),
                                        )
                                        .selectable(false),
                                    )
                                    .on_hover_text(
                                        "No separate on-disk folder (OS runtime or unmounted drive)",
                                    );
                            }
                        }

                        let (basis_r, _) = ui.allocate_exact_size(
                            Vec2::new(panel_w, SUB_LINE2_H + 2.0),
                            Sense::hover(),
                        );
                        let basis_cap_w = (sub_w - SIZE_COL_W - SPACE_16).clamp(180.0, 560.0);
                        let mut db_ui = table_cell_ui(
                            ui,
                            Rect::from_min_max(
                                Pos2::new(sub_x0, basis_r.min.y),
                                Pos2::new(
                                    (sub_x0 + basis_cap_w).min(cols.size.max_x),
                                    basis_r.max.y,
                                ),
                            ),
                            Layout::left_to_right(Align::Center),
                        );
                        draw_word_capped_text_cached(
                            &mut db_ui,
                            &dep.basis,
                            FontId::new(10.5, FontFamily::Proportional),
                            pal.subtle_text,
                            basis_cap_w,
                            st.truncate_cache,
                        );
                    }
                }
                ui.add_space(SPACE_8 + 3.0);
            }
            row_hovered
        },
    );

    let r_full = row_outer_rect.response.rect;
    if expanded {
        st.expanded_row_heights.insert(app.id.clone(), r_full.height());
    }
    let inner_row_rect = Rect::from_min_max(
        Pos2::new(panel_left + inset + indent_x * 0.5, r_full.min.y),
        Pos2::new(panel_left + panel_w - inset, r_full.max.y),
    );
    if expanded {
        let drawer_rect = inner_row_rect.shrink2(Vec2::new(2.0, 1.0));
        let (fill_poly, rim_mesh) = translate_cached_rim_for_rect(
            drawer_rect,
            RADIUS_PANEL,
            ctx.ppp,
            pal,
            st.cached_rim,
        );
        ui.painter().set(
            row_slot,
            egui::Shape::convex_polygon(fill_poly, pal.panel_alt, Stroke::NONE),
        );
        ui.painter().add(egui::Shape::mesh(rim_mesh));
    } else if row_outer_rect.inner {
        let mut m = egui::epaint::Mesh::default();
        add_solid_rect_quad(&mut m, inner_row_rect, pal.widget_hover);
        ui.painter().set(row_slot, egui::Shape::mesh(m));
    }
    row_clicked_out
}

fn draw_family_ledger_row(
    ui: &mut egui::Ui,
    ctx: &UnitRowCtx<'_>,
    st: &mut UnitRowMut<'_>,
    fam: &AppFamily,
    record_first_size_rect: bool,
    emit: &mut dyn FnMut(UiAction),
) {
    let pal = ctx.pal;
    let cols = ctx.cols;
    let panel_left = ctx.panel_left;
    let panel_w = ctx.panel_w;
    let inset = ctx.inset;
    let fam_expanded = st.expanded_families.contains(&fam.id);
    let bg_slot = ui.painter().add(egui::Shape::Noop);
    let header_hover_slot = ui.painter().add(egui::Shape::Noop);

    let lead_app = &ctx.rep.apps[fam.member_indices[0]];
    let basis_lbl = fam.basis_label();
    let count_lbl = format!("{} apps", fam.member_indices.len());

    let outer = ui.allocate_ui_with_layout(
        Vec2::new(panel_w, 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let (sum_rect, _) = ui.allocate_exact_size(
                Vec2::new(panel_w, APP_ROW_H + SPACE_4 * 2.0),
                Sense::hover(),
            );
            let y0 = sum_rect.min.y + SPACE_4;
            let y1 = sum_rect.max.y - SPACE_4;

            // 1. Chevron
            {
                let mut c_ui = table_cell_ui(
                    ui,
                    cols.chevron.rect(y0, y1),
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                );
                draw_chevron(
                    &mut c_ui,
                    fam_expanded,
                    if fam_expanded { pal.accent_line } else { pal.muted },
                );
            }

            // 2. Family name + lead icon + member count chip + grouping basis chip
            let mut truncated_name_tip: Option<String> = None;
            {
                let name_rect = cols.name.rect(y0, y1);
                let icon_rect = Rect::from_center_size(
                    Pos2::new(
                        (name_rect.min.x + APP_ICON_SIZE * 0.5).round(),
                        name_rect.center().y.round(),
                    ),
                    Vec2::splat(APP_ICON_SIZE),
                );
                if let Some(tex_id) =
                    get_or_load_app_icon_tex(ui.ctx(), st.icon_textures, lead_app)
                {
                    ui.painter().image(
                        tex_id,
                        icon_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    draw_fallback_app_icon(ui.painter(), icon_rect, pal, ctx.ppp);
                }

                let text_rect = Rect::from_min_max(
                    Pos2::new(
                        (icon_rect.max.x + SPACE_8).min(name_rect.max.x),
                        name_rect.min.y,
                    ),
                    name_rect.max,
                );
                let (name_font, fallback_strong) = prop_strong_font(ctx.has_prop_strong, 13.0);
                let mut n_ui = table_cell_ui(ui, text_rect, Layout::left_to_right(Align::Center));
                n_ui.spacing_mut().item_spacing.x = SPACE_8;

                let chip_font = FontId::new(10.5, FontFamily::Proportional);
                let count_w = n_ui.fonts(|f| {
                    f.layout_no_wrap(count_lbl.clone(), chip_font.clone(), pal.muted)
                        .size()
                        .x
                }) + 16.0;
                let basis_w = n_ui.fonts(|f| {
                    f.layout_no_wrap(basis_lbl.clone(), chip_font.clone(), pal.accent_pill_text)
                        .size()
                        .x
                }) + 16.0;
                let measured_name_w = n_ui.fonts(|f| {
                    f.layout_no_wrap(fam.display_name.clone(), name_font.clone(), pal.text)
                        .size()
                        .x
                });
                let show_basis_chip = text_rect.width() > count_w + basis_w + 110.0;
                let reserve_w = count_w
                    + SPACE_8
                    + if show_basis_chip {
                        basis_w + SPACE_8
                    } else {
                        0.0
                    };
                let max_name_w = (text_rect.width() - reserve_w).max(48.0);
                if measured_name_w > max_name_w + 0.5 {
                    truncated_name_tip = Some(fam.display_name.clone());
                }
                let text_alloc_w = measured_name_w.min(max_name_w);
                let (t_rect, _) =
                    n_ui.allocate_exact_size(Vec2::new(text_alloc_w, y1 - y0), Sense::hover());
                let mut t_ui =
                    table_cell_ui(&mut n_ui, t_rect, Layout::left_to_right(Align::Center));
                let mut rt = RichText::new(&fam.display_name)
                    .font(name_font)
                    .color(pal.text);
                if fallback_strong {
                    rt = rt.strong();
                }
                t_ui.add(egui::Label::new(rt).truncate().selectable(false));

                let chip_bg = if fam_expanded { pal.panel_alt } else { pal.panel };
                paint_chip(
                    &mut n_ui,
                    &count_lbl,
                    ChipStyle {
                        fill: Color32::TRANSPARENT,
                        stroke: pal.border,
                        text: pal.muted,
                    },
                    chip_bg,
                );
                if show_basis_chip {
                    paint_chip(&mut n_ui, &basis_lbl, qualifier_chip_style(pal), chip_bg);
                }
            }

            // 3. Publisher
            let mut truncated_pub_tip: Option<String> = None;
            if !fam.publisher.is_empty() {
                let pub_rect = cols.publisher.rect(y0, y1);
                let pub_font = FontId::new(11.0, FontFamily::Proportional);
                let pub_w = ui.fonts(|f| {
                    f.layout_no_wrap(fam.publisher.clone(), pub_font.clone(), pal.subtle_text)
                        .size()
                        .x
                });
                if pub_w > pub_rect.width() + 0.5 {
                    truncated_pub_tip = Some(fam.publisher.clone());
                }
                let mut p_ui = table_cell_ui(ui, pub_rect, Layout::left_to_right(Align::Center));
                p_ui.add(
                    egui::Label::new(
                        RichText::new(&fam.publisher)
                            .font(pub_font)
                            .color(pal.subtle_text),
                    )
                    .truncate()
                    .selectable(false),
                );
            }

            // 4. Category: shown ONLY if all members share it, else blank
            if let Some(shared_cat) = fam.shared_category {
                let cat_rect = cols.category.rect(y0, y1);
                let cat_font = FontId::new(11.0, FontFamily::Proportional);
                let cat_col = if shared_cat == y_core::model::AppCategory::Uncategorized {
                    pal.subtle_text
                } else {
                    pal.muted
                };
                let (cat_text, _) =
                    truncate_word_boundary_px(ui, shared_cat.label(), &cat_font, cat_rect.width());
                draw_optical_label_in_rect(ui, cat_rect, &cat_text, cat_font, cat_col, false, false);
            }

            // 5. Growth delta (sum across members)
            let delta_info =
                format_delta_display(fam.effective_growth(&ctx.rep.apps, ctx.per_drive_mode), pal);
            let delta_rect = cols.delta.rect(y0, y1);
            match delta_info.kind {
                DeltaCellKind::NewPill => {
                    let mut d_ui =
                        table_cell_ui(ui, delta_rect, Layout::right_to_left(Align::Center));
                    let resp = paint_status_chip(&mut d_ui, StatusKind::Success, "new", pal);
                    if let Some(tip) = delta_info.tip.as_deref() {
                        let _ = resp.on_hover_text(tip);
                    }
                }
                DeltaCellKind::SignedValue { ref text, color } => {
                    let mut d_ui =
                        table_cell_ui(ui, delta_rect, Layout::right_to_left(Align::Center));
                    let d_font = mono_font_for(&d_ui, 11.5, false);
                    let resp = d_ui.add(
                        egui::Label::new(RichText::new(text).font(d_font).color(color))
                            .selectable(false),
                    );
                    if let Some(tip) = delta_info.tip.as_deref() {
                        let _ = resp.on_hover_text(tip);
                    }
                }
                DeltaCellKind::Blank => {
                    if let Some(tip) = delta_info.tip.as_deref() {
                        let hover_id = ui.make_persistent_id(("fam_delta_tip", &fam.id));
                        let _ = ui
                            .interact(delta_rect, hover_id, Sense::hover())
                            .on_hover_text(tip);
                    }
                }
            }

            // 6. Family Total Size (sum of member unit totals)
            let display_bytes = fam.effective_bytes(&ctx.rep.apps, ctx.per_drive_mode);
            let size_cell_rect = cols.size.rect(y0, y1);
            if record_first_size_rect {
                *st.first_row_size_rect = Some(size_cell_rect);
            }
            {
                let text_rect = Rect::from_min_max(
                    size_cell_rect.min,
                    Pos2::new(size_cell_rect.max.x, (y1 - 4.0).max(y0 + 14.0)),
                );
                let mut s_ui =
                    table_cell_ui(ui, text_rect, Layout::right_to_left(Align::Center));
                s_ui.spacing_mut().item_spacing.x = 6.0;
                let s_font = mono_font_for(&s_ui, 13.0, false);
                if ctx.per_drive_mode.is_some() && fam.total_bytes != display_bytes {
                    let comp_font = mono_font_for(&s_ui, 10.5, false);
                    s_ui.add(
                        egui::Label::new(
                            RichText::new(format!("of {}", format_size_2dec(fam.total_bytes)))
                                .font(comp_font)
                                .color(pal.muted),
                        )
                        .selectable(false),
                    );
                }
                s_ui.add(
                    egui::Label::new(
                        RichText::new(format_size_2dec(display_bytes))
                            .font(s_font)
                            .color(pal.text),
                    )
                    .selectable(false),
                );

                let row_surface = if fam_expanded { pal.panel_alt } else { pal.panel };
                let track_rect = Rect::from_min_max(
                    Pos2::new(
                        (size_cell_rect.max.x - PROP_BAR_W).max(size_cell_rect.min.x),
                        y1 - PROP_BAR_H - 1.0,
                    ),
                    Pos2::new(size_cell_rect.max.x, y1 - 1.0),
                );
                let track_col = lerp_color(row_surface, pal.border, 0.55);
                ui.painter()
                    .rect_filled(track_rect, Rounding::ZERO, track_col);

                let ratio = if ctx.max_bar_bytes > 0 {
                    (display_bytes as f64 / ctx.max_bar_bytes as f64).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let fill_w = if display_bytes == 0 {
                    0.0
                } else {
                    (track_rect.width() * ratio as f32).clamp(1.0, track_rect.width())
                };
                if fill_w > 0.0 {
                    let fill_rect = Rect::from_min_size(
                        track_rect.min,
                        Vec2::new(fill_w, track_rect.height()),
                    );
                    paint_horizontal_theme_sweep_bar(
                        ui.painter(),
                        track_rect,
                        fill_rect,
                        1.0,
                        pal,
                    );
                }
            }

            let sum_click_rect = Rect::from_min_max(
                Pos2::new(panel_left + inset, sum_rect.min.y),
                Pos2::new(panel_left + panel_w - inset, sum_rect.max.y),
            );
            let sum_resp = ui.interact(
                sum_click_rect,
                ui.make_persistent_id(("family_row_toggle", &fam.id)),
                Sense::click(),
            );
            let mut cell_tooltip: Option<String> = None;
            if let Some(pos) = sum_resp.hover_pos() {
                if size_cell_rect.contains(pos) {
                    if let Some(dl) = ctx.per_drive_mode {
                        let share_pct = if ctx.total_mode_bytes > 0 {
                            (display_bytes as f64 / ctx.total_mode_bytes as f64) * 100.0
                        } else {
                            0.0
                        };
                        cell_tooltip = Some(format!(
                            "On {dl}:\\: {} ({:.2}% share) · Sum of {} units · Global total: {}",
                            format_exact_bytes(display_bytes),
                            share_pct,
                            fam.member_indices.len(),
                            format_exact_bytes(fam.total_bytes)
                        ));
                    } else {
                        let share_pct = if ctx.total_attributed_bytes > 0 {
                            (fam.total_bytes as f64 / ctx.total_attributed_bytes as f64) * 100.0
                        } else {
                            0.0
                        };
                        cell_tooltip = Some(format!(
                            "{} · {:.2}% share (sum of {} units)",
                            format_exact_bytes(fam.total_bytes),
                            share_pct,
                            fam.member_indices.len()
                        ));
                    }
                } else if cols.delta.rect(y0, y1).contains(pos) {
                    cell_tooltip = delta_info.tip.clone();
                } else if cols.category.rect(y0, y1).contains(pos) {
                    cell_tooltip = fam
                        .shared_category
                        .map(|c| format!("{} (shared by all {} members)", c.label(), fam.member_indices.len()));
                } else if cols.publisher.rect(y0, y1).contains(pos) {
                    cell_tooltip = truncated_pub_tip;
                } else if cols.name.rect(y0, y1).contains(pos) {
                    cell_tooltip = Some(match truncated_name_tip {
                        Some(name_tip) => format!("{name_tip} — Basis: {}", fam.basis_detail),
                        None => format!("Family basis: {}", fam.basis_detail),
                    });
                }
            }
            let header_hovered = sum_resp.hovered();
            let fam_clicked = sum_resp.clicked();
            if header_hovered {
                let mut m = egui::epaint::Mesh::default();
                add_solid_rect_quad(&mut m, sum_click_rect, pal.widget_hover);
                ui.painter().set(header_hover_slot, egui::Shape::mesh(m));
            }
            if let Some(tip) = cell_tooltip {
                let _ = sum_resp.on_hover_text(tip);
            }
            if fam_clicked {
                if fam_expanded {
                    st.expanded_families.remove(&fam.id);
                } else {
                    st.expanded_families.insert(fam.id.clone());
                }
            }

            if fam_expanded {
                let sub_x0 = (cols.name.min_x + SPACE_16).min(cols.name.max_x - 80.0);
                let sub_w = (cols.size.max_x - sub_x0).max(160.0);
                let (basis_rect, _) =
                    ui.allocate_exact_size(Vec2::new(panel_w, 18.0), Sense::hover());
                let mut b_ui = table_cell_ui(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(sub_x0, basis_rect.min.y),
                        Pos2::new(cols.size.max_x, basis_rect.max.y),
                    ),
                    Layout::left_to_right(Align::Center),
                );
                let basis_line = format!("Family basis: {}", fam.basis_detail);
                draw_word_capped_text_cached(
                    &mut b_ui,
                    &basis_line,
                    FontId::new(10.5, FontFamily::Proportional),
                    pal.subtle_text,
                    sub_w,
                    st.truncate_cache,
                );

                let sep_top_y = ui.cursor().min.y;
                paint_solid_px_rect(
                    ui.painter(),
                    Rect::from_min_size(Pos2::new(sub_x0, sep_top_y), Vec2::new(sub_w, 1.0)),
                    pal.border,
                );

                for (mi, &m_idx) in fam.member_indices.iter().enumerate() {
                    let member_toggled = draw_unit_ledger_row(
                        ui,
                        ctx,
                        st,
                        m_idx,
                        SPACE_16,
                        pal.panel_alt,
                        false,
                        emit,
                    );
                    if member_toggled {
                        st.expanded_family_heights.remove(&fam.id);
                    }
                    let curr_m_exp = st.expanded_apps.contains(&ctx.rep.apps[m_idx].id);
                    let next_m_exp = fam
                        .member_indices
                        .get(mi + 1)
                        .is_some_and(|&ni| st.expanded_apps.contains(&ctx.rep.apps[ni].id));
                    if !curr_m_exp && mi + 1 < fam.member_indices.len() && !next_m_exp {
                        let sep_y = (ui.cursor().min.y - 1.0).round();
                        paint_solid_px_rect(
                            ui.painter(),
                            Rect::from_min_size(Pos2::new(sub_x0, sep_y), Vec2::new(sub_w, 1.0)),
                            pal.border,
                        );
                    }
                }
                ui.add_space(SPACE_4);
            }
        },
    );

    let r_full = outer.response.rect;
    if fam_expanded {
        st.expanded_family_heights
            .insert(fam.id.clone(), r_full.height());
        let inner_rect = Rect::from_min_max(
            Pos2::new(panel_left + inset, r_full.min.y),
            Pos2::new(panel_left + panel_w - inset, r_full.max.y),
        );
        let mut bg_mesh = egui::epaint::Mesh::default();
        add_solid_rect_quad(&mut bg_mesh, inner_rect, pal.panel_alt);
        ui.painter().set(bg_slot, egui::Shape::mesh(bg_mesh));

        let accent_bar = Rect::from_min_max(
            inner_rect.min,
            Pos2::new((inner_rect.min.x + 3.0).min(inner_rect.max.x), inner_rect.max.y),
        );
        paint_solid_px_rect(ui.painter(), accent_bar, pal.accent_line);
        paint_solid_px_rect(
            ui.painter(),
            Rect::from_min_size(inner_rect.min, Vec2::new(inner_rect.width(), 1.0)),
            pal.border,
        );
        paint_solid_px_rect(
            ui.painter(),
            Rect::from_min_size(
                Pos2::new(inner_rect.min.x, (inner_rect.max.y - 1.0).round()),
                Vec2::new(inner_rect.width(), 1.0),
            ),
            pal.border,
        );
    }
}

fn draw_reverse_dependency_overlay(
    ctx: &egui::Context,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    target_id: &str,
    pal: &ThemePalette,
) {
    let Some(rev) = rep
        .reverse_dependencies
        .iter()
        .find(|r| r.target_id == target_id)
    else {
        ui_state.open_reverse_dep_id = None;
        return;
    };

    let screen = ctx.screen_rect();
    let area_w = (screen.width() * 0.48).clamp(380.0, 500.0);
    let anchor_offset = Vec2::new(-SPACE_16, 176.0);

    egui::Area::new(egui::Id::new("reverse_dep_overlay"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_TOP, anchor_offset)
        .show(ctx, |ui| {
            draw_crisp_frame(
                ui,
                pal.panel,
                pal.accent_line,
                8.0,
                Margin::same(SPACE_12),
                |ui| {
                    ui.set_width(area_w - SPACE_16 - SPACE_8);
                    ui.spacing_mut().item_spacing.y = SPACE_4;

                    let mut close_clicked = false;
                    draw_split_row(
                        ui,
                        24.0,
                        136.0,
                        Sense::hover(),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_8;
                            let (title_font, _) = prop_strong_font(ui_state.has_prop_strong, 14.0);
                            let max_t_w = ui.available_width().max(60.0);
                            draw_word_capped_text_cached(
                                ui,
                                &rev.target_name,
                                title_font,
                                pal.text,
                                max_t_w,
                                &mut ui_state.truncate_cache,
                            );
                        },
                        |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_8;
                            if draw_z_button(
                                ui,
                                "Close",
                                Vec2::new(56.0, 24.0),
                                11.5,
                                6.0,
                                true,
                                false,
                                ButtonVariant::SecondaryOutline,
                                pal,
                            )
                            .clicked()
                            {
                                close_clicked = true;
                            }
                            if draw_z_button(
                                ui,
                                "All",
                                Vec2::new(48.0, 24.0),
                                11.5,
                                6.0,
                                true,
                                false,
                                ButtonVariant::SecondaryOutline,
                                pal,
                            )
                            .on_hover_text("Open full Reverse Dependencies view")
                            .clicked()
                            {
                                ui_state.open_reverse_dep_id = None;
                                ui_state.active_view = ActiveView::Dependencies;
                            }
                        },
                    );
                    if close_clicked {
                        ui_state.open_reverse_dep_id = None;
                        return;
                    }

                    // Meta line: kind, used by N apps, binary importers, own size
                    draw_split_row(
                        ui,
                        20.0,
                        SIZE_COL_W,
                        Sense::hover(),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_4;
                            paint_chip(
                                ui,
                                rev.kind.label(),
                                ChipStyle {
                                    fill: Color32::TRANSPARENT,
                                    stroke: pal.border,
                                    text: pal.muted,
                                },
                                pal.panel,
                            );
                            let used_lbl = if rev.dependent_count == 1 {
                                "used by 1 app".to_string()
                            } else {
                                format!("used by {} apps", rev.dependent_count)
                            };
                            paint_chip(
                                ui,
                                &used_lbl,
                                ChipStyle {
                                    fill: pal.accent_soft_fill,
                                    stroke: pal.accent_border,
                                    text: pal.accent_pill_text,
                                },
                                pal.panel,
                            );
                            if rev.binary_importer_count > 0 {
                                paint_chip(
                                    ui,
                                    &format!("{} binary", rev.binary_importer_count),
                                    ChipStyle {
                                        fill: Color32::TRANSPARENT,
                                        stroke: pal.border,
                                        text: pal.muted,
                                    },
                                    pal.panel,
                                );
                            }
                        },
                        |ui| {
                            if let Some(own) = rev.own_size_bytes {
                                draw_right_aligned_size(ui, own, 12.0, pal.text, false);
                            }
                        },
                    );

                    ui.add_space(SPACE_4);
                    draw_synced_scroll_area(
                        ui,
                        ("rev_overlay_scroll", target_id),
                        Some(260.0),
                        [false, true],
                        pal,
                        pal.panel,
                        |ui| {
                            ui.spacing_mut().item_spacing.y = SPACE_4;
                            for c in &rev.consumers {
                                egui::Frame::none()
                                    .fill(pal.panel_alt)
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(Margin::symmetric(SPACE_8, SPACE_4))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        draw_split_row(
                                            ui,
                                            SUB_LINE1_H,
                                            SIZE_COL_W,
                                            Sense::hover(),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = SPACE_4;
                                                let conf_resp = paint_chip(
                                                    ui,
                                                    c.confidence.label(),
                                                    confidence_chip_style(c.confidence, pal),
                                                    pal.panel_alt,
                                                );
                                                let (cn_font, _) =
                                                    prop_strong_font(ui_state.has_prop_strong, 12.0);
                                                let name_max_w =
                                                    (ui.max_rect().max.x - conf_resp.rect.max.x - SPACE_4)
                                                        .max(60.0);
                                                draw_word_capped_text_cached(
                                                    ui,
                                                    &c.app_name,
                                                    cn_font,
                                                    pal.text,
                                                    name_max_w,
                                                    &mut ui_state.truncate_cache,
                                                );
                                            },
                                            |ui| {
                                                draw_right_aligned_size(
                                                    ui,
                                                    c.app_total_bytes,
                                                    12.0,
                                                    pal.muted,
                                                    false,
                                                );
                                            },
                                        );
                                        draw_split_row(
                                            ui,
                                            SUB_LINE2_H,
                                            SIZE_COL_W,
                                            Sense::hover(),
                                            |ui| {
                                                let max_b_w = ui.available_width().max(80.0);
                                                draw_word_capped_text_cached(
                                                    ui,
                                                    &c.basis,
                                                    FontId::new(10.5, FontFamily::Proportional),
                                                    pal.muted,
                                                    max_b_w,
                                                    &mut ui_state.truncate_cache,
                                                );
                                            },
                                            |_| {},
                                        );
                                    });
                            }
                        },
                    );
                },
            );
        });
}

fn draw_pinned_bottom_groups(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    pal: &ThemePalette,
    body_budget_h: Option<f32>,
    emit: &mut dyn FnMut(UiAction),
) -> Option<(&'static str, Rect)> {
    ui.spacing_mut().item_spacing.y = SPACE_4;
    let aligned_w = (ui.available_width() - SCROLL_GUTTER_W).max(200.0);

    if ui_state.ledger_collapsed {
        let first_reconcile = ui_state
            .cached_report
            .as_ref()
            .and_then(|cr| cr.sys_reconcile.first())
            .map(|r| (r.2.clone(), r.3));
        let collapsed_summary = format!(
            "Win/sys {}  ·  Your files {}  ·  Shared caches {}  ·  Unattributed {}",
            format_size_2dec(rep.windows_system_total_bytes),
            format_size_2dec(rep.your_files_total_bytes),
            format_size_2dec(rep.shared_caches_total_bytes),
            format_size_2dec(rep.unattributed_total_bytes),
        );
        let row_resp = draw_split_row(
            ui,
            22.0,
            104.0,
            Sense::click(),
            |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_8;
                draw_chevron(ui, false, pal.muted);
                draw_truncated_text(
                    ui,
                    "Pinned storage ledger",
                    FontId::new(11.0, FontFamily::Proportional),
                    pal.text,
                    false,
                );
                draw_truncated_text(
                    ui,
                    &collapsed_summary,
                    FontId::new(11.0, FontFamily::Proportional),
                    pal.muted,
                    false,
                );
            },
            |ui| {
                if let Some((exact, diff)) = &first_reconcile {
                    if *diff == 0 {
                        ui.add_space(SPACE_8);
                        draw_truncated_text(
                            ui,
                            "diff 0 B",
                            FontId::new(11.0, FontFamily::Monospace),
                            pal.status_green,
                            false,
                        )
                        .on_hover_text(exact);
                        let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                        draw_status_glyph(
                            ui.painter(),
                            dot_r.center(),
                            StatusKind::Success,
                            pal.status_green,
                        );
                    } else {
                        paint_status_chip(
                            ui,
                            StatusKind::Danger,
                            &format!("diff {diff} B"),
                            pal,
                        );
                    }
                }
            },
        );
        let hovered = row_resp.hovered();
        let clicked = row_resp.clicked();
        let _ = row_resp.on_hover_text("Click to expand Pinned storage ledger");
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if clicked {
            ui_state.ledger_collapsed = false;
            emit(UiAction::UpdateLedgerLayout {
                collapsed: false,
                height_px: ui_state.ledger_height_px,
            });
        }
        return None;
    }

    let (hdr_rect, hdr_resp) = ui.allocate_exact_size(Vec2::new(aligned_w, 18.0), Sense::click());
    if hdr_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    {
        let mut h_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(hdr_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        h_ui.spacing_mut().item_spacing.x = SPACE_4;
        draw_chevron(&mut h_ui, true, if hdr_resp.hovered() { pal.text } else { pal.muted });
        h_ui.label(
            RichText::new("Pinned storage ledger")
                .size(10.5)
                .color(if hdr_resp.hovered() { pal.text } else { pal.subtle_text }),
        );
    }
    let hdr_clicked = hdr_resp.clicked();
    let _ = hdr_resp.on_hover_text("Click to collapse Pinned storage ledger; drag top border to resize");
    if hdr_clicked {
        ui_state.ledger_collapsed = true;
        emit(UiAction::UpdateLedgerLayout {
            collapsed: true,
            height_px: ui_state.ledger_height_px,
        });
        return None;
    }

    let any_expanded = ui_state.expand_windows_system
        || ui_state.expand_shared_caches
        || ui_state.expand_your_files
        || ui_state.expand_unattributed;
    let unattrib_tick = lerp_color(pal.border, pal.muted, 0.55);

    let sys_count = ui_state
        .cached_report
        .as_ref()
        .map_or(rep.windows_system.len(), |cr| cr.sys_reconcile.len());
    let reconcile_reserve_h = if sys_count > 0 {
        SPACE_4 + (sys_count as f32) * 24.0
    } else {
        0.0
    };
    let list_avail_h = (ui.available_height() - reconcile_reserve_h - SPACE_4).max(48.0);
    let cards_box_max_h = match body_budget_h {
        Some(budget) => budget.min(list_avail_h),
        None => {
            if any_expanded {
                (240.0_f32).min(list_avail_h)
            } else {
                list_avail_h
            }
        }
    }
    .max(48.0);
    let scroll_to_bottom = ui_state.scroll_ledger_bottom;

    let was_unattrib_expanded = ui_state.expand_unattributed;
    let expand_windows_system = &mut ui_state.expand_windows_system;
    let expand_your_files = &mut ui_state.expand_your_files;
    let expand_shared_caches = &mut ui_state.expand_shared_caches;
    let expand_unattributed = &mut ui_state.expand_unattributed;
    let unattrib_exists_cache = &mut ui_state.unattrib_exists_cache;
    let first_unattrib_web_search_rect = &mut ui_state.first_unattrib_web_search_rect;
    let unattrib_mid_row_rect = &mut ui_state.unattrib_mid_row_rect;
    let unattrib_search_query = &mut ui_state.unattrib_search_query;
    let unattrib_facets = &mut ui_state.unattrib_facets;
    let open_unattrib_facet_popup = &mut ui_state.open_unattrib_facet_popup;
    let unattrib_sort = &mut ui_state.unattrib_sort;
    let cached_unattrib = &mut ui_state.cached_unattrib;
    let mut unattrib_facet_anchor: Option<(&'static str, Rect)> = None;

    let mut draw_cards = |ui: &mut egui::Ui, frame_w: f32| {
        ui.set_width(frame_w.max(180.0));
        ui.spacing_mut().item_spacing.y = 0.0;

                draw_pinned_group_card(
                    ui,
                    "Windows & system",
                    rep.windows_system_total_bytes,
                    "ESTIMATE · system remainder + OS folders + pagefile/hiberfil",
                    pal.muted,
                    None,
                    expand_windows_system,
                    true,
                    pal,
                    |ui| {
                        for sys in &rep.windows_system {
                            for rf in &sys.root_files {
                                let _ = draw_simple_item_row(
                                    ui,
                                    &rf.path,
                                    rf.bytes,
                                    Some(LedgerRowBadge::Status(StatusKind::Danger, "ROOT")),
                                    pal,
                                );
                            }
                            for osf in &sys.os_folders {
                                let resp = draw_simple_item_row(
                                    ui,
                                    &osf.path,
                                    osf.bytes,
                                    Some(LedgerRowBadge::Status(StatusKind::Danger, "SYSTEM")),
                                    pal,
                                );
                                let _ = resp.on_hover_text(osf.basis_label());
                            }
                            if sys.hardlink_overlap_bytes > 0 {
                                let resp = draw_simple_item_row(
                                    ui,
                                    &format!("{}:\\ hardlink deduplication offset", sys.drive_letter),
                                    sys.hardlink_overlap_bytes,
                                    Some(LedgerRowBadge::Accent("HARDLINK")),
                                    pal,
                                );
                                let _ = resp.on_hover_text(
                                    "NTFS hardlinked files walked via multiple paths on this drive; subtracted once so folder totals reconcile against physical drive used bytes",
                                );
                            }
                            let _ = draw_simple_item_row(
                                ui,
                                &format!("{}:\\Windows & system remainder (estimate)", sys.drive_letter),
                                sys.remainder_bytes,
                                Some(LedgerRowBadge::Accent("ESTIMATE")),
                                pal,
                            );
                        }
                    },
                );

                draw_pinned_group_card(
                    ui,
                    "Your files",
                    rep.your_files_total_bytes,
                    "Shell folders & Personal-files zones · read-only",
                    pal.muted,
                    None,
                    expand_your_files,
                    true,
                    pal,
                    |ui| {
                        for (vis_idx, item) in rep.your_files.iter().enumerate() {
                            draw_ledger_folder_item_row(
                                ui,
                                vis_idx,
                                &item.path,
                                item.bytes,
                                None,
                                pal,
                                unattrib_exists_cache,
                                &mut None,
                                &mut None,
                                emit,
                            );
                        }
                    },
                );

                draw_pinned_group_card(
                    ui,
                    "Shared & system caches",
                    rep.shared_caches_total_bytes,
                    &format!("{} regenerable Temp / DXCache / Package Cache folders", rep.shared_caches.len()),
                    pal.status_green,
                    Some(StatusKind::Success),
                    expand_shared_caches,
                    true,
                    pal,
                    |ui| {
                        for (vis_idx, item) in rep.shared_caches.iter().take(40).enumerate() {
                            draw_ledger_folder_item_row(
                                ui,
                                vis_idx,
                                &item.path,
                                item.bytes,
                                Some(LedgerRowBadge::Status(StatusKind::Success, "GREEN")),
                                pal,
                                unattrib_exists_cache,
                                &mut None,
                                &mut None,
                                emit,
                            );
                        }
                    },
                );

                draw_pinned_group_card(
                    ui,
                    "Unattributed",
                    rep.unattributed_total_bytes,
                    &format!("{} folders with no verified app owner", rep.unattributed.len()),
                    unattrib_tick,
                    None,
                    expand_unattributed,
                    false,
                    pal,
                    |ui| {
                        let cache_stale = cached_unattrib.as_ref().is_none_or(|c| {
                            c.scan_ts != rep.scan_timestamp_unix
                                || c.count != rep.unattributed.len()
                                || c.search_query != *unattrib_search_query
                                || c.facets != *unattrib_facets
                                || c.sort != *unattrib_sort
                        });
                        if cache_stale {
                            let (filtered_items, sub_bytes) =
                                filter_and_sort_unattributed_with_facets(
                                    &rep.unattributed,
                                    unattrib_search_query,
                                    unattrib_facets,
                                    *unattrib_sort,
                                );
                            let visible_indices = filtered_items.into_iter().map(|(i, _)| i).collect();
                            *cached_unattrib = Some(CachedUnattributedList {
                                scan_ts: rep.scan_timestamp_unix,
                                count: rep.unattributed.len(),
                                search_query: unattrib_search_query.clone(),
                                facets: unattrib_facets.clone(),
                                sort: *unattrib_sort,
                                visible_indices,
                                filtered_sub_bytes: sub_bytes,
                            });
                        }
                        let cu = cached_unattrib.as_ref().unwrap();
                        let filtered_count = cu.visible_indices.len();
                        let filtered_sub_bytes = cu.filtered_sub_bytes;
                        let any_unattrib_filter = !unattrib_search_query.trim().is_empty()
                            || !unattrib_facets.is_default()
                            || *unattrib_sort != UnattributedSortMode::SizeDesc;
                        ui.add_space(2.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
                            ui.add_space(SPACE_16);
                            draw_crisp_text_input(
                                ui,
                                "unattrib_filter_input",
                                unattrib_search_query,
                                220.0,
                                "Filter folder name or path...",
                                pal.input_bg,
                                pal,
                            );
                            if !unattrib_search_query.is_empty()
                                && draw_z_button(
                                    ui,
                                    "Clear",
                                    Vec2::new(48.0, 24.0),
                                    11.0,
                                    RADIUS_CONTROL,
                                    true,
                                    false,
                                    ButtonVariant::SubtleOutline,
                                    pal,
                                )
                                .clicked()
                            {
                                unattrib_search_query.clear();
                            }
                            for sort_opt in UnattributedSortMode::ALL_OPTIONS {
                                let selected = *unattrib_sort == sort_opt;
                                if draw_filter_radio_chip(ui, sort_opt.short_label(), selected, pal)
                                    && !selected
                                {
                                    *unattrib_sort = sort_opt;
                                }
                            }
                            if any_unattrib_filter
                                && draw_z_button(
                                    ui,
                                    "Reset filters",
                                    Vec2::new(86.0, 24.0),
                                    11.0,
                                    RADIUS_CONTROL,
                                    true,
                                    false,
                                    ButtonVariant::SubtleOutline,
                                    pal,
                                )
                                .clicked()
                            {
                                unattrib_search_query.clear();
                                *unattrib_facets = UnattributedFacetFilters::default();
                                *open_unattrib_facet_popup = None;
                                *unattrib_sort = UnattributedSortMode::SizeDesc;
                            }
                        });
                        ui.add_space(SPACE_4);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
                            ui.add_space(SPACE_16);

                            let drv_lbl = match unattrib_facets.drive {
                                None => "Drive: All".to_string(),
                                Some(d) => format!("Drive: {}:\\", d.to_ascii_uppercase()),
                            };
                            let drv_open = *open_unattrib_facet_popup == Some("unattrib_drive");
                            let drv_resp = draw_facet_selector_button(
                                ui,
                                "unattrib_drive",
                                &drv_lbl,
                                drv_open,
                                unattrib_facets.drive.is_some(),
                                pal,
                            );
                            if drv_resp.clicked() {
                                *open_unattrib_facet_popup =
                                    if drv_open { None } else { Some("unattrib_drive") };
                            }
                            if *open_unattrib_facet_popup == Some("unattrib_drive") {
                                unattrib_facet_anchor = Some(("unattrib_drive", drv_resp.rect));
                            }

                            let size_lbl = if unattrib_facets.size_band == SizeBandFilter::All {
                                "Size: All".to_string()
                            } else {
                                format!("Size: {}", unattrib_facets.size_band.label())
                            };
                            let size_open = *open_unattrib_facet_popup == Some("unattrib_size");
                            let size_resp = draw_facet_selector_button(
                                ui,
                                "unattrib_size",
                                &size_lbl,
                                size_open,
                                unattrib_facets.size_band != SizeBandFilter::All,
                                pal,
                            );
                            if size_resp.clicked() {
                                *open_unattrib_facet_popup =
                                    if size_open { None } else { Some("unattrib_size") };
                            }
                            if *open_unattrib_facet_popup == Some("unattrib_size") {
                                unattrib_facet_anchor = Some(("unattrib_size", size_resp.rect));
                            }

                            let zone_lbl = if unattrib_facets.location_zone == UnattributedLocationZone::All {
                                "Zone: All".to_string()
                            } else {
                                format!("Zone: {}", unattrib_facets.location_zone.label())
                            };
                            let zone_open = *open_unattrib_facet_popup == Some("unattrib_zone");
                            let zone_resp = draw_facet_selector_button(
                                ui,
                                "unattrib_zone",
                                &zone_lbl,
                                zone_open,
                                unattrib_facets.location_zone != UnattributedLocationZone::All,
                                pal,
                            );
                            if zone_resp.clicked() {
                                *open_unattrib_facet_popup =
                                    if zone_open { None } else { Some("unattrib_zone") };
                            }
                            if *open_unattrib_facet_popup == Some("unattrib_zone") {
                                unattrib_facet_anchor = Some(("unattrib_zone", zone_resp.rect));
                            }

                            let is_filtered = !unattrib_search_query.trim().is_empty()
                                || !unattrib_facets.is_default();
                            let stats_txt = format!(
                                "{} of {} folders · {}",
                                filtered_count,
                                rep.unattributed.len(),
                                format_size_2dec(filtered_sub_bytes),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(stats_txt)
                                        .size(11.0)
                                        .color(if is_filtered { pal.text } else { pal.muted }),
                                )
                                .selectable(false),
                            );
                        });
                        ui.add_space(2.0);

                        if cu.visible_indices.is_empty() {
                            ui.horizontal(|ui| {
                                ui.add_space(SPACE_16);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new("No unattributed folders match the current filter.")
                                            .size(11.5)
                                            .color(pal.muted),
                                    )
                                    .selectable(false),
                                );
                            });
                        } else {
                            for (vis_idx, &idx) in cu.visible_indices.iter().enumerate() {
                                let item = &rep.unattributed[idx];
                                let chip = item
                                    .is_loose_files
                                    .then(|| LedgerRowBadge::Accent("loose files"));
                                draw_ledger_folder_item_row(
                                    ui,
                                    vis_idx,
                                    &item.path,
                                    item.bytes,
                                    chip,
                                    pal,
                                    unattrib_exists_cache,
                                    first_unattrib_web_search_rect,
                                    unattrib_mid_row_rect,
                                    emit,
                                );
                            }
                        }
                    },
                );
    };

    // Bounded outer 1px-bordered frame: scroll viewport clips strictly inside it above reconciliation at every window size (FIX 2).
    let inner_scroll_max_h = (cards_box_max_h - 2.0).max(44.0);
    let inner_box_w = if any_expanded || body_budget_h.is_some() {
        (ui.available_width() - 2.0).max(180.0)
    } else {
        (aligned_w - 2.0).max(180.0)
    };
    draw_crisp_frame(
        ui,
        pal.panel,
        pal.border,
        RADIUS_PANEL,
        Margin::ZERO,
        |ui| {
            ui.set_width(inner_box_w);
            draw_synced_scroll_area(
                ui,
                "pinned_groups_scroll",
                Some(inner_scroll_max_h),
                [false, true],
                pal,
                pal.panel,
                |ui| {
                    let w = (ui.available_width() - SCROLL_GUTTER_W).max(180.0);
                    draw_cards(ui, w);
                    if scroll_to_bottom {
                        let last_row = Rect::from_min_size(
                            Pos2::new(
                                ui.min_rect().left(),
                                (ui.cursor().min.y - 22.0).max(ui.min_rect().top()),
                            ),
                            Vec2::new(40.0, 22.0),
                        );
                        ui.scroll_to_rect(last_row, Some(Align::BOTTOM));
                    }
                },
            );
        },
    );
    if was_unattrib_expanded && !ui_state.expand_unattributed {
        ui_state.unattrib_search_query.clear();
        ui_state.unattrib_facets = UnattributedFacetFilters::default();
        ui_state.open_unattrib_facet_popup = None;
        ui_state.unattrib_sort = UnattributedSortMode::SizeDesc;
    }

    if sys_count > 0 {
        ui.add_space(SPACE_4);
    }
    // Quiet exact reconciliation ledger line (green status dot + tabular Regular diff 0 B; alert chip only if diff != 0).
    for sys_i in 0..sys_count {
        let (summary, exact, diff) = {
            let cr = ui_state.cached_report.as_ref().unwrap();
            let entry = &cr.sys_reconcile[sys_i];
            (entry.1.as_str(), entry.2.as_str(), entry.3)
        };

        ui.allocate_ui_with_layout(
            Vec2::new(aligned_w, 20.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                draw_split_row(
                    ui,
                    20.0,
                    92.0,
                    Sense::hover(),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_4;
                        let resp = draw_truncated_text(
                            ui,
                            summary,
                            FontId::new(11.0, FontFamily::Proportional),
                            pal.subtle_text,
                            false,
                        );
                        let _ = resp.on_hover_text(exact);
                    },
                    |ui| {
                        if diff == 0 {
                            ui.add_space(SPACE_8);
                            let _ = draw_truncated_text(
                                ui,
                                "diff 0 B",
                                FontId::new(11.0, FontFamily::Monospace),
                                pal.status_green,
                                false,
                            );
                            let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                            draw_status_glyph(
                                ui.painter(),
                                dot_r.center(),
                                StatusKind::Success,
                                pal.status_green,
                            );
                        } else {
                            paint_status_chip(
                                ui,
                                StatusKind::Danger,
                                &format!("diff {diff} B"),
                                pal,
                            );
                        }
                    },
                );
            },
        );
    }

    unattrib_facet_anchor
}

fn draw_pinned_group_card(
    ui: &mut egui::Ui,
    title: &str,
    total_bytes: u64,
    subtitle: &str,
    accent: Color32,
    status_icon: Option<StatusKind>,
    expanded: &mut bool,
    draw_bottom_divider: bool,
    pal: &ThemePalette,
    body: impl FnOnce(&mut egui::Ui),
) {
    let bg_slot = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::none()
        .fill(Color32::TRANSPARENT)
        .inner_margin(Margin::symmetric(SPACE_12, 3.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = SPACE_4;
            let resp = draw_split_row(
                ui,
                PINNED_ROW_H - SPACE_4,
                SIZE_COL_W,
                Sense::click(),
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_8;
                    let (tick_r, _) = ui.allocate_exact_size(Vec2::new(2.0, 12.0), Sense::hover());
                    let snapped_tick = snap_rect_to_pixels(tick_r, ui.ctx().pixels_per_point());
                    paint_solid_px_rect(ui.painter(), snapped_tick, accent);
                    draw_chevron(ui, *expanded, if *expanded { accent } else { pal.muted });
                    if let Some(kind) = status_icon {
                        let (ir, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        draw_status_glyph(ui.painter(), ir.center(), kind, accent);
                    }
                    let title_font = FontId::new(12.0, FontFamily::Proportional);
                    draw_truncated_text(ui, title, title_font.clone(), pal.text, false);
                    draw_baseline_paired_text(
                        ui,
                        subtitle,
                        FontId::new(11.0, FontFamily::Proportional),
                        pal.subtle_text,
                        title_font,
                    );
                },
                |ui| {
                    draw_right_aligned_size(ui, total_bytes, 12.5, pal.text, false);
                },
            );
            if resp.clicked() {
                *expanded = !*expanded;
                if *expanded {
                    resp.scroll_to_me(Some(Align::TOP));
                }
            }
            if *expanded {
                body(ui);
            }
            resp.hovered()
        });

    let r = inner.response.rect;
    if inner.inner {
        let mut m = egui::epaint::Mesh::default();
        add_solid_rect_quad(&mut m, r, pal.widget_hover);
        ui.painter().set(bg_slot, egui::Shape::mesh(m));
    }
    if draw_bottom_divider {
        let sep = Rect::from_min_size(
            Pos2::new(r.min.x, (r.max.y - 1.0).round()),
            Vec2::new(r.width(), 1.0),
        );
        paint_solid_px_rect(ui.painter(), sep, pal.border);
    }
}

fn draw_ledger_row_badge(ui: &mut egui::Ui, badge: LedgerRowBadge, pal: &ThemePalette) {
    match badge {
        LedgerRowBadge::Accent(lbl) => {
            paint_chip(ui, lbl, qualifier_chip_style(pal), pal.panel);
        }
        LedgerRowBadge::Status(kind, lbl) => {
            paint_status_chip(ui, kind, lbl, pal);
        }
    }
}

fn draw_simple_item_row(
    ui: &mut egui::Ui,
    path: &str,
    bytes: u64,
    badge: Option<LedgerRowBadge>,
    pal: &ThemePalette,
) -> egui::Response {
    let hover_slot = ui.painter().add(egui::Shape::Noop);
    let row_resp = draw_split_row(
        ui,
        18.0,
        SIZE_COL_W,
        Sense::hover(),
        |ui| {
            ui.add_space(SPACE_16);
            if let Some(b) = badge {
                draw_ledger_row_badge(ui, b, pal);
            }
            draw_truncated_text(
                ui,
                path,
                FontId::new(11.0, FontFamily::Monospace),
                pal.text,
                false,
            );
        },
        |ui| {
            draw_right_aligned_size(ui, bytes, 11.5, pal.muted, false);
        },
    );
    let full_hover_rect = Rect::from_min_max(
        Pos2::new(row_resp.rect.min.x - SPACE_12, row_resp.rect.min.y),
        Pos2::new(row_resp.rect.max.x + SPACE_12, row_resp.rect.max.y),
    );
    if ui.rect_contains_pointer(full_hover_rect) {
        let mut m = egui::epaint::Mesh::default();
        add_solid_rect_quad(&mut m, full_hover_rect, pal.widget_hover);
        ui.painter().set(hover_slot, egui::Shape::mesh(m));
    }
    row_resp
}

#[allow(clippy::too_many_arguments)]
fn draw_ledger_folder_item_row(
    ui: &mut egui::Ui,
    row_idx: usize,
    path: &str,
    bytes: u64,
    badge: Option<LedgerRowBadge>,
    pal: &ThemePalette,
    exists_cache: &mut HashMap<String, bool>,
    first_web_search_rect: &mut Option<Rect>,
    mid_row_rect: &mut Option<Rect>,
    emit: &mut dyn FnMut(UiAction),
) {
    const ROW_H: f32 = 22.0;
    const RIGHT_CLUSTER_W: f32 = SIZE_COL_W + SPACE_8 + 148.0;
    let avail_w = ui.available_width().max(RIGHT_CLUSTER_W + 60.0);
    let hover_slot = ui.painter().add(egui::Shape::Noop);
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, ROW_H), Sense::hover());
    if !ui.clip_rect().expand2(Vec2::new(0.0, 24.0)).intersects(row_rect) {
        return;
    }

    let full_hover_rect = Rect::from_min_max(
        Pos2::new(row_rect.min.x - SPACE_12, row_rect.min.y),
        Pos2::new(row_rect.max.x + SPACE_12, row_rect.max.y),
    );
    if ui.rect_contains_pointer(full_hover_rect) {
        let mut m = egui::epaint::Mesh::default();
        add_solid_rect_quad(&mut m, full_hover_rect, pal.widget_hover);
        ui.painter().set(hover_slot, egui::Shape::mesh(m));
    }
    if row_idx == 3 {
        *mid_row_rect = Some(row_rect);
    }

    let right_rect = Rect::from_min_max(
        Pos2::new((row_rect.max.x - RIGHT_CLUSTER_W).max(row_rect.min.x), row_rect.min.y),
        row_rect.max,
    );
    let left_rect = Rect::from_min_max(
        row_rect.min,
        Pos2::new((right_rect.min.x - SPACE_8).max(row_rect.min.x), row_rect.max.y),
    );

    let mut left_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(left_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    left_ui.set_clip_rect(
        left_rect
            .expand2(Vec2::new(1.0, 1.0))
            .intersect(ui.clip_rect()),
    );
    left_ui.spacing_mut().item_spacing.x = SPACE_8;
    left_ui.add_space(SPACE_16);
    if let Some(b) = badge {
        draw_ledger_row_badge(&mut left_ui, b, pal);
    }
    draw_truncated_text(
        &mut left_ui,
        path,
        FontId::new(11.0, FontFamily::Monospace),
        pal.text,
        false,
    );

    let mut right_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(right_rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right_ui.set_clip_rect(
        right_rect
            .expand2(Vec2::new(1.0, 1.0))
            .intersect(ui.clip_rect()),
    );
    right_ui.spacing_mut().item_spacing.x = 0.0;

    // 1. Fixed SIZE_COL_W column flush with ledger size alignment (YS1)
    let (size_slot, _) = right_ui.allocate_exact_size(Vec2::new(SIZE_COL_W, ROW_H), Sense::hover());
    let mut size_ui = right_ui.new_child(
        UiBuilder::new()
            .max_rect(size_slot)
            .layout(Layout::right_to_left(Align::Center)),
    );
    draw_right_aligned_size(&mut size_ui, bytes, 11.5, pal.muted, false);

    right_ui.add_space(SPACE_8);

    // 2. Web search (opens default browser via OS ShellExecute; exact constructed query on hover)
    let (search_query, search_url) = crate::unattributed_assist::build_unattributed_search_url(path);
    let web_resp = draw_z_button(
        &mut right_ui,
        "Web search",
        Vec2::new(0.0, 18.0),
        10.5,
        RADIUS_PILL,
        true,
        false,
        ButtonVariant::SubtleOutline,
        pal,
    );
    if row_idx == 0 {
        *first_web_search_rect = Some(web_resp.rect);
    }
    if web_resp.clicked() {
        emit(UiAction::OpenWebSearch(search_url));
    }
    let _ = web_resp.on_hover_text(format!("Web search (Google): {search_query}"));

    right_ui.add_space(SPACE_4);

    // 3. Locate (opens Explorer selecting target folder, or disabled with "no longer on disk")
    let exists = *exists_cache
        .entry(path.to_string())
        .or_insert_with(|| std::path::Path::new(path).exists());
    let loc_resp = draw_z_button(
        &mut right_ui,
        "Locate",
        Vec2::new(0.0, 18.0),
        10.5,
        RADIUS_PILL,
        exists,
        false,
        ButtonVariant::SubtleOutline,
        pal,
    );
    if exists {
        if loc_resp.clicked() {
            emit(UiAction::LocateInExplorer(path.to_string()));
        }
        let _ = loc_resp.on_hover_text(crate::unattributed_assist::build_explorer_select_command_line(path));
    } else {
        let _ = loc_resp.on_hover_text("no longer on disk");
    }
}

fn draw_dependencies_view(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    report: Option<&ScanReport>,
    pal: &ThemePalette,
) {
    draw_split_row(
        ui,
        HEADER_CONTROL_H,
        264.0,
        Sense::hover(),
        |ui| {
            ui.label(
                RichText::new("Reverse Dependencies (\"What else uses this?\")")
                    .size(16.0)
                    .color(pal.text),
            );
        },
        |ui| {
            draw_crisp_text_input(
                ui,
                "rev_dep_search",
                &mut ui_state.dep_search_query,
                250.0,
                "Filter dependency or app...",
                pal.header_input_bg,
                pal,
            );
        },
    );
    ui.add_space(SPACE_4);

    let Some(rep) = report else {
        ui.label(RichText::new("Run a scan first to inspect dependencies.").color(pal.muted));
        return;
    };

    if !rep.catalog_source.is_empty() {
        ui.label(
            RichText::new(format!("Dependency catalog source: {}", rep.catalog_source))
                .size(11.0)
                .color(pal.muted),
        );
        ui.add_space(SPACE_4);
    }

    let q = ui_state.dep_search_query.trim().to_ascii_lowercase();
    draw_synced_scroll_area(ui, "reverse_deps_scroll", None, [false, false], pal, pal.bg, |ui| {
            ui.spacing_mut().item_spacing.y = SPACE_4;
            for dep in &rep.reverse_dependencies {
                if !q.is_empty() {
                    let dep_hit = dep.target_name.to_ascii_lowercase().contains(&q);
                    let app_hit = dep
                        .consumers
                        .iter()
                        .any(|c| c.app_name.to_ascii_lowercase().contains(&q));
                    if !dep_hit && !app_hit {
                        continue;
                    }
                }
                let expanded = ui_state.expanded_deps.contains(&dep.target_id);
                draw_crisp_frame(
                    ui,
                    pal.panel,
                    if expanded { pal.accent_line } else { pal.border },
                    6.0,
                    Margin::symmetric(SPACE_12, SPACE_8),
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = SPACE_4;
                        let resp = draw_reverse_group_header(ui, dep, expanded, ui_state.has_prop_strong, pal);
                        if resp.clicked() {
                            if expanded {
                                ui_state.expanded_deps.remove(&dep.target_id);
                            } else {
                                ui_state.expanded_deps.insert(dep.target_id.clone());
                            }
                        }
                        if expanded {
                            for c in &dep.consumers {
                                draw_split_row(
                                    ui,
                                    18.0,
                                    SIZE_COL_W,
                                    Sense::hover(),
                                    |ui| {
                                        ui.add_space(SPACE_16);
                                        paint_chip(
                                            ui,
                                            c.confidence.label(),
                                            confidence_chip_style(c.confidence, pal),
                                            pal.panel,
                                        );
                                        let (cf_font, cf_fb) = prop_strong_font(ui_state.has_prop_strong, 12.0);
                                        draw_truncated_text(
                                            ui,
                                            &c.app_name,
                                            cf_font,
                                            pal.text,
                                            cf_fb,
                                        );
                                        draw_truncated_text(
                                            ui,
                                            &c.basis,
                                            FontId::new(10.5, FontFamily::Proportional),
                                            pal.muted,
                                            false,
                                        );
                                    },
                                    |ui| {
                                        draw_right_aligned_size(
                                            ui,
                                            c.app_total_bytes,
                                            11.5,
                                            pal.muted,
                                            false,
                                        );
                                    },
                                );
                            }
                        }
                    },
                );
            }
        });
}

fn draw_reverse_group_header(
    ui: &mut egui::Ui,
    dep: &ReverseDependencyEntry,
    expanded: bool,
    has_prop_strong: bool,
    pal: &ThemePalette,
) -> egui::Response {
    draw_split_row(
        ui,
        APP_ROW_H,
        180.0,
        Sense::click(),
        |ui| {
            ui.spacing_mut().item_spacing.x = SPACE_8;
            draw_chevron(
                ui,
                expanded,
                if expanded { pal.accent_line } else { pal.muted },
            );
            let (dfont, dfb) = prop_strong_font(has_prop_strong, 13.0);
            draw_truncated_text(
                ui,
                &dep.target_name,
                dfont,
                pal.text,
                dfb,
            );
            paint_chip(
                ui,
                dep.kind.label(),
                ChipStyle {
                    fill: Color32::TRANSPARENT,
                    stroke: pal.border,
                    text: pal.muted,
                },
                pal.panel,
            );
            if dep.kind == DependencyKind::SharedRuntime || dep.binary_importer_count > 0 {
                paint_chip(
                    ui,
                    &format!("{} binary importers", dep.binary_importer_count),
                    ChipStyle {
                        fill: pal.accent_soft_fill,
                        stroke: pal.accent_border,
                        text: pal.accent_pill_text,
                    },
                    pal.panel,
                );
            }
        },
        |ui| {
            ui.spacing_mut().item_spacing.x = SPACE_8;
            ui.label(
                RichText::new(format!("{} apps", dep.dependent_count))
                    .font(FontId::new(12.5, FontFamily::Monospace))
                    .color(pal.text),
            );
            if let Some(own) = dep.own_size_bytes {
                draw_right_aligned_size(ui, own, 12.0, pal.muted, false);
            }
        },
    )
}

fn draw_theme_preset_swatch_chip(
    ui: &mut egui::Ui,
    preset: ThemePresetChoice,
    selected: bool,
    pal: &ThemePalette,
    custom_rgb: [u8; 3],
) -> bool {
    let label = preset.label();
    let font_id = FontId::new(11.5, FontFamily::Proportional);
    let text_col = if selected { pal.text } else { pal.muted };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_col));
    let swatch_sz = 10.0_f32;
    let swatch_gap = 6.0_f32;
    let h_pad = 8.0_f32;
    let desired_w = (h_pad * 2.0 + swatch_sz + swatch_gap + galley.size().x).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, 24.0), Sense::click());
    let (fill, border_col, draw_fg) = if resp.is_pointer_button_down_on() {
        (pal.widget_active, pal.accent_border, pal.text)
    } else if resp.hovered() {
        (
            pal.widget_hover,
            pal.accent_border,
            if selected { pal.text } else { pal.accent_line },
        )
    } else if selected {
        (pal.accent_soft_fill, pal.accent_border, pal.text)
    } else {
        (pal.widget, pal.border, pal.muted)
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
    let sw_rgb = if preset == ThemePresetChoice::Custom {
        custom_rgb
    } else {
        preset.base_rgb()
    };
    let sw_col = Color32::from_rgb(sw_rgb[0], sw_rgb[1], sw_rgb[2]);
    let sw_rect = Rect::from_center_size(
        Pos2::new(snapped.min.x + h_pad + swatch_sz * 0.5, snapped.center().y),
        Vec2::splat(swatch_sz),
    );
    paint_crisp_bordered_box(
        ui.painter(),
        sw_rect,
        3.0,
        sw_col,
        if selected { pal.text } else { pal.border },
        1.0,
        ppp,
    );
    let text_rect = Rect::from_min_max(
        Pos2::new(snapped.min.x + h_pad + swatch_sz + swatch_gap, snapped.min.y),
        snapped.max,
    );
    let text_pos = optical_galley_pos(ui, text_rect, &galley, None, false);
    ui.painter().galley(text_pos, galley, draw_fg);
    resp.clicked()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SettingsFormGridLayout {
    input_w: f32,
    selector_w: f32,
    action_w: f32,
    stacked: bool,
}

fn measure_filter_radio_group_w(ui: &egui::Ui, labels: &[&str], pal: &ThemePalette) -> f32 {
    let font_id = FontId::new(11.5, FontFamily::Proportional);
    let chips_w: f32 = labels
        .iter()
        .map(|lbl| {
            let galley = ui.fonts(|f| f.layout_no_wrap((*lbl).to_owned(), font_id.clone(), pal.text));
            (galley.size().x + SPACE_16).round()
        })
        .sum();
    let gaps_w = SPACE_4 * (labels.len().saturating_sub(1) as f32);
    (chips_w + gaps_w).ceil()
}

fn compute_settings_form_grid(avail_w: f32, selector_w: Option<f32>) -> SettingsFormGridLayout {
    let action_w = FORM_GRID_ACTION_W;
    if let Some(sel_w) = selector_w {
        let single_row_input_w = avail_w - sel_w - action_w - SPACE_8 * 2.0;
        if single_row_input_w >= 150.0 {
            SettingsFormGridLayout {
                input_w: single_row_input_w,
                selector_w: sel_w,
                action_w,
                stacked: false,
            }
        } else {
            SettingsFormGridLayout {
                input_w: avail_w.max(120.0),
                selector_w: (avail_w - action_w - SPACE_8).max(120.0),
                action_w,
                stacked: true,
            }
        }
    } else {
        SettingsFormGridLayout {
            input_w: (avail_w - action_w - SPACE_8).max(120.0),
            selector_w: 0.0,
            action_w,
            stacked: false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_settings_form_grid_row_with_rect(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    text: &mut String,
    hint: &str,
    action_label: &str,
    avail_w: f32,
    selector_w: Option<f32>,
    pal: &ThemePalette,
    mut selector_fn: impl FnMut(&mut egui::Ui),
) -> (bool, Rect) {
    let grid = compute_settings_form_grid(avail_w, selector_w);
    let mut clicked = false;
    let mut action_rect = Rect::NOTHING;
    if grid.stacked {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = SPACE_4;
            draw_crisp_text_input(ui, id_salt, text, grid.input_w, hint, pal.input_bg, pal);
            draw_split_row(
                ui,
                FORM_GRID_ROW_H,
                grid.action_w,
                Sense::hover(),
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_4;
                    selector_fn(ui);
                },
                |ui| {
                    let resp = draw_z_button(
                        ui,
                        action_label,
                        Vec2::new(grid.action_w, FORM_GRID_ROW_H),
                        11.5,
                        RADIUS_CONTROL,
                        true,
                        false,
                        ButtonVariant::SecondaryOutline,
                        pal,
                    );
                    action_rect = resp.rect;
                    if resp.clicked() {
                        clicked = true;
                    }
                },
            );
        });
    } else {
        let (row_rect, _) =
            ui.allocate_exact_size(Vec2::new(avail_w, FORM_GRID_ROW_H), Sense::hover());
        let input_rect = Rect::from_min_max(
            row_rect.min,
            Pos2::new(
                (row_rect.min.x + grid.input_w).min(row_rect.max.x),
                row_rect.max.y,
            ),
        );
        let mut input_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(input_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        draw_crisp_text_input(
            &mut input_ui,
            id_salt,
            text,
            grid.input_w,
            hint,
            pal.input_bg,
            pal,
        );

        if selector_w.is_some() {
            let sel_min_x = (input_rect.max.x + SPACE_8).min(row_rect.max.x);
            let selector_rect = Rect::from_min_max(
                Pos2::new(sel_min_x, row_rect.min.y),
                Pos2::new((sel_min_x + grid.selector_w).min(row_rect.max.x), row_rect.max.y),
            );
            let mut sel_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(selector_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            sel_ui.set_clip_rect(selector_rect.expand2(Vec2::splat(1.0)).intersect(ui.clip_rect()));
            sel_ui.spacing_mut().item_spacing.x = SPACE_4;
            selector_fn(&mut sel_ui);
        }

        let btn_rect = Rect::from_min_max(
            Pos2::new((row_rect.max.x - grid.action_w).max(row_rect.min.x), row_rect.min.y),
            row_rect.max,
        );
        let mut action_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(btn_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let resp = draw_z_button(
            &mut action_ui,
            action_label,
            Vec2::new(grid.action_w, FORM_GRID_ROW_H),
            11.5,
            RADIUS_CONTROL,
            true,
            false,
            ButtonVariant::SecondaryOutline,
            pal,
        );
        action_rect = resp.rect;
        if resp.clicked() {
            clicked = true;
        }
    }
    (clicked, action_rect)
}

#[allow(clippy::too_many_arguments)]
fn draw_settings_form_grid_row(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    text: &mut String,
    hint: &str,
    action_label: &str,
    avail_w: f32,
    selector_w: Option<f32>,
    pal: &ThemePalette,
    selector_fn: impl FnMut(&mut egui::Ui),
) -> bool {
    draw_settings_form_grid_row_with_rect(
        ui,
        id_salt,
        text,
        hint,
        action_label,
        avail_w,
        selector_w,
        pal,
        selector_fn,
    )
    .0
}

fn settings_drive_pod_cols(target_count: usize, pane_inner_w: f32) -> usize {
    if target_count <= 1 {
        1
    } else {
        (((pane_inner_w + SPACE_8) / (SETTINGS_POD_MIN_W + SPACE_8)).floor() as usize)
            .clamp(1, target_count)
    }
}

fn drive_pod_natural_inner_h(non_locked_exclusions: usize, has_stats: bool) -> f32 {
    let mut h = 22.0;
    if has_stats {
        h += SPACE_4 + 16.0;
    }
    if non_locked_exclusions > 0 {
        h += SPACE_8
            + (non_locked_exclusions as f32) * 20.0
            + (non_locked_exclusions.saturating_sub(1) as f32) * SPACE_4;
    }
    h + SPACE_8 + FORM_GRID_ROW_H
}

fn draw_appearance_theme_section(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    config: &AppConfig,
    pane_inner_w: f32,
    pal: &ThemePalette,
    emit: &mut dyn FnMut(UiAction),
) {
    ui.label(
        RichText::new("Appearance & Theme")
            .size(16.0)
            .color(pal.text),
    );
    ui.label(
        RichText::new(
            "One monochrome shade ladder across the whole UI. Backgrounds stay pure neutral; status badges keep semantic icons + labels.",
        )
        .size(11.5)
        .color(pal.muted),
    );
    ui.add_space(SPACE_4);

    let card_w = pane_inner_w.max(240.0);
    draw_crisp_frame(
        ui,
        pal.panel,
        pal.border,
        RADIUS_PANEL,
        Margin::same(SPACE_12),
        |ui| {
            let inner_w = (card_w - SPACE_12 * 2.0 - 2.0).max(200.0);
            ui.set_width(inner_w);
            ui.spacing_mut().item_spacing.y = SPACE_8;

            // Row 1: Theme mode + curated presets + Custom
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
                ui.label(RichText::new("Mode:").size(11.5).color(pal.muted));
                for mode in [ThemeMode::Dark, ThemeMode::Light] {
                    let lbl = match mode {
                        ThemeMode::Dark => "Dark",
                        ThemeMode::Light => "Light",
                    };
                    if draw_filter_radio_chip(ui, lbl, config.theme_mode == mode, pal)
                        && config.theme_mode != mode
                    {
                        emit(UiAction::SetTheme {
                            mode,
                            preset: config.theme_preset,
                            custom_rgb: config.theme_custom_rgb,
                        });
                    }
                }
                ui.add_space(SPACE_8);
                ui.label(RichText::new("Colour:").size(11.5).color(pal.muted));
                for preset in ThemePresetChoice::PRESETS {
                    let selected = config.theme_preset == preset;
                    if draw_theme_preset_swatch_chip(ui, preset, selected, pal, config.theme_custom_rgb)
                        && !selected
                    {
                        emit(UiAction::SetTheme {
                            mode: config.theme_mode,
                            preset,
                            custom_rgb: config.theme_custom_rgb,
                        });
                    }
                }
                let custom_sel = config.theme_preset == ThemePresetChoice::Custom;
                if draw_theme_preset_swatch_chip(
                    ui,
                    ThemePresetChoice::Custom,
                    custom_sel,
                    pal,
                    config.theme_custom_rgb,
                ) && !custom_sel
                {
                    ui_state.custom_hex_input = format_hex_rgb(config.theme_custom_rgb);
                    ui_state.custom_hex_valid = true;
                    emit(UiAction::SetTheme {
                        mode: config.theme_mode,
                        preset: ThemePresetChoice::Custom,
                        custom_rgb: config.theme_custom_rgb,
                    });
                }
            });

            // Row 2: Custom RGB/Hex picker (shown when Custom is active)
            if config.theme_preset == ThemePresetChoice::Custom {
                let mut next_rgb = config.theme_custom_rgb;
                let mut custom_changed = false;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
                    ui.label(RichText::new("Hex:").size(11.5).color(pal.muted));
                    let hex_id = ui.make_persistent_id("custom_theme_hex");
                    if !ui.ctx().memory(|m| m.has_focus(hex_id)) && ui_state.custom_hex_valid {
                        ui_state.custom_hex_input = format_hex_rgb(next_rgb);
                    }
                    let hex_resp = draw_crisp_text_input(
                        ui,
                        "custom_theme_hex",
                        &mut ui_state.custom_hex_input,
                        92.0,
                        "#7CCBF2",
                        pal.input_bg,
                        pal,
                    );
                    if hex_resp.changed() {
                        if let Some(parsed) = parse_hex_rgb(&ui_state.custom_hex_input) {
                            ui_state.custom_hex_valid = true;
                            if parsed != next_rgb {
                                next_rgb = parsed;
                                custom_changed = true;
                            }
                        } else {
                            ui_state.custom_hex_valid = false;
                        }
                    }

                    for (ch_lbl, ch_idx) in [("R", 0usize), ("G", 1), ("B", 2)] {
                        ui.label(RichText::new(ch_lbl).size(11.0).color(pal.muted));
                        let mut v = next_rgb[ch_idx];
                        let sl = ui.add(
                            egui::Slider::new(&mut v, 0..=255)
                                .show_value(true)
                                .trailing_fill(true),
                        );
                        if sl.changed() && v != next_rgb[ch_idx] {
                            next_rgb[ch_idx] = v;
                            ui_state.custom_hex_input = format_hex_rgb(next_rgb);
                            ui_state.custom_hex_valid = true;
                            custom_changed = true;
                        }
                    }
                    if !ui_state.custom_hex_valid {
                        paint_status_chip(ui, StatusKind::Warning, "Enter #RRGGBB", pal);
                    }
                });
                if custom_changed {
                    emit(UiAction::SetTheme {
                        mode: config.theme_mode,
                        preset: ThemePresetChoice::Custom,
                        custom_rgb: next_rgb,
                    });
                }
            }

            // Row 3: Live 4-step monochrome shade ladder preview + contrast readout
            let ladder = ThemeLadder::generate(config.theme_preset, config.theme_custom_rgb);
            let steps = [
                ("Ink / Deep", pal.accent_line),
                ("Border", pal.accent_border),
                ("Source", ladder.source),
                ("Soft fill", pal.accent_soft_fill),
            ];
            let ink_cr = contrast_ratio(pal.accent_line, pal.panel);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
                ui.label(RichText::new("Shade ladder:").size(11.0).color(pal.muted));
                let ppp = ui.ctx().pixels_per_point();
                for (step_name, col) in steps {
                    let hex = format_hex_rgb([col.r(), col.g(), col.b()]);
                    draw_crisp_frame(
                        ui,
                        pal.panel_alt,
                        pal.border,
                        RADIUS_PILL,
                        Margin::symmetric(SPACE_8, SPACE_4),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_4;
                            let (sr, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                            paint_crisp_bordered_box(
                                ui.painter(),
                                sr,
                                2.5,
                                col,
                                pal.border,
                                1.0,
                                ppp,
                            );
                            ui.label(
                                RichText::new(format!("{step_name} {hex}"))
                                    .font(FontId::new(10.5, FontFamily::Monospace))
                                    .color(pal.text),
                            );
                        },
                    );
                }
                ui.label(
                    RichText::new(format!("Text/icon contrast {:.1}:1", ink_cr))
                        .font(FontId::new(10.5, FontFamily::Monospace))
                        .color(pal.muted),
                );
            });

            if pal.near_status_hue {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_4;
                    paint_status_chip(ui, StatusKind::Warning, "Near status hue", pal);
                    ui.label(
                        RichText::new(
                            "Semantic status badges always pair green/amber/red with semantic icons + text labels.",
                        )
                        .size(11.0)
                        .color(pal.subtle_text),
                    );
                });
            }
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_scan_scope_section(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    draft: &mut AppConfig,
    report: Option<&ScanReport>,
    sys_win_key: &str,
    pane_inner_w: f32,
    pal: &ThemePalette,
    changed: &mut bool,
) {
    ui.label(
        RichText::new("Scan Scope & Exclusions")
            .size(16.0)
            .color(pal.text),
    );
    ui.label(
        RichText::new(
            "Default: system drive only (Windows excluded). Excluded drives are never walked or sized (including InstallLocation), and apps solely on them are omitted.",
        )
        .size(11.5)
        .color(pal.muted),
    );
    ui.add_space(SPACE_4);

    let cols = settings_drive_pod_cols(draft.targets.len(), pane_inner_w);
    let card_w = if cols > 1 {
        ((pane_inner_w - SPACE_8 * (cols - 1) as f32) / cols as f32)
            .floor()
            .max(240.0)
    } else {
        pane_inner_w.max(240.0)
    };

    let target_meta = |target: &y_core::config::DriveScanTarget| {
        let drv_info = report.and_then(|r| r.drives.iter().find(|d| d.letter == target.letter));
        let is_sys =
            drv_info.is_some_and(|d| d.is_system_drive) || target.letter.eq_ignore_ascii_case(&'C');
        let has_stats = drv_info.is_some() || is_sys;
        let non_locked = target
            .exclusions
            .iter()
            .filter(|ex| !(is_sys && normalize_path_key(ex) == sys_win_key))
            .count();
        drive_pod_natural_inner_h(non_locked, has_stats)
    };

    let mut draw_drive_target_card =
        |ui: &mut egui::Ui,
         target: &mut y_core::config::DriveScanTarget,
         width_px: f32,
         min_inner_h: f32| {
            let drv_info = report.and_then(|r| r.drives.iter().find(|d| d.letter == target.letter));
            let is_sys = drv_info.is_some_and(|d| d.is_system_drive)
                || target.letter.eq_ignore_ascii_case(&'C');
            let locked_win_path = if is_sys {
                target
                    .exclusions
                    .iter()
                    .find(|ex| normalize_path_key(ex) == sys_win_key)
                    .cloned()
            } else {
                None
            };
            let stats_str = if let Some(di) = drv_info {
                if is_sys {
                    format!(
                        "System · {} free · {}",
                        format_size_2dec(di.free_bytes),
                        format_size_2dec(di.total_bytes),
                    )
                } else {
                    format!(
                        "{} free · {}",
                        format_size_2dec(di.free_bytes),
                        format_size_2dec(di.total_bytes),
                    )
                }
            } else if is_sys {
                "System drive".to_string()
            } else {
                String::new()
            };
            let non_locked: Vec<(usize, String)> = target
                .exclusions
                .iter()
                .enumerate()
                .filter(|(_, ex)| !(is_sys && normalize_path_key(ex) == sys_win_key))
                .map(|(i, ex)| (i, ex.clone()))
                .collect();
            let own_h = drive_pod_natural_inner_h(non_locked.len(), !stats_str.is_empty());
            let extra_h = (min_inner_h - own_h).max(0.0);

            ui.allocate_ui_with_layout(
                Vec2::new(width_px, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    draw_crisp_frame(
                        ui,
                        pal.panel,
                        pal.border,
                        RADIUS_PANEL,
                        Margin::same(SPACE_12),
                        |ui| {
                            let inner_card_w = (width_px - SPACE_12 * 2.0 - 2.0).max(180.0);
                            ui.set_width(inner_card_w);
                            ui.set_min_height(min_inner_h);
                            ui.spacing_mut().item_spacing.y = SPACE_4;

                            // Title line: Drive letter + Include in scope checkbox beside it + compact C: Windows-lock badge
                            ui.allocate_ui_with_layout(
                                Vec2::new(inner_card_w, 22.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = SPACE_8;
                                    draw_truncated_text(
                                        ui,
                                        &format!("Drive {}:\\", target.letter),
                                        FontId::new(13.0, FontFamily::Proportional),
                                        pal.text,
                                        false,
                                    );
                                    if draw_crisp_checkbox(
                                        ui,
                                        &mut target.enabled,
                                        "Include in scope",
                                        pal,
                                    ) {
                                        *changed = true;
                                    }
                                    if let Some(locked_path) = locked_win_path.as_ref() {
                                        let _ = paint_status_chip(
                                            ui,
                                            StatusKind::Danger,
                                            "Windows locked",
                                            pal,
                                        )
                                        .on_hover_text(format!("{locked_path} — Always excluded"));
                                    }
                                },
                            );

                            if !stats_str.is_empty() {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(inner_card_w, 16.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        let resp = draw_truncated_text(
                                            ui,
                                            &stats_str,
                                            FontId::new(11.0, FontFamily::Proportional),
                                            pal.muted,
                                            false,
                                        );
                                        if let Some(di) = drv_info {
                                            let _ = resp.on_hover_text(format!(
                                                "Free: {} · Used: {} · Total: {}",
                                                format_exact_bytes(di.free_bytes),
                                                format_exact_bytes(di.used_bytes),
                                                format_exact_bytes(di.total_bytes),
                                            ));
                                        }
                                    },
                                );
                            }

                            let mut remove_idx: Option<usize> = None;
                            if !non_locked.is_empty() {
                                ui.add_space(SPACE_4);
                                for (idx, ex) in non_locked {
                                    draw_split_row(
                                        ui,
                                        20.0,
                                        56.0,
                                        Sense::hover(),
                                        |ui| {
                                            draw_truncated_text(
                                                ui,
                                                &ex,
                                                FontId::new(11.0, FontFamily::Monospace),
                                                pal.text,
                                                false,
                                            );
                                        },
                                        |ui| {
                                            if draw_z_button(
                                                ui,
                                                "Remove",
                                                Vec2::new(56.0, 20.0),
                                                11.0,
                                                RADIUS_CONTROL,
                                                true,
                                                false,
                                                ButtonVariant::SubtleOutline,
                                                pal,
                                            )
                                            .clicked()
                                            {
                                                remove_idx = Some(idx);
                                            }
                                        },
                                    );
                                }
                            }
                            if let Some(idx) = remove_idx {
                                target.exclusions.remove(idx);
                                *changed = true;
                            }

                            ui.add_space(SPACE_4 + extra_h);
                            let input = ui_state
                                .new_exclusion_inputs
                                .entry(target.letter)
                                .or_default();
                            if draw_settings_form_grid_row(
                                ui,
                                ("exclusion_input", target.letter),
                                input,
                                &format!("{}:\\FolderToExclude", target.letter),
                                "Add exclusion",
                                inner_card_w,
                                None,
                                pal,
                                |_| {},
                            ) {
                                let trimmed = input.trim().to_string();
                                if !trimmed.is_empty()
                                    && !target
                                        .exclusions
                                        .iter()
                                        .any(|e| normalize_path_key(e) == normalize_path_key(&trimmed))
                                {
                                    target.exclusions.push(trimmed);
                                    input.clear();
                                    *changed = true;
                                }
                            }
                        },
                    );
                },
            );
        };

    if cols > 1 {
        let mut idx = 0;
        while idx < draft.targets.len() {
            if idx > 0 {
                ui.add_space(SPACE_4);
            }
            let end = (idx + cols).min(draft.targets.len());
            let row_min_h = draft.targets[idx..end]
                .iter()
                .map(target_meta)
                .fold(0.0_f32, f32::max);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_8;
                for target in &mut draft.targets[idx..end] {
                    draw_drive_target_card(ui, target, card_w, row_min_h);
                }
            });
            idx = end;
        }
    } else {
        for (idx, target) in draft.targets.iter_mut().enumerate() {
            if idx > 0 {
                ui.add_space(SPACE_4);
            }
            let h = target_meta(target);
            draw_drive_target_card(ui, target, card_w, h);
        }
    }
}

fn draw_rulebook_removable_chip(
    ui: &mut egui::Ui,
    label: &str,
    name_font: &FontId,
    max_inner_w: f32,
    pal: &ThemePalette,
) -> bool {
    let text_w = ui
        .fonts(|f| f.layout_no_wrap(label.to_owned(), name_font.clone(), pal.text))
        .size()
        .x
        .ceil();
    let inner_w = (text_w + SPACE_4 + 48.0).min(max_inner_w.max(64.0));
    let chip_inner_h = 20.0_f32;
    let mut remove_clicked = false;
    draw_crisp_frame(
        ui,
        pal.panel_alt,
        pal.border,
        RADIUS_CONTROL,
        Margin::symmetric(SPACE_8, SPACE_4),
        |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(inner_w, chip_inner_h),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_4;
                    draw_truncated_text(ui, label, name_font.clone(), pal.text, false);
                    if draw_z_button(
                        ui,
                        "Remove",
                        Vec2::new(48.0, chip_inner_h),
                        11.0,
                        RADIUS_PILL,
                        true,
                        false,
                        ButtonVariant::SubtleOutline,
                        pal,
                    )
                    .clicked()
                    {
                        remove_clicked = true;
                    }
                },
            );
        },
    );
    remove_clicked
}

fn draw_rulebook_section(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    draft: &mut AppConfig,
    pane_inner_w: f32,
    pal: &ThemePalette,
    changed: &mut bool,
) -> Rect {
    let neutral_pin_style = ChipStyle {
        fill: Color32::TRANSPARENT,
        stroke: pal.border,
        text: pal.muted,
    };

    draw_split_row(
        ui,
        24.0,
        124.0,
        Sense::hover(),
        |ui| {
            ui.label(
                RichText::new("Attribution Zone Rulebook")
                    .size(16.0)
                    .color(pal.text),
            );
        },
        |ui| {
            if draft.zones != ZoneRulebook::default()
                && draw_z_button(
                    ui,
                    "Reset defaults",
                    Vec2::new(104.0, 24.0),
                    11.5,
                    RADIUS_CONTROL,
                    true,
                    false,
                    ButtonVariant::SecondaryOutline,
                    pal,
                )
                .clicked()
            {
                draft.zones = ZoneRulebook::default();
                *changed = true;
            }
        },
    );
    ui.label(
        RichText::new(
            "Rules match drive-root folder names or explicit paths. Registered install roots keep priority; Drive Root Rule tests top-level drive folders; Manual pins override inference.",
        )
        .size(11.5)
        .color(pal.muted),
    );
    ui.add_space(SPACE_4);

    let card_w = pane_inner_w.max(240.0);
    let rulebook_frame = draw_crisp_frame(
        ui,
        pal.panel,
        pal.border,
        RADIUS_PANEL,
        Margin::same(SPACE_12),
        |ui| {
            let inner_w = (card_w - SPACE_12 * 2.0 - 2.0).max(200.0);
            ui.set_width(inner_w);
            ui.spacing_mut().item_spacing.y = SPACE_4;

            let name_font = FontId::new(11.5, FontFamily::Monospace);
            let mut remove_name_idx: Option<usize> = None;

            let mut draw_chip_group =
                |ui: &mut egui::Ui, kind: ZoneRuleKind, col_w: f32| {
                    let group_items: Vec<(usize, String)> = draft
                        .zones
                        .root_zones
                        .iter()
                        .enumerate()
                        .filter(|(_, rz)| rz.kind == kind)
                        .map(|(i, rz)| (i, rz.name.clone()))
                        .collect();

                    ui.allocate_ui_with_layout(
                        Vec2::new(col_w, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(col_w);
                            ui.spacing_mut().item_spacing.y = SPACE_4;
                            ui.label(
                                RichText::new(kind.label())
                                    .size(11.5)
                                    .color(pal.muted),
                            );
                            if group_items.is_empty() {
                                ui.label(
                                    RichText::new("None configured.")
                                        .size(10.5)
                                        .color(pal.subtle_text),
                                );
                            } else {
                                let max_row_w = col_w.max(120.0);
                                let mut rows: Vec<Vec<(usize, String)>> = Vec::new();
                                let mut cur_row: Vec<(usize, String)> = Vec::new();
                                let mut cur_w = 0.0_f32;
                                for (idx, name) in group_items {
                                    let text_w = ui
                                        .fonts(|f| {
                                            f.layout_no_wrap(name.clone(), name_font.clone(), pal.text)
                                        })
                                        .size()
                                        .x;
                                    let chip_w = text_w + SPACE_8 * 2.0 + SPACE_4 + 48.0 + 4.0;
                                    let next_w = if cur_row.is_empty() {
                                        chip_w
                                    } else {
                                        cur_w + SPACE_8 + chip_w
                                    };
                                    if !cur_row.is_empty() && next_w > max_row_w {
                                        rows.push(std::mem::take(&mut cur_row));
                                        cur_w = chip_w;
                                    } else {
                                        cur_w = next_w;
                                    }
                                    cur_row.push((idx, name));
                                }
                                if !cur_row.is_empty() {
                                    rows.push(cur_row);
                                }
                                for row in rows {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = SPACE_8;
                                        let max_chip_inner_w = (col_w - SPACE_8 * 2.0 - 2.0).max(64.0);
                                        for (idx, name) in row {
                                            if draw_rulebook_removable_chip(
                                                ui,
                                                &name,
                                                &name_font,
                                                max_chip_inner_w,
                                                pal,
                                            ) {
                                                remove_name_idx = Some(idx);
                                            }
                                        }
                                    });
                                }
                            }
                        },
                    );
                };

            if inner_w >= SETTINGS_TWO_COL_MIN_W {
                let col_w = ((inner_w - SPACE_12) * 0.5).floor();
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_12;
                    draw_chip_group(ui, ZoneRuleKind::AppContainer, col_w);
                    draw_chip_group(ui, ZoneRuleKind::PersonalFiles, col_w);
                });
            } else {
                draw_chip_group(ui, ZoneRuleKind::AppContainer, inner_w);
                ui.add_space(SPACE_4);
                draw_chip_group(ui, ZoneRuleKind::PersonalFiles, inner_w);
            }

            if let Some(idx) = remove_name_idx {
                if draft.zones.remove_root_zone(idx) {
                    *changed = true;
                }
            }

            ui.add_space(SPACE_8);
            ui.label(
                RichText::new("Explicit folder zones:")
                    .size(11.5)
                    .color(pal.muted),
            );
            if draft.zones.explicit_zones.is_empty() {
                ui.label(
                    RichText::new("No explicit folder zones configured (e.g. D:\\Media -> Personal files).")
                        .size(10.5)
                        .color(pal.subtle_text),
                );
            } else {
                let mut remove_path_idx: Option<usize> = None;
                for kind in [ZoneRuleKind::AppContainer, ZoneRuleKind::PersonalFiles] {
                    let matching: Vec<(usize, String)> = draft
                        .zones
                        .explicit_zones
                        .iter()
                        .enumerate()
                        .filter(|(_, ez)| ez.kind == kind)
                        .map(|(i, ez)| (i, ez.path.clone()))
                        .collect();
                    if matching.is_empty() {
                        continue;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
                        ui.label(
                            RichText::new(format!("{}:", kind.label()))
                                .size(10.5)
                                .color(pal.muted),
                        );
                        let max_chip_inner_w = (inner_w - SPACE_8 * 2.0 - 2.0).max(64.0);
                        for (idx, path) in matching {
                            if draw_rulebook_removable_chip(
                                ui,
                                &path,
                                &name_font,
                                max_chip_inner_w,
                                pal,
                            ) {
                                remove_path_idx = Some(idx);
                            }
                        }
                    });
                }
                if let Some(idx) = remove_path_idx {
                    if draft.zones.remove_explicit_zone(idx) {
                        *changed = true;
                    }
                }
            }

            ui.add_space(SPACE_8);
            ui.label(
                RichText::new("Manual folder pins (override inference):")
                    .size(11.5)
                    .color(pal.muted),
            );
            if draft.zones.pins.is_empty() {
                ui.label(
                    RichText::new("No manual pins configured (App / Group / Personal / Ignore).")
                        .size(10.5)
                        .color(pal.subtle_text),
                );
            } else {
                let mut remove_pin_idx: Option<usize> = None;
                for (idx, pin) in draft.zones.pins.iter().enumerate() {
                    ui.allocate_ui_with_layout(
                        Vec2::new(inner_w, 20.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_8;
                            draw_truncated_text(
                                ui,
                                &pin.path,
                                FontId::new(11.0, FontFamily::Monospace),
                                pal.text,
                                false,
                            );
                            paint_chip(ui, pin.kind.label(), neutral_pin_style, pal.panel);
                            if draw_z_button(
                                ui,
                                "Remove",
                                Vec2::new(56.0, 20.0),
                                11.0,
                                RADIUS_PILL,
                                true,
                                false,
                                ButtonVariant::SubtleOutline,
                                pal,
                            )
                            .clicked()
                            {
                                remove_pin_idx = Some(idx);
                            }
                        },
                    );
                }
                if let Some(idx) = remove_pin_idx {
                    if draft.zones.remove_pin(idx) {
                        *changed = true;
                    }
                }
            }

            ui.add_space(SPACE_8);
            ui.label(RichText::new("Add").size(11.5).color(pal.muted));
            let zone_sel_w = measure_filter_radio_group_w(
                ui,
                &[
                    ZoneRuleKind::AppContainer.label(),
                    ZoneRuleKind::PersonalFiles.label(),
                ],
                pal,
            );
            let pin_sel_w = measure_filter_radio_group_w(
                ui,
                &[
                    PinKind::App.short_label(),
                    PinKind::Group.short_label(),
                    PinKind::Personal.short_label(),
                    PinKind::Ignore.short_label(),
                ],
                pal,
            );
            let shared_selector_w = zone_sel_w.max(pin_sel_w);
            if draw_settings_form_grid_row(
                ui,
                "zone_name_input",
                &mut ui_state.new_container_name_input,
                "Folder name (e.g. Tools, Media)...",
                "Add root zone",
                inner_w,
                Some(shared_selector_w),
                pal,
                |ui| {
                    for kind in [ZoneRuleKind::AppContainer, ZoneRuleKind::PersonalFiles] {
                        let selected = ui_state.new_container_name_kind == kind;
                        if draw_filter_radio_chip(ui, kind.label(), selected, pal) {
                            ui_state.new_container_name_kind = kind;
                        }
                    }
                },
            ) {
                let candidate = ui_state.new_container_name_input.clone();
                if draft
                    .zones
                    .add_root_zone(&candidate, ui_state.new_container_name_kind)
                {
                    ui_state.new_container_name_input.clear();
                    *changed = true;
                }
            }
            ui.add_space(SPACE_4);
            if draw_settings_form_grid_row(
                ui,
                "zone_path_input",
                &mut ui_state.new_container_path_input,
                r"D:\Media or D:\Work\PortableTools",
                "Add path zone",
                inner_w,
                Some(shared_selector_w),
                pal,
                |ui| {
                    for kind in [ZoneRuleKind::AppContainer, ZoneRuleKind::PersonalFiles] {
                        let selected = ui_state.new_container_path_kind == kind;
                        if draw_filter_radio_chip(ui, kind.label(), selected, pal) {
                            ui_state.new_container_path_kind = kind;
                        }
                    }
                },
            ) {
                let candidate = ui_state.new_container_path_input.clone();
                if draft
                    .zones
                    .add_explicit_zone(&candidate, ui_state.new_container_path_kind)
                {
                    ui_state.new_container_path_input.clear();
                    *changed = true;
                }
            }
            ui.add_space(SPACE_4);
            if draw_settings_form_grid_row(
                ui,
                "zone_pin_input",
                &mut ui_state.new_pin_path_input,
                r"D:\Apps\CustomSuite or D:\Games\Orphan",
                "Pin folder",
                inner_w,
                Some(shared_selector_w),
                pal,
                |ui| {
                    for pk in [
                        PinKind::App,
                        PinKind::Group,
                        PinKind::Personal,
                        PinKind::Ignore,
                    ] {
                        let selected = ui_state.new_pin_kind == pk;
                        if draw_filter_radio_chip(ui, pk.short_label(), selected, pal) {
                            ui_state.new_pin_kind = pk;
                        }
                    }
                },
            ) {
                let candidate = ui_state.new_pin_path_input.clone();
                if draft.zones.set_pin(&candidate, ui_state.new_pin_kind) {
                    ui_state.new_pin_path_input.clear();
                    *changed = true;
                }
            }
        },
    );

    rulebook_frame.response.rect
}

fn draw_settings_about_card(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    pane_inner_w: f32,
    pal: &ThemePalette,
) {
    let card_w = pane_inner_w.max(240.0);
    draw_crisp_frame(
        ui,
        pal.panel,
        pal.border,
        RADIUS_PANEL,
        Margin::same(SPACE_12),
        |ui| {
            ui.set_width((card_w - SPACE_12 * 2.0 - 2.0).max(200.0));
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), HEADER_CONTROL_H),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_8;
                    let _ = draw_wt_header_lockup(
                        ui,
                        &mut ui_state.wt_logo_tex,
                        "y-storage",
                        &format!(
                            "v{} · WT family read-only storage & dependency inspector",
                            env!("CARGO_PKG_VERSION")
                        ),
                        pal.muted,
                        pal,
                    );
                },
            );
        },
    );
}

fn draw_settings_view(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    config: &AppConfig,
    report: Option<&ScanReport>,
    system_windows_dir: &str,
    pal: &ThemePalette,
    emit: &mut dyn FnMut(UiAction),
) {
    let mut draft = config.clone();
    let mut changed = false;
    let sys_win_key = normalize_path_key(system_windows_dir);

    if ui_state.settings_focus_containers {
        ui_state.settings_section = SettingsSection::Rulebook;
        ui_state.settings_focus_containers = false;
    }

    ui.spacing_mut().item_spacing.y = SPACE_4;
    ui.label(RichText::new("Settings").size(16.0).color(pal.text));
    ui.add_space(SPACE_4);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(SPACE_8, SPACE_4);
        for section in SettingsSection::ALL {
            let selected = ui_state.settings_section == section;
            if draw_filter_radio_chip(ui, section.label(), selected, pal) {
                ui_state.settings_section = section;
            }
        }
    });
    ui.add_space(SPACE_8);

    let pane_inner_w = (ui.available_width().max(240.0) - SCROLL_GUTTER_W).max(240.0);
    draw_synced_scroll_area(
        ui,
        ("settings_scroll", ui_state.settings_section as u8),
        None,
        [false, false],
        pal,
        pal.bg,
        |ui| {
            ui.spacing_mut().item_spacing.y = SPACE_4;
            match ui_state.settings_section {
                SettingsSection::Appearance => {
                    draw_appearance_theme_section(
                        ui,
                        ui_state,
                        config,
                        pane_inner_w,
                        pal,
                        emit,
                    );
                }
                SettingsSection::ScanScope => {
                    draw_scan_scope_section(
                        ui,
                        ui_state,
                        &mut draft,
                        report,
                        &sys_win_key,
                        pane_inner_w,
                        pal,
                        &mut changed,
                    );
                }
                SettingsSection::Rulebook => {
                    let _ = draw_rulebook_section(
                        ui,
                        ui_state,
                        &mut draft,
                        pane_inner_w,
                        pal,
                        &mut changed,
                    );
                }
            }
            ui.add_space(SPACE_12);
            draw_settings_about_card(ui, ui_state, pane_inner_w, pal);
        },
    );

    if changed {
        ui_state.ledger_page = 0;
        ui_state.ledger_scroll_to_top = true;
        emit(UiAction::SaveConfig(draft));
    }
}

#[cfg(test)]
mod tests {
    use y_core::config::ContainerRulebook;
    use super::*;
    use egui::{Pos2, RawInput, Rect, Vec2};
    use y_core::model::{
        AppCategory, AttributedApp, AttributionBasis, BreakdownItem, DependencyRecord, DriveInfo,
        NoTouchCategory, RegistryDiscoveryStats, ReverseConsumer, SystemDriveBreakdown,
        UnattributedItem,
    };

    #[test]
    fn ui_renders_main_expanded_and_settings_views_cleanly() {
        let report = ScanReport {
            scan_timestamp_unix: 1000,
            scan_timestamp_display: "2026-11-26 18:00:00".into(),
            duration_ms: 8250,
            drives: vec![DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 200_000,
                free_bytes: 100_000,
                used_bytes: 100_000,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec!["C:\\Windows".into()],
                last_scanned_unix: None,
                last_scanned_display: None,
            }],
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: 1,
                raw_with_install_location: 1,
                canonical_apps: 1,
                canonical_with_direct_install_location: 1,
                canonical_with_inferred_install_location: 0,
            },
            apps: vec![AttributedApp {
                id: "reg:discord".into(),
                display_name: "Discord".into(),
                publishers: vec!["Discord Inc.".into()],
                install_location: Some("C:\\Users\\Win\\AppData\\Local\\Discord".into()),
                total_bytes: 50_000,
                previous_bytes: Some(40_000),
                growth: GrowthDelta::Changed(10_000),
                per_drive_growth: Default::default(),
                no_touch: None,
                bases: vec![AttributionBasis::InstallLocation, AttributionBasis::ExactName],
                items: vec![
                    BreakdownItem {
                        path: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
                        bytes: 30_000,
                        zone: Some(ZoneKind::Body),
                        qualifier: None,
                        basis: Some(AttributionBasis::InstallLocation),
                        basis_detail: None,
                        no_touch: None,
                        drive_letter: 'C',
                    },
                    BreakdownItem {
                        path: "C:\\Users\\Win\\AppData\\Roaming\\discord".into(),
                        bytes: 20_000,
                        zone: Some(ZoneKind::Red),
                        qualifier: Some(LocationQualifier::RoamingConfig),
                        basis: Some(AttributionBasis::ExactName),
                        basis_detail: None,
                        no_touch: None,
                        drive_letter: 'C',
                    },
                ],
                carved_out_apps: vec![],
                dependencies: vec![DependencyRecord {
                    target_id: "ea-app".into(),
                    target_name: "EA App".into(),
                    kind: DependencyKind::PlatformLauncher,
                    present: true,
                    own_size_bytes: Some(12_000),
                    shared: true,
                    basis: "catalog: test".into(),
                    confidence: DependencyConfidence::Proven,
                    imported_by_binary: false,
                }],
                display_icon: None,
                icon_rgba: None,
                category: y_core::model::AppCategory::default(),
                category_basis: String::new(),
            }],
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![BreakdownItem {
                path: "C:\\Users\\Win\\Documents".into(),
                bytes: 10_000,
                zone: None,
                qualifier: None,
                basis: None,
                basis_detail: None,
                no_touch: None,
                drive_letter: 'C',
            }],
            your_files_total_bytes: 10_000,
            windows_system: vec![SystemDriveBreakdown {
                drive_letter: 'C',
                root_files: vec![],
                os_folders: vec![],
                hardlink_overlap_bytes: 0,
                remainder_bytes: 35_000,
                total_bytes: 35_000,
                scanned_non_system_bytes: 65_000,
                drive_used_bytes: 100_000,
                drive_free_bytes: 100_000,
                drive_total_bytes: 200_000,
            }],
            windows_system_total_bytes: 35_000,
            unattributed: vec![UnattributedItem {
                path: "C:\\Users\\Win\\AppData\\Roaming\\TabNine".into(),
                bytes: 5_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Red),
                is_loose_files: false,
            }],
            unattributed_total_bytes: 5_000,
            reverse_dependencies: vec![ReverseDependencyEntry {
                target_id: "ea-app".into(),
                target_name: "EA App".into(),
                kind: DependencyKind::PlatformLauncher,
                present: true,
                own_size_bytes: Some(12_000),
                dependent_count: 1,
                binary_importer_count: 0,
                consumers: vec![ReverseConsumer {
                    app_id: "reg:discord".into(),
                    app_name: "Discord".into(),
                    app_total_bytes: 50_000,
                    confidence: DependencyConfidence::Proven,
                    basis: "catalog: test".into(),
                    imported_by_binary: false,
                }],
            }],
            installed_registry_names: vec![],
            applied_containers: None,
            applied_zones: None,
            catalog_source: "embedded".into(),
            warnings: vec![],
            evidence_curated_conflicts: vec![],
        };

        let ctx = egui::Context::default();
        let mut state = StorageUiState::default();
        state.expanded_apps.insert("reg:discord".into());
        state.open_reverse_dep_id = Some("ea-app".into());
        let cfg = AppConfig::default();

        for view in [ActiveView::Main, ActiveView::Dependencies, ActiveView::Settings] {
            state.active_view = view;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            };
            let out = ctx.run(raw, |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            });
            assert!(!out.shapes.is_empty());
        }
    }

    #[test]
    fn word_boundary_truncation_never_splits_mid_word_and_ends_with_ellipsis() {
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let long_basis = "manifest: __Installer/installerdata.xml (EAAntiCheat) + fingerprint: eaanticheat.gameservicelauncher.exe, eaanticheat.gameservicelauncher.dll + catalog: EA Sports title requires EA App client & EABackgroundService";
        let font = FontId::new(10.5, FontFamily::Proportional);
        let mut capped_out = String::new();
        let mut truncated_flag = false;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 200.0))),
                ..Default::default()
            },
            |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let (capped, trunc) = truncate_word_boundary_px(ui, long_basis, &font, 320.0);
                    capped_out = capped;
                    truncated_flag = trunc;
                });
            },
        );
        assert!(truncated_flag);
        assert!(capped_out.ends_with('…'));
        assert!(!capped_out.contains("eaanticheat.gameservice…"));
        let body = capped_out.trim_end_matches('…');
        assert!(long_basis.starts_with(body));
        let next_char = long_basis[body.len()..].chars().next().unwrap();
        assert!(
            next_char.is_whitespace()
                || matches!(next_char, '\\' | '/' | '.' | ',' | ';' | ')' | '|' | ':' | '+'),
            "expected segment boundary after {body:?}, next char was {next_char:?}"
        );
    }

    #[test]
    fn segment_boundary_truncation_splits_dotted_filenames_and_paths() {
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let sample = "fingerprint: eaanticheat.gameservicelauncher.exe + C:\\Program Files\\Epic Games\\Epic Online Services\\EpicOnlineServicesHost.exe";
        let font = FontId::new(10.5, FontFamily::Proportional);
        let mut capped_out = String::new();
        let mut truncated_flag = false;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 200.0))),
                ..Default::default()
            },
            |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let (capped, trunc) = truncate_word_boundary_px(ui, sample, &font, 230.0);
                    capped_out = capped;
                    truncated_flag = trunc;
                });
            },
        );
        assert!(truncated_flag);
        assert!(capped_out.ends_with('…'));
        assert!(!capped_out.ends_with(",…") && !capped_out.ends_with("+…"));
        let body = capped_out.trim_end_matches('…');
        assert!(sample.starts_with(body));
        let next_char = sample[body.len()..].chars().next().unwrap();
        assert!(
            next_char.is_whitespace() || matches!(next_char, '\\' | '/' | '.' | ',' | ';' | ')' | '|' | '+'),
            "expected clean segment boundary after {body:?}, next char was {next_char:?}"
        );
    }

    #[test]
    fn row_virtualization_renders_only_visible_window_and_matches_expanded_height() {
        let mut apps = Vec::with_capacity(262);
        for i in 0..262usize {
            let deps = if i == 0 {
                vec![
                    DependencyRecord {
                        target_id: "ea-app".into(),
                        target_name: "EA App".into(),
                        kind: DependencyKind::PlatformLauncher,
                        present: true,
                        own_size_bytes: Some(1_000_000),
                        shared: true,
                        basis: "manifest + fingerprint".into(),
                        confidence: DependencyConfidence::Proven,
                        imported_by_binary: false,
                    },
                    DependencyRecord {
                        target_id: "dx".into(),
                        target_name: "DirectX".into(),
                        kind: DependencyKind::SharedRuntime,
                        present: true,
                        own_size_bytes: None,
                        shared: true,
                        basis: "PE import".into(),
                        confidence: DependencyConfidence::Proven,
                        imported_by_binary: true,
                    },
                ]
            } else {
                vec![]
            };
            apps.push(AttributedApp {
                id: format!("app:{i:03}"),
                display_name: format!("App {i:03}"),
                publishers: vec!["Publisher".into()],
                install_location: None,
                total_bytes: 1_000_000 - (i as u64) * 1000,
                previous_bytes: None,
                growth: GrowthDelta::NoBaseline,
                per_drive_growth: Default::default(),
                no_touch: None,
                bases: vec![AttributionBasis::ExactName],
                items: vec![BreakdownItem {
                    path: format!("C:\\Apps\\App{i:03}"),
                    bytes: 1_000_000 - (i as u64) * 1000,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::ExactName),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: 'C',
                }],
                carved_out_apps: vec![],
                dependencies: deps,
                display_icon: None,
                icon_rgba: None,
                category: y_core::model::AppCategory::default(),
                category_basis: String::new(),
            });
        }
        let expanded_estimate = estimate_expanded_row_height(&apps[0]);
        let report = ScanReport {
            scan_timestamp_unix: 2000,
            scan_timestamp_display: "2026-11-26 22:00:00".into(),
            duration_ms: 1000,
            drives: vec![DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 500_000_000,
                free_bytes: 250_000_000,
                used_bytes: 250_000_000,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            }],
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: 262,
                raw_with_install_location: 262,
                canonical_apps: 262,
                canonical_with_direct_install_location: 262,
                canonical_with_inferred_install_location: 0,
            },
            apps,
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![],
            your_files_total_bytes: 0,
            windows_system: vec![SystemDriveBreakdown {
                drive_letter: 'C',
                root_files: vec![],
                os_folders: vec![],
                hardlink_overlap_bytes: 0,
                remainder_bytes: 250_000_000,
                total_bytes: 250_000_000,
                scanned_non_system_bytes: 0,
                drive_used_bytes: 250_000_000,
                drive_free_bytes: 250_000_000,
                drive_total_bytes: 500_000_000,
            }],
            windows_system_total_bytes: 250_000_000,
            unattributed: vec![],
            unattributed_total_bytes: 0,
            reverse_dependencies: vec![],
            installed_registry_names: vec![],
            applied_containers: None,
            applied_zones: None,
            catalog_source: "embedded".into(),
            warnings: vec![],
            evidence_curated_conflicts: vec![],
        };

        let ctx = egui::Context::default();
        let mut state = StorageUiState::default();
        state.expanded_apps.insert("app:000".into());
        let cfg = AppConfig::default();
        for _ in 0..2 {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            });
        }

        let actual_h = *state.expanded_row_heights.get("app:000").expect("expanded row measured");
        assert!(
            (actual_h - expanded_estimate).abs() <= 1.0,
            "expected expanded height estimate {expanded_estimate} ~= actual {actual_h}"
        );
        // Virtualization keeps per-frame shape count bounded to visible rows (~270 shapes with "new" status pills vs >2,500 unvirtualized).
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
            ..Default::default()
        };
        let out = ctx.run(raw, |c| {
            draw_app_ui(
                c,
                &mut state,
                &cfg,
                Some(&report),
                false,
                None,
                "C:\\Windows",
                &mut |_| {},
            );
        });
        assert!(out.shapes.len() < 340, "expected virtualized shape count < 340, got {}", out.shapes.len());
    }

    #[test]
    fn unattributed_rows_render_locate_and_disabled_missing_folder_cleanly() {
        let existing_dir = std::env::temp_dir()
            .to_string_lossy()
            .into_owned();
        let missing_dir = r"C:\__y_storage_missing_unattributed_folder_test__";
        let report = ScanReport {
            scan_timestamp_unix: 3000,
            scan_timestamp_display: "2026-11-26 23:30:00".into(),
            duration_ms: 500,
            drives: vec![DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 500_000_000,
                free_bytes: 250_000_000,
                used_bytes: 250_000_000,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            }],
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: 0,
                raw_with_install_location: 0,
                canonical_apps: 0,
                canonical_with_direct_install_location: 0,
                canonical_with_inferred_install_location: 0,
            },
            apps: vec![],
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![],
            your_files_total_bytes: 0,
            windows_system: vec![SystemDriveBreakdown {
                drive_letter: 'C',
                root_files: vec![],
                os_folders: vec![],
                hardlink_overlap_bytes: 0,
                remainder_bytes: 249_990_000,
                total_bytes: 249_990_000,
                scanned_non_system_bytes: 10_000,
                drive_used_bytes: 250_000_000,
                drive_free_bytes: 250_000_000,
                drive_total_bytes: 500_000_000,
            }],
            windows_system_total_bytes: 249_990_000,
            unattributed: vec![
                UnattributedItem {
                    path: existing_dir,
                    bytes: 6_000,
                    drive_letter: 'C',
                    zone_hint: None,
                    is_loose_files: false,
                },
                UnattributedItem {
                    path: missing_dir.into(),
                    bytes: 4_000,
                    drive_letter: 'C',
                    zone_hint: None,
                    is_loose_files: false,
                },
            ],
            unattributed_total_bytes: 10_000,
            reverse_dependencies: vec![],
            installed_registry_names: vec![],
            applied_containers: None,
            applied_zones: None,
            catalog_source: "embedded".into(),
            warnings: vec![],
            evidence_curated_conflicts: vec![],
        };

        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut state = StorageUiState::default();
        state.expand_unattributed = true;
        state.ledger_height_px = Some(280.0);
        let cfg = AppConfig::default();

        for step in 0..2 {
            let mut events = Vec::new();
            if step == 1 {
                if let Some(wr) = state.first_unattrib_web_search_rect {
                    events.push(egui::Event::PointerMoved(wr.center()));
                }
            }
            let out = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                    time: Some(step as f64),
                    events,
                    ..Default::default()
                },
                |c| {
                    draw_app_ui(
                        c,
                        &mut state,
                        &cfg,
                        Some(&report),
                        false,
                        None,
                        "C:\\Windows",
                        &mut |_| {},
                    );
                },
            );
            assert!(!out.shapes.is_empty());
        }
        assert!(state.first_unattrib_web_search_rect.is_some());
        assert_eq!(state.unattrib_exists_cache.get(missing_dir), Some(&false));
    }

    #[test]
    fn button_hierarchy_reserves_accent_for_primary_active_and_hover() {
        for preset in [ThemePresetChoice::Blue, ThemePresetChoice::Pink, ThemePresetChoice::Green] {
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                let pal = ThemePalette::from_config(mode, preset, default_custom_theme_rgb());

                assert_eq!(
                    resolve_button_colors(
                        ButtonVariant::PrimaryOutline,
                        true,
                        false,
                        false,
                        false,
                        &pal
                    ),
                    (pal.accent_soft_fill, pal.accent_border, pal.accent_line)
                );
                assert_eq!(
                    resolve_button_colors(
                        ButtonVariant::PrimaryOutline,
                        true,
                        false,
                        true,
                        false,
                        &pal
                    ),
                    (pal.widget_hover, pal.accent_border, pal.accent_line)
                );
                assert_eq!(
                    resolve_checkbox_colors(true, false, &pal),
                    (pal.accent_soft_fill, pal.accent_border, pal.accent_line)
                );

                // 2. SecondaryOutline: neutral at rest, accent on hover
                assert_eq!(
                    resolve_button_colors(ButtonVariant::SecondaryOutline, true, false, false, false, &pal),
                    (Color32::TRANSPARENT, pal.border, pal.text)
                );
                assert_eq!(
                    resolve_button_colors(ButtonVariant::SecondaryOutline, true, false, true, false, &pal),
                    (pal.widget_hover, pal.accent_border, pal.accent_line)
                );

                // 3. SubtleOutline: quiet at rest, accent on hover / active
                assert_eq!(
                    resolve_button_colors(ButtonVariant::SubtleOutline, true, false, false, false, &pal),
                    (Color32::TRANSPARENT, Color32::TRANSPARENT, pal.muted)
                );
                assert_eq!(
                    resolve_button_colors(ButtonVariant::SubtleOutline, true, false, true, false, &pal),
                    (pal.widget_hover, Color32::TRANSPARENT, pal.accent_line)
                );
                assert_eq!(
                    resolve_button_colors(ButtonVariant::SubtleOutline, true, false, false, true, &pal),
                    (pal.accent_soft_fill, pal.accent_border, pal.accent_line)
                );
                assert_eq!(
                    resolve_button_colors(ButtonVariant::SubtleOutline, false, false, false, false, &pal),
                    (
                        Color32::TRANSPARENT,
                        Color32::TRANSPARENT,
                        lerp_color(pal.bg, pal.muted, 0.55)
                    )
                );
            }
        }
    }

    #[test]
    fn ledger_full_height_resize_clamps_and_degrades_cleanly_at_default_and_min_window() {
        let report = ScanReport {
            scan_timestamp_unix: 4000,
            scan_timestamp_display: "2026-11-27 16:00:00".into(),
            duration_ms: 500,
            drives: vec![DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 500_000_000,
                free_bytes: 250_000_000,
                used_bytes: 250_000_000,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec![],
                last_scanned_unix: None,
                last_scanned_display: None,
            }],
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: 1,
                raw_with_install_location: 1,
                canonical_apps: 1,
                canonical_with_direct_install_location: 1,
                canonical_with_inferred_install_location: 0,
            },
            apps: vec![AttributedApp {
                id: "reg:sample".into(),
                display_name: "Sample".into(),
                publishers: vec!["Pub".into()],
                install_location: Some("C:\\Sample".into()),
                total_bytes: 100_000,
                previous_bytes: None,
                growth: GrowthDelta::NoBaseline,
                per_drive_growth: Default::default(),
                no_touch: None,
                bases: vec![AttributionBasis::InstallLocation],
                items: vec![BreakdownItem {
                    path: "C:\\Sample".into(),
                    bytes: 100_000,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::InstallLocation),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: 'C',
                }],
                carved_out_apps: vec!["Child App".into()],
            dependencies: vec![],
                display_icon: None,
                icon_rgba: None,
                category: y_core::model::AppCategory::default(),
                category_basis: String::new(),
            }],
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![],
            your_files_total_bytes: 0,
            windows_system: vec![SystemDriveBreakdown {
                drive_letter: 'C',
                root_files: vec![],
                os_folders: vec![],
                hardlink_overlap_bytes: 0,
                remainder_bytes: 249_900_000,
                total_bytes: 249_900_000,
                scanned_non_system_bytes: 100_000,
                drive_used_bytes: 250_000_000,
                drive_free_bytes: 250_000_000,
                drive_total_bytes: 500_000_000,
            }],
            windows_system_total_bytes: 249_900_000,
            unattributed: vec![],
            unattributed_total_bytes: 0,
            reverse_dependencies: vec![],
            installed_registry_names: vec![],
            applied_containers: None,
            applied_zones: None,
            catalog_source: "embedded".into(),
            warnings: vec![],
            evidence_curated_conflicts: vec![],
        };

        let cfg = AppConfig::default();
        for (w, h) in [(1020.0_f32, 720.0_f32), (760.0_f32, 520.0_f32)] {
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, ThemeMode::Dark);
            let mut state = StorageUiState::default();
            state.ledger_height_px = Some(2000.0);
            let out = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))),
                    ..Default::default()
                },
                |c| {
                    draw_app_ui(
                        c,
                        &mut state,
                        &cfg,
                        Some(&report),
                        false,
                        None,
                        "C:\\Windows",
                        &mut |_| {},
                    );
                },
            );
            assert!(!out.shapes.is_empty());
            let clamped = state.ledger_height_px.expect("ledger height retained");
            assert!(
                clamped >= 156.0 && clamped < h,
                "expected clamped ledger height in [156, {h}), got {clamped}"
            );
            if w > 900.0 {
                assert!(
                    clamped > h * 0.50,
                    "expected default 1020x720 ledger max > 50% ({}), got {clamped}",
                    h * 0.50
                );
            }
        }
    }

    #[test]
    fn text_input_sets_ibeam_cursor_while_buttons_do_not() {
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let cfg = AppConfig::default();
        let mut state = StorageUiState::default();

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(c, &mut state, &cfg, None, false, None, "C:\\Windows", &mut |_| {});
            },
        );

        let filter_rect = ctx
            .memory(|m| m.area_rect(egui::Id::new("header_panel")))
            .map(|_| Pos2::new(120.0, 72.0))
            .unwrap_or(Pos2::new(120.0, 72.0));
        let over_input = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                events: vec![egui::Event::PointerMoved(filter_rect)],
                ..Default::default()
            },
            |c| {
                draw_app_ui(c, &mut state, &cfg, None, false, None, "C:\\Windows", &mut |_| {});
            },
        );
        assert_eq!(over_input.platform_output.cursor_icon, egui::CursorIcon::Text);

        let over_static_or_btn = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                events: vec![egui::Event::PointerMoved(Pos2::new(880.0, 26.0))],
                ..Default::default()
            },
            |c| {
                draw_app_ui(c, &mut state, &cfg, None, false, None, "C:\\Windows", &mut |_| {});
            },
        );
        assert_ne!(
            over_static_or_btn.platform_output.cursor_icon,
            egui::CursorIcon::Text
        );
    }

    fn make_synthetic_report_for_drives(count: usize) -> (ScanReport, AppConfig) {
        let letters = ['C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L'];
        let mut drives = Vec::new();
        let mut targets = Vec::new();
        let mut windows_system = Vec::new();
        for idx in 0..count.min(letters.len()) {
            let dl = letters[idx];
            let is_sys = idx == 0;
            let total_bytes = 500_000_000_000_u64 + (idx as u64) * 125_000_000_000;
            let used_bytes = (total_bytes as f64 * (0.45 + (idx % 4) as f64 * 0.12)) as u64;
            let free_bytes = total_bytes - used_bytes;
            let excl = if is_sys {
                vec!["C:\\Windows".into()]
            } else if idx == 1 {
                vec![format!("{dl}:\\Backup")]
            } else {
                vec![]
            };
            let enabled = !(count > 1 && idx == 1);
            drives.push(DriveInfo {
                letter: dl,
                root: format!("{dl}:\\"),
                total_bytes,
                free_bytes,
                used_bytes,
                is_system_drive: is_sys,
                full_walk_enabled: enabled,
                exclusions: excl.clone(),
                last_scanned_unix: enabled.then_some(4_000),
                last_scanned_display: enabled.then_some("2026-11-27 18:10:00".into()),
            });
            targets.push(y_core::config::DriveScanTarget {
                letter: dl,
                enabled,
                exclusions: excl,
            });
            if enabled {
                windows_system.push(SystemDriveBreakdown {
                    drive_letter: dl,
                    root_files: vec![],
                    os_folders: vec![],
                    hardlink_overlap_bytes: 0,
                    remainder_bytes: used_bytes,
                    total_bytes: used_bytes,
                    scanned_non_system_bytes: 0,
                    drive_used_bytes: used_bytes,
                    drive_free_bytes: free_bytes,
                    drive_total_bytes: total_bytes,
                });
            }
        }
        let containers = ContainerRulebook::default();
        let mut zones = ZoneRulebook::default();
        let _ = zones.add_explicit_zone(r"D:\Media", ZoneRuleKind::PersonalFiles);
        let _ = zones.set_pin(r"D:\Games\Orphan", PinKind::Ignore);
        let mut cfg = AppConfig::default();
        cfg.targets = targets;
        cfg.containers = containers.clone();
        cfg.zones = zones.clone();
        let report = ScanReport {
            scan_timestamp_unix: 4_000,
            scan_timestamp_display: "2026-11-27 18:10:00".into(),
            duration_ms: 1200,
            drives,
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: 1,
                raw_with_install_location: 1,
                canonical_apps: 1,
                canonical_with_direct_install_location: 1,
                canonical_with_inferred_install_location: 0,
            },
            apps: vec![AttributedApp {
                id: "reg:sample".into(),
                display_name: "Sample Tool".into(),
                publishers: vec!["Pub".into()],
                install_location: Some("C:\\Apps\\Sample".into()),
                total_bytes: 100_000,
                previous_bytes: None,
                growth: GrowthDelta::NoBaseline,
                per_drive_growth: Default::default(),
                no_touch: None,
                bases: vec![AttributionBasis::ContainerRule],
                items: vec![BreakdownItem {
                    path: "C:\\Apps\\Sample".into(),
                    bytes: 100_000,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::ContainerRule),
                    basis_detail: Some("layout rule: Apps".into()),
                    no_touch: None,
                    drive_letter: 'C',
                }],
                carved_out_apps: vec!["Child App".into()],
            dependencies: vec![],
                display_icon: None,
                icon_rgba: None,
                category: y_core::model::AppCategory::Utilities,
                category_basis: "rule".into(),
            }],
            shared_caches: vec![],
            shared_caches_total_bytes: 0,
            your_files: vec![],
            your_files_total_bytes: 0,
            windows_system,
            windows_system_total_bytes: 0,
            unattributed: vec![],
            unattributed_total_bytes: 0,
            reverse_dependencies: vec![],
            installed_registry_names: vec![],
            applied_containers: Some(containers),
            applied_zones: Some(zones),
            catalog_source: "embedded".into(),
            warnings: vec![],
            evidence_curated_conflicts: vec![],
        };
        (report, cfg)
    }

    #[test]
    fn ledger_covers_pods_and_filter_row_at_max_height() {
        let (report, cfg) = make_synthetic_report_for_drives(4);
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut state = StorageUiState::default();
        state.expand_unattributed = true;
        state.ledger_height_px = Some(2000.0);

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            },
        );

        let clamped = state.ledger_height_px.expect("ledger height retained");
        let masthead_h = ctx
            .memory(|m| m.area_rect(egui::Id::new("masthead_panel")))
            .map_or(48.0, |r| r.height());
        let expected_max = 720.0 - masthead_h - 4.0;
        assert!(
            (clamped - expected_max).abs() <= 2.0,
            "expected ledger max height {clamped} ~= {expected_max} (covering pods + filter row below masthead {masthead_h})"
        );
    }

    #[test]
    fn pod_scope_controls_and_settings_container_rulebook_render_and_mark_stale() {
        let (report, cfg) = make_synthetic_report_for_drives(3);
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut state = StorageUiState::default();
        state.open_pod_scope_drive = Some('C');

        let main_out = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            },
        );
        assert!(!main_out.shapes.is_empty());
        assert!(!report.is_scope_stale(&cfg));

        let mut modified_cfg = cfg.clone();
        assert!(modified_cfg.zones.add_root_zone("CustomTools", ZoneRuleKind::AppContainer));
        assert!(report.is_scope_stale(&modified_cfg));

        state.active_view = ActiveView::Settings;
        state.settings_focus_containers = true;
        let settings_out = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &modified_cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            },
        );
        assert!(!settings_out.shapes.is_empty());
    }

    #[test]
    fn adaptive_drive_pod_grid_fits_1_2_5_10_drives_without_overflow() {
        for count in [1usize, 2, 5, 10] {
            let (report, cfg) = make_synthetic_report_for_drives(count);
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, ThemeMode::Dark);
            let mut state = StorageUiState::default();

            let out = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                    ..Default::default()
                },
                |c| {
                    draw_app_ui(
                        c,
                        &mut state,
                        &cfg,
                        Some(&report),
                        false,
                        None,
                        "C:\\Windows",
                        &mut |_| {},
                    );
                },
            );
            let pod_r = state.first_drive_pod_rect.expect("drive pod rendered");
            assert!(pod_r.width() >= 140.0 && pod_r.height() >= 60.0);
            for cs in &out.shapes {
                let bb = cs.shape.visual_bounding_rect();
                if bb.is_positive() {
                    assert!(
                        bb.max.x <= 1020.5 && bb.min.x >= -0.5,
                        "shape overflow for {count} drives: {bb:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn expanded_app_drawer_locations_render_locate_and_web_search_with_missing_folder_guard() {
        let existing_dir = std::env::temp_dir().to_string_lossy().into_owned();
        let missing_loc = r"C:\__y_storage_missing_app_location_test__";
        let (mut report, cfg) = make_synthetic_report_for_drives(2);
        report.apps = vec![AttributedApp {
            id: "reg:loc-actions".into(),
            display_name: "Location Actions Test".into(),
            publishers: vec!["TestPub".into()],
            install_location: Some(existing_dir.clone()),
            total_bytes: 800_000,
            previous_bytes: None,
            growth: GrowthDelta::NoBaseline,
            per_drive_growth: Default::default(),
            no_touch: None,
            bases: vec![AttributionBasis::InstallLocation, AttributionBasis::ExactName],
            items: vec![
                BreakdownItem {
                    path: existing_dir.clone(),
                    bytes: 600_000,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::InstallLocation),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: 'C',
                },
                BreakdownItem {
                    path: missing_loc.to_string(),
                    bytes: 200_000,
                    zone: Some(ZoneKind::Red),
                    qualifier: None,
                    basis: Some(AttributionBasis::ExactName),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: 'C',
                },
            ],
            carved_out_apps: vec!["Child App".into()],
            dependencies: vec![],
            display_icon: None,
            icon_rgba: None,
            category: y_core::model::AppCategory::Utilities,
            category_basis: "rule".into(),
        }];

        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut state = StorageUiState::default();
        state.expanded_apps.insert("reg:loc-actions".into());
        let out = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            },
        );
        assert!(!out.shapes.is_empty());
        assert_eq!(state.unattrib_exists_cache.get(&existing_dir), Some(&true));
        assert_eq!(state.unattrib_exists_cache.get(missing_loc), Some(&false));
    }

    #[test]
    fn per_drive_size_mode_relabels_header_and_renders_companion_totals() {
        let (mut report, cfg) = make_synthetic_report_for_drives(2);
        report.apps[0].items.push(BreakdownItem {
            path: r"D:\Data\SampleTool".into(),
            bytes: 5_000_000_000,
            zone: Some(ZoneKind::Body),
            qualifier: None,
            basis: Some(AttributionBasis::InstallLocation),
            basis_detail: None,
            no_touch: None,
            drive_letter: 'D',
        });
        report.apps[0].total_bytes = 5_000_100_000;

        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut state = StorageUiState::default();
        state.facets.drive = Some('C');

        let out = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut state,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    "C:\\Windows",
                    &mut |_| {},
                );
            },
        );
        assert!(!out.shapes.is_empty());
    }

    #[test]
    fn growth_noise_floor_blanks_under_one_mb_and_uses_new_pill() {
        let dark = ThemePalette::for_mode(ThemeMode::Dark);
        let light = ThemePalette::for_mode(ThemeMode::Light);
        assert_ne!(dark.status_amber, dark.status_red);
        assert_ne!(light.status_amber, light.status_red);

        let sub_pos = format_delta_display(GrowthDelta::Changed(614_810), &dark);
        assert!(matches!(sub_pos.kind, DeltaCellKind::Blank));
        assert!(sub_pos.tip.as_deref().is_some_and(|t| t.contains("614,810 bytes")));

        let sub_neg = format_delta_display(GrowthDelta::Changed(-30_802), &dark);
        assert!(matches!(sub_neg.kind, DeltaCellKind::Blank));
        assert!(sub_neg.tip.as_deref().is_some_and(|t| t.contains("30,802 bytes")));

        let at_floor = format_delta_display(GrowthDelta::Changed(1_048_576), &dark);
        assert!(matches!(
            at_floor.kind,
            DeltaCellKind::SignedValue { ref text, .. } if text == "+1.00 MB"
        ));

        let is_new = format_delta_display(GrowthDelta::New(42), &dark);
        assert!(matches!(is_new.kind, DeltaCellKind::NewPill));

        let pod_colors = resolve_button_colors(
            ButtonVariant::PrimaryOutline,
            true,
            false,
            false,
            false,
            &dark,
        );
        assert_eq!(pod_colors.0, dark.accent_soft_fill);
        assert_eq!(pod_colors.1, dark.accent_border);
        assert_eq!(pod_colors.2, dark.accent_line);

        let dep_ctx = egui::Context::default();
        configure_fonts_and_theme(&dep_ctx, ThemeMode::Dark);
        let mut dep_capped = String::new();
        let _ = dep_ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 120.0))),
                ..Default::default()
            },
            |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let (capped, trunc) = truncate_word_boundary_px(
                        ui,
                        "Microsoft Edge Elevation Service (MicrosoftEdgeElevationServiceExtraLongToken)",
                        &FontId::new(12.0, FontFamily::Proportional),
                        240.0,
                    );
                    assert!(trunc);
                    dep_capped = capped;
                });
            },
        );
        assert_eq!(dep_capped, "Microsoft Edge Elevation Service…");
    }

    #[test]
    fn ledger_clips_above_reconciliation_across_min_and_default_windows() {
        let (mut report, cfg) = make_synthetic_report_for_drives(4);
        for i in 0..80 {
            report.unattributed.push(UnattributedItem {
                path: format!(r"C:\Users\Win\AppData\Local\UnattribItem{i:03}"),
                bytes: 50_000_000 - (i as u64) * 100_000,
                zone_hint: None,
                drive_letter: 'C',
                is_loose_files: false,
            });
        }
        for (w, h, budget) in [
            (760.0_f32, 520.0_f32, Some(220.0_f32)),
            (760.0_f32, 520.0_f32, Some(900.0_f32)),
            (1020.0_f32, 720.0_f32, Some(280.0_f32)),
            (1020.0_f32, 720.0_f32, Some(2000.0_f32)),
        ] {
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, ThemeMode::Dark);
            let mut state = StorageUiState::default();
            state.ledger_collapsed = false;
            state.expand_unattributed = true;
            state.ledger_height_px = budget;
            state.scroll_ledger_bottom = true;
            for _ in 0..2 {
                let out = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))),
                        ..Default::default()
                    },
                    |c| {
                        draw_app_ui(
                            c,
                            &mut state,
                            &cfg,
                            Some(&report),
                            false,
                            None,
                            r"C:\Windows",
                            &mut |_| {},
                        );
                    },
                );
                assert!(!out.shapes.is_empty());
            }
        }
    }

    #[test]
    fn monochrome_theme_engine_presets_custom_contrast_and_status_tokens() {
        let dark = ThemePalette::for_mode(ThemeMode::Dark);
        let light = ThemePalette::for_mode(ThemeMode::Light);

        // Neutral surfaces strictly untinted (R == G == B) in both Dark and Light across all presets
        for preset in ThemePresetChoice::PRESETS {
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                let p = ThemePalette::from_config(mode, preset, default_custom_theme_rgb());
                for surface in [p.bg, p.panel, p.panel_alt, p.border] {
                    assert_eq!(surface.r(), surface.g());
                    assert_eq!(surface.g(), surface.b());
                }
                if mode == ThemeMode::Light && matches!(preset, ThemePresetChoice::Blue | ThemePresetChoice::Pink) {
                    let expected = if preset == ThemePresetChoice::Blue {
                        Color32::from_rgb(0x00, 0x90, 0xD9)
                    } else {
                        Color32::from_rgb(0xFA, 0x29, 0xAA)
                    };
                    assert_eq!(p.accent_line, expected);
                    assert!(contrast_ratio(p.accent_line, p.panel) >= 3.0);
                } else {
                    assert!(
                        contrast_ratio(p.accent_line, p.bg) >= 4.5
                            && contrast_ratio(p.accent_line, p.panel) >= 4.5,
                        "preset {preset:?} in {mode:?} failed 4.5:1 contrast"
                    );
                    assert!(
                        contrast_ratio(p.accent_pill_text, p.accent_soft_fill) >= 4.5,
                        "preset {preset:?} in {mode:?} failed pill contrast"
                    );
                }
            }
        }

        // Default WT Blue tokens match WT_UI_LIBRARY §2.1
        assert_eq!(dark.bg, Color32::from_rgb(0x0A, 0x0A, 0x0A));
        assert_eq!(light.bg, Color32::from_rgb(0xF2, 0xF2, 0xF2));
        assert_eq!(dark.accent_line, Color32::from_rgb(0x69, 0xCB, 0xFC));
        assert_eq!(light.accent_line, Color32::from_rgb(0x00, 0x90, 0xD9));

        // Raw yellow custom color normalizes to >= 4.5:1 in Light mode
        let custom_yellow_light =
            ThemePalette::from_config(ThemeMode::Light, ThemePresetChoice::Custom, [0xFA, 0xCC, 0x15]);
        assert!(contrast_ratio(custom_yellow_light.accent_line, custom_yellow_light.panel) >= 4.5);
        assert!(custom_yellow_light.near_status_hue);

        // Very dark custom color normalizes to >= 4.5:1 in Dark mode
        let custom_dark_ink =
            ThemePalette::from_config(ThemeMode::Dark, ThemePresetChoice::Custom, [0x08, 0x10, 0x18]);
        assert!(contrast_ratio(custom_dark_ink.accent_line, custom_dark_ink.panel) >= 4.5);

        // Qualifier pill belongs to active monochrome family (never hardcoded pink on non-pink themes)
        let green_dark =
            ThemePalette::from_config(ThemeMode::Dark, ThemePresetChoice::Green, [0, 0, 0]);
        let q = qualifier_chip_style(&green_dark);
        assert_eq!(q.fill, green_dark.accent_soft_fill);
        assert_eq!(q.stroke, green_dark.accent_border);
        assert_eq!(q.text, green_dark.accent_pill_text);
        assert!(green_dark.near_status_hue);
    }

    #[test]
    fn rounded_corners_emit_antialiased_intermediate_coverage_across_dpi_scales() {
        let pal = ThemePalette::for_mode(ThemeMode::Dark);
        for ppp in [1.0_f32, 1.25, 1.5] {
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, ThemeMode::Dark);
            let w = (220.0 * ppp).round() as usize;
            let h = (140.0 * ppp).round() as usize;
            let mut raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(220.0, 140.0))),
                ..Default::default()
            };
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(ppp);
            let mut box_rect_out = Rect::NOTHING;
            let out = ctx.run(raw, |c| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(pal.bg))
                    .show(c, |ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                        box_rect_out = paint_crisp_bordered_box(
                            ui.painter(),
                            r,
                            RADIUS_PILL,
                            pal.accent_soft_fill,
                            pal.accent_border,
                            1.0,
                            c.pixels_per_point(),
                        );
                    });
            });
            let mut painter = crate::sw_painter::SoftwarePainter::new();
            painter.apply_textures(&out.textures_delta);
            let clipped = ctx.tessellate(out.shapes, out.pixels_per_point);
            painter.paint_and_blit(
                std::ptr::null_mut(),
                w,
                h,
                out.pixels_per_point,
                &clipped,
                crate::theme::CLEAR_BG_DARK_RGB,
                None,
            );
            let fb = painter.backbuffer_pixels();
            let px0 = (box_rect_out.min.x * ppp).round() as usize;
            let py0 = (box_rect_out.min.y * ppp).round() as usize;
            let corner_sz = (RADIUS_PILL * ppp).ceil() as usize + 2;
            let mut intermediate_corner_px = 0usize;
            for py in py0..(py0 + corner_sz).min(h) {
                for px in px0..(px0 + corner_sz).min(w) {
                    let b = fb[py * w + px] & 0xFF;
                    if b > pal.accent_soft_fill.b() as u32 && b < pal.accent_border.b() as u32 {
                        intermediate_corner_px += 1;
                    }
                }
            }
            assert!(
                intermediate_corner_px > 0,
                "expected anti-aliased intermediate coverage in corner at ppp={ppp}"
            );
        }
    }

    #[test]
    fn rounded_box_9slice_uses_fast_interior_quads_and_preserves_corner_aa() {
        let pal = ThemePalette::for_mode(ThemeMode::Dark);
        let mesh = build_feathered_rounded_box_mesh(
            Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(800.0, 400.0)),
            RADIUS_PANEL,
            pal.panel,
            pal.border,
            1.0,
            1.25,
        );
        // 3 interior axis-aligned quads (6 tris) + 4 * 16 corner fans (64 tris) + 3 perimeter rings
        // instead of 132 full-height center fan triangles.
        let tri_count = mesh.indices.len() / 3;
        assert!(tri_count <= 520, "unexpectedly dense 9-slice mesh: {tri_count}");
    }

    #[test]
    fn pod_control_row_include_and_exclusions_share_font_size_and_baseline() {
        let pal = ThemePalette::for_mode(ThemeMode::Dark);
        let cfg = AppConfig::for_detected_drives(&[DriveInfo {
            letter: 'C',
            root: "C:\\".into(),
            total_bytes: 500_000_000,
            free_bytes: 250_000_000,
            used_bytes: 250_000_000,
            is_system_drive: true,
            full_walk_enabled: true,
            exclusions: vec!["C:\\Windows".into()],
            last_scanned_unix: None,
            last_scanned_display: None,
        }], r"C:\Windows");
        for ppp in [1.0_f32, 1.25, 1.5] {
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, ThemeMode::Dark);
            let mut raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(260.0, 60.0))),
                ..Default::default()
            };
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(ppp);
            let mut inc_y = 0.0_f32;
            let mut exc_y = 0.0_f32;
            let mut inc_sz = 0.0_f32;
            let mut exc_sz = 0.0_f32;
            let _ = cfg.is_drive_enabled('C');
            let _ = ctx.run(raw, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let (ctrl_rect, _) = ui.allocate_exact_size(Vec2::new(220.0, 24.0), Sense::hover());
                    let mut lc_ui = table_cell_ui(
                        ui,
                        ctrl_rect,
                        Layout::left_to_right(Align::Center),
                    );
                    let mut checked = true;
                    draw_compact_pod_checkbox(&mut lc_ui, &mut checked, "Include", 11.0, 24.0, false, "", &pal);
                    draw_z_button(
                        &mut lc_ui,
                        "Exclusions (1)",
                        Vec2::new(0.0, 20.0),
                        11.0,
                        RADIUS_PILL,
                        true,
                        false,
                        ButtonVariant::SubtleOutline,
                        &pal,
                    );
                    let f = FontId::new(11.0, FontFamily::Proportional);
                    let gi = lc_ui.fonts(|fnt| fnt.layout_no_wrap("Include".into(), f.clone(), pal.text));
                    let ge = lc_ui.fonts(|fnt| fnt.layout_no_wrap("Exclusions (1)".into(), f.clone(), pal.muted));
                    let p_inc = optical_galley_pos(&lc_ui, ctrl_rect, &gi, None, false);
                    let btn_rect = Rect::from_center_size(Pos2::new(100.0, ctrl_rect.center().y), Vec2::new(80.0, 20.0));
                    let p_exc = optical_galley_pos(&lc_ui, btn_rect, &ge, None, true);
                    inc_y = p_inc.y;
                    exc_y = p_exc.y;
                    inc_sz = gi.job.sections[0].format.font_id.size;
                    exc_sz = ge.job.sections[0].format.font_id.size;
                });
            });
            assert_eq!(inc_sz, exc_sz);
            assert!(
                ((inc_y - exc_y) * ppp).abs() <= 0.5,
                "baseline drift too large at ppp={ppp}: inc_y={inc_y}, exc_y={exc_y}"
            );
        }
    }

    fn make_synthetic_apps_report(count: usize, scan_ts: u64) -> ScanReport {
        let apps = (0..count)
            .map(|i| AttributedApp {
                id: format!("app:{i:03}"),
                display_name: format!("App {i:03}"),
                publishers: vec![if i % 2 == 0 { "Pub Alpha".into() } else { "Pub Beta".into() }],
                install_location: Some(format!(r"C:\Apps\App{i:03}")),
                total_bytes: ((count - i) as u64) * 1_000_000,
                previous_bytes: Some(((count - i) as u64) * 500_000),
                growth: GrowthDelta::Changed(((count - i) as i64) * 500_000),
                per_drive_growth: Default::default(),
                no_touch: None,
                bases: vec![AttributionBasis::InstallLocation],
                items: vec![BreakdownItem {
                    path: format!(r"C:\Apps\App{i:03}"),
                    bytes: ((count - i) as u64) * 1_000_000,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::InstallLocation),
                    basis_detail: None,
                    no_touch: None,
                    drive_letter: 'C',
                }],
                dependencies: vec![],
                carved_out_apps: vec![],
                display_icon: None,
                icon_rgba: None,
                category: y_core::model::AppCategory::Utilities,
                category_basis: String::new(),
            })
            .collect();
        ScanReport {
            scan_timestamp_unix: scan_ts,
            scan_timestamp_display: "2026-11-28 05:00".into(),
            duration_ms: 10,
            drives: vec![DriveInfo {
                letter: 'C',
                root: "C:\\".into(),
                total_bytes: 500_000_000_000,
                free_bytes: 250_000_000_000,
                used_bytes: 250_000_000_000,
                is_system_drive: true,
                full_walk_enabled: true,
                exclusions: vec![],
                last_scanned_unix: Some(scan_ts),
                last_scanned_display: Some("2026-11-28 05:00".into()),
            }],
            registry_stats: RegistryDiscoveryStats {
                raw_display_name_entries: count,
                raw_with_install_location: count,
                canonical_apps: count,
                canonical_with_direct_install_location: count,
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
            applied_containers: None,
            applied_zones: None,
            catalog_source: "embedded".into(),
            warnings: vec![],
            evidence_curated_conflicts: vec![],
        }
    }

    #[test]
    fn ledger_page_bounds_handles_empty_exact_and_clamped_pages() {
        assert_eq!(ledger_page_bounds(0, 0, LEDGER_PAGE_SIZE), (0, 1, 0, 0));
        assert_eq!(ledger_page_bounds(0, 3, LEDGER_PAGE_SIZE), (0, 1, 0, 0));
        assert_eq!(ledger_page_bounds(50, 0, LEDGER_PAGE_SIZE), (0, 1, 0, 50));
        assert_eq!(ledger_page_bounds(50, 5, LEDGER_PAGE_SIZE), (0, 1, 0, 50));
        assert_eq!(ledger_page_bounds(51, 0, LEDGER_PAGE_SIZE), (0, 2, 0, 50));
        assert_eq!(ledger_page_bounds(51, 1, LEDGER_PAGE_SIZE), (1, 2, 50, 51));
        assert_eq!(ledger_page_bounds(296, 1, LEDGER_PAGE_SIZE), (1, 6, 50, 100));
        assert_eq!(ledger_page_bounds(296, 5, LEDGER_PAGE_SIZE), (5, 6, 250, 296));
        assert_eq!(ledger_page_bounds(296, 99, LEDGER_PAGE_SIZE), (5, 6, 250, 296));
    }

    #[test]
    fn ledger_pagination_resets_to_page_1_on_filter_sort_search_scope_and_rescan() {
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let pal = ThemePalette::for_mode(ThemeMode::Dark);
        let report1 = make_synthetic_apps_report(120, 100);
        let report2 = make_synthetic_apps_report(120, 200);
        let mut st = StorageUiState::default();

        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        assert_eq!(st.cached_filtered.as_ref().unwrap().visible_indices.len(), 120);

        // Filter chip change -> page 1
        st.ledger_page = 2;
        st.ledger_scroll_to_top = false;
        st.filter_chip = AppFilterChip::Grew;
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        assert_eq!(st.ledger_page, 0);
        assert!(st.ledger_scroll_to_top);
        assert_eq!(st.cached_filtered.as_ref().unwrap().visible_indices.len(), 120);

        // Search change -> page 1
        st.ledger_page = 1;
        st.ledger_scroll_to_top = false;
        st.search_query = "App 0".into();
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        assert_eq!(st.ledger_page, 0);
        assert!(st.ledger_scroll_to_top);

        // Sort change -> page 1
        st.search_query.clear();
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        st.ledger_page = 2;
        st.ledger_scroll_to_top = false;
        st.sort_mode = AppSortMode::Name;
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        assert_eq!(st.ledger_page, 0);
        assert!(st.ledger_scroll_to_top);

        // Facet change -> page 1
        st.ledger_page = 2;
        st.ledger_scroll_to_top = false;
        st.facets.publisher = Some("Pub Alpha".into());
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        assert_eq!(st.ledger_page, 0);
        assert!(st.ledger_scroll_to_top);
        assert_eq!(st.cached_filtered.as_ref().unwrap().visible_indices.len(), 60);

        // Scope (system drive) change -> page 1
        st.facets = FacetFilters::default();
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'C', pal.theme_key);
        st.ledger_page = 1;
        st.ledger_scroll_to_top = false;
        ensure_report_and_filter_caches(&ctx, &mut st, &report1, &pal, 'D', pal.theme_key);
        assert_eq!(st.ledger_page, 0);
        assert!(st.ledger_scroll_to_top);

        // Rescan timestamp change -> page 1
        st.ledger_page = 2;
        st.ledger_scroll_to_top = false;
        ensure_report_and_filter_caches(&ctx, &mut st, &report2, &pal, 'D', pal.theme_key);
        assert_eq!(st.ledger_page, 0);
        assert!(st.ledger_scroll_to_top);
    }

    #[test]
    fn ledger_header_hover_cells_match_columns_at_100_and_125_dpi() {
        let report = make_synthetic_apps_report(120, 100);
        let cfg = AppConfig::default();
        for ppp in [1.0_f32, 1.25] {
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, ThemeMode::Dark);
            let mut st = StorageUiState::default();
            st.ledger_collapsed = true;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
                ..Default::default()
            };
            let mut raw = raw;
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(ppp);
            let _ = ctx.run(raw, |c| {
                draw_app_ui(c, &mut st, &cfg, Some(&report), false, None, r"C:\Windows", &mut |_| {});
            });
            let rects: Vec<(AppSortMode, Rect)> = st.header_rects.iter().filter_map(|x| *x).collect();
            assert_eq!(rects.len(), 5);
            for (idx, (_, r)) in rects.iter().enumerate() {
                assert!((r.height() - TABLE_HEADER_H).abs() <= 0.1, "height mismatch at ppp={ppp}");
                assert!(r.width() >= 80.0);
                if idx > 0 {
                    assert!(
                        r.min.x >= rects[idx - 1].1.max.x + SPACE_12 - 0.5,
                        "headers overlap at idx={idx}, ppp={ppp}"
                    );
                }
            }
        }
    }

    #[test]
    fn ledger_drawer_lives_inside_its_page() {
        let report = make_synthetic_apps_report(120, 100);
        let cfg = AppConfig::default();
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut st = StorageUiState::default();
        st.ledger_collapsed = true;
        // Expand app:000 (on page 1) and check page 2 shapes vs page 1 shapes
        st.expanded_apps.insert("app:000".into());

        st.ledger_page = 1; // page 2 (rows 50..100)
        let raw_p2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw_p2, |c| {
            draw_app_ui(c, &mut st, &cfg, Some(&report), false, None, r"C:\Windows", &mut |_| {});
        });
        assert_eq!(st.ledger_page, 1);
        assert!(
            !st.expanded_row_heights.contains_key("app:000"),
            "page 1 drawer should not be laid out when viewing page 2"
        );

        st.ledger_page = 0; // page 1 (rows 0..50)
        let raw_p1 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1020.0, 720.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw_p1, |c| {
            draw_app_ui(c, &mut st, &cfg, Some(&report), false, None, r"C:\Windows", &mut |_| {});
        });
        assert!(
            st.expanded_row_heights.contains_key("app:000"),
            "page 1 drawer should be laid out when returning to page 1"
        );
    }

    #[test]
    fn settings_top_tabs_and_narrow_wrap_render_cleanly() {
        let (report, cfg) = make_synthetic_report_for_drives(3);
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut st = StorageUiState::default();
        st.active_view = ActiveView::Settings;
        st.settings_focus_containers = true;

        // Wide viewport: focus flag selects Rulebook tab and clears itself
        let wide_out = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1275.0, 900.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut st,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    r"C:\Windows",
                    &mut |_| {},
                );
            },
        );
        assert!(!wide_out.shapes.is_empty());
        assert_eq!(st.settings_section, SettingsSection::Rulebook);
        assert!(!st.settings_focus_containers);

        // Render all 3 tabs across standard (1020x720) and narrow wrapping (420x720) widths
        for size in [Vec2::new(1020.0, 720.0), Vec2::new(420.0, 720.0)] {
            for section in SettingsSection::ALL {
                st.settings_section = section;
                let out = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |c| {
                        draw_app_ui(
                            c,
                            &mut st,
                            &cfg,
                            Some(&report),
                            false,
                            None,
                            r"C:\Windows",
                            &mut |_| {},
                        );
                    },
                );
                assert!(!out.shapes.is_empty());
            }
        }
    }

    #[test]
    fn settings_form_grid_and_drive_pod_row_heights_align() {
        for avail_w in [600.0_f32, 820.0, 1200.0] {
            let with_sel = compute_settings_form_grid(avail_w, Some(268.0));
            assert!(!with_sel.stacked);
            assert_eq!(with_sel.action_w, FORM_GRID_ACTION_W);
            assert_eq!(with_sel.selector_w, 268.0);
            assert!(
                (with_sel.input_w + SPACE_8 + with_sel.selector_w + SPACE_8 + with_sel.action_w
                    - avail_w)
                    .abs()
                    < 0.01
            );

            let no_sel = compute_settings_form_grid(avail_w, None);
            assert!(!no_sel.stacked);
            assert_eq!(no_sel.action_w, FORM_GRID_ACTION_W);
            assert!((no_sel.input_w + SPACE_8 + no_sel.action_w - avail_w).abs() < 0.01);
        }

        let narrow = compute_settings_form_grid(420.0, Some(268.0));
        assert!(narrow.stacked);
        assert_eq!(narrow.action_w, FORM_GRID_ACTION_W);

        assert_eq!(settings_drive_pod_cols(4, 500.0), 1);
        assert_eq!(settings_drive_pod_cols(4, 660.0), 2);
        assert_eq!(settings_drive_pod_cols(4, 1000.0), 3);
        assert_eq!(settings_drive_pod_cols(4, 1340.0), 4);

        let h0 = drive_pod_natural_inner_h(0, true);
        let h1 = drive_pod_natural_inner_h(1, true);
        let h2 = drive_pod_natural_inner_h(2, true);
        assert!(h0 < h1 && h1 < h2);
        assert!(drive_pod_natural_inner_h(0, false) < h0);
    }

    #[test]
    fn settings_rulebook_add_buttons_share_axis_and_wide_pane_is_fluid() {
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let pal = ThemePalette::for_mode(ThemeMode::Dark);
        let mut s1 = String::new();
        let mut s2 = String::new();
        let mut s3 = String::new();
        let mut rects = [Rect::NOTHING; 3];

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 720.0))),
                ..Default::default()
            },
            |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    let avail_w = 900.0;
                    let zone_sel_w = measure_filter_radio_group_w(
                        ui,
                        &[
                            ZoneRuleKind::AppContainer.label(),
                            ZoneRuleKind::PersonalFiles.label(),
                        ],
                        &pal,
                    );
                    let pin_sel_w = measure_filter_radio_group_w(
                        ui,
                        &[
                            PinKind::App.short_label(),
                            PinKind::Group.short_label(),
                            PinKind::Personal.short_label(),
                            PinKind::Ignore.short_label(),
                        ],
                        &pal,
                    );
                    let shared_sel_w = zone_sel_w.max(pin_sel_w);
                    assert!(
                        pin_sel_w > zone_sel_w,
                        "4-option pin selector ({pin_sel_w}) should govern shared column width ({zone_sel_w})"
                    );

                    rects[0] = draw_settings_form_grid_row_with_rect(
                        ui,
                        "test_r1",
                        &mut s1,
                        "hint1",
                        "Add root zone",
                        avail_w,
                        Some(shared_sel_w),
                        &pal,
                        |ui| {
                            let _ = draw_filter_radio_chip(
                                ui,
                                ZoneRuleKind::AppContainer.label(),
                                true,
                                &pal,
                            );
                            let _ = draw_filter_radio_chip(
                                ui,
                                ZoneRuleKind::PersonalFiles.label(),
                                false,
                                &pal,
                            );
                        },
                    )
                    .1;
                    rects[1] = draw_settings_form_grid_row_with_rect(
                        ui,
                        "test_r2",
                        &mut s2,
                        "hint2",
                        "Add path zone",
                        avail_w,
                        Some(shared_sel_w),
                        &pal,
                        |ui| {
                            let _ = draw_filter_radio_chip(
                                ui,
                                ZoneRuleKind::AppContainer.label(),
                                false,
                                &pal,
                            );
                            let _ = draw_filter_radio_chip(
                                ui,
                                ZoneRuleKind::PersonalFiles.label(),
                                true,
                                &pal,
                            );
                        },
                    )
                    .1;
                    rects[2] = draw_settings_form_grid_row_with_rect(
                        ui,
                        "test_r3",
                        &mut s3,
                        "hint3",
                        "Pin folder",
                        avail_w,
                        Some(shared_sel_w),
                        &pal,
                        |ui| {
                            for pk in [
                                PinKind::App,
                                PinKind::Group,
                                PinKind::Personal,
                                PinKind::Ignore,
                            ] {
                                let _ = draw_filter_radio_chip(
                                    ui,
                                    pk.short_label(),
                                    pk == PinKind::App,
                                    &pal,
                                );
                            }
                        },
                    )
                    .1;
                });
            },
        );

        for r in rects {
            assert!((r.min.x - rects[0].min.x).abs() < 0.5, "left edge mismatch: {r:?} vs {:?}", rects[0]);
            assert!((r.max.x - rects[0].max.x).abs() < 0.5, "right edge mismatch: {r:?} vs {:?}", rects[0]);
        }

        // Wide viewport (1600x900) expands fluidly past the old 860px cap
        let (report, cfg) = make_synthetic_report_for_drives(4);
        let mut st = StorageUiState::default();
        st.active_view = ActiveView::Settings;
        for section in [SettingsSection::ScanScope, SettingsSection::Rulebook] {
            st.settings_section = section;
            let out = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                    ..Default::default()
                },
                |c| {
                    draw_app_ui(
                        c,
                        &mut st,
                        &cfg,
                        Some(&report),
                        false,
                        None,
                        r"C:\Windows",
                        &mut |_| {},
                    );
                },
            );
            let max_x = out
                .shapes
                .iter()
                .map(|cs| cs.shape.visual_bounding_rect().max.x)
                .filter(|x| x.is_finite() && *x <= 1600.0)
                .fold(0.0_f32, f32::max);
            assert!(
                max_x > 1400.0,
                "expected fluid wide content pane > 1400px at 1600px viewport for {section:?}, got {max_x}"
            );
        }
    }

    #[test]
    fn settings_rulebook_zone_chip_labels_share_baseline() {
        fn collect_text_positions(shape: &egui::Shape, out: &mut Vec<(String, f32)>) {
            match shape {
                egui::Shape::Text(ts) => {
                    out.push((ts.galley.text().to_owned(), ts.pos.y));
                }
                egui::Shape::Vec(vec) => {
                    for s in vec {
                        collect_text_positions(s, out);
                    }
                }
                _ => {}
            }
        }

        let (report, cfg) = make_synthetic_report_for_drives(2);
        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let mut st = StorageUiState::default();
        st.active_view = ActiveView::Settings;
        st.settings_section = SettingsSection::Rulebook;

        let out = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                ..Default::default()
            },
            |c| {
                draw_app_ui(
                    c,
                    &mut st,
                    &cfg,
                    Some(&report),
                    false,
                    None,
                    r"C:\Windows",
                    &mut |_| {},
                );
            },
        );

        let mut text_ys: Vec<(String, f32)> = Vec::new();
        for cs in &out.shapes {
            collect_text_positions(&cs.shape, &mut text_ys);
        }

        for kind in [ZoneRuleKind::AppContainer, ZoneRuleKind::PersonalFiles] {
            let names: Vec<&str> = cfg
                .zones
                .root_zones
                .iter()
                .filter(|rz| rz.kind == kind)
                .map(|rz| rz.name.as_str())
                .collect();
            assert!(names.len() >= 2);
            let ys: Vec<(&str, f32)> = names
                .iter()
                .map(|&name| {
                    let y = text_ys
                        .iter()
                        .find(|(txt, _)| txt == name)
                        .map(|(_, y)| *y)
                        .unwrap_or_else(|| panic!("missing chip label {name}"));
                    (name, y)
                })
                .collect();
            let first_y = ys[0].1;
            let row1: Vec<(&str, f32)> = ys
                .iter()
                .copied()
                .filter(|(_, y)| (*y - first_y).abs() < 8.0)
                .collect();
            assert!(
                row1.len() >= 2,
                "expected >= 2 chips on first row of {kind:?}, got {row1:?}"
            );
            for (name, y) in &row1[1..] {
                assert!(
                    (*y - first_y).abs() < 0.5,
                    "{kind:?} chip {name} y={y} differs from first chip {} y={first_y}",
                    row1[0].0
                );
            }
        }
    }

    #[test]
    fn fixture_6_pagination_counts_family_as_one_top_level_row() {
        let mut report = make_synthetic_apps_report(50, 100);
        // Add 4 same-publisher units forming 1 family -> 54 apps total, 51 top-level entries.
        let fam_specs = [
            ("reg:orb-browser", "Orb Browser", 900_000_000_u64, AppCategory::Browsers, None),
            (
                "sys:orb-runtime",
                "Orb Browser Runtime",
                400_000_000_u64,
                AppCategory::SystemAndRuntimes,
                Some(NoTouchCategory::SystemRuntime),
            ),
            ("reg:orb-sat1", "Orb Browser Helper", 5_000_000_u64, AppCategory::Utilities, None),
            ("reg:orb-sat2", "Orb Browser Updater", 1_000_000_u64, AppCategory::Utilities, None),
        ];
        for (id, name, bytes, cat, nt) in fam_specs {
            report.apps.push(AttributedApp {
                id: id.into(),
                display_name: name.into(),
                publishers: vec!["Orb Systems Inc.".into()],
                install_location: Some(format!(r"C:\Program Files\Orb\{id}")),
                total_bytes: bytes,
                previous_bytes: None,
                growth: GrowthDelta::Changed(0),
                per_drive_growth: Default::default(),
                no_touch: nt,
                bases: vec![AttributionBasis::InstallLocation],
                items: vec![BreakdownItem {
                    path: format!(r"C:\Program Files\Orb\{id}"),
                    bytes,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::InstallLocation),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: 'C',
                }],
                dependencies: vec![],
                carved_out_apps: vec![],
                display_icon: None,
                icon_rgba: None,
                category: cat,
                category_basis: String::new(),
            });
        }

        let ctx = egui::Context::default();
        configure_fonts_and_theme(&ctx, ThemeMode::Dark);
        let pal = ThemePalette::for_mode(ThemeMode::Dark);
        let mut st = StorageUiState::default();
        ensure_report_and_filter_caches(&ctx, &mut st, &report, &pal, 'C', pal.theme_key);

        let cf = st.cached_filtered.as_ref().unwrap();
        assert_eq!(cf.visible_indices.len(), 54);
        assert_eq!(cf.visible_entries.len(), 51);
        assert_eq!(ledger_page_bounds(cf.visible_entries.len(), 0, LEDGER_PAGE_SIZE), (0, 2, 0, 50));
        assert_eq!(ledger_page_bounds(cf.visible_entries.len(), 1, LEDGER_PAGE_SIZE), (1, 2, 50, 51));
    }

    #[test]
    fn family_header_expanded_members_and_notouch_drawer_render_cleanly() {
        let mut report = make_synthetic_apps_report(4, 100);
        // Mixed-category family -> shared_category is None (header shows no category)
        let fam_a = [
            ("reg:zephyr-main", "Zephyr Browser", 250_000_000_u64, AppCategory::Browsers, None),
            (
                "sys:zephyr-rt",
                "Zephyr Browser Runtime",
                120_000_000_u64,
                AppCategory::SystemAndRuntimes,
                Some(NoTouchCategory::SystemRuntime),
            ),
        ];
        for (id, name, bytes, cat, nt) in fam_a {
            report.apps.push(AttributedApp {
                id: id.into(),
                display_name: name.into(),
                publishers: vec!["Zephyr Corp.".into()],
                install_location: Some(format!(r"C:\Program Files\{id}")),
                total_bytes: bytes,
                previous_bytes: None,
                growth: GrowthDelta::Changed(2_000_000),
                per_drive_growth: Default::default(),
                no_touch: nt,
                bases: vec![AttributionBasis::InstallLocation],
                items: vec![BreakdownItem {
                    path: format!(r"C:\Program Files\{id}"),
                    bytes,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::InstallLocation),
                    basis_detail: None,
                    no_touch: nt,
                    drive_letter: 'C',
                }],
                dependencies: vec![],
                carved_out_apps: vec![],
                display_icon: None,
                icon_rgba: None,
                category: cat,
                category_basis: String::new(),
            });
        }

        let cfg = AppConfig::default();
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let ctx = egui::Context::default();
            configure_fonts_and_theme(&ctx, mode);
            let mut st = StorageUiState::default();
            st.ledger_collapsed = true;
            st.expanded_families.insert("family:zephyr:zephyr-browser".into());
            st.expanded_apps.insert("sys:zephyr-rt".into());

            let out = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120.0, 800.0))),
                    ..Default::default()
                },
                |c| {
                    draw_app_ui(
                        c,
                        &mut st,
                        &cfg,
                        Some(&report),
                        false,
                        None,
                        r"C:\Windows",
                        &mut |_| {},
                    );
                },
            );
            assert!(!out.shapes.is_empty());
            assert!(
                st.expanded_family_heights
                    .contains_key("family:zephyr:zephyr-browser"),
                "expanded family height should be measured in {mode:?}"
            );
            assert!(
                st.expanded_row_heights.contains_key("sys:zephyr-rt"),
                "NO-TOUCH member drawer inside expanded family should render and measure in {mode:?}"
            );
        }
    }
}
