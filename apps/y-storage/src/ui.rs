use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use egui::{
    Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Key, Layout, Margin,
    Order, Pos2, Rect, RichText, Rounding, Sense, Stroke, TextStyle, UiBuilder, Vec2,
};
use y_core::config::{AppConfig, AppFilterChip, AppSortMode, SortDirection, ThemeMode};
use y_core::discovery::{normalize_app_token, normalize_path_key};
use y_core::model::{
    DependencyConfidence, DependencyKind, GrowthDelta, LocationQualifier,
    NO_KNOWN_DEPENDENCIES_LABEL, ReverseDependencyEntry, ScanReport, ZoneKind,
};
use y_platform_windows::scanner::ScanProgressSnapshot;

use crate::list_query::filter_and_sort_apps;

// Pastel and semantic constants synced directly with z-controller (DESIGN.md §2.1).
pub const LOGO_BLUE: Color32 = Color32::from_rgb(84, 185, 242); // #54B9F2
pub const LOGO_BLUE_READABLE: Color32 = Color32::from_rgb(47, 155, 232); // #2F9BE8
pub const LOGO_PINK: Color32 = Color32::from_rgb(247, 168, 216); // #F7A8D8
pub const LOGO_LAVENDER: Color32 = Color32::from_rgb(216, 166, 255); // #D8A6FF
pub const STATUS_GREEN: Color32 = Color32::from_rgb(40, 214, 108); // #28D66C
pub const STATUS_RED: Color32 = Color32::from_rgb(240, 68, 88); // #F04458

#[inline]
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let u = t.clamp(0.0, 1.0);
    let r = (a.r() as f32 + (b.r() as f32 - a.r() as f32) * u).round() as u8;
    let g = (a.g() as f32 + (b.g() as f32 - a.g() as f32) * u).round() as u8;
    let bl = (a.b() as f32 + (b.b() as f32 - a.b() as f32) * u).round() as u8;
    Color32::from_rgb(r, g, bl)
}

// Single multiples-of-4 spacing scale used uniformly everywhere.
pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;

// Multi-tier corner-radius scale from DESIGN.md (paired with z-controller synced controls).
const RADIUS_PANEL: f32 = 8.0;
const RADIUS_CONTROL: f32 = 6.0;
const RADIUS_PILL: f32 = 4.0;
const BADGE_PILL_H: f32 = 18.0;
const RADIUS_BADGE_PILL: f32 = BADGE_PILL_H * 0.5;

const SIZE_COL_W: f32 = 96.0;
const DELTA_COL_W: f32 = 96.0;
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

#[derive(Debug, Clone)]
pub enum UiAction {
    StartRescan,
    ToggleTheme(ThemeMode),
    SaveConfig(AppConfig),
    UpdateListView {
        sort_mode: AppSortMode,
        sort_direction: SortDirection,
        filter_chip: AppFilterChip,
        search_query: String,
    },
    UpdateLedgerLayout {
        collapsed: bool,
        height_px: Option<f32>,
    },
}

#[derive(Debug, Clone, Copy)]
struct ChipStyle {
    fill: Color32,
    stroke: Color32,
    text: Color32,
}

#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub subtle_text: Color32,
    pub hint_text: Color32,
    pub input_bg: Color32,
    pub header_input_bg: Color32,
    pub widget: Color32,
    pub widget_hover: Color32,
    pub widget_active: Color32,
    pub accent_line: Color32,
    pub lavender_text: Color32,
    pub proven_text: Color32,
    pub status_green: Color32,
    pub status_red: Color32,
    pub delta_grew: Color32,
    pub no_touch_outline: Color32,
    pub pill_blue_fill: Color32,
    pub pill_green_fill: Color32,
    pub pill_lav_fill: Color32,
    pub pill_red_fill: Color32,
    pub scrollbar_thumb: Color32,
    pub scrollbar_thumb_hover: Color32,
}

impl ThemePalette {
    #[must_use]
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                bg: Color32::from_rgb(10, 10, 16),        // #0A0A10 (z-controller bg)
                panel: Color32::from_rgb(20, 20, 31),     // #14141F (z-controller panel)
                panel_alt: Color32::from_rgb(16, 16, 26), // #10101A (z-controller faint_bg)
                border: Color32::from_rgb(38, 38, 58),    // #26263A (z-controller border)
                text: Color32::from_rgb(237, 237, 242),   // #EDEDF2 (z-controller text)
                muted: Color32::from_rgb(154, 154, 176),  // #9A9AB0 (z-controller muted)
                subtle_text: Color32::from_rgb(130, 130, 154), // #82829A (pure R==G axis synced with z-controller)
                hint_text: Color32::from_rgb(100, 100, 122),   // #64647A (pure R==G axis dim placeholder hint)
                input_bg: Color32::from_rgb(10, 10, 16),       // #0A0A10 (z-controller extreme_bg)
                header_input_bg: Color32::from_rgb(10, 10, 16), // #0A0A10
                widget: Color32::from_rgb(24, 24, 38),         // #181826 (z-controller widget)
                widget_hover: Color32::from_rgb(31, 31, 49),   // #1F1F31 (z-controller widget_hover)
                widget_active: Color32::from_rgb(24, 34, 48),  // #182230 (z-controller widget_active)
                accent_line: LOGO_BLUE,                        // #54B9F2 (z-controller accent_line)
                lavender_text: LOGO_LAVENDER,                  // #D8A6FF
                proven_text: Color32::from_rgb(124, 203, 255), // #7CCBFF (crisp blue ink inside Dark outline+tint pill)
                status_green: STATUS_GREEN,                    // #28D66C
                status_red: STATUS_RED,                        // #F04458
                delta_grew: STATUS_RED,                        // #F04458 (ADD 3 saturated status)
                no_touch_outline: STATUS_RED,                  // #F04458 (ADD 3 saturated status)
                pill_blue_fill: Color32::from_rgb(18, 28, 40), // #121C28 (z-controller pill_blue_fill)
                pill_green_fill: Color32::from_rgb(16, 46, 32), // #102E20 (ADD 3 saturated green tint)
                pill_lav_fill: Color32::from_rgb(32, 24, 46),
                pill_red_fill: Color32::from_rgb(44, 18, 30),  // #2C121E (ADD 3 saturated red tint)
                scrollbar_thumb: Color32::from_rgb(82, 86, 122), // #52567A (FIX 4b minimum visible Dark contrast)
                scrollbar_thumb_hover: Color32::from_rgb(122, 134, 184), // #7A86B8
            },
            ThemeMode::Light => Self {
                bg: Color32::from_rgb(238, 244, 250),     // #EEF4FA (z-controller bg)
                panel: Color32::from_rgb(255, 255, 255),  // #FFFFFF (z-controller panel)
                panel_alt: Color32::from_rgb(244, 248, 252), // #F4F8FC (z-controller faint_bg)
                border: Color32::from_rgb(217, 228, 240), // #D9E4F0 (z-controller border)
                text: Color32::from_rgb(28, 39, 51),      // #1C2733 (z-controller text)
                muted: Color32::from_rgb(95, 113, 134),   // #5F7186 (z-controller muted)
                subtle_text: Color32::from_rgb(116, 132, 153), // #748499 (DESIGN.md T4 exception)
                hint_text: Color32::from_rgb(152, 166, 184),   // #98A6B8
                input_bg: Color32::from_rgb(238, 244, 250),    // #EEF4FA (z-controller extreme_bg on #FFFFFF cards)
                header_input_bg: Color32::from_rgb(255, 255, 255), // #FFFFFF (crisp contrast on #EEF4FA header canvas)
                widget: Color32::from_rgb(245, 248, 252),      // #F5F8FC (z-controller widget)
                widget_hover: Color32::from_rgb(233, 242, 250), // #E9F2FA (z-controller widget_hover)
                widget_active: Color32::from_rgb(220, 235, 248), // #DCEBF8 (z-controller widget_active)
                accent_line: LOGO_BLUE_READABLE,               // #2F9BE8 (z-controller accent_line)
                lavender_text: Color32::from_rgb(120, 62, 184), // #783EB8 (Light contrast exception)
                proven_text: Color32::from_rgb(21, 101, 184),   // #1565B8 (FIX 4c Light outline+tint ink)
                status_green: Color32::from_rgb(15, 168, 79),   // #0FA84F (ADD 3 saturated Light green)
                status_red: Color32::from_rgb(224, 36, 68),     // #E02444 (ADD 3 saturated Light red)
                delta_grew: Color32::from_rgb(224, 36, 68),     // #E02444 (ADD 3 saturated Light delta)
                no_touch_outline: Color32::from_rgb(224, 36, 68), // #E02444 (ADD 3 saturated Light NO-TOUCH)
                pill_blue_fill: Color32::from_rgb(227, 242, 253), // #E3F2FD (FIX 4c soft Light blue tint)
                pill_green_fill: Color32::from_rgb(228, 249, 236), // #E4F9EC (ADD 3 saturated Light green tint)
                pill_lav_fill: Color32::from_rgb(243, 236, 255),
                pill_red_fill: Color32::from_rgb(253, 232, 237), // #FDE8ED (ADD 3 saturated Light red tint)
                scrollbar_thumb: Color32::from_rgb(152, 176, 202), // #98B0CA (FIX 4b Light contrast)
                scrollbar_thumb_hover: Color32::from_rgb(100, 140, 184), // #648CB8
            },
        }
    }
}

struct CachedAppMeta {
    total_str: String,
    total_tip: String,
    delta_display: Option<(String, Color32, String)>,
    name_w_px: Option<f32>,
    pub_w_px: Option<f32>,
}

struct CachedReportData {
    scan_ts: u64,
    dark_mode: bool,
    apps_count: usize,
    app_idx_by_id: HashMap<String, usize>,
    apps: Vec<CachedAppMeta>,
    max_unfiltered_bytes: u64,
    total_attributed_bytes: u64,
    attributed_label: String,
    attributed_tip: String,
    rev_counts: HashMap<String, usize>,
    rev_by_app_token: HashMap<String, (String, usize)>,
    sys_reconcile: Vec<(char, String, String, i128)>,
}

struct CachedFilteredList {
    scan_ts: u64,
    sort_mode: AppSortMode,
    sort_direction: SortDirection,
    filter_chip: AppFilterChip,
    search_query: String,
    sys_drive: char,
    visible_indices: Vec<usize>,
}

struct CachedRimMesh {
    w_q: i32,
    h_q: i32,
    r_q: i32,
    mesh: egui::epaint::Mesh,
    fill_poly: Vec<Pos2>,
}

pub struct StorageUiState {
    pub active_view: ActiveView,
    pub sort_mode: AppSortMode,
    pub sort_direction: SortDirection,
    pub filter_chip: AppFilterChip,
    pub search_query: String,
    pub dep_search_query: String,
    pub expanded_apps: HashSet<String>,
    pub expanded_deps: HashSet<String>,
    pub open_reverse_dep_id: Option<String>,
    pub expand_shared_caches: bool,
    pub expand_windows_system: bool,
    pub expand_your_files: bool,
    pub expand_unattributed: bool,
    pub new_exclusion_inputs: HashMap<char, String>,
    pub applied_theme: Option<ThemeMode>,
    pub has_prop_strong: bool,
    pub ledger_collapsed: bool,
    pub ledger_height_px: Option<f32>,
    ledger_drag_start_h: Option<f32>,
    pub icon_textures: HashMap<String, egui::TextureHandle>,
    pub icon_cache_scan_ts: Option<u64>,
    pub first_row_size_rect: Option<Rect>,
    cached_report: Option<CachedReportData>,
    cached_filtered: Option<CachedFilteredList>,
    cached_rim: Option<CachedRimMesh>,
    expanded_row_heights: HashMap<String, f32>,
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
            dep_search_query: String::new(),
            expanded_apps: HashSet::new(),
            expanded_deps: HashSet::new(),
            open_reverse_dep_id: None,
            expand_shared_caches: false,
            expand_windows_system: false,
            expand_your_files: false,
            expand_unattributed: false,
            new_exclusion_inputs: HashMap::new(),
            applied_theme: None,
            has_prop_strong: false,
            ledger_collapsed: false,
            ledger_height_px: None,
            ledger_drag_start_h: None,
            icon_textures: HashMap::new(),
            icon_cache_scan_ts: None,
            first_row_size_rect: None,
            cached_report: None,
            cached_filtered: None,
            cached_rim: None,
            expanded_row_heights: HashMap::new(),
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
            ledger_collapsed: cfg.ledger_collapsed,
            ledger_height_px: cfg.ledger_height_px,
            ..Self::default()
        }
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

