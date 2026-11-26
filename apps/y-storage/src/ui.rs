use std::collections::HashSet;

use egui::{
    Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Layout, Margin, Pos2,
    Rect, RichText, Rounding, Stroke, TextStyle, Vec2,
};
use y_core::config::{AppConfig, ThemeMode};
use y_core::discovery::normalize_path_key;
use y_core::model::{
    AttributionBasis, DependencyConfidence, DependencyKind, GrowthDelta, LocationQualifier,
    NO_KNOWN_DEPENDENCIES_LABEL, ScanReport, ZoneKind, format_bytes, format_growth_delta,
};
use y_platform_windows::scanner::ScanProgressSnapshot;

pub const LOGO_BLUE: Color32 = Color32::from_rgb(84, 185, 242); // #54B9F2
pub const LOGO_BLUE_READABLE: Color32 = Color32::from_rgb(47, 155, 232); // #2F9BE8
pub const LOGO_PINK: Color32 = Color32::from_rgb(247, 168, 216); // #F7A8D8
pub const LOGO_LAVENDER: Color32 = Color32::from_rgb(216, 166, 255); // #D8A6FF
pub const STATUS_GREEN: Color32 = Color32::from_rgb(40, 214, 108); // #28D66C
pub const STATUS_RED: Color32 = Color32::from_rgb(240, 68, 88); // #F04458

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
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
    pub pill_blue_fill: Color32,
    pub pill_pink_fill: Color32,
    pub pill_green_fill: Color32,
    pub pill_lav_fill: Color32,
    pub pill_red_fill: Color32,
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
                pill_blue_fill: Color32::from_rgb(18, 30, 44),
                pill_pink_fill: Color32::from_rgb(42, 22, 36),
                pill_green_fill: Color32::from_rgb(16, 38, 26),
                pill_lav_fill: Color32::from_rgb(32, 24, 46),
                pill_red_fill: Color32::from_rgb(46, 20, 26),
            },
            ThemeMode::Light => Self {
                bg: Color32::from_rgb(238, 244, 250),     // #EEF4FA
                panel: Color32::from_rgb(255, 255, 255),  // #FFFFFF
                panel_alt: Color32::from_rgb(245, 249, 253),
                border: Color32::from_rgb(212, 224, 238), // #D4E0EE
                text: Color32::from_rgb(28, 39, 51),      // #1C2733
                muted: Color32::from_rgb(95, 113, 134),   // #5F7186
                widget: Color32::from_rgb(244, 248, 252),
                widget_hover: Color32::from_rgb(231, 241, 250),
                accent_line: LOGO_BLUE_READABLE,
                pill_blue_fill: Color32::from_rgb(232, 245, 255),
                pill_pink_fill: Color32::from_rgb(253, 240, 249),
                pill_green_fill: Color32::from_rgb(231, 249, 238),
                pill_lav_fill: Color32::from_rgb(244, 237, 255),
                pill_red_fill: Color32::from_rgb(254, 234, 237),
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
    pub expand_shared_caches: bool,
    pub expand_windows_system: bool,
    pub expand_your_files: bool,
    pub expand_unattributed: bool,
    pub new_exclusion_inputs: std::collections::HashMap<char, String>,
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
            expand_shared_caches: false,
            expand_windows_system: false,
            expand_your_files: false,
            expand_unattributed: false,
            new_exclusion_inputs: std::collections::HashMap::new(),
            applied_theme: None,
        }
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
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
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
    style.spacing.item_spacing = Vec2::new(SPACE_SM, SPACE_SM);
    style.spacing.button_padding = Vec2::new(10.0, 4.0);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_width = 8.0;
    ctx.set_style(style);
}

fn paint_pill(ui: &mut egui::Ui, label: &str, fill: Color32, stroke: Color32, text_col: Color32) {
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, stroke))
        .rounding(Rounding::same(4.0))
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .size(11.0)
                    .color(text_col)
                    .strong(),
            );
        });
}

