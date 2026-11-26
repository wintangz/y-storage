use std::collections::{HashMap, HashSet};

use egui::{
    Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Key, Layout, Margin,
    Order, Pos2, Rect, RichText, Rounding, Sense, Stroke, TextStyle, UiBuilder, Vec2,
};
use y_core::config::{AppConfig, AppFilterChip, AppSortMode, SortDirection, ThemeMode};
use y_core::discovery::normalize_path_key;
use y_core::model::{
    DependencyConfidence, DependencyKind, GrowthDelta, LocationQualifier,
    NO_KNOWN_DEPENDENCIES_LABEL, ReverseDependencyEntry, ScanReport, ZoneKind,
};
use y_platform_windows::scanner::ScanProgressSnapshot;

use crate::list_query::filter_and_sort_apps;

// Pastel accent constants (used for accents, bars, and chips only; never large fills or body text).
pub const LOGO_BLUE: Color32 = Color32::from_rgb(84, 185, 242); // #54B9F2
pub const LOGO_BLUE_READABLE: Color32 = Color32::from_rgb(47, 155, 232); // #2F9BE8
pub const LOGO_PINK: Color32 = Color32::from_rgb(247, 168, 216); // #F7A8D8
pub const LOGO_LAVENDER: Color32 = Color32::from_rgb(216, 166, 255); // #D8A6FF

// Single multiples-of-4 spacing scale used uniformly everywhere.
pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;

// Multi-tier corner-radius scale from DESIGN.md (paired with z-controller synced controls).
const RADIUS_PANEL: f32 = 8.0;
const RADIUS_CONTROL: f32 = 6.0;
const RADIUS_PILL: f32 = 4.0;
const RADIUS_BADGE: f32 = 2.0;

const SIZE_COL_W: f32 = 96.0;
const DELTA_COL_W: f32 = 88.0;
const SCROLL_GUTTER_W: f32 = 6.0 + SPACE_8;
const HEADER_CONTROL_H: f32 = 28.0;
const APP_ROW_H: f32 = 30.0;
const PINNED_ROW_H: f32 = 24.0;
const SUB_LINE1_H: f32 = 20.0;
const SUB_LINE2_H: f32 = 16.0;
const SEGOE_UI_FONT_PATH: &str = r"C:\Windows\Fonts\segoeui.ttf";

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
    pub widget: Color32,
    pub widget_hover: Color32,
    pub widget_active: Color32,
    pub accent_line: Color32,
    pub lavender_text: Color32,
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
                bg: Color32::from_rgb(10, 10, 16),        // #0A0A10
                panel: Color32::from_rgb(20, 20, 31),     // #14141F
                panel_alt: Color32::from_rgb(16, 16, 26), // #10101A (z-controller faint_bg)
                border: Color32::from_rgb(38, 38, 58),    // #26263A
                text: Color32::from_rgb(237, 237, 242),   // #EDEDF2
                muted: Color32::from_rgb(154, 154, 176),  // #9A9AB0
                subtle_text: Color32::from_rgb(114, 116, 140), // #72748C
                hint_text: Color32::from_rgb(86, 88, 110), // #56586E (distinct dim hint)
                input_bg: Color32::from_rgb(10, 10, 16),  // #0A0A10
                widget: Color32::from_rgb(24, 24, 38),    // #181826 (z-controller widget)
                widget_hover: Color32::from_rgb(31, 31, 49), // #1F1F31 (z-controller widget_hover)
                widget_active: Color32::from_rgb(24, 34, 48), // #182230 (z-controller widget_active)
                accent_line: LOGO_BLUE,
                lavender_text: LOGO_LAVENDER,
                status_green: Color32::from_rgb(40, 214, 108),  // #28D66C
                status_red: Color32::from_rgb(240, 68, 88),     // #F04458
                delta_grew: LOGO_PINK,
                no_touch_outline: Color32::from_rgb(206, 92, 108),
                pill_blue_fill: Color32::from_rgb(18, 28, 40),  // #121C28 (z-controller pill_blue_fill)
                pill_green_fill: Color32::from_rgb(16, 38, 26),
                pill_lav_fill: Color32::from_rgb(32, 24, 46),
                pill_red_fill: Color32::from_rgb(46, 20, 26),
                scrollbar_thumb: Color32::from_rgb(46, 46, 70), // #2E2E46
                scrollbar_thumb_hover: LOGO_BLUE,
            },
            ThemeMode::Light => Self {
                bg: Color32::from_rgb(238, 244, 250),     // #EEF4FA
                panel: Color32::from_rgb(255, 255, 255),  // #FFFFFF
                panel_alt: Color32::from_rgb(244, 248, 252), // #F4F8FC (z-controller faint_bg)
                border: Color32::from_rgb(217, 228, 240), // #D9E4F0 (z-controller border)
                text: Color32::from_rgb(28, 39, 51),      // #1C2733
                muted: Color32::from_rgb(95, 113, 134),   // #5F7186
                subtle_text: Color32::from_rgb(116, 132, 153), // #748499
                hint_text: Color32::from_rgb(152, 166, 184), // #98A6B8 (distinct muted hint)
                input_bg: Color32::from_rgb(255, 255, 255), // #FFFFFF (crisp contrast vs #EEF4FA header)
                widget: Color32::from_rgb(245, 248, 252), // #F5F8FC (z-controller widget)
                widget_hover: Color32::from_rgb(233, 242, 250), // #E9F2FA (z-controller widget_hover)
                widget_active: Color32::from_rgb(220, 235, 248), // #DCEBF8 (z-controller widget_active)
                accent_line: LOGO_BLUE_READABLE,
                lavender_text: Color32::from_rgb(120, 62, 184),
                status_green: Color32::from_rgb(20, 138, 68),
                status_red: Color32::from_rgb(206, 40, 64),
                delta_grew: Color32::from_rgb(196, 48, 88),
                no_touch_outline: Color32::from_rgb(188, 64, 80),
                pill_blue_fill: Color32::from_rgb(235, 246, 255), // #EBF6FF (z-controller pill_blue_fill)
                pill_green_fill: Color32::from_rgb(230, 248, 237),
                pill_lav_fill: Color32::from_rgb(243, 236, 255),
                pill_red_fill: Color32::from_rgb(253, 234, 237),
                scrollbar_thumb: Color32::from_rgb(192, 208, 226), // #C0D0E2
                scrollbar_thumb_hover: LOGO_BLUE_READABLE,
            },
        }
    }
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
    if let Ok(bytes) = std::fs::read(SEGOE_UI_FONT_PATH) {
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert("segoe_ui".to_owned(), FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe_ui".to_owned());
        }
        ctx.set_fonts(fonts);
    }
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
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.text);
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