fn format_delta_display(delta: GrowthDelta, pal: &ThemePalette) -> Option<(String, Color32, String)> {
    match delta {
        GrowthDelta::NoBaseline => Some((
            "new".to_string(),
            pal.muted,
            "No baseline from a previous scan".to_string(),
        )),
        GrowthDelta::New(b) => Some((
            "new".to_string(),
            pal.muted,
            format!("New since previous scan ({} total, no prior baseline)", format_exact_bytes(b)),
        )),
        GrowthDelta::Changed(0) => None,
        GrowthDelta::Changed(d) if d > 0 => Some((
            format!("+{}", format_size_2dec(d as u64)),
            pal.delta_grew,
            format!("+{} since previous scan", format_exact_bytes(d as u64)),
        )),
        GrowthDelta::Changed(d) => Some((
            format!("-{}", format_size_2dec(d.unsigned_abs())),
            pal.status_green,
            format!(
                "-{} since previous scan",
                format_exact_bytes(d.unsigned_abs())
            ),
        )),
    }
}

pub fn configure_fonts_and_theme(ctx: &egui::Context, mode: ThemeMode) {
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
    ctx.set_style(style);
    apply_theme_visuals(ctx, mode);
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

pub fn apply_theme_visuals(ctx: &egui::Context, mode: ThemeMode) {
    let pal = ThemePalette::for_mode(mode);
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
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, pal.accent_line);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = pal.widget_active;
    visuals.widgets.active.weak_bg_fill = pal.widget_active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, LOGO_PINK);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    visuals.widgets.open.bg_fill = pal.widget_hover;
    visuals.widgets.open.weak_bg_fill = pal.widget_hover;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, pal.accent_line);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.open.rounding = Rounding::same(6.0);

    style.visuals = visuals;
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

fn corner_unit_arcs() -> &'static [[(f32, f32); 17]; 4] {
    static ARCS: OnceLock<[[(f32, f32); 17]; 4]> = OnceLock::new();
    ARCS.get_or_init(|| {
        let ranges = [
            (std::f32::consts::PI, 1.5 * std::f32::consts::PI),
            (1.5 * std::f32::consts::PI, 2.0 * std::f32::consts::PI),
            (0.0, 0.5 * std::f32::consts::PI),
            (0.5 * std::f32::consts::PI, std::f32::consts::PI),
        ];
        let mut out = [[(0.0_f32, 0.0_f32); 17]; 4];
        for (c_idx, &(a0, a1)) in ranges.iter().enumerate() {
            for k in 0..=16usize {
                let theta = a0 + (a1 - a0) * (k as f32 / 16.0);
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

fn build_rounded_rect_polygon(rect: Rect, radius: f32) -> Vec<Pos2> {
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(rect, radius, &mut ring);
    ring.into_iter().map(|(p, _)| p).collect()
}

/// Builds origin-relative gradient-rim mesh + inner polygon once per `(w, h, rounding)` and translates on scroll frames.
fn build_cached_rim_geometry(w: f32, h: f32, rounding: f32) -> CachedRimMesh {
    let local_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w.max(1.0), h.max(1.0)));
    let fill_poly = build_rounded_rect_polygon(
        local_rect.shrink(1.0),
        (rounding - 1.0).max(0.0),
    );
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(local_rect, rounding, &mut ring);
    let n = ring.len();
    const BANDS: usize = 5;
    let offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.4_f32, 0.0_f32];
    let alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8];

    let mut mesh = egui::epaint::Mesh::default();
    if n >= 8 {
        let lw = local_rect.width().max(1.0);
        let lh = local_rect.height().max(1.0);
        mesh.vertices.reserve(n * BANDS);
        mesh.indices.reserve(n * (BANDS - 1) * 6);
        for (p, norm) in &ring {
            let t = ((p.x / lw) * 0.65 + (p.y / lh) * 0.35).clamp(0.0, 1.0);
            let c = if t <= 0.5 {
                lerp_color(LOGO_BLUE, LOGO_LAVENDER, t * 2.0)
            } else {
                lerp_color(LOGO_LAVENDER, LOGO_PINK, (t - 0.5) * 2.0)
            };
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
        mesh,
        fill_poly,
    }
}

fn translate_cached_rim_for_rect(
    rect: Rect,
    rounding: f32,
    cache: &mut Option<CachedRimMesh>,
) -> (Vec<Pos2>, egui::epaint::Mesh) {
    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);
    let w_q = (w * 2.0).round() as i32;
    let h_q = (h * 2.0).round() as i32;
    let r_q = (rounding * 10.0).round() as i32;
    let hit = cache
        .as_ref()
        .is_some_and(|c| c.w_q == w_q && c.h_q == h_q && c.r_q == r_q);
    if !hit {
        *cache = Some(build_cached_rim_geometry(w, h, rounding));
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

/// Paints an unbroken 1px perimeter ring strictly inside `rect` (safe even when fill is `TRANSPARENT`).
fn paint_crisp_ring_mesh(
    painter: &egui::Painter,
    rect: Rect,
    inset: f32,
    rounding: f32,
    border_color: Color32,
) {
    if border_color == Color32::TRANSPARENT || rect.width() <= 2.0 || rect.height() <= 2.0 {
        return;
    }
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(rect, rounding, &mut ring);
    let n = ring.len();
    if n < 8 {
        return;
    }
    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices.reserve(n * 2);
    mesh.indices.reserve(n * 6);
    for &(p, norm) in &ring {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: p,
            uv: egui::epaint::WHITE_UV,
            color: border_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: p - norm * inset,
            uv: egui::epaint::WHITE_UV,
            color: border_color,
        });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let o0 = (i * 2) as u32;
        let i0 = o0 + 1;
        let o1 = (j * 2) as u32;
        let i1 = o1 + 1;
        mesh.indices.extend_from_slice(&[o0, o1, i0, i0, o1, i1]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Ported verbatim from z-controller (`ui.rs:398-458`): diagonal gradient rim (LOGO_BLUE -> LOGO_LAVENDER -> LOGO_PINK) bounded strictly inside `rect`.
#[allow(dead_code)]
fn paint_active_card_gradient_border(painter: &egui::Painter, rect: Rect, rounding: f32) {
    if rect.width() <= 4.0 || rect.height() <= 4.0 {
        return;
    }
    let mut ring = Vec::with_capacity(68);
    push_rounded_rect_perimeter(rect, rounding, &mut ring);
    let n = ring.len();
    if n < 8 {
        return;
    }

    const BANDS: usize = 5;
    let offsets = [-4.5_f32, -2.0_f32, -1.6_f32, -0.4_f32, 0.0_f32];
    let alphas = [0_u8, 52_u8, 255_u8, 255_u8, 0_u8];

    let w = rect.width().max(1.0);
    let h = rect.height().max(1.0);

    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices.reserve(n * BANDS);
    mesh.indices.reserve(n * (BANDS - 1) * 6);

    for (p, norm) in &ring {
        let t = (((p.x - rect.left()) / w) * 0.65 + ((p.y - rect.top()) / h) * 0.35).clamp(0.0, 1.0);
        let c = if t <= 0.5 {
            lerp_color(LOGO_BLUE, LOGO_LAVENDER, t * 2.0)
        } else {
            lerp_color(LOGO_LAVENDER, LOGO_PINK, (t - 0.5) * 2.0)
        };
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

/// Paints an unbroken rounded rectangle border & optional fill, safe for `Color32::TRANSPARENT` fills at 96 DPI.
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
    let inset = (border_px / ppp.max(0.5)).max(1.0 / ppp.max(0.5));
    let inner = snapped.shrink(inset);
    if inner.width() > 0.0 && inner.height() > 0.0 && fill != Color32::TRANSPARENT {
        painter.rect_filled(
            inner,
            Rounding::same((rounding - inset).max(0.0)),
            fill,
        );
    }
    paint_crisp_ring_mesh(painter, snapped, inset, rounding, border_color);
    snapped
}

/// Container frame with a guaranteed continuous 1px outline at 96 DPI and high DPI.
fn draw_crisp_frame<R>(
    ui: &mut egui::Ui,
    fill: Color32,
    border_color: Color32,
    rounding: f32,
    inner_margin: Margin,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let outer_slot = ui.painter().add(egui::Shape::Noop);
    let inner_slot = ui.painter().add(egui::Shape::Noop);
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
    let inset = 1.0 / ppp.max(0.5);
    ui.painter().set(
        outer_slot,
        egui::Shape::rect_filled(snapped, Rounding::same(rounding), border_color),
    );
    let inner = snapped.shrink(inset);
    if inner.width() > 0.0 && inner.height() > 0.0 {
        ui.painter().set(
            inner_slot,
            egui::Shape::rect_filled(inner, Rounding::same((rounding - inset).max(0.0)), fill),
        );
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
    let ink_center_y = if let Some(font_id) = galley
        .job
        .sections
        .first()
        .map(|s| s.format.font_id.clone())
    {
        let text = galley.job.text.as_str();
        let has_lower = text.chars().any(|c| c.is_lowercase());
        let has_upper_or_digit = text.chars().any(|c| c.is_uppercase() || c.is_ascii_digit());
        let probe = if has_lower && !has_upper_or_digit { "x" } else { "Hx" };
        let ref_g = ui.fonts(|f| f.layout_no_wrap(probe.to_owned(), font_id, Color32::WHITE));
        if ref_g.mesh_bounds.is_positive() {
            ref_g.mesh_bounds.center().y
        } else if galley.mesh_bounds.is_positive() {
            galley.mesh_bounds.center().y
        } else {
            galley.size().y * 0.5
        }
    } else if galley.mesh_bounds.is_positive() {
        galley.mesh_bounds.center().y
    } else {
        galley.size().y * 0.5
    };
    let y = outer_rect.center().y - ink_center_y;
    Pos2::new(snap_scalar_to_px(x, ppp), snap_scalar_to_px(y, ppp))
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
    let resp = ui.interact(rect, ui.make_persistent_id(("opt_lbl", rect.min.x.to_bits(), rect.min.y.to_bits(), text)), Sense::hover());
    ui.painter().galley(pos, galley, color);
    resp
}

#[inline]
fn wt_gradient_color_at(t: f32) -> Color32 {
    let clamped = t.clamp(0.0, 1.0);
    if clamped <= 0.5 {
        lerp_color(LOGO_BLUE, LOGO_LAVENDER, clamped * 2.0)
    } else {
        lerp_color(LOGO_LAVENDER, LOGO_PINK, (clamped - 0.5) * 2.0)
    }
}

/// Paints the 3-stop WT horizontal gradient (`LOGO_BLUE -> LOGO_LAVENDER -> LOGO_PINK`) across `full_track`, clipped to `fill_rect` with crisp rounded caps.
fn paint_horizontal_wt_gradient_bar(
    painter: &egui::Painter,
    full_track: Rect,
    fill_rect: Rect,
    rounding: f32,
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

    let mut xs: Vec<f32> = Vec::with_capacity(20);
    xs.push(x0);
    if r > 0.25 {
        for k in 1..=5 {
            let a = (k as f32) / 6.0 * std::f32::consts::FRAC_PI_2;
            xs.push(x0 + r * (1.0 - a.cos()));
        }
    }
    let track_mid_x = full_track.center().x;
    if track_mid_x > x0 + r + 0.5 && track_mid_x < x1 - r - 0.5 {
        xs.push(track_mid_x);
    }
    if r > 0.25 {
        for k in 1..=5 {
            let a = (k as f32) / 6.0 * std::f32::consts::FRAC_PI_2;
            xs.push(x1 - r * (1.0 - a.sin()));
        }
    }
    xs.push(x1);
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    if xs.len() < 2 {
        let col = wt_gradient_color_at((fill_rect.center().x - full_track.min.x) / full_track.width());
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
        let col = wt_gradient_color_at(t);
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

/// Renders an outline button whose states, accent outlines, and foreground colours match z-controller across both themes.
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
    let h_pad = if min_size.y <= 22.0 { SPACE_8 } else { SPACE_12 };
    let font_id = FontId::new(font_size, FontFamily::Proportional);
    let measure_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id.clone(), pal.text));
    let desired_w = (measure_galley.size().x + h_pad * 2.0).max(min_size.x).round();
    let desired_h = min_size.y.round();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, desired_h), sense);

    let (fill, border_col, text_color) = if !enabled {
        (Color32::TRANSPARENT, pal.border, pal.muted)
    } else if resp.is_pointer_button_down_on() {
        (pal.widget_active, LOGO_PINK, pal.text)
    } else if resp.hovered() {
        (pal.widget_hover, pal.accent_line, pal.text)
    } else if active_highlight {
        (pal.pill_red_fill, LOGO_PINK, pal.text)
    } else {
        match variant {
            ButtonVariant::PrimaryOutline => (pal.pill_blue_fill, pal.accent_line, pal.accent_line),
            ButtonVariant::SecondaryOutline => (
                Color32::TRANSPARENT,
                lerp_color(pal.border, pal.accent_line, 0.72),
                pal.accent_line,
            ),
            ButtonVariant::SubtleOutline => (Color32::TRANSPARENT, pal.border, pal.muted),
        }
    };

    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, rounding, fill, border_col, 1.0, ppp);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let text_pos = optical_galley_pos(ui, snapped, &galley, None, true);
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
    inner.inner
}

/// Crisp 1px-bordered toggle checkbox matching the z-controller palette.
fn draw_crisp_checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    label: &str,
    pal: &ThemePalette,
) -> bool {
    let mut toggled = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_8;
        let (box_rect, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
        let ppp = ui.ctx().pixels_per_point();
        let fill = if *checked {
            pal.pill_blue_fill
        } else if resp.hovered() {
            pal.widget_hover
        } else {
            pal.input_bg
        };
        let border_col = if *checked || resp.hovered() {
            pal.accent_line
        } else {
            pal.border
        };
        let snapped =
            paint_crisp_bordered_box(ui.painter(), box_rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
        if *checked {
            let c = snapped.center();
            let st = Stroke::new(1.75_f32, pal.accent_line);
            ui.painter()
                .line_segment([Pos2::new(c.x - 4.0, c.y + 0.5), Pos2::new(c.x - 1.0, c.y + 3.5)], st);
            ui.painter()
                .line_segment([Pos2::new(c.x - 1.0, c.y + 3.5), Pos2::new(c.x + 4.5, c.y - 3.0)], st);
        }
        let lbl_resp = ui.add(
            egui::Label::new(RichText::new(label).size(12.0).color(pal.text)).sense(Sense::click()),
        );
        if resp.clicked() || lbl_resp.clicked() {
            *checked = !*checked;
            toggled = true;
        }
    });
    toggled
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
    if matches!(zone, ZoneKind::Body) {
        return paint_chip(ui, zone.short_label(), zone_chip_style(zone, pal), container_bg);
    }
    let (fill, stroke, text_col, dot_col) = match zone {
        ZoneKind::Green => (pal.pill_green_fill, pal.status_green, pal.status_green, STATUS_GREEN),
        ZoneKind::Red => (pal.pill_red_fill, pal.status_red, pal.status_red, STATUS_RED),
        ZoneKind::Body => unreachable!(),
    };
    let font_id = FontId::new(10.5, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(zone.short_label().to_owned(), font_id, text_col));
    let dot_d = 5.0_f32;
    let dot_gap = 4.0_f32;
    let h_pad = 7.0_f32;
    let desired_w = (h_pad * 2.0 + dot_d + dot_gap + galley.size().x).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, BADGE_PILL_H), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(
        ui.painter(),
        rect,
        RADIUS_BADGE_PILL,
        fill,
        stroke,
        1.0,
        ppp,
    );
    let dot_cx = snap_scalar_to_px(snapped.min.x + h_pad + dot_d * 0.5, ppp);
    let dot_cy = snap_scalar_to_px(snapped.center().y, ppp);
    ui.painter()
        .circle_filled(Pos2::new(dot_cx, dot_cy), dot_d * 0.5, dot_col);
    let label_box = Rect::from_min_max(
        Pos2::new(snapped.min.x + h_pad + dot_d + dot_gap, snapped.min.y),
        snapped.max,
    );
    let text_pos = optical_galley_pos(ui, label_box, &galley, None, false);
    ui.painter().galley(text_pos, galley, text_col);
    resp
}

