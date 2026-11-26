use std::collections::{HashMap, HashSet};

use egui::{
    Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Key, Layout, Margin,
    Order, Pos2, Rect, RichText, Rounding, Sense, Stroke, TextStyle, UiBuilder, Vec2,
};
use y_core::config::{AppConfig, ThemeMode};
use y_core::discovery::normalize_path_key;
use y_core::model::{
    DependencyConfidence, DependencyKind, GrowthDelta, LocationQualifier,
    NO_KNOWN_DEPENDENCIES_LABEL, ReverseDependencyEntry, ScanReport, ZoneKind,
};
use y_platform_windows::scanner::ScanProgressSnapshot;

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

const SIZE_COL_W: f32 = 96.0;
const DELTA_COL_W: f32 = 88.0;
const HEADER_CONTROL_H: f32 = 28.0;
const APP_ROW_H: f32 = 24.0;
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
    pub widget: Color32,
    pub widget_hover: Color32,
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
    pub proven_solid_fill: Color32,
    pub proven_solid_text: Color32,
}

impl ThemePalette {
    #[must_use]
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                bg: Color32::from_rgb(10, 10, 16),        // #0A0A10
                panel: Color32::from_rgb(20, 20, 31),     // #14141F
                panel_alt: Color32::from_rgb(15, 15, 24), // #0F0F18
                border: Color32::from_rgb(38, 38, 58),    // #26263A
                text: Color32::from_rgb(237, 237, 242),   // #EDEDF2
                muted: Color32::from_rgb(154, 154, 176),  // #9A9AB0
                widget: Color32::from_rgb(24, 24, 38),
                widget_hover: Color32::from_rgb(33, 33, 52),
                accent_line: LOGO_BLUE,
                lavender_text: LOGO_LAVENDER,
                status_green: Color32::from_rgb(40, 214, 108),  // #28D66C
                status_red: Color32::from_rgb(240, 68, 88),     // #F04458
                delta_grew: LOGO_PINK,
                no_touch_outline: Color32::from_rgb(206, 92, 108),
                pill_blue_fill: Color32::from_rgb(18, 30, 44),
                pill_green_fill: Color32::from_rgb(16, 38, 26),
                pill_lav_fill: Color32::from_rgb(32, 24, 46),
                pill_red_fill: Color32::from_rgb(46, 20, 26),
                proven_solid_fill: LOGO_BLUE,
                proven_solid_text: Color32::from_rgb(10, 10, 16),
            },
            ThemeMode::Light => Self {
                bg: Color32::from_rgb(238, 244, 250),     // #EEF4FA
                panel: Color32::from_rgb(255, 255, 255),  // #FFFFFF
                panel_alt: Color32::from_rgb(244, 248, 253),
                border: Color32::from_rgb(212, 224, 238), // #D4E0EE
                text: Color32::from_rgb(28, 39, 51),      // #1C2733
                muted: Color32::from_rgb(95, 113, 134),   // #5F7186
                widget: Color32::from_rgb(244, 248, 252),
                widget_hover: Color32::from_rgb(231, 241, 250),
                accent_line: LOGO_BLUE_READABLE,
                lavender_text: Color32::from_rgb(120, 62, 184),
                status_green: Color32::from_rgb(20, 138, 68),
                status_red: Color32::from_rgb(206, 40, 64),
                delta_grew: Color32::from_rgb(196, 48, 88),
                no_touch_outline: Color32::from_rgb(188, 64, 80),
                pill_blue_fill: Color32::from_rgb(232, 245, 255),
                pill_green_fill: Color32::from_rgb(230, 248, 237),
                pill_lav_fill: Color32::from_rgb(243, 236, 255),
                pill_red_fill: Color32::from_rgb(253, 234, 237),
                proven_solid_fill: LOGO_BLUE_READABLE,
                proven_solid_text: Color32::WHITE,
            },
        }
    }
}