/// Paints an unbroken rounded rectangle border by layering pixel-snapped outer (border) and inner (fill) filled rects, eliminating 96-DPI horizontal stroke dropout in SoftwarePainter.
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
    if border_color != Color32::TRANSPARENT {
        painter.rect_filled(snapped, Rounding::same(rounding), border_color);
    }
    let inner = snapped.shrink(inset);
    if inner.width() > 0.0 && inner.height() > 0.0 && fill != Color32::TRANSPARENT {
        painter.rect_filled(
            inner,
            Rounding::same((rounding - inset).max(0.0)),
            fill,
        );
    }
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

/// Renders a button whose states and continuous 1px rounded outline mirror z-controller across both themes.
fn draw_z_button(
    ui: &mut egui::Ui,
    label: &str,
    min_size: Vec2,
    font_size: f32,
    rounding: f32,
    enabled: bool,
    active_highlight: bool,
    pal: &ThemePalette,
) -> egui::Response {
    let font_id = FontId::new(font_size, FontFamily::Proportional);
    let text_color = if !enabled {
        pal.muted
    } else if active_highlight {
        pal.accent_line
    } else {
        pal.text
    };
    let h_pad = if min_size.y <= 22.0 { SPACE_8 } else { SPACE_12 };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let desired_w = (galley.size().x + h_pad * 2.0).max(min_size.x).round();
    let desired_h = min_size.y.round();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, desired_h), sense);

    let (fill, border_col) = if !enabled {
        (pal.panel_alt, pal.border)
    } else if resp.is_pointer_button_down_on() {
        (pal.widget_active, LOGO_PINK)
    } else if resp.hovered() {
        (pal.widget_hover, pal.accent_line)
    } else if active_highlight {
        (pal.pill_blue_fill, pal.accent_line)
    } else {
        (pal.widget, pal.border)
    };

    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, rounding, fill, border_col, 1.0, ppp);
    let text_pos = Pos2::new(
        (snapped.center().x - galley.size().x * 0.5).round(),
        (snapped.center().y - galley.size().y * 0.5).round(),
    );
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
        pal.input_bg,
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

/// Vertical scroll area styled identically to z-controller: slim palette thumb, inside gutter, zero card overlap.
fn draw_synced_scroll_area(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    max_height: Option<f32>,
    auto_shrink: [bool; 2],
    pal: &ThemePalette,
    rail_bg: Color32,
    body: impl FnOnce(&mut egui::Ui),
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
        sa.show(ui, |ui| {
            // Restore inner widget fills so buttons inside scrolled cards still paint with pal.widget.
            {
                let v = ui.visuals_mut();
                v.extreme_bg_color = pal.bg;
                v.widgets.inactive.bg_fill = pal.widget;
                v.widgets.hovered.bg_fill = pal.widget_hover;
                v.widgets.active.bg_fill = pal.widget_active;
            }
            body(ui);
        });
    });
}