fn zone_chip_style(zone: ZoneKind, pal: &ThemePalette) -> ChipStyle {
    match zone {
        ZoneKind::Green => ChipStyle {
            fill: pal.pill_green_fill,
            stroke: pal.status_green,
            text: pal.status_green,
        },
        ZoneKind::Red => ChipStyle {
            fill: pal.pill_red_fill,
            stroke: pal.status_red,
            text: pal.status_red,
        },
        ZoneKind::Body => ChipStyle {
            fill: Color32::TRANSPARENT,
            stroke: pal.border,
            text: pal.muted,
        },
    }
}

fn qualifier_chip_style(pal: &ThemePalette) -> ChipStyle {
    ChipStyle {
        fill: pal.pill_lav_fill,
        stroke: LOGO_LAVENDER,
        text: pal.lavender_text,
    }
}

fn no_touch_chip_style(pal: &ThemePalette) -> ChipStyle {
    ChipStyle {
        fill: pal.pill_red_fill,
        stroke: pal.no_touch_outline,
        text: pal.no_touch_outline,
    }
}

fn confidence_chip_style(conf: DependencyConfidence, pal: &ThemePalette) -> ChipStyle {
    match conf {
        DependencyConfidence::Proven => ChipStyle {
            fill: pal.pill_blue_fill,
            stroke: pal.accent_line,
            text: pal.proven_text,
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
                c.is_whitespace() || matches!(c, ',' | ';' | ':' | '+')
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
    let resp = ui.add(
        egui::Label::new(RichText::new(&display).font(font_id).color(color)).selectable(false),
    );
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
    let resp = ui.add(
        egui::Label::new(RichText::new(&display).font(font_id).color(color)).selectable(false),
    );
    if truncated {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

/// Renders single-line truncated text and automatically shows a full-text tooltip on hover whenever truncated.
fn draw_truncated_text(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
    strong: bool,
) -> egui::Response {
    let measured_w = ui.fonts(|f| {
        f.layout_no_wrap(text.to_owned(), font_id.clone(), color)
            .size()
            .x
    });
    let mut rt = RichText::new(text).font(font_id).color(color);
    if strong {
        rt = rt.strong();
    }
    let resp = ui.add(egui::Label::new(rt).truncate());
    if measured_w > resp.rect.width() + 0.5 {
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
    let rt = RichText::new(format_size_2dec(bytes))
        .font(font_id)
        .color(color);
    ui.add(egui::Label::new(rt).selectable(false))
        .on_hover_text(format_exact_bytes(bytes));
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
    let (fill, border) = if pressed {
        (pal.widget_active, LOGO_PINK)
    } else if hovered {
        (pal.widget_hover, pal.accent_line)
    } else {
        (
            Color32::TRANSPARENT,
            lerp_color(pal.border, pal.accent_line, 0.72),
        )
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, 6.0, fill, border, 1.0, ppp);
    let c = snapped.center();
    match mode {
        ThemeMode::Dark => {
            // Sun glyph synced with z-controller outline icon button
            let fg = if pressed || hovered { pal.text } else { pal.accent_line };
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
            // Crescent moon glyph synced with z-controller (cutout matches transparent header canvas)
            let cutout = if pressed || hovered { fill } else { pal.bg };
            ui.painter()
                .circle_filled(c + Vec2::new(-0.5, 0.2), 5.3, pal.accent_line);
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
    fn compute(origin_x: f32, row_w: f32) -> Self {
        let gap = SPACE_12;
        let chev_w = 16.0;
        let chev_gap = SPACE_8;
        let pub_w = 152.0;
        let cat_w = 128.0;
        let delta_w = DELTA_COL_W;
        let size_w = SIZE_COL_W;
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
    size: ColSpan,
}

impl LocationsTableColumns {
    fn compute(origin_x: f32, sub_w: f32) -> Self {
        let gap = SPACE_12;
        let zone_w = 78.0;
        let basis_w = 170.0;
        let size_w = SIZE_COL_W;
        let fixed_w = zone_w + basis_w + size_w + gap * 3.0;
        let path_w = (sub_w - fixed_w).max(120.0);

        let zone = ColSpan::new(origin_x, zone_w);
        let path = ColSpan::new(zone.max_x + gap, path_w);
        let basis = ColSpan::new(path.max_x + gap, basis_w);
        let size = ColSpan::new(basis.max_x + gap, (origin_x + sub_w - (basis.max_x + gap)).max(size_w));
        Self {
            zone,
            path,
            basis,
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

    if hovered {
        ui.painter().rect_filled(cell_rect, Rounding::same(3.0), pal.widget_hover);
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
    let ppp = ui.ctx().pixels_per_point();
    let start_x = if right_aligned {
        (cell_rect.max.x - total_w).max(cell_rect.min.x)
    } else {
        cell_rect.min.x
    };
    let align_rect = Rect::from_min_max(Pos2::new(start_x, cell_rect.min.y), cell_rect.max);
    let text_pos = optical_galley_pos(ui, align_rect, &galley, None, false);
    ui.painter().galley(text_pos, galley, fg);
    if is_active {
        let arrow_c = Pos2::new(
            snap_scalar_to_px(start_x + total_w - 4.0, ppp),
            snap_scalar_to_px(cell_rect.center().y, ppp),
        );
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
        (pal.widget_active, LOGO_PINK, pal.text)
    } else if resp.hovered() {
        (pal.widget_hover, pal.accent_line, pal.text)
    } else if selected {
        (pal.pill_blue_fill, pal.accent_line, pal.text)
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
    dark_mode: bool,
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
        ui_state.truncate_cache.clear();
    }

    let report_stale = ui_state.cached_report.as_ref().is_none_or(|c| {
        c.scan_ts != rep.scan_timestamp_unix
            || c.dark_mode != dark_mode
            || c.apps_count != rep.apps.len()
    });
    if report_stale {
        let mut max_unfiltered_bytes = 0u64;
        let mut total_attributed_bytes = 0u64;
        let mut app_idx_by_id = HashMap::with_capacity(rep.apps.len());
        let mut apps = Vec::with_capacity(rep.apps.len());

        for (idx, app) in rep.apps.iter().enumerate() {
            max_unfiltered_bytes = max_unfiltered_bytes.max(app.total_bytes);
            total_attributed_bytes += app.total_bytes;
            app_idx_by_id.insert(app.id.clone(), idx);
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
        let sys_reconcile = rep
            .windows_system
            .iter()
            .map(|sys| {
                let apps_d = rep.total_apps_bytes_on_drive(sys.drive_letter);
                let cache_d = rep.total_shared_caches_on_drive(sys.drive_letter);
                let yf_d = rep.total_your_files_on_drive(sys.drive_letter);
                let un_d = rep.total_unattributed_on_drive(sys.drive_letter);
                let sum_all = apps_d + cache_d + yf_d + un_d + sys.total_bytes + sys.drive_free_bytes;
                let diff = sum_all as i128 - sys.drive_total_bytes as i128;
                let summary = format!(
                    "{}:\\ reconciliation · Apps {} + Caches {} + Your files {} + Unattributed {} + Win/sys {} + Free {} = {}",
                    sys.drive_letter,
                    format_size_2dec(apps_d),
                    format_size_2dec(cache_d),
                    format_size_2dec(yf_d),
                    format_size_2dec(un_d),
                    format_size_2dec(sys.total_bytes),
                    format_size_2dec(sys.drive_free_bytes),
                    format_size_2dec(sys.drive_total_bytes),
                );
                let exact = format!(
                    "{}:\\ exact: {} + {} + {} + {} + {} + {} = {} (diff {} B)",
                    sys.drive_letter,
                    format_exact_bytes(apps_d),
                    format_exact_bytes(cache_d),
                    format_exact_bytes(yf_d),
                    format_exact_bytes(un_d),
                    format_exact_bytes(sys.total_bytes),
                    format_exact_bytes(sys.drive_free_bytes),
                    format_exact_bytes(sys.drive_total_bytes),
                    diff,
                );
                (sys.drive_letter, summary, exact, diff)
            })
            .collect();

        ui_state.cached_report = Some(CachedReportData {
            scan_ts: rep.scan_timestamp_unix,
            dark_mode,
            apps_count: rep.apps.len(),
            app_idx_by_id,
            apps,
            max_unfiltered_bytes,
            total_attributed_bytes,
            attributed_label: format!("{} attributed", format_size_2dec(total_attributed_bytes)),
            attributed_tip: format!(
                "Total attributed app bytes across all drives: {}",
                format_exact_bytes(total_attributed_bytes)
            ),
            rev_counts,
            rev_by_app_token,
            sys_reconcile,
        });
    }

    let filter_stale = ui_state.cached_filtered.as_ref().is_none_or(|f| {
        f.scan_ts != rep.scan_timestamp_unix
            || f.sort_mode != ui_state.sort_mode
            || f.sort_direction != ui_state.sort_direction
            || f.filter_chip != ui_state.filter_chip
            || f.search_query != ui_state.search_query
            || f.sys_drive != sys_drive_letter
    });
    if filter_stale {
        let filtered = filter_and_sort_apps(
            &rep.apps,
            ui_state.sort_mode,
            ui_state.sort_direction,
            ui_state.filter_chip,
            &ui_state.search_query,
            sys_drive_letter,
        );
        let idx_map = &ui_state.cached_report.as_ref().unwrap().app_idx_by_id;
        let visible_indices = filtered
            .into_iter()
            .filter_map(|a| idx_map.get(&a.id).copied())
            .collect();
        ui_state.cached_filtered = Some(CachedFilteredList {
            scan_ts: rep.scan_timestamp_unix,
            sort_mode: ui_state.sort_mode,
            sort_direction: ui_state.sort_direction,
            filter_chip: ui_state.filter_chip,
            search_query: ui_state.search_query.clone(),
            sys_drive: sys_drive_letter,
            visible_indices,
        });
    }
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
    3.0 + COLLAPSED_ROW_OUTER_H
        + SPACE_4
        + 18.0
        + SPACE_4
        + 18.0
        + SUB_HEADER_H
        + locs_h
        + (SPACE_8 * 2.0)
        + 18.0
        + deps_h
        + (SPACE_8 + 3.0)
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
    let mode = config.theme_mode;
    if ui_state.applied_theme != Some(mode) {
        apply_theme_visuals(ctx, mode);
        ui_state.applied_theme = Some(mode);
    }
    ui_state.has_prop_strong = ctx.fonts(|f| {
        f.families()
            .iter()
            .any(|fam| *fam == FontFamily::Name("ProportionalStrong".into()))
    });
    let pal = ThemePalette::for_mode(mode);

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
            mode == ThemeMode::Dark,
        );
    }

    if ui_state.open_reverse_dep_id.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
        ui_state.open_reverse_dep_id = None;
    }

    // Top Header (no divider line beneath header)
    egui::TopBottomPanel::top("header_panel")
        .show_separator_line(false)
        .frame(
            egui::Frame::none()
                .fill(pal.bg)
                .stroke(Stroke::NONE)
                .inner_margin(Margin::symmetric(SPACE_16, SPACE_12)),
        )
        .show(ctx, |ui| {
            // Row 1: Title, quiet Read-only + scan metadata, [Theme Toggle Icon][Rescan][Settings]
            draw_split_row(
                ui,
                HEADER_CONTROL_H,
                236.0,
                Sense::hover(),
                |ui| {
                    ui.label(
                        RichText::new("y-storage")
                            .size(16.0)
                            .color(pal.text),
                    );
                    if scanning {
                        let status = if let Some(p) = progress {
                            format!(
                                "Read-only · {} · {} dirs · {}",
                                p.phase,
                                p.dirs_walked,
                                format_size_2dec(p.bytes_counted)
                            )
                        } else {
                            "Read-only · Scanning...".to_string()
                        };
                        draw_truncated_text(
                            ui,
                            &status,
                            FontId::new(11.5, FontFamily::Proportional),
                            pal.delta_grew,
                            false,
                        );
                    } else if let Some(rep) = report {
                        let status = format!(
                            "Read-only · Last scan: {} · {:.2}s",
                            rep.scan_timestamp_display,
                            rep.duration_ms as f64 / 1000.0
                        );
                        draw_truncated_text(
                            ui,
                            &status,
                            FontId::new(11.5, FontFamily::Proportional),
                            pal.muted,
                            false,
                        );
                    } else {
                        draw_truncated_text(
                            ui,
                            "Read-only · No cached scan — click Rescan",
                            FontId::new(11.5, FontFamily::Proportional),
                            pal.muted,
                            false,
                        );
                    }
                },
                |ui| {
                    // Right-to-left placement -> rightmost Settings, then Rescan, then Theme Icon immediately left of Rescan.
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
                        ui_state.active_view = match ui_state.active_view {
                            ActiveView::Main => ActiveView::Settings,
                            ActiveView::Settings | ActiveView::Dependencies => ActiveView::Main,
                        };
                    }

                    let rescan_lbl = if scanning { "Scanning..." } else { "Rescan" };
                    if draw_z_button(
                        ui,
                        rescan_lbl,
                        Vec2::new(76.0, HEADER_CONTROL_H),
                        12.0,
                        RADIUS_CONTROL,
                        !scanning,
                        false,
                        ButtonVariant::PrimaryOutline,
                        &pal,
                    )
                    .clicked()
                    {
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

            // Row 2: Signature Drive Instrument Header — 16pt headline free space + 2px proportional bar (used / total)
            if let Some(rep) = report {
                if !rep.drives.is_empty() {
                    ui.add_space(SPACE_8);
                    let n = rep.drives.len().max(1);
                    let avail = ui.available_width();
                    let tile_w = ((avail - SPACE_8 * ((n - 1) as f32)) / (n as f32)).max(132.0);
                    let ppp = ctx.pixels_per_point();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_8;
                        for drv in &rep.drives {
                            let ratio = if drv.total_bytes > 0 {
                                (drv.used_bytes as f64 / drv.total_bytes as f64).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            draw_crisp_frame(
                                ui,
                                pal.panel,
                                pal.border,
                                RADIUS_PANEL,
                                Margin::symmetric(SPACE_8, SPACE_4),
                                |ui| {
                                    let inner_w = (tile_w - SPACE_16 - 2.0).max(112.0);
                                    ui.set_width(inner_w);
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = SPACE_4;
                                        // Line 1: Drive letter + monospace used on left, headline 14.5pt monospace free space on right
                                        draw_split_row(
                                            ui,
                                            20.0,
                                            104.0,
                                            Sense::hover(),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = SPACE_4;
                                                ui.label(
                                                    RichText::new(format!("{}:\\", drv.letter))
                                                        .size(12.0)
                                                        .color(pal.text),
                                                );
                                                let used_font = mono_font_for(ui, 10.5, false);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(format!(
                                                            "{} used",
                                                            format_size_2dec(drv.used_bytes)
                                                        ))
                                                        .font(used_font)
                                                        .color(pal.subtle_text),
                                                    )
                                                    .selectable(false),
                                                )
                                                .on_hover_text(format!(
                                                    "Used: {} / Total: {}",
                                                    format_exact_bytes(drv.used_bytes),
                                                    format_exact_bytes(drv.total_bytes)
                                                ));
                                            },
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = SPACE_4;
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new("free")
                                                            .size(10.5)
                                                            .color(pal.muted),
                                                    )
                                                    .selectable(false),
                                                );
                                                let free_font = mono_font_for(ui, 14.5, false);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(format_size_2dec(drv.free_bytes))
                                                            .font(free_font)
                                                            .color(pal.text),
                                                    )
                                                    .selectable(false),
                                                )
                                                .on_hover_text(format!(
                                                    "Free: {} / Total: {}",
                                                    format_exact_bytes(drv.free_bytes),
                                                    format_exact_bytes(drv.total_bytes)
                                                ));
                                            },
                                        );

                                        // Line 2: Chunky 7px rounded capacity bar in signature 3-stop WT horizontal gradient (red only if drive unreachable/0 total)
                                        let bar_w = inner_w.floor().max(80.0);
                                        let (bar_rect, bar_resp) = ui.allocate_exact_size(
                                            Vec2::new(bar_w, 7.0),
                                            Sense::hover(),
                                        );
                                        let snapped_bar = paint_crisp_bordered_box(
                                            ui.painter(),
                                            bar_rect,
                                            3.5,
                                            pal.bg,
                                            pal.border,
                                            1.0,
                                            ppp,
                                        );
                                        let inset = 1.0 / ppp.max(0.5);
                                        let inner_track = snapped_bar.shrink(inset);
                                        let inner_px = (inner_track.width() * ppp).round();
                                        let fill_px = if drv.total_bytes == 0 || drv.used_bytes == 0 {
                                            0.0
                                        } else {
                                            ((inner_px as f64) * ratio)
                                                .round()
                                                .clamp(3.0, inner_px as f64) as f32
                                        };
                                        if drv.total_bytes == 0 && inner_track.width() > 4.0 {
                                            ui.painter().rect_filled(
                                                inner_track,
                                                Rounding::same(2.5),
                                                STATUS_RED,
                                            );
                                        } else if fill_px > 0.0 && inner_track.height() > 0.0 {
                                            let fill_rect = Rect::from_min_size(
                                                inner_track.min,
                                                Vec2::new(fill_px / ppp.max(0.5), inner_track.height()),
                                            );
                                            paint_horizontal_wt_gradient_bar(
                                                ui.painter(),
                                                inner_track,
                                                fill_rect,
                                                2.5,
                                            );
                                        }
                                        let _ = bar_resp.on_hover_text(format!(
                                            "{:.2}% used ({:.0} / {:.0} px · {})",
                                            ratio * 100.0,
                                            fill_px,
                                            inner_px,
                                            if drv.full_walk_enabled {
                                                "Full walk"
                                            } else {
                                                "Install paths only"
                                            }
                                        ));
                                    });
                                },
                            );
                        }
                    });
                }
            }

            if ui_state.active_view == ActiveView::Main {
                let total_apps_count = report.map_or(0, |r| r.apps.len());
                let shown_count = ui_state
                    .cached_filtered
                    .as_ref()
                    .map_or(0, |cf| cf.visible_indices.len());
                let mut list_view_dirty = false;

                ui.add_space(SPACE_8);
                // Row 3: Search input (column headers handle sorting)
                let mut search_dirty = false;
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), HEADER_CONTROL_H),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.label(RichText::new("Filter:").size(12.0).color(pal.muted));
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
                                11.5,
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
                    },
                );
                if search_dirty {
                    list_view_dirty = true;
                }

                ui.add_space(SPACE_4);
                // Row 4: Single-select filter chip row + live "N of 262 apps" count (left) + invariant attributed total (right)
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
                            if draw_filter_radio_chip(ui, lbl, selected, &pal) && !selected {
                                ui_state.filter_chip = chip;
                                list_view_dirty = true;
                            }
                        }
                        if report.is_some() {
                            let filtered_active = ui_state.filter_chip != AppFilterChip::All
                                || !ui_state.search_query.trim().is_empty();
                            let count_txt = format!("{shown_count} of {total_apps_count} apps");
                            let count_rt = RichText::new(count_txt)
                                .size(11.5)
                                .color(if filtered_active { pal.text } else { pal.muted });
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

                if list_view_dirty {
                    emit(UiAction::UpdateListView {
                        sort_mode: ui_state.sort_mode,
                        sort_direction: ui_state.sort_direction,
                        filter_chip: ui_state.filter_chip,
                        search_query: ui_state.search_query.clone(),
                    });
                }
            }

            if let Some(rep) = report {
                for w in &rep.warnings {
                    ui.add_space(SPACE_4);
                    draw_truncated_text(
                        ui,
                        &format!("Warning: {w}"),
                        FontId::new(11.0, FontFamily::Proportional),
                        pal.status_red,
                        false,
                    );
                }
            }
        });

    match ui_state.active_view {
        ActiveView::Main => {
            // Collapsible & drag-resizable Pinned Bottom Panel rendered BEFORE CentralPanel so only CentralPanel scrolls.
            if let Some(rep) = report {
                const LEDGER_MIN_H: f32 = 156.0;
                let max_ledger_h = (ctx.screen_rect().height() * 0.50).max(LEDGER_MIN_H + 24.0);
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
                        panel = panel.exact_height(h.clamp(LEDGER_MIN_H, max_ledger_h));
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
                            let next_h = (cur - grip_resp.drag_delta().y).clamp(LEDGER_MIN_H, max_ledger_h);
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
                    ui.painter().rect_filled(
                        Rect::from_min_size(Pos2::new(sr.left(), top_y), Vec2::new(sr.width(), 1.0)),
                        Rounding::ZERO,
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
                            let sys_lines = ui_state
                                .cached_report
                                .as_ref()
                                .map_or(1, |cr| cr.sys_reconcile.len().max(1)) as f32;
                            (h - (SPACE_16 + 22.0 + sys_lines * 24.0 + SPACE_4 * 2.0)).max(88.0)
                        })
                    } else {
                        None
                    };
                    draw_pinned_bottom_groups(ui, ui_state, rep, &pal, body_budget_h, emit);
                });
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg)
                        .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                )
                .show(ctx, |ui| {
                    draw_main_app_list(ui, ui_state, report, &pal, emit);
                });

            if let (Some(rep), Some(target_id)) = (report, ui_state.open_reverse_dep_id.clone()) {
                draw_reverse_dependency_overlay(ctx, ui_state, rep, &target_id, &pal);
            }
        }
        ActiveView::Dependencies => {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg)
                        .inner_margin(Margin::symmetric(SPACE_16, SPACE_12)),
                )
                .show(ctx, |ui| {
                    draw_dependencies_view(ui, ui_state, report, &pal);
                });
        }
        ActiveView::Settings => {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg)
                        .inner_margin(Margin::symmetric(SPACE_16, SPACE_12)),
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
    let mut cached_report = ui_state.cached_report.take();
    let visible_indices: Vec<usize> = ui_state
        .cached_filtered
        .as_ref()
        .map(|f| f.visible_indices.clone())
        .unwrap_or_default();
    let max_unfiltered_bytes = cached_report
        .as_ref()
        .map_or_else(|| rep.apps.iter().map(|a| a.total_bytes).max().unwrap_or(0), |c| c.max_unfiltered_bytes);
    let total_attributed_bytes = cached_report
        .as_ref()
        .map_or_else(|| rep.apps.iter().map(|a| a.total_bytes).sum(), |c| c.total_attributed_bytes);
    let shown = visible_indices.len();

    let avail_w = ui.available_width();
    let avail_h = ui.available_height();
    let panel_left = ui.cursor().min.x;
    let panel_w = (avail_w - SCROLL_GUTTER_W).max(320.0);
    let ppp = ui.ctx().pixels_per_point();
    let inset = 1.0 / ppp.max(0.5);

    let outer_border_slot = ui.painter().add(egui::Shape::Noop);
    let inner_fill_slot = ui.painter().add(egui::Shape::Noop);
    let header_bg_slot = ui.painter().add(egui::Shape::Noop);

    let panel_top_y = ui.cursor().min.y;
    ui.add_space(inset);

    let hdr_content_x0 = panel_left + inset + SPACE_12;
    let hdr_content_w = (panel_w - (inset * 2.0 + SPACE_12 * 2.0)).max(200.0);
    let cols = MainTableColumns::compute(hdr_content_x0, hdr_content_w);

    // Sticky column-header strip fixed outside ScrollArea; clicking sortable columns changes sort mode/direction.
    let (hdr_row_rect, _) =
        ui.allocate_exact_size(Vec2::new(panel_w, TABLE_HEADER_H), Sense::hover());
    let hdr_y0 = hdr_row_rect.min.y;
    let hdr_y1 = hdr_row_rect.max.y;

    let mut sort_clicked: Option<AppSortMode> = None;
    if draw_sortable_col_header(
        ui,
        cols.name.rect(hdr_y0, hdr_y1),
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
        cols.publisher.rect(hdr_y0, hdr_y1),
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
        cols.category.rect(hdr_y0, hdr_y1),
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
        cols.delta.rect(hdr_y0, hdr_y1),
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
    if draw_sortable_col_header(
        ui,
        cols.size.rect(hdr_y0, hdr_y1),
        "main_size",
        "Total size",
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
        emit(UiAction::UpdateListView {
            sort_mode: ui_state.sort_mode,
            sort_direction: ui_state.sort_direction,
            filter_chip: ui_state.filter_chip,
            search_query: ui_state.search_query.clone(),
        });
    }

    let hdr_div_rect = Rect::from_min_size(
        Pos2::new(panel_left + inset, hdr_y1),
        Vec2::new((panel_w - inset * 2.0).max(0.0), 1.0),
    );
    ui.painter()
        .rect_filled(hdr_div_rect, Rounding::ZERO, pal.border);
    ui.advance_cursor_after_rect(hdr_div_rect);

    // Precompute cumulative row y-offsets across all `shown` rows (`<= 262` floats) for exact O(log N) viewport virtualization.
    let mut row_tops = Vec::with_capacity(shown + 1);
    row_tops.push(0.0_f32);
    for &app_idx in &visible_indices {
        let app = &rep.apps[app_idx];
        let h = if ui_state.expanded_apps.contains(&app.id) {
            ui_state
                .expanded_row_heights
                .get(&app.id)
                .copied()
                .unwrap_or_else(|| estimate_expanded_row_height(app))
        } else {
            COLLAPSED_ROW_OUTER_H
        };
        let next = row_tops.last().copied().unwrap_or(0.0) + h;
        row_tops.push(next);
    }
    let total_virtual_h = row_tops.last().copied().unwrap_or(0.0);

    let body_max_h = (avail_h - (TABLE_HEADER_H + 1.0 + inset * 2.0 + 2.0)).max(80.0);
    let expanded_apps = &mut ui_state.expanded_apps;
    let expanded_row_heights = &mut ui_state.expanded_row_heights;
    let icon_textures = &mut ui_state.icon_textures;
    let open_reverse_dep_id = &mut ui_state.open_reverse_dep_id;
    let cached_rim = &mut ui_state.cached_rim;
    let truncate_cache = &mut ui_state.truncate_cache;
    let first_row_size_rect = &mut ui_state.first_row_size_rect;
    let mut show_bottom_fade = false;

    draw_synced_scroll_area_viewport(
        ui,
        "main_app_scroll",
        Some(body_max_h),
        [false, true],
        pal,
        pal.bg,
        |ui, viewport| {
            ui.set_width(panel_w);
            ui.spacing_mut().item_spacing.y = 0.0;
            show_bottom_fade = shown > 0 && viewport.max.y + 1.5 < total_virtual_h;

            let body_clip = Rect::from_min_max(
                Pos2::new(panel_left + inset, ui.clip_rect().min.y),
                Pos2::new(panel_left + panel_w - inset, ui.clip_rect().max.y),
            )
            .intersect(ui.clip_rect());
            ui.set_clip_rect(body_clip);

            if shown == 0 {
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
                .min(shown.saturating_sub(1));
            let end_vis = row_tops[first_vis + 1..]
                .partition_point(|&bot| bot < vis_y1)
                .saturating_add(first_vis + 1)
                .min(shown);

            let top_skip = row_tops[first_vis];
            if top_skip > 0.0 {
                ui.allocate_exact_size(Vec2::new(panel_w, top_skip), Sense::hover());
            }

            for row_idx in first_vis..end_vis {
                let app_idx = visible_indices[row_idx];
                let app = &rep.apps[app_idx];
                let expanded = expanded_apps.contains(&app.id);
                let row_slot = ui.painter().add(egui::Shape::Noop);

                let row_outer_rect = ui
                    .allocate_ui_with_layout(
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
                                    cols.chevron.rect(y0, y1),
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
                                let name_rect = cols.name.rect(y0, y1);
                                let icon_rect = Rect::from_center_size(
                                    Pos2::new(
                                        (name_rect.min.x + APP_ICON_SIZE * 0.5).round(),
                                        name_rect.center().y.round(),
                                    ),
                                    Vec2::splat(APP_ICON_SIZE),
                                );
                                if let Some(tex_id) =
                                    get_or_load_app_icon_tex(ui.ctx(), icon_textures, app)
                                {
                                    ui.painter().image(
                                        tex_id,
                                        icon_rect,
                                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                } else {
                                    draw_fallback_app_icon(ui.painter(), icon_rect, pal, ppp);
                                }

                                let text_rect = Rect::from_min_max(
                                    Pos2::new((icon_rect.max.x + SPACE_8).min(name_rect.max.x), name_rect.min.y),
                                    name_rect.max,
                                );
                                let (name_font, fallback_strong) = prop_strong_font(has_prop_strong, 13.0);
                                let mut n_ui = table_cell_ui(
                                    ui,
                                    text_rect,
                                    Layout::left_to_right(Align::Center),
                                );
                                n_ui.spacing_mut().item_spacing.x = SPACE_8;
                                let measured_name_w = if let Some(w) = cached_report
                                    .as_ref()
                                    .and_then(|cr| cr.apps[app_idx].name_w_px)
                                {
                                    w
                                } else {
                                    let w = n_ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            app.display_name.clone(),
                                            name_font.clone(),
                                            pal.text,
                                        )
                                        .size()
                                        .x
                                    });
                                    if let Some(cr) = cached_report.as_mut() {
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
                                    let chip_reserve = chip_text_w + 14.0 + SPACE_8;
                                    let max_name_w = (text_rect.width() - chip_reserve).max(40.0);
                                    if measured_name_w > max_name_w + 0.5 {
                                        truncated_name_tip = Some(app.display_name.clone());
                                    }
                                    let text_alloc_w = measured_name_w.min(max_name_w);
                                    let (t_rect, _) = n_ui.allocate_exact_size(
                                        Vec2::new(text_alloc_w, y1 - y0),
                                        Sense::hover(),
                                    );
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
                                    paint_chip(
                                        &mut n_ui,
                                        chip_lbl,
                                        no_touch_chip_style(pal),
                                        if expanded { pal.panel_alt } else { pal.panel },
                                    );
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
                                let pub_w = if let Some(w) = cached_report
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
                                    if let Some(cr) = cached_report.as_mut() {
                                        cr.apps[app_idx].pub_w_px = Some(w);
                                    }
                                    w
                                };
                                if pub_w > pub_rect.width() + 0.5 {
                                    truncated_pub_tip = Some(first_pub.clone());
                                }
                                let mut p_ui = table_cell_ui(
                                    ui,
                                    pub_rect,
                                    Layout::left_to_right(Align::Center),
                                );
                                p_ui.add(
                                    egui::Label::new(
                                    RichText::new(first_pub).font(pub_font).color(pal.subtle_text),
                                    )
                                    .truncate()
                                    .selectable(false),
                                );
                            }

                            // 4. Category (quiet label; muted for assigned categories, subtle for Uncategorized)
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

                            // 5. Growth delta (monospace Regular numerals for +/- deltas)
                            let delta_info = cached_report
                                .as_ref()
                                .and_then(|cr| cr.apps[app_idx].delta_display.clone())
                                .or_else(|| format_delta_display(app.growth, pal));
                            if let Some((ref d_text, d_col, _)) = delta_info {
                                let mut d_ui = table_cell_ui(
                                    ui,
                                    cols.delta.rect(y0, y1),
                                    Layout::right_to_left(Align::Center),
                                );
                                let d_font = if d_text == "new" {
                                    FontId::new(11.0, FontFamily::Proportional)
                                } else {
                                    mono_font_for(&d_ui, 11.5, false)
                                };
                                d_ui.add(
                                    egui::Label::new(
                                        RichText::new(d_text).font(d_font).color(d_col),
                                    )
                                    .selectable(false),
                                );
                            }

                            // 5. Total size (Consolas Regular numerals + thin <= 2px quiet proportion bar)
                            let size_cell_rect = cols.size.rect(y0, y1);
                            if row_idx == 0 {
                                *first_row_size_rect = Some(size_cell_rect);
                            }
                            {
                                let text_rect = Rect::from_min_max(
                                    size_cell_rect.min,
                                    Pos2::new(size_cell_rect.max.x, (y1 - 4.0).max(y0 + 14.0)),
                                );
                                let mut s_ui = table_cell_ui(
                                    ui,
                                    text_rect,
                                    Layout::right_to_left(Align::Center),
                                );
                                let s_font = mono_font_for(&s_ui, 13.0, false);
                                let total_label = cached_report
                                    .as_ref()
                                    .map(|cr| cr.apps[app_idx].total_str.as_str())
                                    .unwrap_or("");
                                if total_label.is_empty() {
                                    s_ui.add(
                                        egui::Label::new(
                                            RichText::new(format_size_2dec(app.total_bytes))
                                                .font(s_font)
                                                .color(pal.text),
                                        )
                                        .selectable(false),
                                    );
                                } else {
                                    s_ui.add(
                                        egui::Label::new(
                                            RichText::new(total_label)
                                                .font(s_font)
                                                .color(pal.text),
                                        )
                                        .selectable(false),
                                    );
                                }

                                // Thin 2px muted proportion indicator relative to largest unfiltered app.
                                let row_surface = if expanded { pal.panel_alt } else { pal.panel };
                                let track_rect = Rect::from_min_max(
                                    Pos2::new((size_cell_rect.max.x - PROP_BAR_W).max(size_cell_rect.min.x), y1 - PROP_BAR_H - 1.0),
                                    Pos2::new(size_cell_rect.max.x, y1 - 1.0),
                                );
                                let track_col = lerp_color(row_surface, pal.border, 0.55);
                                ui.painter()
                                    .rect_filled(track_rect, Rounding::same(1.0), track_col);

                                let ratio = if max_unfiltered_bytes > 0 {
                                    (app.total_bytes as f64 / max_unfiltered_bytes as f64).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                let fill_w = if app.total_bytes == 0 {
                                    0.0
                                } else {
                                    (track_rect.width() * ratio as f32).clamp(1.0, track_rect.width())
                                };
                                if fill_w > 0.0 {
                                    let bar_col = lerp_color(
                                        row_surface,
                                        pal.accent_line,
                                        if pal.bg.r() < 128 { 0.52 } else { 0.45 },
                                    );
                                    let fill_rect = Rect::from_min_size(
                                        track_rect.min,
                                        Vec2::new(fill_w, track_rect.height()),
                                    );
                                    ui.painter()
                                        .rect_filled(fill_rect, Rounding::same(1.0), bar_col);
                                }
                            }

                            let sum_click_rect = Rect::from_min_max(
                                Pos2::new(panel_left + inset, sum_rect.min.y),
                                Pos2::new(panel_left + panel_w - inset, sum_rect.max.y),
                            );
                            let sum_resp = ui.interact(
                                sum_click_rect,
                                ui.make_persistent_id(("app_row_toggle", &app.id)),
                                Sense::click(),
                            );
                            let mut cell_tooltip: Option<String> = None;
                            if let Some(pos) = sum_resp.hover_pos() {
                                if size_cell_rect.contains(pos) {
                                    cell_tooltip = cached_report
                                        .as_ref()
                                        .map(|cr| cr.apps[app_idx].total_tip.clone())
                                        .or_else(|| {
                                            let share_pct = if total_attributed_bytes > 0 {
                                                (app.total_bytes as f64 / total_attributed_bytes as f64) * 100.0
                                            } else {
                                                0.0
                                            };
                                            Some(format!(
                                                "{} · {:.2}% share",
                                                format_exact_bytes(app.total_bytes),
                                                share_pct
                                            ))
                                        });
                                } else if cols.delta.rect(y0, y1).contains(pos) {
                                    cell_tooltip = delta_info.map(|(_, _, tip)| tip);
                                } else if cols.category.rect(y0, y1).contains(pos) {
                                    let basis_lbl = if app.category_basis.is_empty() {
                                        "default".to_string()
                                    } else {
                                        app.category_basis.clone()
                                    };
                                    cell_tooltip = Some(format!("{} — {}", app.category.label(), basis_lbl));
                                } else if cols.publisher.rect(y0, y1).contains(pos) {
                                    cell_tooltip = truncated_pub_tip;
                                } else if cols.name.rect(y0, y1).contains(pos) {
                                    cell_tooltip = truncated_name_tip;
                                }
                            }
                            let row_hovered = sum_resp.hovered();
                            let row_clicked = sum_resp.clicked();
                            if let Some(tip) = cell_tooltip {
                                let _ = sum_resp.on_hover_text(tip);
                            }
                            if row_clicked {
                                if expanded {
                                    expanded_apps.remove(&app.id);
                                } else {
                                    expanded_apps.insert(app.id.clone());
                                }
                            }

                            if expanded {
                                let sub_x0 = cols.name.min_x;
                                let sub_w = (cols.size.max_x - sub_x0).max(200.0);
                                let loc_cols = LocationsTableColumns::compute(sub_x0, sub_w);
                                let dep_cols = DependenciesTableColumns::compute(sub_x0, sub_w);

                                // Expanded header strip: Category pill + classification rule basis (+ reverse-dep trigger when this app is a dependency)
                                ui.add_space(SPACE_4);
                                let (cat_row_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(panel_w, 18.0),
                                    Sense::hover(),
                                );
                                let mut cat_ui = table_cell_ui(
                                    ui,
                                    Rect::from_min_max(
                                        Pos2::new(sub_x0, cat_row_rect.min.y),
                                        Pos2::new(cols.size.max_x, cat_row_rect.max.y),
                                    ),
                                    Layout::left_to_right(Align::Center),
                                );
                                cat_ui.spacing_mut().item_spacing.x = SPACE_8;
                                cat_ui.label(
                                    RichText::new("Category")
                                        .size(11.0)
                                        .color(pal.muted),
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
                                let rev_match = cached_report.as_ref().and_then(|cr| {
                                    cr.rev_by_app_token
                                        .get(&normalize_app_token(&app.display_name))
                                        .cloned()
                                });
                                let mut used_btn_w = 0.0_f32;
                                if let Some((ref rev_id, rev_cnt)) = rev_match {
                                    if rev_cnt > 0 {
                                        let lbl = if rev_cnt == 1 {
                                            "used by 1 app >".to_string()
                                        } else {
                                            format!("used by {rev_cnt} apps >")
                                        };
                                        let is_open =
                                            open_reverse_dep_id.as_deref() == Some(rev_id.as_str());
                                        let btn_resp = draw_z_button(
                                            &mut cat_ui,
                                            &lbl,
                                            Vec2::new(0.0, 18.0),
                                            10.5,
                                            4.0,
                                            true,
                                            is_open,
                                            ButtonVariant::SecondaryOutline,
                                            pal,
                                        );
                                        used_btn_w = btn_resp.rect.width() + SPACE_8;
                                        if btn_resp
                                            .on_hover_text("Show apps depending on this component")
                                            .clicked()
                                        {
                                            *open_reverse_dep_id = if is_open {
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
                                    truncate_cache,
                                );

                                // Sub-table 1: Locations (Zone · Path · Basis · Size)
                                ui.add_space(SPACE_4);
                                let (loc_title_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(panel_w, 18.0),
                                    Sense::hover(),
                                );
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

                                let (loc_hdr_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(panel_w, SUB_HEADER_H),
                                    Sense::hover(),
                                );
                                let ly0 = loc_hdr_rect.min.y;
                                let ly1 = loc_hdr_rect.max.y;
                                draw_static_col_header(
                                    ui,
                                    loc_cols.zone.rect(ly0, ly1),
                                    "Zone",
                                    10.5,
                                    false,
                                    pal,
                                );
                                draw_static_col_header(
                                    ui,
                                    loc_cols.path.rect(ly0, ly1),
                                    "Path",
                                    10.5,
                                    false,
                                    pal,
                                );
                                draw_static_col_header(
                                    ui,
                                    loc_cols.basis.rect(ly0, ly1),
                                    "Basis",
                                    10.5,
                                    false,
                                    pal,
                                );
                                draw_static_col_header(
                                    ui,
                                    loc_cols.size.rect(ly0, ly1),
                                    "Size",
                                    10.5,
                                    true,
                                    pal,
                                );
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        Pos2::new(sub_x0, ly1 - 1.0),
                                        Vec2::new(sub_w, 1.0),
                                    ),
                                    Rounding::ZERO,
                                    pal.border,
                                );

                                for (loc_i, item) in app.items.iter().enumerate() {
                                    if loc_i > 0 {
                                        let div_y = ui.cursor().min.y;
                                        ui.painter().rect_filled(
                                            Rect::from_min_size(
                                                Pos2::new(sub_x0, div_y),
                                                Vec2::new(sub_w, 1.0),
                                            ),
                                            Rounding::ZERO,
                                            pal.border,
                                        );
                                        ui.add_space(1.0);
                                    }
                                    let (l_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(panel_w, SUB_LINE1_H + 2.0),
                                        Sense::hover(),
                                    );
                                    let ry0 = l_rect.min.y;
                                    let ry1 = l_rect.max.y;

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
                                                LocationQualifier::MachineShared => {
                                                    "ProgramData · shared"
                                                }
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
                                                    pal.lavender_text,
                                                )
                                                .size()
                                                .x
                                            });
                                            let q_reserve = q_text_w + 14.0 + SPACE_8;
                                            let max_p_w = (p_rect.width() - q_reserve).max(80.0);
                                            let measured_p_w = p_ui.fonts(|f| {
                                                f.layout_no_wrap(
                                                    item.path.clone(),
                                                    path_font.clone(),
                                                    pal.text,
                                                )
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
                                            draw_truncated_text(
                                                &mut pt_ui,
                                                &item.path,
                                                path_font,
                                                pal.text,
                                                false,
                                            );
                                            paint_chip(
                                                &mut p_ui,
                                                q_lbl,
                                                qualifier_chip_style(pal),
                                                pal.panel_alt,
                                            );
                                        } else {
                                            let mut p_ui = table_cell_ui(
                                                ui,
                                                p_rect,
                                                Layout::left_to_right(Align::Center),
                                            );
                                            draw_truncated_text(
                                                &mut p_ui,
                                                &item.path,
                                                path_font,
                                                pal.text,
                                                false,
                                            );
                                        }
                                    }

                                    {
                                        let b_rect = loc_cols.basis.rect(ry0, ry1);
                                        let mut b_ui = table_cell_ui(
                                            ui,
                                            b_rect,
                                            Layout::left_to_right(Align::Center),
                                        );
                                        let basis_str = item
                                            .basis
                                            .map(|b| b.label())
                                            .unwrap_or("—");
                                        draw_word_capped_text_cached(
                                            &mut b_ui,
                                            basis_str,
                                            FontId::new(10.5, FontFamily::Proportional),
                                            pal.subtle_text,
                                            b_rect.width(),
                                            truncate_cache,
                                        );
                                    }

                                    {
                                        let mut s_ui = table_cell_ui(
                                            ui,
                                            loc_cols.size.rect(ry0, ry1),
                                            Layout::right_to_left(Align::Center),
                                        );
                                        draw_right_aligned_size(
                                            &mut s_ui,
                                            item.bytes,
                                            11.5,
                                            pal.text,
                                            false,
                                        );
                                    }
                                }

                                ui.add_space(SPACE_8);
                                let sec_div_y = ui.cursor().min.y;
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        Pos2::new(sub_x0, sec_div_y),
                                        Vec2::new(sub_w, 1.0),
                                    ),
                                    Rounding::ZERO,
                                    pal.border,
                                );
                                ui.add_space(SPACE_8);

                                // Sub-table 2: Dependencies (Dependency name · Kind · Confidence · Used by · Own size)
                                let (dep_title_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(panel_w, 18.0),
                                    Sense::hover(),
                                );
                                let mut dt_ui = table_cell_ui(
                                    ui,
                                    Rect::from_min_max(
                                        Pos2::new(sub_x0, dep_title_rect.min.y),
                                        Pos2::new(cols.size.max_x, dep_title_rect.max.y),
                                    ),
                                    Layout::left_to_right(Align::Center),
                                );
                                dt_ui.spacing_mut().item_spacing.x = SPACE_8;
                                dt_ui.label(
                                    RichText::new("Dependencies")
                                        .size(11.0)
                                        .color(pal.muted),
                                );
                                dt_ui.label(
                                    RichText::new(format!(
                                        "{} · shared sizes excluded from app total",
                                        app.dependencies.len()
                                    ))
                                    .size(10.5)
                                    .color(pal.subtle_text),
                                );

                                if app.dependencies.is_empty() {
                                    let (empty_r, _) = ui.allocate_exact_size(
                                        Vec2::new(panel_w, SUB_LINE1_H),
                                        Sense::hover(),
                                    );
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
                                    let (dep_hdr_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(panel_w, SUB_HEADER_H),
                                        Sense::hover(),
                                    );
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
                                    ui.painter().rect_filled(
                                        Rect::from_min_size(
                                            Pos2::new(sub_x0, dy1 - 1.0),
                                            Vec2::new(sub_w, 1.0),
                                        ),
                                        Rounding::ZERO,
                                        pal.border,
                                    );

                                    for (dep_i, dep) in app.dependencies.iter().enumerate() {
                                        if dep_i > 0 {
                                            let div_y = ui.cursor().min.y;
                                            ui.painter().rect_filled(
                                                Rect::from_min_size(
                                                    Pos2::new(sub_x0, div_y),
                                                    Vec2::new(sub_w, 1.0),
                                                ),
                                                Rounding::ZERO,
                                                pal.border,
                                            );
                                            ui.add_space(2.0);
                                        }
                                        let rev_n = cached_report
                                            .as_ref()
                                            .and_then(|cr| cr.rev_counts.get(&dep.target_id).copied())
                                            .unwrap_or(0);
                                        let is_open_rev = open_reverse_dep_id.as_deref()
                                            == Some(dep.target_id.as_str());

                                        let (d_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(panel_w, SUB_LINE1_H + 2.0),
                                            Sense::hover(),
                                        );
                                        let r1y0 = d_rect.min.y;
                                        let r1y1 = d_rect.max.y;

                                        // 1. Dependency name
                                        {
                                            let mut dn_ui = table_cell_ui(
                                                ui,
                                                dep_cols.name.rect(r1y0, r1y1),
                                                Layout::left_to_right(Align::Center),
                                            );
                                            draw_truncated_text(
                                                &mut dn_ui,
                                                &dep.target_name,
                                                FontId::new(12.0, FontFamily::Proportional),
                                                pal.text,
                                                false,
                                            );
                                        }

                                        // 2. Kind
                                        {
                                            let mut dk_ui = table_cell_ui(
                                                ui,
                                                dep_cols.kind.rect(r1y0, r1y1),
                                                Layout::left_to_right(Align::Center),
                                            );
                                            draw_truncated_text(
                                                &mut dk_ui,
                                                dep.kind.label(),
                                                FontId::new(11.0, FontFamily::Proportional),
                                                pal.subtle_text,
                                                false,
                                            );
                                        }

                                        // 3. Confidence (+ shared / missing)
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
                                                paint_chip(
                                                    &mut dc_ui,
                                                    "missing",
                                                    no_touch_chip_style(pal),
                                                    pal.panel_alt,
                                                );
                                            }
                                        }

                                        // 4. Used by
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
                                                    ButtonVariant::SecondaryOutline,
                                                    pal,
                                                )
                                                .on_hover_text("Open reverse dependency list (\"What else uses this?\")")
                                                .clicked()
                                                {
                                                    *open_reverse_dep_id = if is_open_rev {
                                                        None
                                                    } else {
                                                        Some(dep.target_id.clone())
                                                    };
                                                }
                                            } else {
                                                du_ui.label(
                                                    RichText::new("—")
                                                        .size(11.0)
                                                        .color(pal.subtle_text),
                                                );
                                            }
                                        }

                                        // 5. Own size (quiet Regular Consolas, never Strong)
                                        {
                                            let mut ds_ui = table_cell_ui(
                                                ui,
                                                dep_cols.own_size.rect(r1y0, r1y1),
                                                Layout::right_to_left(Align::Center),
                                            );
                                            if let Some(own) = dep.own_size_bytes {
                                                draw_right_aligned_size(
                                                    &mut ds_ui,
                                                    own,
                                                    11.5,
                                                    pal.muted,
                                                    false,
                                                );
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
                                                    .on_hover_text("No separate on-disk folder (OS runtime or unmounted drive)");
                                            }
                                        }

                                        // Single word-boundary capped line ending in "…" (cached)
                                        let (basis_r, _) = ui.allocate_exact_size(
                                            Vec2::new(panel_w, SUB_LINE2_H + 2.0),
                                            Sense::hover(),
                                        );
                                        let basis_cap_w =
                                            (sub_w - SIZE_COL_W - SPACE_16).clamp(180.0, 560.0);
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
                                            truncate_cache,
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
                    expanded_row_heights.insert(app.id.clone(), r_full.height());
                }
                let inner_row_rect = Rect::from_min_max(
                    Pos2::new(panel_left + inset, r_full.min.y),
                    Pos2::new(panel_left + panel_w - inset, r_full.max.y),
                );
                if expanded {
                    let drawer_rect = inner_row_rect.shrink2(Vec2::new(2.0, 1.0));
                    let (fill_poly, rim_mesh) =
                        translate_cached_rim_for_rect(drawer_rect, RADIUS_PANEL, cached_rim);
                    ui.painter().set(
                        row_slot,
                        egui::Shape::convex_polygon(fill_poly, pal.panel_alt, Stroke::NONE),
                    );
                    ui.painter().add(egui::Shape::mesh(rim_mesh));
                } else if row_outer_rect.inner {
                    ui.painter().set(
                        row_slot,
                        egui::Shape::rect_filled(inner_row_rect, Rounding::ZERO, pal.widget_hover),
                    );
                }

                let next_expanded = visible_indices
                    .get(row_idx + 1)
                    .is_some_and(|&next_idx| expanded_apps.contains(&rep.apps[next_idx].id));
                if !expanded && row_idx + 1 < shown && !next_expanded {
                    let sep = Rect::from_min_size(
                        Pos2::new(inner_row_rect.min.x, (inner_row_rect.max.y - 1.0).round()),
                        Vec2::new(inner_row_rect.width(), 1.0),
                    );
                    ui.painter().rect_filled(sep, Rounding::ZERO, pal.border);
                }
            }

            let bot_skip = (total_virtual_h - row_tops[end_vis]).max(0.0);
            if bot_skip > 0.0 {
                ui.allocate_exact_size(Vec2::new(panel_w, bot_skip), Sense::hover());
            }
        },
    );
    ui_state.cached_report = cached_report;

    ui.add_space(inset + 2.0);
    let panel_bot_y = ui.cursor().min.y;
    let raw_panel_rect = Rect::from_min_max(
        Pos2::new(panel_left, panel_top_y),
        Pos2::new(panel_left + panel_w, panel_bot_y),
    );
    let snapped_panel = snap_rect_to_pixels(raw_panel_rect, ppp);
    ui.painter().set(
        outer_border_slot,
        egui::Shape::rect_filled(snapped_panel, Rounding::same(RADIUS_PANEL), pal.border),
    );
    let inner_panel = snapped_panel.shrink(inset);
    if inner_panel.width() > 0.0 && inner_panel.height() > 0.0 {
        let inner_r = (RADIUS_PANEL - inset).max(0.0);
        ui.painter().set(
            inner_fill_slot,
            egui::Shape::rect_filled(inner_panel, Rounding::same(inner_r), pal.panel),
        );
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
        if show_bottom_fade && inner_panel.height() > TABLE_HEADER_H + 30.0 {
            let fade_rect = Rect::from_min_max(
                Pos2::new(
                    inner_panel.min.x,
                    (inner_panel.max.y - 22.0).max(hdr_y1 + 1.0),
                ),
                Pos2::new(
                    (inner_panel.max.x - 10.0).max(inner_panel.min.x),
                    inner_panel.max.y,
                ),
            );
            paint_bottom_fade_overlay(ui.painter(), fade_rect, pal.panel);
        }
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
                            let (title_font, title_fallback_strong) =
                                prop_strong_font(ui_state.has_prop_strong, 14.0);
                            let mut title_rt = RichText::new(&rev.target_name)
                                .font(title_font)
                                .color(pal.text);
                            if title_fallback_strong {
                                title_rt = title_rt.strong();
                            }
                            ui.add(egui::Label::new(title_rt).truncate().selectable(false));
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
                                ButtonVariant::PrimaryOutline,
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
                                    fill: pal.pill_lav_fill,
                                    stroke: pal.border,
                                    text: pal.lavender_text,
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
                                    fill: pal.pill_blue_fill,
                                    stroke: pal.accent_line,
                                    text: pal.proven_text,
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
                                                paint_chip(
                                                    ui,
                                                    c.confidence.label(),
                                                    confidence_chip_style(c.confidence, pal),
                                                    pal.panel_alt,
                                                );
                                                let (cn_font, cn_fallback_strong) =
                                                    prop_strong_font(ui_state.has_prop_strong, 12.0);
                                                let mut cn_rt = RichText::new(&c.app_name)
                                                    .font(cn_font)
                                                    .color(pal.text);
                                                if cn_fallback_strong {
                                                    cn_rt = cn_rt.strong();
                                                }
                                                ui.add(egui::Label::new(cn_rt).truncate().selectable(false));
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
                                                draw_truncated_text(
                                                    ui,
                                                    &c.basis,
                                                    FontId::new(10.5, FontFamily::Proportional),
                                                    pal.muted,
                                                    false,
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
) {
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
                ui.label(
                    RichText::new("Pinned storage ledger")
                        .size(11.0)
                        .color(pal.text),
                );
                draw_truncated_text(
                    ui,
                    &collapsed_summary,
                    FontId::new(10.5, FontFamily::Proportional),
                    pal.muted,
                    false,
                );
            },
            |ui| {
                if let Some((exact, diff)) = &first_reconcile {
                    if *diff == 0 {
                        ui.add_space(SPACE_8);
                        ui.add(
                            egui::Label::new(
                                RichText::new("diff 0 B")
                                    .font(FontId::new(11.0, FontFamily::Monospace))
                                    .color(pal.status_green),
                            )
                            .selectable(false),
                        )
                        .on_hover_text(exact);
                        let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 4.0, STATUS_GREEN);
                    } else {
                        paint_chip(
                            ui,
                            &format!("diff {diff} B"),
                            ChipStyle {
                                fill: pal.pill_red_fill,
                                stroke: pal.status_red,
                                text: pal.status_red,
                            },
                            pal.panel_alt,
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
        return;
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
        return;
    }

    let any_expanded = ui_state.expand_windows_system
        || ui_state.expand_shared_caches
        || ui_state.expand_your_files
        || ui_state.expand_unattributed;
    let unattrib_tick = lerp_color(pal.border, pal.muted, 0.55);

    let mut draw_cards = |ui: &mut egui::Ui, frame_w: f32| {
        ui.spacing_mut().item_spacing.y = 0.0;
        draw_crisp_frame(
            ui,
            pal.panel,
            pal.border,
            RADIUS_PANEL,
            Margin::ZERO,
            |ui| {
                ui.set_width((frame_w - 2.0).max(180.0));
                ui.spacing_mut().item_spacing.y = 0.0;

                draw_pinned_group_card(
                    ui,
                    "Windows & system",
                    rep.windows_system_total_bytes,
                    "ESTIMATE · system remainder + pagefile/hiberfil",
                    pal.muted,
                    &mut ui_state.expand_windows_system,
                    true,
                    pal,
                    |ui| {
                        for sys in &rep.windows_system {
                            for rf in &sys.root_files {
                                draw_simple_item_row(ui, &rf.path, rf.bytes, Some(("ROOT", no_touch_chip_style(pal))), pal);
                            }
                            draw_simple_item_row(
                                ui,
                                &format!("{}:\\Windows & system remainder (estimate)", sys.drive_letter),
                                sys.remainder_bytes,
                                Some(("ESTIMATE", qualifier_chip_style(pal))),
                                pal,
                            );
                        }
                    },
                );

                draw_pinned_group_card(
                    ui,
                    "Your files",
                    rep.your_files_total_bytes,
                    "Personal shell folders · unzoned · read-only",
                    pal.muted,
                    &mut ui_state.expand_your_files,
                    true,
                    pal,
                    |ui| {
                        for item in &rep.your_files {
                            draw_simple_item_row(ui, &item.path, item.bytes, None, pal);
                        }
                    },
                );

                draw_pinned_group_card(
                    ui,
                    "Shared & system caches",
                    rep.shared_caches_total_bytes,
                    &format!("{} regenerable Temp / DXCache / Package Cache folders", rep.shared_caches.len()),
                    STATUS_GREEN,
                    &mut ui_state.expand_shared_caches,
                    true,
                    pal,
                    |ui| {
                        for item in rep.shared_caches.iter().take(40) {
                            draw_simple_item_row(
                                ui,
                                &item.path,
                                item.bytes,
                                Some(("GREEN", zone_chip_style(ZoneKind::Green, pal))),
                                pal,
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
                    &mut ui_state.expand_unattributed,
                    false,
                    pal,
                    |ui| {
                        for item in rep.unattributed.iter().take(40) {
                            draw_simple_item_row(ui, &item.path, item.bytes, None, pal);
                        }
                        if rep.unattributed.len() > 40 {
                            ui.label(
                                RichText::new(format!(
                                    "... and {} smaller unattributed folders",
                                    rep.unattributed.len() - 40
                                ))
                                .size(10.5)
                                .color(pal.subtle_text),
                            );
                        }
                    },
                );
            },
        );
    };

    if any_expanded || body_budget_h.is_some() {
        draw_synced_scroll_area(
            ui,
            "pinned_groups_scroll",
            Some(body_budget_h.unwrap_or(152.0)),
            [false, body_budget_h.is_none()],
            pal,
            pal.panel_alt,
            |ui| {
                let w = ui.available_width();
                draw_cards(ui, w);
            },
        );
    } else {
        draw_cards(ui, aligned_w);
    }

    // Quiet exact reconciliation ledger line (green status dot + tabular Regular diff 0 B; alert chip only if diff != 0).
    let sys_count = ui_state
        .cached_report
        .as_ref()
        .map_or(0, |cr| cr.sys_reconcile.len());
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
                            FontId::new(10.5, FontFamily::Proportional),
                            pal.subtle_text,
                            false,
                        );
                        let _ = resp.on_hover_text(exact);
                    },
                    |ui| {
                        if diff == 0 {
                            ui.add_space(SPACE_8);
                            ui.add(
                                egui::Label::new(
                                    RichText::new("diff 0 B")
                                        .font(FontId::new(11.0, FontFamily::Monospace))
                                        .color(pal.status_green),
                                )
                                .selectable(false),
                            );
                            let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                            ui.painter().circle_filled(dot_r.center(), 4.0, STATUS_GREEN);
                        } else {
                            paint_chip(
                                ui,
                                &format!("diff {diff} B"),
                                ChipStyle {
                                    fill: pal.pill_red_fill,
                                    stroke: pal.status_red,
                                    text: pal.status_red,
                                },
                                pal.panel_alt,
                            );
                        }
                    },
                );
            },
        );
    }
}