pub struct StorageUiState {
    pub active_view: ActiveView,
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

fn format_delta_display(delta: GrowthDelta, pal: &ThemePalette) -> (String, Color32, String) {
    match delta {
        GrowthDelta::NoBaseline => (
            "—".to_string(),
            pal.muted,
            "First recorded scan (no prior baseline)".to_string(),
        ),
        GrowthDelta::New(0) => (
            "NEW".to_string(),
            pal.delta_grew,
            "Newly detected since previous scan".to_string(),
        ),
        GrowthDelta::New(b) => (
            format!("+{}", format_size_2dec(b)),
            pal.delta_grew,
            format!("New since previous scan (+{})", format_exact_bytes(b)),
        ),
        GrowthDelta::Changed(0) => (
            "0 B".to_string(),
            pal.muted,
            "0 bytes change since previous scan".to_string(),
        ),
        GrowthDelta::Changed(d) if d > 0 => (
            format!("+{}", format_size_2dec(d as u64)),
            pal.delta_grew,
            format!("+{} since previous scan", format_exact_bytes(d as u64)),
        ),
        GrowthDelta::Changed(d) => (
            format!("-{}", format_size_2dec(d.unsigned_abs())),
            pal.status_green,
            format!(
                "-{} since previous scan",
                format_exact_bytes(d.unsigned_abs())
            ),
        ),
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
    visuals.override_text_color = Some(pal.text);
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

    visuals.widgets.active.bg_fill = pal.pill_blue_fill;
    visuals.widgets.active.weak_bg_fill = pal.pill_blue_fill;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, LOGO_PINK);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.text);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    style.visuals = visuals;
    style.animation_time = 0.0;
    style.spacing.item_spacing = Vec2::new(SPACE_8, SPACE_4);
    style.spacing.button_padding = Vec2::new(SPACE_8, SPACE_4);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_width = 8.0;
    ctx.set_style(style);
}