fn paint_chip(ui: &mut egui::Ui, label: &str, style: ChipStyle, container_bg: Color32) -> egui::Response {
    let fill = if style.fill == Color32::TRANSPARENT {
        container_bg
    } else {
        style.fill
    };
    draw_crisp_frame(
        ui,
        fill,
        style.stroke,
        RADIUS_BADGE,
        Margin::symmetric(5.0, 1.0),
        |ui| {
            ui.label(
                RichText::new(label)
                    .size(10.5)
                    .color(style.text)
                    .strong(),
            );
        },
    )
    .response
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
        fill: Color32::TRANSPARENT,
        stroke: pal.no_touch_outline,
        text: pal.no_touch_outline,
    }
}

fn confidence_chip_style(conf: DependencyConfidence, pal: &ThemePalette) -> ChipStyle {
    match conf {
        DependencyConfidence::Proven => ChipStyle {
            fill: pal.pill_blue_fill,
            stroke: pal.accent_line,
            text: pal.accent_line,
        },
        DependencyConfidence::Inferred => ChipStyle {
            fill: Color32::TRANSPARENT,
            stroke: pal.muted,
            text: pal.muted,
        },
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
    let mut rt = RichText::new(format_size_2dec(bytes))
        .size(size_px)
        .color(color);
    if strong {
        rt = rt.strong();
    }
    ui.add(egui::Label::new(rt).selectable(false))
        .on_hover_text(format_exact_bytes(bytes));
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
    let (fill, border) = if resp.is_pointer_button_down_on() {
        (pal.widget_active, LOGO_PINK)
    } else if hovered {
        (pal.widget_hover, pal.accent_line)
    } else {
        (pal.widget, pal.border)
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, 6.0, fill, border, 1.0, ppp);
    let c = snapped.center();
    match mode {
        ThemeMode::Dark => {
            // Sun glyph synced with z-controller
            let fg = if hovered { pal.accent_line } else { pal.text };
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
            // Crescent moon glyph synced with z-controller
            ui.painter()
                .circle_filled(c + Vec2::new(-0.5, 0.2), 5.3, pal.accent_line);
            ui.painter()
                .circle_filled(c + Vec2::new(2.2, -1.9), 4.4, fill);
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

fn draw_sort_mode_button(
    ui: &mut egui::Ui,
    mode: AppSortMode,
    active_mode: AppSortMode,
    active_dir: SortDirection,
    pal: &ThemePalette,
) -> bool {
    let is_active = mode == active_mode;
    let label = match mode {
        AppSortMode::Size => "Size",
        AppSortMode::Name => {
            if is_active {
                match active_dir {
                    SortDirection::Asc => "Name A-Z",
                    SortDirection::Desc => "Name Z-A",
                }
            } else {
                "Name"
            }
        }
        AppSortMode::Growth => "Growth",
    };

    let arrow_space = if is_active { 14.0 } else { 0.0 };
    let font_id = FontId::new(12.0, FontFamily::Proportional);
    let text_color = if is_active { pal.accent_line } else { pal.muted };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let desired_w = (galley.size().x + SPACE_12 * 2.0 + arrow_space)
        .max(60.0)
        .round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, 26.0), Sense::click());

    let (fill, border_col, draw_fg) = if resp.is_pointer_button_down_on() {
        (pal.widget_active, LOGO_PINK, pal.text)
    } else if resp.hovered() {
        (
            pal.widget_hover,
            pal.accent_line,
            if is_active { pal.accent_line } else { pal.text },
        )
    } else if is_active {
        (pal.pill_blue_fill, pal.accent_line, pal.accent_line)
    } else {
        (pal.widget, pal.border, pal.muted)
    };

    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, 6.0, fill, border_col, 1.0, ppp);
    let content_w = galley.size().x + arrow_space;
    let start_x = (snapped.center().x - content_w * 0.5).round();
    let text_pos = Pos2::new(start_x, (snapped.center().y - galley.size().y * 0.5).round());
    ui.painter().galley(text_pos, galley, draw_fg);
    if is_active {
        let arrow_c = Pos2::new(
            (start_x + content_w - 4.0).round(),
            snapped.center().y.round(),
        );
        draw_sort_arrow(ui.painter(), arrow_c, active_dir == SortDirection::Desc, draw_fg);
    }

    let tip = match mode {
        AppSortMode::Size => "Sort by total storage footprint (click to flip desc/asc)",
        AppSortMode::Name => "Sort alphabetically by name (click to flip A-Z / Z-A)",
        AppSortMode::Growth => {
            "Sort by growth delta vs previous scan (click to flip desc/asc; no-baseline apps follow at bottom)"
        }
    };
    let clicked = resp.clicked();
    let _ = resp.on_hover_text(tip);
    clicked
}

fn draw_filter_radio_chip(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    pal: &ThemePalette,
) -> bool {
    let font_id = FontId::new(11.5, FontFamily::Proportional);
    let text_color = if selected { pal.accent_line } else { pal.muted };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, text_color));
    let desired_w = (galley.size().x + SPACE_16).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, 22.0), Sense::click());
    let (fill, border_col, draw_fg) = if resp.is_pointer_button_down_on() {
        (pal.widget_active, LOGO_PINK, pal.text)
    } else if resp.hovered() {
        (
            pal.widget_hover,
            pal.accent_line,
            if selected { pal.accent_line } else { pal.text },
        )
    } else if selected {
        (pal.pill_blue_fill, pal.accent_line, pal.accent_line)
    } else {
        (pal.widget, pal.border, pal.muted)
    };
    let ppp = ui.ctx().pixels_per_point();
    let snapped = paint_crisp_bordered_box(ui.painter(), rect, RADIUS_PILL, fill, border_col, 1.0, ppp);
    let text_pos = Pos2::new(
        (snapped.center().x - galley.size().x * 0.5).round(),
        (snapped.center().y - galley.size().y * 0.5).round(),
    );
    ui.painter().galley(text_pos, galley, draw_fg);
    resp.clicked()
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
    let pal = ThemePalette::for_mode(mode);

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
                            .strong()
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
                                        // Line 1: Drive letter + used on left, headline 16pt free space on right
                                        draw_split_row(
                                            ui,
                                            20.0,
                                            (inner_w * 0.46).clamp(92.0, 102.0),
                                            Sense::hover(),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = SPACE_4;
                                                ui.label(
                                                    RichText::new(format!("{}:\\", drv.letter))
                                                        .size(12.5)
                                                        .strong()
                                                        .color(pal.text),
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(format!(
                                                            "{} used",
                                                            format_size_2dec(drv.used_bytes)
                                                        ))
                                                        .size(10.5)
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
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(format_size_2dec(drv.free_bytes))
                                                            .size(16.0)
                                                            .strong()
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

                                        // Line 2: Full-width 6px 2px-radius bordered track filled proportionally: fill_px = round(inner_px * (used / total))
                                        let bar_w = inner_w.floor().max(80.0);
                                        let (bar_rect, bar_resp) = ui.allocate_exact_size(
                                            Vec2::new(bar_w, 6.0),
                                            Sense::hover(),
                                        );
                                        let snapped_bar = paint_crisp_bordered_box(
                                            ui.painter(),
                                            bar_rect,
                                            RADIUS_BADGE,
                                            pal.input_bg,
                                            pal.border,
                                            1.0,
                                            ppp,
                                        );
                                        let inset = 1.0 / ppp.max(0.5);
                                        let inner_track = snapped_bar.shrink(inset);
                                        let inner_px = (inner_track.width() * ppp).round();
                                        let fill_px = if drv.used_bytes == 0 {
                                            0.0
                                        } else {
                                            ((inner_px as f64) * ratio)
                                                .round()
                                                .clamp(1.0, inner_px as f64) as f32
                                        };
                                        if fill_px > 0.0 && inner_track.height() > 0.0 {
                                            let fill_rect = Rect::from_min_size(
                                                inner_track.min,
                                                Vec2::new(fill_px / ppp.max(0.5), inner_track.height()),
                                            );
                                            let bar_col = if drv.full_walk_enabled {
                                                pal.accent_line
                                            } else {
                                                LOGO_LAVENDER
                                            };
                                            ui.painter().rect_filled(
                                                fill_rect,
                                                Rounding::ZERO,
                                                bar_col,
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
                let sys_drive_letter = report
                    .and_then(|r| {
                        r.drives
                            .iter()
                            .find(|d| d.is_system_drive)
                            .map(|d| d.letter.to_ascii_uppercase())
                    })
                    .unwrap_or('C');
                let total_apps_count = report.map_or(0, |r| r.apps.len());
                let shown_count = report.map_or(0, |r| {
                    filter_and_sort_apps(
                        &r.apps,
                        ui_state.sort_mode,
                        ui_state.sort_direction,
                        ui_state.filter_chip,
                        &ui_state.search_query,
                        sys_drive_letter,
                    )
                    .len()
                });
                let mut list_view_dirty = false;

                ui.add_space(SPACE_8);
                // Row 3: Search input (left) + Segmented Sort Control (right)
                let mut search_dirty = false;
                let mut sort_dirty = false;
                draw_split_row(
                    ui,
                    HEADER_CONTROL_H,
                    300.0,
                    Sense::hover(),
                    |ui| {
                        ui.label(RichText::new("Filter:").size(12.0).color(pal.muted));
                        let prev_q = ui_state.search_query.clone();
                        draw_crisp_text_input(
                            ui,
                            "main_app_filter",
                            &mut ui_state.search_query,
                            260.0,
                            "Search app name, publisher, or path...",
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
                                &pal,
                            )
                            .clicked()
                        {
                            ui_state.search_query.clear();
                            search_dirty = true;
                        }
                    },
                    |ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_4;
                        // Right-to-left placement -> visually left-to-right: Sort: [Size] [Name] [Growth]
                        for mode_btn in [AppSortMode::Growth, AppSortMode::Name, AppSortMode::Size] {
                            if draw_sort_mode_button(
                                ui,
                                mode_btn,
                                ui_state.sort_mode,
                                ui_state.sort_direction,
                                &pal,
                            ) {
                                let (nm, nd) = mode_btn.click_transition(
                                    ui_state.sort_mode,
                                    ui_state.sort_direction,
                                );
                                ui_state.sort_mode = nm;
                                ui_state.sort_direction = nd;
                                sort_dirty = true;
                            }
                        }
                        ui.add_space(SPACE_4);
                        ui.label(RichText::new("Sort:").size(12.0).color(pal.muted));
                    },
                );
                if search_dirty || sort_dirty {
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
                            let mut count_rt = RichText::new(count_txt)
                                .size(11.5)
                                .color(if filtered_active { pal.text } else { pal.muted });
                            if filtered_active {
                                count_rt = count_rt.strong();
                            }
                            ui.add(egui::Label::new(count_rt).selectable(false));
                        }
                    },
                    |ui| {
                        if let Some(rep) = report {
                            let apps_sum: u64 = rep.apps.iter().map(|a| a.total_bytes).sum();
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{} attributed", format_size_2dec(apps_sum)))
                                        .size(11.5)
                                        .color(pal.muted),
                                )
                                .selectable(false),
                            )
                            .on_hover_text(format!(
                                "Total attributed app bytes across all drives: {}",
                                format_exact_bytes(apps_sum)
                            ));
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
            // Pinned Bottom Panel rendered BEFORE CentralPanel so only CentralPanel app list scrolls and bottom groups never clip.
            if let Some(rep) = report {
                egui::TopBottomPanel::bottom("pinned_groups_panel")
                    .show_separator_line(false)
                    .frame(
                        egui::Frame::none()
                            .fill(pal.panel_alt)
                            .stroke(Stroke::NONE)
                            .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                    )
                    .show(ctx, |ui| {
                        let sr = ctx.screen_rect();
                        let top_y = (ui.max_rect().top() - SPACE_8).round();
                        ui.painter().rect_filled(
                            Rect::from_min_size(Pos2::new(sr.left(), top_y), Vec2::new(sr.width(), 1.0)),
                            Rounding::ZERO,
                            pal.border,
                        );
                        draw_pinned_bottom_groups(ui, ui_state, rep, &pal);
                    });
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg)
                        .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                )
                .show(ctx, |ui| {
                    draw_main_app_list(ui, ui_state, report, &pal);
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

    let rev_counts: HashMap<&str, usize> = rep
        .reverse_dependencies
        .iter()
        .map(|r| (r.target_id.as_str(), r.dependent_count))
        .collect();

    let sys_drive_letter = rep
        .drives
        .iter()
        .find(|d| d.is_system_drive)
        .map(|d| d.letter.to_ascii_uppercase())
        .unwrap_or('C');
    let visible_apps = filter_and_sort_apps(
        &rep.apps,
        ui_state.sort_mode,
        ui_state.sort_direction,
        ui_state.filter_chip,
        &ui_state.search_query,
        sys_drive_letter,
    );

    draw_synced_scroll_area(ui, "main_app_scroll", None, [false, false], pal, pal.bg, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let shown = visible_apps.len();

        if shown == 0 {
            draw_crisp_frame(
                ui,
                pal.panel,
                pal.border,
                RADIUS_PANEL,
                Margin::same(SPACE_12),
                |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("No matching apps for the active filter/search.")
                            .size(12.5)
                            .color(pal.muted),
                    );
                },
            );
            return;
        }

        // Single calm 8px-rounded ledger container with hairline row dividers (zero uniform card soup).
        draw_crisp_frame(
            ui,
            pal.panel,
            pal.border,
            RADIUS_PANEL,
            Margin::ZERO,
            |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;

                for (row_idx, app) in visible_apps.iter().enumerate() {
                    let expanded = ui_state.expanded_apps.contains(&app.id);
                    let row_slot = ui.painter().add(egui::Shape::Noop);
                    let accent_bar_slot = ui.painter().add(egui::Shape::Noop);

                    let row_res = egui::Frame::none()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(Margin::symmetric(SPACE_12, SPACE_4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y = SPACE_4;

                            // Dominant quantity (T2: 13.5pt Strong) vs primary identity (T3: 13.0pt Regular) vs meta (T4: 11.0pt subtle)
                            let right_col_w = DELTA_COL_W + SPACE_8 + SIZE_COL_W;
                            let header_resp = draw_split_row(
                                ui,
                                APP_ROW_H,
                                right_col_w,
                                Sense::click(),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = SPACE_8;
                                    draw_chevron(
                                        ui,
                                        expanded,
                                        if expanded { pal.accent_line } else { pal.muted },
                                    );
                                    draw_truncated_text(
                                        ui,
                                        &app.display_name,
                                        FontId::new(13.0, FontFamily::Proportional),
                                        pal.text,
                                        false,
                                    );
                                    if let Some(nt) = app.no_touch {
                                        paint_chip(
                                            ui,
                                            nt.label(),
                                            no_touch_chip_style(pal),
                                            if expanded { pal.panel_alt } else { pal.panel },
                                        );
                                    }
                                    if let Some(first_pub) = app.publishers.first() {
                                        draw_truncated_text(
                                            ui,
                                            first_pub,
                                            FontId::new(11.0, FontFamily::Proportional),
                                            pal.subtle_text,
                                            false,
                                        );
                                    }
                                },
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = SPACE_8;
                                    let (sz_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(SIZE_COL_W, APP_ROW_H),
                                        Sense::hover(),
                                    );
                                    let mut sz_ui = ui.new_child(
                                        UiBuilder::new()
                                            .max_rect(sz_rect)
                                            .layout(Layout::right_to_left(Align::Center)),
                                    );
                                    draw_right_aligned_size(&mut sz_ui, app.total_bytes, 13.5, pal.text, true);

                                    let (d_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(DELTA_COL_W, APP_ROW_H),
                                        Sense::hover(),
                                    );
                                    if let Some((d_text, d_col, d_tip)) = format_delta_display(app.growth, pal) {
                                        let mut d_ui = ui.new_child(
                                            UiBuilder::new()
                                                .max_rect(d_rect)
                                                .layout(Layout::right_to_left(Align::Center)),
                                        );
                                        d_ui.add(
                                            egui::Label::new(RichText::new(&d_text).size(11.5).color(d_col))
                                                .selectable(false),
                                        )
                                        .on_hover_text(d_tip);
                                    }
                                },
                            );

                            if header_resp.clicked() {
                                if expanded {
                                    ui_state.expanded_apps.remove(&app.id);
                                } else {
                                    ui_state.expanded_apps.insert(app.id.clone());
                                }
                            }

                            if expanded {
                                ui.add_space(SPACE_4);
                                // Sub-section 1: Hairline-divided Location Breakdown (zero nested cards)
                                ui.label(
                                    RichText::new(format!("Locations ({})", app.items.len()))
                                        .size(11.0)
                                        .strong()
                                        .color(pal.muted),
                                );
                                for (loc_i, item) in app.items.iter().enumerate() {
                                    if loc_i > 0 {
                                        let r = Rect::from_min_size(
                                            ui.cursor().min,
                                            Vec2::new(ui.available_width(), 1.0),
                                        );
                                        ui.painter().rect_filled(r, Rounding::ZERO, pal.border);
                                        ui.add_space(SPACE_4);
                                    }
                                    draw_split_row(
                                        ui,
                                        SUB_LINE1_H,
                                        SIZE_COL_W,
                                        Sense::hover(),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.x = SPACE_4;
                                            if let Some(zone) = item.zone {
                                                paint_chip(
                                                    ui,
                                                    zone.short_label(),
                                                    zone_chip_style(zone, pal),
                                                    pal.panel_alt,
                                                );
                                            }
                                            if let Some(qual) = item.qualifier {
                                                let q_lbl = match qual {
                                                    LocationQualifier::RoamingConfig => "Roaming config",
                                                    LocationQualifier::MachineShared => "ProgramData · shared",
                                                };
                                                paint_chip(ui, q_lbl, qualifier_chip_style(pal), pal.panel_alt);
                                            }
                                            draw_truncated_text(
                                                ui,
                                                &item.path,
                                                FontId::new(11.5, FontFamily::Monospace),
                                                pal.text,
                                                false,
                                            );
                                        },
                                        |ui| {
                                            draw_right_aligned_size(ui, item.bytes, 11.5, pal.text, false);
                                        },
                                    );
                                    if let Some(basis) = item.basis {
                                        draw_split_row(
                                            ui,
                                            SUB_LINE2_H,
                                            SIZE_COL_W,
                                            Sense::hover(),
                                            |ui| {
                                                let basis_str = format!("Basis: {}", basis.label());
                                                draw_truncated_text(
                                                    ui,
                                                    &basis_str,
                                                    FontId::new(10.5, FontFamily::Proportional),
                                                    pal.subtle_text,
                                                    false,
                                                );
                                            },
                                            |_| {},
                                        );
                                    }
                                }

                                ui.add_space(SPACE_4);
                                let div_rect = Rect::from_min_size(
                                    ui.cursor().min,
                                    Vec2::new(ui.available_width(), 1.0),
                                );
                                ui.painter().rect_filled(div_rect, Rounding::ZERO, pal.border);
                                ui.add_space(SPACE_8);

                                // Sub-section 2: Hairline-divided Dependencies (kind as quiet text, no solid pastel fill, quiet shared sizes)
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = SPACE_8;
                                    ui.label(
                                        RichText::new("Dependencies")
                                            .size(11.5)
                                            .strong()
                                            .color(pal.muted),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{} · shared sizes excluded from app total",
                                            app.dependencies.len()
                                        ))
                                        .size(10.5)
                                        .color(pal.subtle_text),
                                    );
                                });

                                if app.dependencies.is_empty() {
                                    ui.add_sized(
                                        Vec2::new(ui.available_width(), SUB_LINE1_H),
                                        egui::Label::new(
                                            RichText::new(NO_KNOWN_DEPENDENCIES_LABEL)
                                                .size(11.5)
                                                .color(pal.subtle_text),
                                        ),
                                    );
                                } else {
                                    for (dep_i, dep) in app.dependencies.iter().enumerate() {
                                        if dep_i > 0 {
                                            let r = Rect::from_min_size(
                                                ui.cursor().min,
                                                Vec2::new(ui.available_width(), 1.0),
                                            );
                                            ui.painter().rect_filled(r, Rounding::ZERO, pal.border);
                                            ui.add_space(SPACE_4);
                                        }
                                        let rev_n = rev_counts
                                            .get(dep.target_id.as_str())
                                            .copied()
                                            .unwrap_or(0);
                                        let is_open_rev = ui_state.open_reverse_dep_id.as_deref()
                                            == Some(dep.target_id.as_str());

                                        draw_split_row(
                                            ui,
                                            SUB_LINE1_H,
                                            SIZE_COL_W,
                                            Sense::hover(),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                                draw_truncated_text(
                                                    ui,
                                                    &dep.target_name,
                                                    FontId::new(12.0, FontFamily::Proportional),
                                                    pal.text,
                                                    false,
                                                );
                                                ui.label(
                                                    RichText::new(dep.kind.label())
                                                        .size(11.0)
                                                        .color(pal.subtle_text),
                                                );
                                                paint_chip(
                                                    ui,
                                                    dep.confidence.label(),
                                                    confidence_chip_style(dep.confidence, pal),
                                                    pal.panel_alt,
                                                );
                                                if dep.shared {
                                                    paint_chip(
                                                        ui,
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
                                                        ui,
                                                        "missing",
                                                        no_touch_chip_style(pal),
                                                        pal.panel_alt,
                                                    );
                                                }
                                                if rev_n > 0 {
                                                    let lbl = if rev_n == 1 {
                                                        "used by 1 app >".to_string()
                                                    } else {
                                                        format!("used by {rev_n} apps >")
                                                    };
                                                    if draw_z_button(
                                                        ui,
                                                        &lbl,
                                                        Vec2::new(0.0, 18.0),
                                                        10.5,
                                                        RADIUS_PILL,
                                                        true,
                                                        is_open_rev,
                                                        pal,
                                                    )
                                                    .on_hover_text("Open reverse dependency list (\"What else uses this?\")")
                                                    .clicked()
                                                    {
                                                        ui_state.open_reverse_dep_id =
                                                            if is_open_rev {
                                                                None
                                                            } else {
                                                                Some(dep.target_id.clone())
                                                            };
                                                    }
                                                }
                                            },
                                            |ui| {
                                                if let Some(own) = dep.own_size_bytes {
                                                    draw_right_aligned_size(ui, own, 11.5, pal.muted, false);
                                                } else {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new("—")
                                                                .size(11.5)
                                                                .color(pal.subtle_text),
                                                        )
                                                        .selectable(false),
                                                    )
                                                    .on_hover_text("No separate on-disk folder (OS runtime or unmounted drive)");
                                                }
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
                                                    &dep.basis,
                                                    FontId::new(10.5, FontFamily::Proportional),
                                                    pal.subtle_text,
                                                    false,
                                                );
                                            },
                                            |_| {},
                                        );
                                    }
                                }
                                ui.add_space(SPACE_4);
                            }
                            header_resp.hovered()
                        });

                    let r = row_res.response.rect;
                    if expanded {
                        ui.painter().set(
                            row_slot,
                            egui::Shape::rect_filled(r, Rounding::ZERO, pal.panel_alt),
                        );
                        // 2px left accent bar + top/bottom crisp hairlines highlight the elevated inspected app.
                        let bar_rect = Rect::from_min_size(r.min, Vec2::new(2.0, r.height()));
                        ui.painter().set(
                            accent_bar_slot,
                            egui::Shape::rect_filled(bar_rect, Rounding::ZERO, pal.accent_line),
                        );
                        ui.painter().rect_filled(
                            Rect::from_min_size(r.min, Vec2::new(r.width(), 1.0)),
                            Rounding::ZERO,
                            pal.accent_line,
                        );
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(r.min.x, (r.max.y - 1.0).round()),
                                Vec2::new(r.width(), 1.0),
                            ),
                            Rounding::ZERO,
                            pal.accent_line,
                        );
                    } else if row_res.inner {
                        ui.painter().set(
                            row_slot,
                            egui::Shape::rect_filled(r, Rounding::ZERO, pal.widget_hover),
                        );
                    }

                    if !expanded && row_idx + 1 < shown {
                        let sep = Rect::from_min_size(
                            Pos2::new(r.min.x, (r.max.y - 1.0).round()),
                            Vec2::new(r.width(), 1.0),
                        );
                        ui.painter().rect_filled(sep, Rounding::ZERO, pal.border);
                    }
                }
            },
        );
    });
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
                            draw_truncated_text(
                                ui,
                                &rev.target_name,
                                FontId::new(14.0, FontFamily::Proportional),
                                pal.text,
                                true,
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
                                    text: pal.accent_line,
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
                                draw_right_aligned_size(ui, own, 12.0, pal.text, true);
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
                                                draw_truncated_text(
                                                    ui,
                                                    &c.app_name,
                                                    FontId::new(12.0, FontFamily::Proportional),
                                                    pal.text,
                                                    true,
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
) {
    ui.spacing_mut().item_spacing.y = SPACE_4;
    let aligned_w = (ui.available_width() - SCROLL_GUTTER_W).max(200.0);
    ui.label(
        RichText::new("Pinned storage ledger")
            .size(10.5)
            .strong()
            .color(pal.subtle_text),
    );

    let any_expanded = ui_state.expand_windows_system
        || ui_state.expand_shared_caches
        || ui_state.expand_your_files
        || ui_state.expand_unattributed;

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
                    LOGO_LAVENDER,
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
                    pal.accent_line,
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
                    pal.status_green,
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
                    pal.delta_grew,
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

    if any_expanded {
        draw_synced_scroll_area(
            ui,
            "pinned_groups_scroll",
            Some(152.0),
            [false, true],
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

    // Quiet exact reconciliation ledger line (green status dot + tabular diff 0 B; alert chip only if diff != 0).
    for sys in &rep.windows_system {
        let apps_d = rep.total_apps_bytes_on_drive(sys.drive_letter);
        let cache_d = rep.total_shared_caches_on_drive(sys.drive_letter);
        let yf_d = rep.total_your_files_on_drive(sys.drive_letter);
        let un_d = rep.total_unattributed_on_drive(sys.drive_letter);
        let sum_all = apps_d + cache_d + yf_d + un_d + sys.total_bytes + sys.drive_free_bytes;
        let diff = sum_all as i128 - sys.drive_total_bytes as i128;

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
                        let resp = draw_truncated_text(
                            ui,
                            &summary,
                            FontId::new(10.5, FontFamily::Proportional),
                            pal.subtle_text,
                            false,
                        );
                        let _ = resp.on_hover_text(exact);
                    },
                    |ui| {
                        if diff == 0 {
                            ui.add_space(SPACE_12);
                            ui.add(
                                egui::Label::new(
                                    RichText::new("diff 0 B")
                                        .size(11.0)
                                        .color(pal.muted),
                                )
                                .selectable(false),
                            );
                            let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter().circle_filled(dot_r.center(), 3.0, pal.status_green);
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
                    draw_right_aligned_size(ui, total_bytes, 12.5, pal.text, true);
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
                    .strong()
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
                        let resp = draw_reverse_group_header(ui, dep, expanded, pal);
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
                                        draw_truncated_text(
                                            ui,
                                            &c.app_name,
                                            FontId::new(12.0, FontFamily::Proportional),
                                            pal.text,
                                            false,
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
            draw_truncated_text(
                ui,
                &dep.target_name,
                FontId::new(13.0, FontFamily::Proportional),
                pal.text,
                true,
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
                        text: pal.accent_line,
                    },
                    pal.panel,
                );
            }
        },
        |ui| {
            ui.spacing_mut().item_spacing.x = SPACE_8;
            ui.label(
                RichText::new(format!("{} apps", dep.dependent_count))
                    .size(12.5)
                    .strong()
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
            .strong()
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
                                        .strong()
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
}