fn draw_chevron(ui: &mut egui::Ui, expanded: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::click());
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
    ui.painter().rect(rect, Rounding::same(6.0), bg, Stroke::new(1.0_f32, border));
    let c = rect.center();
    match mode {
        ThemeMode::Dark => {
            ui.painter()
                .circle_stroke(c, 4.2, Stroke::new(1.4_f32, LOGO_BLUE));
            for i in 0..8 {
                let a = (i as f32) * std::f32::consts::FRAC_PI_4;
                let p0 = Pos2::new(c.x + a.cos() * 6.4, c.y + a.sin() * 6.4);
                let p1 = Pos2::new(c.x + a.cos() * 8.4, c.y + a.sin() * 8.4);
                ui.painter()
                    .line_segment([p0, p1], Stroke::new(1.3_f32, LOGO_BLUE));
            }
        }
        ThemeMode::Light => {
            ui.painter()
                .circle_filled(c, 6.2, LOGO_LAVENDER);
            ui.painter()
                .circle_filled(Pos2::new(c.x + 2.8, c.y - 2.0), 5.0, bg);
        }
    }
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

    // Top Header + Drive Strip + Filter Bar
    egui::TopBottomPanel::top("header_panel")
        .frame(
            egui::Frame::none()
                .fill(pal.bg)
                .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
        )
        .show(ctx, |ui| {
            // Row 1: Title, status, Rescan, Settings, Theme toggle
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("y-storage")
                        .size(18.0)
                        .strong()
                        .color(pal.text),
                );
                paint_pill(
                    ui,
                    "READ-ONLY v1",
                    pal.pill_blue_fill,
                    pal.accent_line,
                    pal.accent_line,
                );
                if scanning {
                    if let Some(p) = progress {
                        ui.label(
                            RichText::new(format!(
                                "{} · {} dirs · {}",
                                p.phase,
                                p.dirs_walked,
                                format_bytes(p.bytes_counted)
                            ))
                            .size(12.0)
                            .color(LOGO_PINK),
                        );
                    } else {
                        ui.label(RichText::new("Scanning...").size(12.0).color(LOGO_PINK));
                    }
                } else if let Some(rep) = report {
                    ui.label(
                        RichText::new(format!(
                            "Last scan: {} ({:.2}s)",
                            rep.scan_timestamp_display,
                            rep.duration_ms as f64 / 1000.0
                        ))
                        .size(12.0)
                        .color(pal.muted),
                    );
                } else {
                    ui.label(
                        RichText::new("No cached scan — click Rescan")
                            .size(12.0)
                            .color(pal.muted),
                    );
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if draw_theme_toggle_button(ui, mode, &pal) {
                        let next = match mode {
                            ThemeMode::Dark => ThemeMode::Light,
                            ThemeMode::Light => ThemeMode::Dark,
                        };
                        emit(UiAction::ToggleTheme(next));
                    }
                    let settings_lbl = match ui_state.active_view {
                        ActiveView::Settings => "Back to Apps",
                        _ => "Settings",
                    };
                    if ui.button(settings_lbl).clicked() {
                        ui_state.active_view = match ui_state.active_view {
                            ActiveView::Settings => ActiveView::Main,
                            _ => ActiveView::Settings,
                        };
                    }
                    let deps_lbl = match ui_state.active_view {
                        ActiveView::Dependencies => "Back to Apps",
                        _ => "Dependencies",
                    };
                    if ui.button(deps_lbl).clicked() {
                        ui_state.active_view = match ui_state.active_view {
                            ActiveView::Dependencies => ActiveView::Main,
                            _ => ActiveView::Dependencies,
                        };
                    }
                    let rescan_btn = egui::Button::new(
                        RichText::new(if scanning { "Scanning..." } else { "Rescan" })
                            .color(if scanning { pal.muted } else { pal.accent_line })
                            .strong(),
                    );
                    if ui.add_enabled(!scanning, rescan_btn).clicked() {
                        emit(UiAction::StartRescan);
                    }
                });
            });

            ui.add_space(SPACE_SM);

            // Row 2: Per-drive Used / Free strip
            if let Some(rep) = report {
                ui.horizontal_wrapped(|ui| {
                    for drv in &rep.drives {
                        let frac = if drv.total_bytes > 0 {
                            (drv.used_bytes as f32 / drv.total_bytes as f32).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        egui::Frame::none()
                            .fill(pal.panel)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .rounding(Rounding::same(6.0))
                            .inner_margin(Margin::symmetric(8.0, 5.0))
                            .show(ui, |ui| {
                                ui.set_min_width(186.0);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{}:\\", drv.letter))
                                                .strong()
                                                .size(12.5)
                                                .color(pal.text),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{} free / {}",
                                                format_bytes(drv.free_bytes),
                                                format_bytes(drv.total_bytes)
                                            ))
                                            .size(11.5)
                                            .color(pal.muted),
                                        );
                                    });
                                    let (bar_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(172.0, 5.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        bar_rect,
                                        Rounding::same(2.5),
                                        pal.panel_alt,
                                    );
                                    let fill_w = (bar_rect.width() * frac).max(2.0);
                                    let fill_rect = Rect::from_min_size(
                                        bar_rect.min,
                                        Vec2::new(fill_w, bar_rect.height()),
                                    );
                                    let bar_col = if drv.full_walk_enabled {
                                        LOGO_BLUE
                                    } else {
                                        LOGO_LAVENDER
                                    };
                                    ui.painter().rect_filled(
                                        fill_rect,
                                        Rounding::same(2.5),
                                        bar_col,
                                    );
                                    ui.label(
                                        RichText::new(if drv.full_walk_enabled {
                                            "Full walk"
                                        } else {
                                            "Install paths only"
                                        })
                                        .size(10.5)
                                        .color(pal.muted),
                                    );
                                });
                            });
                    }
                });
            }

            if ui_state.active_view == ActiveView::Main {
                ui.add_space(SPACE_SM);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Filter:").size(12.5).color(pal.muted));
                    ui.add(
                        egui::TextEdit::singleline(&mut ui_state.search_query)
                            .desired_width(300.0)
                            .hint_text("Search app name, publisher, or path..."),
                    );
                    if !ui_state.search_query.is_empty() && ui.button("Clear").clicked() {
                        ui_state.search_query.clear();
                    }
                    if let Some(rep) = report {
                        let apps_sum: u64 = rep.apps.iter().map(|a| a.total_bytes).sum();
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} apps · {} attributed",
                                    rep.apps.len(),
                                    format_bytes(apps_sum)
                                ))
                                .size(12.0)
                                .color(pal.muted),
                            );
                        });
                    }
                });
            }
        });

    match ui_state.active_view {
        ActiveView::Main => {
            // Pinned Bottom Groups: Windows & system, Shared & system caches, Your files, Unattributed + Reconciliation
            if let Some(rep) = report {
                egui::TopBottomPanel::bottom("pinned_groups_panel")
                    .frame(
                        egui::Frame::none()
                            .fill(pal.panel_alt)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
                    )
                    .show(ctx, |ui| {
                        draw_pinned_bottom_groups(ui, ui_state, rep, &pal);
                    });
            }

            // Main App List
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg)
                        .inner_margin(Margin::symmetric(SPACE_MD, SPACE_XS)),
                )
                .show(ctx, |ui| {
                    draw_main_app_list(ui, ui_state, report, &pal);
                });
        }
        ActiveView::Dependencies => {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(pal.bg)
                        .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
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
                        .inner_margin(Margin::symmetric(SPACE_MD, SPACE_SM)),
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
                    .size(14.0)
                    .color(pal.muted),
            );
        });
        return;
    };

    let q = ui_state.search_query.trim().to_ascii_lowercase();
    egui::ScrollArea::vertical()
        .id_salt("main_app_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
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
                    .inner_margin(Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let header_resp = ui
                            .horizontal(|ui| {
                                draw_chevron(ui, expanded, pal.accent_line);
                                ui.label(
                                    RichText::new(&app.display_name)
                                        .size(13.5)
                                        .strong()
                                        .color(pal.text),
                                );
                                if let Some(first_pub) = app.publishers.first() {
                                    ui.label(
                                        RichText::new(first_pub)
                                            .size(11.5)
                                            .color(pal.muted),
                                    );
                                }
                                if let Some(nt) = app.no_touch {
                                    paint_pill(
                                        ui,
                                        nt.label(),
                                        pal.pill_lav_fill,
                                        LOGO_LAVENDER,
                                        LOGO_LAVENDER,
                                    );
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format_bytes(app.total_bytes))
                                            .size(13.5)
                                            .strong()
                                            .color(pal.text),
                                    );
                                    let delta_str = format_growth_delta(app.growth);
                                    let (d_fill, d_stroke, d_text) = match app.growth {
                                        GrowthDelta::NoBaseline => (
                                            pal.panel_alt,
                                            pal.border,
                                            pal.muted,
                                        ),
                                        GrowthDelta::New(_) => (
                                            pal.pill_blue_fill,
                                            pal.accent_line,
                                            pal.accent_line,
                                        ),
                                        GrowthDelta::Changed(d) if d > 0 => (
                                            pal.pill_pink_fill,
                                            LOGO_PINK,
                                            LOGO_PINK,
                                        ),
                                        GrowthDelta::Changed(d) if d < 0 => (
                                            pal.pill_green_fill,
                                            STATUS_GREEN,
                                            STATUS_GREEN,
                                        ),
                                        GrowthDelta::Changed(_) => (
                                            pal.panel_alt,
                                            pal.border,
                                            pal.muted,
                                        ),
                                    };
                                    paint_pill(ui, &delta_str, d_fill, d_stroke, d_text);
                                    ui.label(
                                        RichText::new(format!("{} loc", app.items.len()))
                                            .size(11.0)
                                            .color(pal.muted),
                                    );
                                });
                            })
                            .response
                            .interact(egui::Sense::click());

                        if header_resp.clicked() {
                            if expanded {
                                ui_state.expanded_apps.remove(&app.id);
                            } else {
                                ui_state.expanded_apps.insert(app.id.clone());
                            }
                        }

                        if expanded {
                            ui.add_space(SPACE_XS);
                            for item in &app.items {
                                egui::Frame::none()
                                    .fill(pal.panel_alt)
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(Margin::symmetric(8.0, 4.0))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            if let Some(zone) = item.zone {
                                                let (z_lbl, z_fill, z_stroke, z_txt) = match (zone, item.qualifier) {
                                                    (ZoneKind::Body, _) => (
                                                        "BODY",
                                                        pal.pill_blue_fill,
                                                        pal.accent_line,
                                                        pal.accent_line,
                                                    ),
                                                    (ZoneKind::Green, _) => (
                                                        "GREEN",
                                                        pal.pill_green_fill,
                                                        STATUS_GREEN,
                                                        STATUS_GREEN,
                                                    ),
                                                    (ZoneKind::Red, Some(LocationQualifier::RoamingConfig)) => (
                                                        "RED · Roaming config",
                                                        pal.pill_red_fill,
                                                        STATUS_RED,
                                                        STATUS_RED,
                                                    ),
                                                    (ZoneKind::Red, _) => (
                                                        "RED",
                                                        pal.pill_pink_fill,
                                                        LOGO_PINK,
                                                        LOGO_PINK,
                                                    ),
                                                };
                                                paint_pill(ui, z_lbl, z_fill, z_stroke, z_txt);
                                            }
                                            if item.qualifier == Some(LocationQualifier::MachineShared) {
                                                paint_pill(
                                                    ui,
                                                    "ProgramData · Machine-shared",
                                                    pal.pill_lav_fill,
                                                    LOGO_LAVENDER,
                                                    LOGO_LAVENDER,
                                                );
                                            }
                                            if let Some(basis) = item.basis {
                                                ui.label(
                                                    RichText::new(basis_short(basis))
                                                        .size(11.0)
                                                        .color(pal.muted),
                                                );
                                            }
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(format_bytes(item.bytes))
                                                            .size(12.0)
                                                            .strong()
                                                            .color(pal.text),
                                                    );
                                                    ui.with_layout(
                                                        Layout::left_to_right(Align::Center),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Label::new(
                                                                    RichText::new(&item.path)
                                                                        .monospace()
                                                                        .size(11.5)
                                                                        .color(pal.text),
                                                                )
                                                                .truncate(),
                                                            )
                                                            .on_hover_text(&item.path);
                                                        },
                                                    );
                                                },
                                            );
                                        });
                                    });
                            }

                            ui.add_space(SPACE_XS);
                            ui.label(
                                RichText::new("Dependencies (not added to app total):")
                                    .size(11.0)
                                    .strong()
                                    .color(pal.muted),
                            );
                            if app.dependencies.is_empty() {
                                ui.label(
                                    RichText::new(NO_KNOWN_DEPENDENCIES_LABEL)
                                        .size(11.5)
                                        .color(pal.muted),
                                );
                            } else {
                                for dep in &app.dependencies {
                                    egui::Frame::none()
                                        .fill(pal.panel_alt)
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(Margin::symmetric(8.0, 4.0))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                let (c_fill, c_stroke, c_txt) = match dep.confidence {
                                                    DependencyConfidence::Proven => (
                                                        pal.pill_blue_fill,
                                                        pal.accent_line,
                                                        pal.accent_line,
                                                    ),
                                                    DependencyConfidence::Inferred => (
                                                        pal.pill_lav_fill,
                                                        LOGO_LAVENDER,
                                                        LOGO_LAVENDER,
                                                    ),
                                                };
                                                paint_pill(ui, dep.confidence.label(), c_fill, c_stroke, c_txt);
                                                ui.label(
                                                    RichText::new(&dep.target_name)
                                                        .size(12.0)
                                                        .strong()
                                                        .color(pal.text),
                                                );
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} · {} · {}",
                                                        dep.kind.label(),
                                                        if dep.present { "present" } else { "missing" },
                                                        if dep.shared { "shared" } else { "private" }
                                                    ))
                                                    .size(11.0)
                                                    .color(if dep.present { pal.muted } else { STATUS_RED }),
                                                );
                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        let own_lbl = dep
                                                            .own_size_bytes
                                                            .map(|b| format!("own: {}", format_bytes(b)))
                                                            .unwrap_or_else(|| "OS / no own folder".into());
                                                        ui.label(
                                                            RichText::new(own_lbl)
                                                                .size(11.5)
                                                                .color(pal.muted),
                                                        );
                                                        ui.with_layout(
                                                            Layout::left_to_right(Align::Center),
                                                            |ui| {
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        RichText::new(&dep.basis)
                                                                            .size(11.0)
                                                                            .color(pal.muted),
                                                                    )
                                                                    .truncate(),
                                                                )
                                                                .on_hover_text(&dep.basis);
                                                            },
                                                        );
                                                    },
                                                );
                                            });
                                        });
                                }
                            }
                        }
                    });
                ui.add_space(SPACE_XS);
            }

            if shown == 0 {
                ui.add_space(24.0);
                ui.label(
                    RichText::new("No matching apps found for the current filter.")
                        .color(pal.muted),
                );
            }
        });
}