fn paint_chip(ui: &mut egui::Ui, label: &str, style: ChipStyle) -> egui::Response {
    egui::Frame::none()
        .fill(style.fill)
        .stroke(Stroke::new(1.0_f32, style.stroke))
        .rounding(Rounding::same(4.0))
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .size(10.5)
                    .color(style.text)
                    .strong(),
            );
        })
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
            fill: pal.pill_blue_fill,
            stroke: pal.accent_line,
            text: pal.accent_line,
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
            fill: pal.proven_solid_fill,
            stroke: pal.proven_solid_fill,
            text: pal.proven_solid_text,
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
    left_ui.set_clip_rect(left_rect.intersect(ui.clip_rect()));
    left_fn(&mut left_ui);

    let mut right_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(right_rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right_ui.set_clip_rect(right_rect.intersect(ui.clip_rect()));
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
    let bg = if resp.hovered() {
        pal.widget_hover
    } else {
        pal.widget
    };
    let border = if resp.hovered() {
        pal.accent_line
    } else {
        pal.border
    };
    ui.painter()
        .rect(rect, Rounding::same(6.0), bg, Stroke::new(1.0_f32, border));
    let c = rect.center();
    match mode {
        ThemeMode::Dark => {
            ui.painter()
                .circle_stroke(c, 4.2, Stroke::new(1.4_f32, LOGO_BLUE));
            for i in 0..8 {
                let a = (i as f32) * std::f32::consts::FRAC_PI_4;
                let p0 = Pos2::new(c.x + a.cos() * 6.5, c.y + a.sin() * 6.5);
                let p1 = Pos2::new(c.x + a.cos() * 8.5, c.y + a.sin() * 8.5);
                ui.painter()
                    .line_segment([p0, p1], Stroke::new(1.3_f32, LOGO_BLUE));
            }
        }
        ThemeMode::Light => {
            ui.painter().circle_filled(c, 6.4, pal.lavender_text);
            ui.painter()
                .circle_filled(Pos2::new(c.x + 3.0, c.y - 2.0), 5.0, bg);
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
            // Row 1: Title, status, [Theme Toggle Icon][Rescan][Settings]
            draw_split_row(
                ui,
                HEADER_CONTROL_H,
                220.0,
                Sense::hover(),
                |ui| {
                    ui.label(
                        RichText::new("y-storage")
                            .size(17.0)
                            .strong()
                            .color(pal.text),
                    );
                    paint_chip(
                        ui,
                        "READ-ONLY",
                        ChipStyle {
                            fill: pal.pill_blue_fill,
                            stroke: pal.accent_line,
                            text: pal.accent_line,
                        },
                    );
                    if scanning {
                        let status = if let Some(p) = progress {
                            format!(
                                "{} · {} dirs · {}",
                                p.phase,
                                p.dirs_walked,
                                format_size_2dec(p.bytes_counted)
                            )
                        } else {
                            "Scanning...".to_string()
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
                            "Last scan: {} · {:.2}s",
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
                            "No cached scan — click Rescan",
                            FontId::new(11.5, FontFamily::Proportional),
                            pal.muted,
                            false,
                        );
                    }
                },
                |ui| {
                    // Right-to-left placement -> rightmost Settings, then Rescan, then Theme Icon immediately left of Rescan.
                    let settings_lbl = match ui_state.active_view {
                        ActiveView::Settings | ActiveView::Dependencies => "Back to Apps",
                        ActiveView::Main => "Settings",
                    };
                    if ui
                        .add_sized(
                            Vec2::new(88.0, HEADER_CONTROL_H),
                            egui::Button::new(settings_lbl),
                        )
                        .clicked()
                    {
                        ui_state.open_reverse_dep_id = None;
                        ui_state.active_view = match ui_state.active_view {
                            ActiveView::Main => ActiveView::Settings,
                            ActiveView::Settings | ActiveView::Dependencies => ActiveView::Main,
                        };
                    }

                    let rescan_btn = egui::Button::new(
                        RichText::new(if scanning { "Scanning..." } else { "Rescan" })
                            .color(if scanning { pal.muted } else { pal.accent_line })
                            .strong(),
                    );
                    if ui
                        .add_enabled_ui(!scanning, |ui| {
                            ui.add_sized(Vec2::new(80.0, HEADER_CONTROL_H), rescan_btn)
                        })
                        .inner
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

            // Row 2: Drive strip — slim proportional bar, used & free figures, free space most prominent
            if let Some(rep) = report {
                if !rep.drives.is_empty() {
                    ui.add_space(SPACE_8);
                    let n = rep.drives.len().max(1);
                    let avail = ui.available_width();
                    let tile_w = ((avail - SPACE_8 * ((n - 1) as f32)) / (n as f32)).max(132.0);
                    ui.horizontal(|ui| {
                        for (idx, drv) in rep.drives.iter().enumerate() {
                            if idx > 0 {
                                ui.add_space(SPACE_4);
                            }
                            let frac = if drv.total_bytes > 0 {
                                (drv.used_bytes as f32 / drv.total_bytes as f32).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            egui::Frame::none()
                                .fill(pal.panel)
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::symmetric(SPACE_8, SPACE_4))
                                .show(ui, |ui| {
                                    let inner_w = (tile_w - SPACE_16).max(112.0);
                                    ui.set_width(inner_w);
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = SPACE_4;
                                        // Line 1: Drive letter + prominent free space
                                        draw_split_row(
                                            ui,
                                            18.0,
                                            (inner_w * 0.72).max(76.0),
                                            Sense::hover(),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(format!("{}:\\", drv.letter))
                                                        .size(12.5)
                                                        .strong()
                                                        .color(pal.text),
                                                );
                                            },
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(format!(
                                                            "{} free",
                                                            format_size_2dec(drv.free_bytes)
                                                        ))
                                                        .size(12.5)
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

                                        // Line 2: slim proportional bar + used figure
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = SPACE_8;
                                            let bar_w = (inner_w - 88.0).clamp(36.0, 76.0);
                                            let (bar_rect, bar_resp) = ui.allocate_exact_size(
                                                Vec2::new(bar_w, 4.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                bar_rect,
                                                Rounding::same(2.0),
                                                pal.panel_alt,
                                            );
                                            let fill_w = (bar_rect.width() * frac).max(2.0);
                                            let fill_rect = Rect::from_min_size(
                                                bar_rect.min,
                                                Vec2::new(fill_w, bar_rect.height()),
                                            );
                                            let bar_col = if drv.full_walk_enabled {
                                                pal.accent_line
                                            } else {
                                                LOGO_LAVENDER
                                            };
                                            ui.painter().rect_filled(
                                                fill_rect,
                                                Rounding::same(2.0),
                                                bar_col,
                                            );
                                            let _ = bar_resp.on_hover_text(format!(
                                                "{:.1}% used ({})",
                                                frac * 100.0,
                                                if drv.full_walk_enabled {
                                                    "Full walk"
                                                } else {
                                                    "Install paths only"
                                                }
                                            ));

                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(format!(
                                                        "{} used",
                                                        format_size_2dec(drv.used_bytes)
                                                    ))
                                                    .size(10.5)
                                                    .color(pal.muted),
                                                )
                                                .selectable(false),
                                            )
                                            .on_hover_text(format!(
                                                "Used: {}",
                                                format_exact_bytes(drv.used_bytes)
                                            ));
                                        });
                                    });
                                });
                        }
                    });
                }
            }

            if ui_state.active_view == ActiveView::Main {
                ui.add_space(SPACE_8);
                draw_split_row(
                    ui,
                    HEADER_CONTROL_H,
                    220.0,
                    Sense::hover(),
                    |ui| {
                        ui.label(RichText::new("Filter:").size(12.0).color(pal.muted));
                        ui.add(
                            egui::TextEdit::singleline(&mut ui_state.search_query)
                                .desired_width(260.0)
                                .hint_text("Search app name, publisher, or path..."),
                        );
                        if !ui_state.search_query.is_empty() && ui.button("Clear").clicked() {
                            ui_state.search_query.clear();
                        }
                    },
                    |ui| {
                        if let Some(rep) = report {
                            let apps_sum: u64 = rep.apps.iter().map(|a| a.total_bytes).sum();
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "{} apps · {} attributed",
                                        rep.apps.len(),
                                        format_size_2dec(apps_sum)
                                    ))
                                    .size(11.5)
                                    .color(pal.muted),
                                )
                                .selectable(false),
                            )
                            .on_hover_text(format!(
                                "Total attributed app bytes: {}",
                                format_exact_bytes(apps_sum)
                            ));
                        }
                    },
                );
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
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(SPACE_16, SPACE_8)),
                    )
                    .show(ctx, |ui| {
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
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("Click Rescan to inventory installed apps and drive usage.")
                    .size(13.5)
                    .color(pal.muted),
            );
        });
        return;
    };

    let rev_counts: HashMap<&str, usize> = rep
        .reverse_dependencies
        .iter()
        .map(|r| (r.target_id.as_str(), r.dependent_count))
        .collect();

    let q = ui_state.search_query.trim().to_ascii_lowercase();
    egui::ScrollArea::vertical()
        .id_salt("main_app_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = SPACE_4;
            let mut shown = 0usize;

            for app in &rep.apps {
                if !q.is_empty() {
                    let name_hit = app.display_name.to_ascii_lowercase().contains(&q);
                    let pub_hit = app
                        .publishers
                        .iter()
                        .any(|p| p.to_ascii_lowercase().contains(&q));
                    let path_hit = app
                        .items
                        .iter()
                        .any(|it| it.path.to_ascii_lowercase().contains(&q));
                    if !name_hit && !pub_hit && !path_hit {
                        continue;
                    }
                }
                shown += 1;
                let expanded = ui_state.expanded_apps.contains(&app.id);
                let frame_stroke = if expanded {
                    Stroke::new(1.0_f32, pal.accent_line)
                } else {
                    Stroke::new(1.0_f32, pal.border)
                };

                egui::Frame::none()
                    .fill(pal.panel)
                    .stroke(frame_stroke)
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(SPACE_8, SPACE_4))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = SPACE_4;

                        // Fixed-height content-hugging collapsed header row
                        let right_col_w = DELTA_COL_W + SPACE_8 + SIZE_COL_W;
                        let header_resp = draw_split_row(
                            ui,
                            APP_ROW_H,
                            right_col_w,
                            Sense::click(),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                draw_chevron(ui, expanded, pal.accent_line);
                                draw_truncated_text(
                                    ui,
                                    &app.display_name,
                                    FontId::new(13.0, FontFamily::Proportional),
                                    pal.text,
                                    true,
                                );
                                if let Some(nt) = app.no_touch {
                                    paint_chip(ui, nt.label(), no_touch_chip_style(pal));
                                }
                                if let Some(first_pub) = app.publishers.first() {
                                    draw_truncated_text(
                                        ui,
                                        first_pub,
                                        FontId::new(11.5, FontFamily::Proportional),
                                        pal.muted,
                                        false,
                                    );
                                }
                            },
                            |ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                // Size column (rightmost)
                                let (sz_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(SIZE_COL_W, APP_ROW_H),
                                    Sense::hover(),
                                );
                                let mut sz_ui = ui.new_child(
                                    UiBuilder::new()
                                        .max_rect(sz_rect)
                                        .layout(Layout::right_to_left(Align::Center)),
                                );
                                draw_right_aligned_size(&mut sz_ui, app.total_bytes, 13.0, pal.text, true);

                                // Growth delta column (left of size column)
                                let (d_text, d_col, d_tip) = format_delta_display(app.growth, pal);
                                let (d_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(DELTA_COL_W, APP_ROW_H),
                                    Sense::hover(),
                                );
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
                            // Sub-section 1: Location Breakdown
                            ui.label(
                                RichText::new(format!("Locations ({})", app.items.len()))
                                    .size(11.0)
                                    .strong()
                                    .color(pal.muted),
                            );
                            for item in &app.items {
                                egui::Frame::none()
                                    .fill(pal.panel_alt)
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(Margin {
                                        left: SPACE_8,
                                        right: 0.0,
                                        top: SPACE_4,
                                        bottom: SPACE_4,
                                    })
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
                                                if let Some(zone) = item.zone {
                                                    paint_chip(
                                                        ui,
                                                        zone.short_label(),
                                                        zone_chip_style(zone, pal),
                                                    );
                                                }
                                                if let Some(qual) = item.qualifier {
                                                    let q_lbl = match qual {
                                                        LocationQualifier::RoamingConfig => {
                                                            "Roaming config"
                                                        }
                                                        LocationQualifier::MachineShared => {
                                                            "ProgramData · shared"
                                                        }
                                                    };
                                                    paint_chip(ui, q_lbl, qualifier_chip_style(pal));
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
                                                draw_right_aligned_size(
                                                    ui,
                                                    item.bytes,
                                                    12.0,
                                                    pal.text,
                                                    true,
                                                );
                                            },
                                        );

                                        if let Some(basis) = item.basis {
                                            draw_split_row(
                                                ui,
                                                SUB_LINE2_H,
                                                SIZE_COL_W,
                                                Sense::hover(),
                                                |ui| {
                                                    let basis_str =
                                                        format!("Basis: {}", basis.label());
                                                    draw_truncated_text(
                                                        ui,
                                                        &basis_str,
                                                        FontId::new(
                                                            10.5,
                                                            FontFamily::Proportional,
                                                        ),
                                                        pal.muted,
                                                        false,
                                                    );
                                                },
                                                |_| {},
                                            );
                                        }
                                    });
                            }

                            ui.add_space(SPACE_4);
                            let line_rect = Rect::from_min_size(
                                ui.cursor().min,
                                Vec2::new(ui.available_width(), 1.0),
                            );
                            ui.painter()
                                .line_segment([line_rect.left_top(), line_rect.right_top()], Stroke::new(1.0_f32, pal.border));
                            ui.add_space(SPACE_4);

                            // Sub-section 2: Clearly titled Dependencies section
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                ui.label(
                                    RichText::new("Dependencies")
                                        .size(12.0)
                                        .strong()
                                        .color(pal.text),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} · shared sizes excluded from app total",
                                        app.dependencies.len()
                                    ))
                                    .size(10.5)
                                    .color(pal.muted),
                                );
                            });

                            if app.dependencies.is_empty() {
                                ui.add_sized(
                                    Vec2::new(ui.available_width(), SUB_LINE1_H),
                                    egui::Label::new(
                                        RichText::new(NO_KNOWN_DEPENDENCIES_LABEL)
                                            .size(11.5)
                                            .color(pal.muted),
                                    ),
                                );
                            } else {
                                for dep in &app.dependencies {
                                    let rev_n =
                                        rev_counts.get(dep.target_id.as_str()).copied().unwrap_or(0);
                                    let is_open_rev =
                                        ui_state.open_reverse_dep_id.as_deref()
                                            == Some(dep.target_id.as_str());

                                    egui::Frame::none()
                                        .fill(pal.panel_alt)
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(Margin {
                                            left: SPACE_8,
                                            right: 0.0,
                                            top: SPACE_4,
                                            bottom: SPACE_4,
                                        })
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
                                                    draw_truncated_text(
                                                        ui,
                                                        &dep.target_name,
                                                        FontId::new(12.0, FontFamily::Proportional),
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
                                                    );
                                                    paint_chip(
                                                        ui,
                                                        dep.confidence.label(),
                                                        confidence_chip_style(dep.confidence, pal),
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
                                                        );
                                                    }
                                                    if !dep.present {
                                                        paint_chip(
                                                            ui,
                                                            "missing",
                                                            no_touch_chip_style(pal),
                                                        );
                                                    }
                                                    if rev_n > 0 {
                                                        let lbl = if rev_n == 1 {
                                                            "used by 1 app >".to_string()
                                                        } else {
                                                            format!("used by {rev_n} apps >")
                                                        };
                                                        let btn = egui::Button::new(
                                                            RichText::new(&lbl)
                                                                .size(10.5)
                                                                .color(if is_open_rev {
                                                                    pal.proven_solid_text
                                                                } else {
                                                                    pal.accent_line
                                                                })
                                                                .strong(),
                                                        )
                                                        .fill(if is_open_rev {
                                                            pal.accent_line
                                                        } else {
                                                            pal.pill_blue_fill
                                                        })
                                                        .stroke(Stroke::new(1.0_f32, pal.accent_line))
                                                        .rounding(Rounding::same(4.0))
                                                        .min_size(Vec2::new(0.0, 18.0));
                                                        if ui
                                                            .add(btn)
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
                                                        draw_right_aligned_size(
                                                            ui,
                                                            own,
                                                            12.0,
                                                            pal.text,
                                                            true,
                                                        );
                                                    } else {
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new("—")
                                                                    .size(11.5)
                                                                    .color(pal.muted),
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
                                                        FontId::new(
                                                            10.5,
                                                            FontFamily::Proportional,
                                                        ),
                                                        pal.muted,
                                                        false,
                                                    );
                                                },
                                                |_| {},
                                            );
                                        });
                                }
                            }
                        }
                    });
            }

            if shown == 0 {
                ui.add_space(SPACE_16);
                ui.label(
                    RichText::new("No matching apps found for the current filter.")
                        .color(pal.muted),
                );
            }
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
    let anchor_offset = Vec2::new(-SPACE_16, 144.0);

    egui::Area::new(egui::Id::new("reverse_dep_overlay"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_TOP, anchor_offset)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(pal.panel)
                .stroke(Stroke::new(1.5_f32, pal.accent_line))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(SPACE_12))
                .show(ui, |ui| {
                    ui.set_width(area_w - SPACE_16 - SPACE_8);
                    ui.spacing_mut().item_spacing.y = SPACE_4;

                    let mut close_clicked = false;
                    draw_split_row(
                        ui,
                        24.0,
                        132.0,
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
                            if ui.button("Close").clicked() {
                                close_clicked = true;
                            }
                            if ui.button("All").on_hover_text("Open full Reverse Dependencies view").clicked() {
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
                    egui::ScrollArea::vertical()
                        .id_salt(("rev_overlay_scroll", target_id))
                        .max_height(260.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
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
                        });
                });
        });
}