fn draw_pinned_group_card(
    ui: &mut egui::Ui,
    title: &str,
    total_bytes: u64,
    subtitle: &str,
    accent: Color32,
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
                    ui.label(RichText::new(title).size(12.0).color(pal.text));
                    draw_truncated_text(
                        ui,
                        subtitle,
                        FontId::new(11.0, FontFamily::Proportional),
                        pal.subtle_text,
                        false,
                    );
                },
                |ui| {
                    draw_right_aligned_size(ui, total_bytes, 12.5, pal.text, false);
                },
            );
            if resp.clicked() {
                *expanded = !*expanded;
            }
            if *expanded {
                body(ui);
            }
            resp.hovered()
        });

    let r = inner.response.rect;
    if inner.inner {
        ui.painter().set(
            bg_slot,
            egui::Shape::rect_filled(r, Rounding::ZERO, pal.widget_hover),
        );
    }
    if draw_bottom_divider {
        let sep = Rect::from_min_size(
            Pos2::new(r.min.x, (r.max.y - 1.0).round()),
            Vec2::new(r.width(), 1.0),
        );
        ui.painter().rect_filled(sep, Rounding::ZERO, pal.border);
    }
}

fn draw_simple_item_row(
    ui: &mut egui::Ui,
    path: &str,
    bytes: u64,
    chip: Option<(&str, ChipStyle)>,
    pal: &ThemePalette,
) {
    draw_split_row(
        ui,
        18.0,
        SIZE_COL_W,
        Sense::hover(),
        |ui| {
            ui.add_space(SPACE_16);
            if let Some((lbl, style)) = chip {
                paint_chip(ui, lbl, style, pal.panel);
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
            draw_chevron(ui, expanded, pal.accent_line);
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
                    fill: pal.pill_lav_fill,
                    stroke: pal.border,
                    text: pal.lavender_text,
                },
                pal.panel,
            );
            if dep.kind == DependencyKind::SharedRuntime || dep.binary_importer_count > 0 {
                paint_chip(
                    ui,
                    &format!("{} binary importers", dep.binary_importer_count),
                    ChipStyle {
                        fill: pal.pill_blue_fill,
                        stroke: pal.accent_line,
                        text: pal.proven_text,
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

    ui.label(
        RichText::new("Scan Scope & Exclusions")
            .size(16.0)
            .color(pal.text),
    );
    ui.add_space(SPACE_4);
    ui.label(
        RichText::new(
            "Default: only the system drive is enabled for a full walk with Windows excluded. Registry InstallLocation folders are always sized individually on all drives.",
        )
        .size(12.0)
        .color(pal.muted),
    );
    ui.add_space(SPACE_8);

    draw_synced_scroll_area(ui, "settings_scroll", None, [false, false], pal, pal.bg, |ui| {
            ui.spacing_mut().item_spacing.y = SPACE_8;
            for target in &mut draft.targets {
                let drv_info = report
                    .and_then(|r| r.drives.iter().find(|d| d.letter == target.letter));
                let is_sys = drv_info.is_some_and(|d| d.is_system_drive)
                    || target.letter.eq_ignore_ascii_case(&'C');

                draw_crisp_frame(
                    ui,
                    pal.panel,
                    pal.border,
                    RADIUS_PANEL,
                    Margin::symmetric(SPACE_12, SPACE_8),
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = SPACE_4;

                        draw_split_row(
                            ui,
                            24.0,
                            184.0,
                            Sense::hover(),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                ui.label(
                                    RichText::new(format!("Drive {}:\\", target.letter))
                                        .size(13.5)
                                        .color(pal.text),
                                );
                                let stats_str = if let Some(di) = drv_info {
                                    if is_sys {
                                        format!(
                                            "System drive · {} free · {} used · {} total",
                                            format_size_2dec(di.free_bytes),
                                            format_size_2dec(di.used_bytes),
                                            format_size_2dec(di.total_bytes),
                                        )
                                    } else {
                                        format!(
                                            "{} free · {} used · {} total",
                                            format_size_2dec(di.free_bytes),
                                            format_size_2dec(di.used_bytes),
                                            format_size_2dec(di.total_bytes),
                                        )
                                    }
                                } else if is_sys {
                                    "System drive".to_string()
                                } else {
                                    String::new()
                                };
                                if !stats_str.is_empty() {
                                    let resp = ui.label(
                                        RichText::new(stats_str)
                                            .size(11.5)
                                            .color(pal.muted),
                                    );
                                    if let Some(di) = drv_info {
                                        let _ = resp.on_hover_text(format!(
                                            "Free: {} · Used: {} · Total: {}",
                                            format_exact_bytes(di.free_bytes),
                                            format_exact_bytes(di.used_bytes),
                                            format_exact_bytes(di.total_bytes),
                                        ));
                                    }
                                }
                            },
                            |ui| {
                                if draw_crisp_checkbox(
                                    ui,
                                    &mut target.enabled,
                                    "Enable full drive walk",
                                    pal,
                                ) {
                                    changed = true;
                                }
                            },
                        );

                        if !target.exclusions.is_empty() {
                            ui.label(RichText::new("Exclusions:").size(11.5).color(pal.muted));
                        }

                        let mut remove_idx: Option<usize> = None;
                        for (idx, ex) in target.exclusions.iter().enumerate() {
                            let is_locked_win = is_sys && normalize_path_key(ex) == sys_win_key;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                ui.add_space(SPACE_8);
                                draw_truncated_text(
                                    ui,
                                    ex,
                                    FontId::new(11.5, FontFamily::Monospace),
                                    pal.text,
                                    false,
                                );
                                if is_locked_win {
                                    paint_chip(
                                        ui,
                                        "LOCKED",
                                        no_touch_chip_style(pal),
                                        pal.panel,
                                    );
                                    ui.label(
                                        RichText::new("System folder (always excluded)")
                                            .size(11.0)
                                            .color(pal.subtle_text),
                                    );
                                } else if draw_z_button(
                                    ui,
                                    "Remove",
                                    Vec2::new(60.0, 22.0),
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
                            });
                        }
                        if let Some(idx) = remove_idx {
                            target.exclusions.remove(idx);
                            changed = true;
                        }

                        let input = ui_state
                            .new_exclusion_inputs
                            .entry(target.letter)
                            .or_default();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_8;
                            if target.exclusions.is_empty() {
                                ui.label(
                                    RichText::new("Exclude folder:")
                                        .size(11.5)
                                        .color(pal.muted),
                                );
                            }
                            draw_crisp_text_input(
                                ui,
                                ("exclusion_input", target.letter),
                                input,
                                280.0,
                                &format!("{}:\\FolderToExclude", target.letter),
                                pal.input_bg,
                                pal,
                            );
                            if draw_z_button(
                                ui,
                                "Add exclusion",
                                Vec2::new(104.0, HEADER_CONTROL_H),
                                12.0,
                                RADIUS_CONTROL,
                                true,
                                false,
                                ButtonVariant::PrimaryOutline,
                                pal,
                            )
                            .clicked()
                            {
                                let trimmed = input.trim().to_string();
                                if !trimmed.is_empty()
                                    && !target
                                        .exclusions
                                        .iter()
                                        .any(|e| normalize_path_key(e) == normalize_path_key(&trimmed))
                                {
                                    target.exclusions.push(trimmed);
                                    input.clear();
                                    changed = true;
                                }
                            }
                        });
                    },
                );
            }
        });

    if changed {
        emit(UiAction::SaveConfig(draft));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, RawInput, Rect, Vec2};
    use y_core::model::{
        AttributedApp, AttributionBasis, BreakdownItem, DependencyRecord, DriveInfo,
        RegistryDiscoveryStats, ReverseConsumer, SystemDriveBreakdown, UnattributedItem,
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
                no_touch: None,
                bases: vec![AttributionBasis::InstallLocation, AttributionBasis::ExactName],
                items: vec![
                    BreakdownItem {
                        path: "C:\\Users\\Win\\AppData\\Local\\Discord".into(),
                        bytes: 30_000,
                        zone: Some(ZoneKind::Body),
                        qualifier: None,
                        basis: Some(AttributionBasis::InstallLocation),
                        no_touch: None,
                        drive_letter: 'C',
                    },
                    BreakdownItem {
                        path: "C:\\Users\\Win\\AppData\\Roaming\\discord".into(),
                        bytes: 20_000,
                        zone: Some(ZoneKind::Red),
                        qualifier: Some(LocationQualifier::RoamingConfig),
                        basis: Some(AttributionBasis::ExactName),
                        no_touch: None,
                        drive_letter: 'C',
                    },
                ],
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
                no_touch: None,
                drive_letter: 'C',
            }],
            your_files_total_bytes: 10_000,
            windows_system: vec![SystemDriveBreakdown {
                drive_letter: 'C',
                root_files: vec![],
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
            catalog_source: "embedded".into(),
            warnings: vec![],
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
                no_touch: None,
                bases: vec![AttributionBasis::ExactName],
                items: vec![BreakdownItem {
                    path: format!("C:\\Apps\\App{i:03}"),
                    bytes: 1_000_000 - (i as u64) * 1000,
                    zone: Some(ZoneKind::Body),
                    qualifier: None,
                    basis: Some(AttributionBasis::ExactName),
                    no_touch: None,
                    drive_letter: 'C',
                }],
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
            catalog_source: "embedded".into(),
            warnings: vec![],
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
        // Virtualization keeps per-frame shape count far below 262 full rows (~400+ shapes if unvirtualized).
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
        assert!(out.shapes.len() < 260, "expected virtualized shape count < 260, got {}", out.shapes.len());
    }
}