fn basis_short(basis: AttributionBasis) -> &'static str {
    match basis {
        AttributionBasis::InstallLocation => "[install]",
        AttributionBasis::ExactName => "[exact]",
        AttributionBasis::Token => "[token]",
        AttributionBasis::Publisher => "[publisher]",
        AttributionBasis::PackageFallback => "[package]",
        AttributionBasis::StandaloneFolder => "[standalone]",
    }
}

fn draw_pinned_bottom_groups(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    rep: &ScanReport,
    pal: &ThemePalette,
) {
    ui.label(
        RichText::new("PINNED STORAGE GROUPS")
            .size(11.0)
            .strong()
            .color(pal.muted),
    );
    ui.add_space(SPACE_XS);

    let any_expanded = ui_state.expand_windows_system
        || ui_state.expand_shared_caches
        || ui_state.expand_your_files
        || ui_state.expand_unattributed;

    let mut draw_cards = |ui: &mut egui::Ui| {
        // 1. Windows & system
        draw_pinned_group_card(
            ui,
            "Windows & system",
            rep.windows_system_total_bytes,
            "ESTIMATE · System remainder + pagefile/hiberfil",
            LOGO_LAVENDER,
            &mut ui_state.expand_windows_system,
            pal,
            |ui| {
                for sys in &rep.windows_system {
                    for rf in &sys.root_files {
                        draw_simple_item_row(ui, &rf.path, rf.bytes, Some("ROOT FILE"), pal);
                    }
                    draw_simple_item_row(
                        ui,
                        &format!("{}:\\Windows & system remainder (estimate)", sys.drive_letter),
                        sys.remainder_bytes,
                        Some("ESTIMATE"),
                        pal,
                    );
                }
            },
        );
        ui.add_space(SPACE_XS);

        // 2. Shared & system caches (GREEN)
        draw_pinned_group_card(
            ui,
            "Shared & system caches (GREEN)",
            rep.shared_caches_total_bytes,
            &format!("{} regenerable Temp / DXCache / Package Cache locations", rep.shared_caches.len()),
            STATUS_GREEN,
            &mut ui_state.expand_shared_caches,
            pal,
            |ui| {
                for item in rep.shared_caches.iter().take(40) {
                    draw_simple_item_row(ui, &item.path, item.bytes, Some("GREEN"), pal);
                }
            },
        );
        ui.add_space(SPACE_XS);

        // 3. Your files
        draw_pinned_group_card(
            ui,
            "Your files",
            rep.your_files_total_bytes,
            "Personal shell folders · Unzoned · Read-only",
            LOGO_BLUE,
            &mut ui_state.expand_your_files,
            pal,
            |ui| {
                for item in &rep.your_files {
                    draw_simple_item_row(ui, &item.path, item.bytes, None, pal);
                }
            },
        );
        ui.add_space(SPACE_XS);

        // 4. Unattributed
        draw_pinned_group_card(
            ui,
            "Unattributed",
            rep.unattributed_total_bytes,
            &format!("{} folders with no verified app owner", rep.unattributed.len()),
            LOGO_PINK,
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
                        .size(11.0)
                        .color(pal.muted),
                    );
                }
            },
        );
    };

    if any_expanded {
        egui::ScrollArea::vertical()
            .id_salt("pinned_groups_scroll")
            .max_height(210.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                draw_cards(ui);
            });
    } else {
        draw_cards(ui);
    }

    ui.add_space(SPACE_XS);
    // Reconciliation line per full-walk enabled drive
    for sys in &rep.windows_system {
        let apps_d = rep.total_apps_bytes_on_drive(sys.drive_letter);
        let cache_d = rep.total_shared_caches_on_drive(sys.drive_letter);
        let yf_d = rep.total_your_files_on_drive(sys.drive_letter);
        let un_d = rep.total_unattributed_on_drive(sys.drive_letter);
        let sum_all = apps_d + cache_d + yf_d + un_d + sys.total_bytes + sys.drive_free_bytes;
        let diff = sum_all as i128 - sys.drive_total_bytes as i128;
        ui.label(
            RichText::new(format!(
                "{}:\\ reconciliation: Apps {} + Caches {} + Your files {} + Unattributed {} + Win/sys {} + Free {} = {} (diff {} B)",
                sys.drive_letter,
                format_bytes(apps_d),
                format_bytes(cache_d),
                format_bytes(yf_d),
                format_bytes(un_d),
                format_bytes(sys.total_bytes),
                format_bytes(sys.drive_free_bytes),
                format_bytes(sys.drive_total_bytes),
                diff
            ))
            .size(11.0)
            .color(if diff == 0 { STATUS_GREEN } else { LOGO_PINK }),
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
        .rounding(Rounding::same(5.0))
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let resp = ui
                .horizontal(|ui| {
                    draw_chevron(ui, *expanded, accent);
                    ui.label(RichText::new(title).size(13.0).strong().color(pal.text));
                    ui.label(RichText::new(subtitle).size(11.5).color(pal.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format_bytes(total_bytes))
                                .size(13.0)
                                .strong()
                                .color(pal.text),
                        );
                    });
                })
                .response
                .interact(egui::Sense::click());
            if resp.clicked() {
                *expanded = !*expanded;
            }
            if *expanded {
                ui.add_space(SPACE_XS);
                body(ui);
            }
        });
}