fn draw_pinned_bottom_groups(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    pal: &ThemePalette,
) {
    ui.spacing_mut().item_spacing.y = SPACE_4;
    ui.label(
        RichText::new("PINNED STORAGE GROUPS")
            .size(10.0)
            .strong()
            .color(pal.muted),
    );

    let any_expanded = ui_state.expand_windows_system
        || ui_state.expand_shared_caches
        || ui_state.expand_your_files
        || ui_state.expand_unattributed;

    let mut draw_cards = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.y = SPACE_4;

        // 1. Windows & system
        draw_pinned_group_card(
            ui,
            "Windows & system",
            rep.windows_system_total_bytes,
            "ESTIMATE · system remainder + pagefile/hiberfil",
            LOGO_LAVENDER,
            &mut ui_state.expand_windows_system,
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

        // 2. Your files
        draw_pinned_group_card(
            ui,
            "Your files",
            rep.your_files_total_bytes,
            "Personal shell folders · unzoned · read-only",
            pal.accent_line,
            &mut ui_state.expand_your_files,
            pal,
            |ui| {
                for item in &rep.your_files {
                    draw_simple_item_row(ui, &item.path, item.bytes, None, pal);
                }
            },
        );

        // 3. Shared & system caches
        draw_pinned_group_card(
            ui,
            "Shared & system caches",
            rep.shared_caches_total_bytes,
            &format!("{} regenerable Temp / DXCache / Package Cache folders", rep.shared_caches.len()),
            pal.status_green,
            &mut ui_state.expand_shared_caches,
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

        // 4. Unattributed
        draw_pinned_group_card(
            ui,
            "Unattributed",
            rep.unattributed_total_bytes,
            &format!("{} folders with no verified app owner", rep.unattributed.len()),
            pal.delta_grew,
            &mut ui_state.expand_unattributed,
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
                        .color(pal.muted),
                    );
                }
            },
        );
    };

    if any_expanded {
        egui::ScrollArea::vertical()
            .id_salt("pinned_groups_scroll")
            .max_height(152.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                draw_cards(ui);
            });
    } else {
        draw_cards(ui);
    }

    // Reconciliation line per full-walk enabled drive
    for sys in &rep.windows_system {
        let apps_d = rep.total_apps_bytes_on_drive(sys.drive_letter);
        let cache_d = rep.total_shared_caches_on_drive(sys.drive_letter);
        let yf_d = rep.total_your_files_on_drive(sys.drive_letter);
        let un_d = rep.total_unattributed_on_drive(sys.drive_letter);
        let sum_all = apps_d + cache_d + yf_d + un_d + sys.total_bytes + sys.drive_free_bytes;
        let diff = sum_all as i128 - sys.drive_total_bytes as i128;

        draw_split_row(
            ui,
            20.0,
            84.0,
            Sense::hover(),
            |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_4;
                let summary = format!(
                    "{}:\\ reconciliation: Apps {} + Caches {} + Your files {} + Unattributed {} + Win/sys {} + Free {} = {}",
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
                    FontId::new(11.0, FontFamily::Proportional),
                    pal.muted,
                    false,
                );
                let _ = resp.on_hover_text(exact);
            },
            |ui| {
                let chip_col = if diff == 0 { pal.status_green } else { pal.status_red };
                let chip_fill = if diff == 0 { pal.pill_green_fill } else { pal.pill_red_fill };
                paint_chip(
                    ui,
                    &format!("diff {diff} B"),
                    ChipStyle {
                        fill: chip_fill,
                        stroke: chip_col,
                        text: chip_col,
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
    pal: &ThemePalette,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0_f32, pal.border))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(SPACE_8, SPACE_4))
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
                    draw_chevron(ui, *expanded, accent);
                    ui.label(RichText::new(title).size(12.5).strong().color(pal.text));
                    draw_truncated_text(
                        ui,
                        subtitle,
                        FontId::new(11.0, FontFamily::Proportional),
                        pal.muted,
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
        });
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
                paint_chip(ui, lbl, style);
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
        260.0,
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
            ui.add(
                egui::TextEdit::singleline(&mut ui_state.dep_search_query)
                    .desired_width(250.0)
                    .hint_text("Filter dependency or app..."),
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
    egui::ScrollArea::vertical()
        .id_salt("reverse_deps_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
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
                egui::Frame::none()
                    .fill(pal.panel)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if expanded { pal.accent_line } else { pal.border },
                    ))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(SPACE_12, SPACE_8))
                    .show(ui, |ui| {
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
                    });
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

    egui::ScrollArea::vertical()
        .id_salt("settings_scroll")
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = SPACE_8;
            for target in &mut draft.targets {
                let drv_info = report
                    .and_then(|r| r.drives.iter().find(|d| d.letter == target.letter));
                let is_sys = drv_info.is_some_and(|d| d.is_system_drive)
                    || target.letter.eq_ignore_ascii_case(&'C');

                egui::Frame::none()
                    .fill(pal.panel)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(SPACE_12, SPACE_8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = SPACE_4;

                        draw_split_row(
                            ui,
                            24.0,
                            180.0,
                            Sense::hover(),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = SPACE_8;
                                ui.label(
                                    RichText::new(format!("Drive {}:\\", target.letter))
                                        .size(13.5)
                                        .strong()
                                        .color(pal.text),
                                );
                                if is_sys {
                                    paint_chip(
                                        ui,
                                        "SYSTEM DRIVE",
                                        ChipStyle {
                                            fill: pal.pill_blue_fill,
                                            stroke: pal.accent_line,
                                            text: pal.accent_line,
                                        },
                                    );
                                }
                                if let Some(di) = drv_info {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} free / {} used / {} total",
                                            format_size_2dec(di.free_bytes),
                                            format_size_2dec(di.used_bytes),
                                            format_size_2dec(di.total_bytes),
                                        ))
                                        .size(11.5)
                                        .color(pal.muted),
                                    )
                                    .on_hover_text(format!(
                                        "Free: {} · Used: {} · Total: {}",
                                        format_exact_bytes(di.free_bytes),
                                        format_exact_bytes(di.used_bytes),
                                        format_exact_bytes(di.total_bytes),
                                    ));
                                }
                            },
                            |ui| {
                                if ui
                                    .checkbox(&mut target.enabled, "Enable full drive walk")
                                    .changed()
                                {
                                    changed = true;
                                }
                            },
                        );

                        ui.label(RichText::new("Exclusions:").size(11.5).color(pal.muted));

                        let mut remove_idx: Option<usize> = None;
                        for (idx, ex) in target.exclusions.iter().enumerate() {
                            let is_locked_win = is_sys && normalize_path_key(ex) == sys_win_key;
                            ui.horizontal(|ui| {
                                ui.add_space(SPACE_8);
                                draw_truncated_text(
                                    ui,
                                    ex,
                                    FontId::new(11.5, FontFamily::Monospace),
                                    pal.text,
                                    false,
                                );
                                if is_locked_win {
                                    paint_chip(ui, "LOCKED · Constitutional", qualifier_chip_style(pal));
                                } else if ui.small_button("Remove").clicked() {
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
                            ui.add(
                                egui::TextEdit::singleline(input)
                                    .desired_width(280.0)
                                    .hint_text(format!("{}:\\FolderToExclude", target.letter)),
                            );
                            if ui.button("Add exclusion").clicked() {
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
                    });
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