fn draw_simple_item_row(
    ui: &mut egui::Ui,
    path: &str,
    bytes: u64,
    tag: Option<&str>,
    pal: &ThemePalette,
) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        if let Some(t) = tag {
            paint_pill(ui, t, pal.pill_lav_fill, LOGO_LAVENDER, LOGO_LAVENDER);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format_bytes(bytes))
                    .size(11.5)
                    .color(pal.muted),
            );
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(path)
                            .monospace()
                            .size(11.5)
                            .color(pal.text),
                    )
                    .truncate(),
                )
                .on_hover_text(path);
            });
        });
    });
}

fn draw_dependencies_view(
    ui: &mut egui::Ui,
    ui_state: &mut StorageUiState,
    report: Option<&ScanReport>,
    pal: &ThemePalette,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Reverse Dependencies (\"What else uses this?\")")
                .size(16.0)
                .strong()
                .color(pal.text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut ui_state.dep_search_query)
                    .desired_width(260.0)
                    .hint_text("Filter dependency or app..."),
            );
        });
    });
    ui.add_space(SPACE_SM);

    let Some(rep) = report else {
        ui.label(RichText::new("Run a scan first to inspect dependencies.").color(pal.muted));
        return;
    };

    let q = ui_state.dep_search_query.trim().to_ascii_lowercase();
    egui::ScrollArea::vertical()
        .id_salt("reverse_deps_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
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
                    .inner_margin(Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let resp = ui
                            .horizontal(|ui| {
                                draw_chevron(ui, expanded, pal.accent_line);
                                ui.label(
                                    RichText::new(&dep.target_name)
                                        .size(13.5)
                                        .strong()
                                        .color(pal.text),
                                );
                                ui.label(
                                    RichText::new(dep.kind.label())
                                        .size(11.5)
                                        .color(pal.muted),
                                );
                                paint_pill(
                                    ui,
                                    if dep.present { "present" } else { "missing" },
                                    if dep.present { pal.pill_green_fill } else { pal.pill_red_fill },
                                    if dep.present { STATUS_GREEN } else { STATUS_RED },
                                    if dep.present { STATUS_GREEN } else { STATUS_RED },
                                );
                                if dep.kind == DependencyKind::SharedRuntime || dep.binary_importer_count > 0 {
                                    paint_pill(
                                        ui,
                                        &format!("{} binary importers", dep.binary_importer_count),
                                        pal.pill_blue_fill,
                                        pal.accent_line,
                                        pal.accent_line,
                                    );
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{} apps", dep.dependent_count))
                                            .size(13.0)
                                            .strong()
                                            .color(pal.text),
                                    );
                                    if let Some(own) = dep.own_size_bytes {
                                        ui.label(
                                            RichText::new(format!("own: {}", format_bytes(own)))
                                                .size(11.5)
                                                .color(pal.muted),
                                        );
                                    }
                                });
                            })
                            .response
                            .interact(egui::Sense::click());
                        if resp.clicked() {
                            if expanded {
                                ui_state.expanded_deps.remove(&dep.target_id);
                            } else {
                                ui_state.expanded_deps.insert(dep.target_id.clone());
                            }
                        }
                        if expanded {
                            ui.add_space(SPACE_XS);
                            for c in &dep.consumers {
                                ui.horizontal(|ui| {
                                    ui.add_space(16.0);
                                    paint_pill(
                                        ui,
                                        c.confidence.label(),
                                        if c.confidence == DependencyConfidence::Proven {
                                            pal.pill_blue_fill
                                        } else {
                                            pal.pill_lav_fill
                                        },
                                        if c.confidence == DependencyConfidence::Proven {
                                            pal.accent_line
                                        } else {
                                            LOGO_LAVENDER
                                        },
                                        if c.confidence == DependencyConfidence::Proven {
                                            pal.accent_line
                                        } else {
                                            LOGO_LAVENDER
                                        },
                                    );
                                    ui.label(RichText::new(&c.app_name).size(12.0).color(pal.text));
                                    ui.label(RichText::new(&c.basis).size(11.0).color(pal.muted));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format_bytes(c.app_total_bytes))
                                                .size(11.5)
                                                .color(pal.muted),
                                        );
                                    });
                                });
                            }
                        }
                    });
                ui.add_space(SPACE_XS);
            }
        });
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
    ui.label(
        RichText::new(
            "Default: only the system drive is enabled for a full walk with Windows excluded. Registry InstallLocation folders are always sized individually on all drives.",
        )
        .size(12.0)
        .color(pal.muted),
    );
    ui.add_space(SPACE_SM);

    egui::ScrollArea::vertical()
        .id_salt("settings_scroll")
        .show(ui, |ui| {
            for target in &mut draft.targets {
                let drv_info = report
                    .and_then(|r| r.drives.iter().find(|d| d.letter == target.letter));
                let is_sys = drv_info.is_some_and(|d| d.is_system_drive)
                    || target.letter.eq_ignore_ascii_case(&'C');

                egui::Frame::none()
                    .fill(pal.panel)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Drive {}:\\", target.letter))
                                    .size(14.0)
                                    .strong()
                                    .color(pal.text),
                            );
                            if is_sys {
                                paint_pill(
                                    ui,
                                    "SYSTEM DRIVE",
                                    pal.pill_blue_fill,
                                    pal.accent_line,
                                    pal.accent_line,
                                );
                            }
                            if let Some(di) = drv_info {
                                ui.label(
                                    RichText::new(format!(
                                        "{} used / {} total",
                                        format_bytes(di.used_bytes),
                                        format_bytes(di.total_bytes)
                                    ))
                                    .size(12.0)
                                    .color(pal.muted),
                                );
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .checkbox(&mut target.enabled, "Enable full drive walk")
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                        });

                        ui.add_space(SPACE_XS);
                        ui.label(
                            RichText::new("Exclusions:")
                                .size(12.0)
                                .color(pal.muted),
                        );

                        let mut remove_idx: Option<usize> = None;
                        for (idx, ex) in target.exclusions.iter().enumerate() {
                            let is_locked_win = is_sys && normalize_path_key(ex) == sys_win_key;
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(ex)
                                        .monospace()
                                        .size(12.0)
                                        .color(pal.text),
                                );
                                if is_locked_win {
                                    paint_pill(
                                        ui,
                                        "LOCKED · Constitutional",
                                        pal.pill_lav_fill,
                                        LOGO_LAVENDER,
                                        LOGO_LAVENDER,
                                    );
                                } else if ui.small_button("Remove").clicked() {
                                    remove_idx = Some(idx);
                                }
                            });
                        }
                        if let Some(idx) = remove_idx {
                            target.exclusions.remove(idx);
                            changed = true;
                        }

                        ui.add_space(SPACE_XS);
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
                ui.add_space(SPACE_SM);
            }
        });

    if changed {
        emit(UiAction::SaveConfig(draft));
    }

    let _ = Align2::CENTER_CENTER;
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, RawInput, Rect, Vec2};
    use y_core::model::{
        AttributedApp, BreakdownItem, DriveInfo, RegistryDiscoveryStats, SystemDriveBreakdown,
        UnattributedItem,
    };

    #[test]
    fn ui_renders_main_expanded_and_settings_views_cleanly() {
        let report = ScanReport {
            scan_timestamp_unix: 1000,
            scan_timestamp_display: "2026-11-26 09:00:00".into(),
            duration_ms: 8500,
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
                dependencies: vec![],
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
                path: "C:\\Users\\Win\\AppData\\Local\\Temp".into(),
                bytes: 5_000,
                drive_letter: 'C',
                zone_hint: Some(ZoneKind::Green),
            }],
            unattributed_total_bytes: 5_000,
            reverse_dependencies: vec![],
        };

        let ctx = egui::Context::default();
        let mut state = StorageUiState::default();
        state.expanded_apps.insert("reg:discord".into());
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
