use egui::Color32;
use y_core::config::{ThemeMode, ThemePresetChoice};

pub const CLEAR_BG_DARK_RGB: u32 = 0x000A_0A0A;
pub const CLEAR_BG_LIGHT_RGB: u32 = 0x00F2_F2F2;

pub const STATUS_GREEN: Color32 = Color32::from_rgb(40, 214, 108); // #28D66C
pub const STATUS_AMBER: Color32 = Color32::from_rgb(245, 184, 61); // #F5B83D
pub const STATUS_RED: Color32 = Color32::from_rgb(240, 68, 88); // #F04458

// Pure neutral surfaces (R == G == B) — backgrounds never carry theme tint (WT_UI_LIBRARY §0.5).
const DARK_BG: Color32 = Color32::from_rgb(10, 10, 10); // #0A0A0A
const DARK_PANEL_ALT: Color32 = Color32::from_rgb(18, 18, 18); // #121212
const DARK_PANEL: Color32 = Color32::from_rgb(22, 22, 22); // #161616
const DARK_WIDGET: Color32 = Color32::from_rgb(28, 28, 28); // #1C1C1C
const DARK_WIDGET_HOVER: Color32 = Color32::from_rgb(38, 38, 38); // #262626
const DARK_BORDER: Color32 = Color32::from_rgb(46, 46, 46); // #2E2E2E

const LIGHT_BG: Color32 = Color32::from_rgb(242, 242, 242); // #F2F2F2
const LIGHT_PANEL_ALT: Color32 = Color32::from_rgb(247, 247, 247); // #F7F7F7
const LIGHT_PANEL: Color32 = Color32::from_rgb(255, 255, 255); // #FFFFFF
const LIGHT_WIDGET: Color32 = Color32::from_rgb(244, 244, 244); // #F4F4F4
const LIGHT_WIDGET_HOVER: Color32 = Color32::from_rgb(234, 234, 234); // #EAEAEA
const LIGHT_BORDER: Color32 = Color32::from_rgb(218, 218, 218); // #DADADA

#[inline]
#[must_use]
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let u = t.clamp(0.0, 1.0);
    let r = (a.r() as f32 + (b.r() as f32 - a.r() as f32) * u).round() as u8;
    let g = (a.g() as f32 + (b.g() as f32 - a.g() as f32) * u).round() as u8;
    let bl = (a.b() as f32 + (b.b() as f32 - a.b() as f32) * u).round() as u8;
    Color32::from_rgb(r, g, bl)
}

#[inline]
fn srgb_channel_to_linear(c: u8) -> f32 {
    let v = (c as f32) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[must_use]
pub fn relative_luminance(col: Color32) -> f32 {
    0.2126 * srgb_channel_to_linear(col.r())
        + 0.7152 * srgb_channel_to_linear(col.g())
        + 0.0722 * srgb_channel_to_linear(col.b())
}

#[must_use]
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[must_use]
pub fn rgb_to_hsl(col: Color32) -> (f32, f32, f32) {
    let r = col.r() as f32 / 255.0;
    let g = col.g() as f32 / 255.0;
    let b = col.b() as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    let d = max - min;
    if d < 1e-5 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h_raw = if (max - r).abs() < 1e-5 {
        ((g - b) / d) + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-5 {
        ((b - r) / d) + 2.0
    } else {
        ((r - g) / d) + 4.0
    };
    (((h_raw * 60.0) % 360.0 + 360.0) % 360.0, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

#[must_use]
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color32 {
    let hue = ((h % 360.0) + 360.0) % 360.0;
    let sat = s.clamp(0.0, 1.0);
    let lit = l.clamp(0.0, 1.0);
    if sat < 1e-5 {
        let v = (lit * 255.0).round() as u8;
        return Color32::from_rgb(v, v, v);
    }
    let q = if lit < 0.5 {
        lit * (1.0 + sat)
    } else {
        lit + sat - lit * sat
    };
    let p = 2.0 * lit - q;
    let hue_to_rgb = |t: f32| -> f32 {
        let mut t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        t = p;
        t
    };
    let hk = hue / 360.0;
    Color32::from_rgb(
        (hue_to_rgb(hk + 1.0 / 3.0) * 255.0).round().clamp(0.0, 255.0) as u8,
        (hue_to_rgb(hk) * 255.0).round().clamp(0.0, 255.0) as u8,
        (hue_to_rgb(hk - 1.0 / 3.0) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

#[must_use]
pub fn parse_hex_rgb(raw: &str) -> Option<[u8; 3]> {
    let trimmed = raw.trim().trim_start_matches('#');
    if trimmed.len() == 6 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
        let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
        let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
        return Some([r, g, b]);
    }
    if trimmed.len() == 3 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&trimmed[0..1], 16).ok()?;
        let g = u8::from_str_radix(&trimmed[1..2], 16).ok()?;
        let b = u8::from_str_radix(&trimmed[2..3], 16).ok()?;
        return Some([r * 17, g * 17, b * 17]);
    }
    None
}

#[must_use]
pub fn format_hex_rgb(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// Returns true if the colour's hue sits close to semantic green, amber/yellow, or red.
#[must_use]
pub fn is_hue_near_status(rgb: [u8; 3]) -> bool {
    let col = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    let (h, s, _) = rgb_to_hsl(col);
    if s < 0.16 {
        return false;
    }
    // Red (<= 22 or >= 332), Amber/Yellow (22..=72), Green (88..=168).
    h <= 72.0 || (88.0..=168.0).contains(&h) || h >= 332.0
}

fn find_lightness_for_min_contrast_up(
    h: f32,
    s: f32,
    start_l: f32,
    bg_a: Color32,
    bg_b: Color32,
    min_ratio: f32,
) -> Color32 {
    let start = hsl_to_rgb(h, s, start_l);
    if contrast_ratio(start, bg_a) >= min_ratio && contrast_ratio(start, bg_b) >= min_ratio {
        return start;
    }
    let mut lo = start_l.clamp(0.0, 0.99);
    let mut hi = 0.99_f32;
    for _ in 0..20 {
        let mid = (lo + hi) * 0.5;
        let c = hsl_to_rgb(h, s, mid);
        if contrast_ratio(c, bg_a) >= min_ratio && contrast_ratio(c, bg_b) >= min_ratio {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hsl_to_rgb(h, s, hi)
}

fn find_lightness_for_min_contrast_down(
    h: f32,
    s: f32,
    start_l: f32,
    bg_a: Color32,
    bg_b: Color32,
    min_ratio: f32,
) -> Color32 {
    let start = hsl_to_rgb(h, s, start_l);
    if contrast_ratio(start, bg_a) >= min_ratio && contrast_ratio(start, bg_b) >= min_ratio {
        return start;
    }
    let mut lo = 0.04_f32;
    let mut hi = start_l.clamp(0.05, 1.0);
    for _ in 0..20 {
        let mid = (lo + hi) * 0.5;
        let c = hsl_to_rgb(h, s, mid);
        if contrast_ratio(c, bg_a) >= min_ratio && contrast_ratio(c, bg_b) >= min_ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hsl_to_rgb(h, s, lo)
}

#[derive(Debug, Clone)]
pub struct ThemeLadder {
    pub deep: Color32,
    pub ink_light: Color32,
    pub accent_light: Color32,
    pub accent_dark: Color32,
    pub source: Color32,
    pub soft_fill_dark: Color32,
    pub pill_text_dark: Color32,
    pub soft_fill_light: Color32,
    pub sweep_dark: [Color32; 4],
    pub sweep_light: [Color32; 4],
    pub near_status_hue: bool,
}

impl ThemeLadder {
    #[must_use]
    pub fn generate(preset: ThemePresetChoice, custom_rgb: [u8; 3]) -> Self {
        let raw_rgb = match preset {
            ThemePresetChoice::Custom => custom_rgb,
            other => other.base_rgb(),
        };
        let raw_col = Color32::from_rgb(raw_rgb[0], raw_rgb[1], raw_rgb[2]);
        let (h, s_raw, l_raw) = rgb_to_hsl(raw_col);
        let near_status = is_hue_near_status(raw_rgb);

        // Exact §0.3 / §2.1 curated steps for WT Blue and WT Pink.
        if preset == ThemePresetChoice::Blue {
            let source = Color32::from_rgb(0x7C, 0xCB, 0xF2); // #7CCBF2
            let ink_light = Color32::from_rgb(0x00, 0x90, 0xD9); // #0090D9
            let accent_dark = Color32::from_rgb(0x69, 0xCB, 0xFC); // #69CBFC
            let soft_fill_dark = Color32::from_rgb(0x25, 0x31, 0x41); // #253141
            let pill_text_dark = Color32::from_rgb(0xAF, 0xE3, 0xFD); // #AFE3FD
            let soft_fill_light = Color32::from_rgb(0xE2, 0xF4, 0xFC); // #E2F4FC
            let deep = Color32::from_rgb(0x00, 0x73, 0xB2);
            return Self {
                deep,
                ink_light,
                accent_light: ink_light,
                accent_dark,
                source,
                soft_fill_dark,
                pill_text_dark,
                soft_fill_light,
                sweep_dark: [
                    Color32::from_rgb(0x28, 0x9C, 0xE0),
                    Color32::from_rgb(0x54, 0xB9, 0xF2),
                    accent_dark,
                    Color32::from_rgb(0xB4, 0xE9, 0xFF),
                ],
                sweep_light: [
                    deep,
                    ink_light,
                    Color32::from_rgb(0x38, 0xAE, 0xEA),
                    source,
                ],
                near_status_hue: false,
            };
        }
        if preset == ThemePresetChoice::Pink {
            let source = Color32::from_rgb(0xF2, 0xA9, 0xD6); // #F2A9D6
            let ink_light = Color32::from_rgb(0xFA, 0x29, 0xAA); // #FA29AA
            let accent_dark = Color32::from_rgb(0xFA, 0x80, 0xCB); // #FA80CB
            let soft_fill_dark = Color32::from_rgb(0x38, 0x2C, 0x3C); // #382C3C
            let pill_text_dark = Color32::from_rgb(0xFB, 0xBB, 0xE3); // #FBBBE3
            let soft_fill_light = Color32::from_rgb(0xFC, 0xEC, 0xF6); // #FCECF6
            let deep = Color32::from_rgb(0xC2, 0x18, 0x7E);
            return Self {
                deep,
                ink_light,
                accent_light: ink_light,
                accent_dark,
                source,
                soft_fill_dark,
                pill_text_dark,
                soft_fill_light,
                sweep_dark: [
                    Color32::from_rgb(0xB8, 0x1E, 0x7E),
                    Color32::from_rgb(0xDF, 0x4E, 0xA8),
                    accent_dark,
                    Color32::from_rgb(0xFC, 0xBB, 0xE3),
                ],
                sweep_light: [
                    deep,
                    ink_light,
                    Color32::from_rgb(0xF6, 0x6A, 0xBF),
                    source,
                ],
                near_status_hue: false,
            };
        }

        // General normalised ladder for Purple, Green, Orange, Yellow, Red, and arbitrary Custom RGB/hex (§0.7).
        let vivid_s = if s_raw < 0.04 { 0.0 } else { s_raw.clamp(0.45, 0.94) };
        let soft_s = if s_raw < 0.04 { 0.0 } else { s_raw.clamp(0.35, 0.86) };

        let source_norm = if l_raw < 0.14 || l_raw > 0.92 {
            hsl_to_rgb(h, soft_s, l_raw.clamp(0.42, 0.72))
        } else {
            raw_col
        };

        let soft_fill_light = lerp_color(LIGHT_PANEL, source_norm, 0.22);
        let ink_light = find_lightness_for_min_contrast_down(
            h,
            vivid_s,
            l_raw.clamp(0.25, 0.56),
            LIGHT_PANEL,
            soft_fill_light,
            4.55,
        );
        // Double-check against neutral LIGHT_BG (#F2F2F2) too so small text passes everywhere in Light mode.
        let ink_light = if contrast_ratio(ink_light, LIGHT_BG) >= 4.5 {
            ink_light
        } else {
            let (ih, is, il) = rgb_to_hsl(ink_light);
            find_lightness_for_min_contrast_down(ih, is, il, LIGHT_BG, LIGHT_PANEL, 4.55)
        };
        let accent_light = find_lightness_for_min_contrast_down(
            h,
            vivid_s,
            l_raw.clamp(0.30, 0.62),
            LIGHT_PANEL,
            LIGHT_BG,
            3.15,
        );
        let deep = find_lightness_for_min_contrast_down(
            h,
            vivid_s,
            0.42,
            LIGHT_PANEL,
            Color32::from_rgb(248, 248, 248),
            4.8,
        );

        let accent_dark = find_lightness_for_min_contrast_up(
            h,
            vivid_s,
            l_raw.clamp(0.55, 0.76),
            DARK_PANEL,
            DARK_WIDGET,
            5.8,
        );
        let soft_fill_dark = lerp_color(DARK_PANEL, accent_dark, 0.16);
        let pill_text_dark = find_lightness_for_min_contrast_up(
            h,
            soft_s,
            0.84,
            soft_fill_dark,
            DARK_PANEL,
            7.2,
        );

        let dark_highlight = find_lightness_for_min_contrast_up(h, soft_s, 0.88, DARK_PANEL, soft_fill_dark, 9.0);
        let mid_light = lerp_color(accent_light, source_norm, 0.45);
        let end_light = if contrast_ratio(source_norm, LIGHT_PANEL) >= 2.1 {
            source_norm
        } else {
            accent_light
        };

        Self {
            deep,
            ink_light,
            accent_light,
            accent_dark,
            source: raw_col,
            soft_fill_dark,
            pill_text_dark,
            soft_fill_light,
            sweep_dark: [deep, source_norm, accent_dark, dark_highlight],
            sweep_light: [deep, ink_light, mid_light, end_light],
            near_status_hue: near_status,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub theme_key: u32,
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
    // Single-hue active theme ladder roles (WT_UI_LIBRARY §0)
    pub accent_line: Color32,
    pub accent_border: Color32,
    pub accent_soft_fill: Color32,
    pub accent_pill_text: Color32,
    #[allow(dead_code)]
    pub accent_deep: Color32,
    #[allow(dead_code)]
    pub accent_source: Color32,
    pub accent_sweep: [Color32; 4],
    pub near_status_hue: bool,
    // Separate semantic §4B status family
    pub status_green: Color32,
    pub status_amber: Color32,
    pub status_red: Color32,
    pub delta_grew: Color32,
    pub no_touch_outline: Color32,
    pub pill_green_fill: Color32,
    #[allow(dead_code)]
    pub pill_amber_fill: Color32,
    pub pill_red_fill: Color32,
    pub scrollbar_thumb: Color32,
    pub scrollbar_thumb_hover: Color32,
}

#[must_use]
pub fn compute_theme_key(mode: ThemeMode, preset: ThemePresetChoice, custom_rgb: [u8; 3]) -> u32 {
    let mode_bit = if mode == ThemeMode::Dark { 1u32 } else { 0u32 };
    let preset_idx = match preset {
        ThemePresetChoice::Blue => 0u32,
        ThemePresetChoice::Pink => 1,
        ThemePresetChoice::Purple => 2,
        ThemePresetChoice::Green => 3,
        ThemePresetChoice::Orange => 4,
        ThemePresetChoice::Yellow => 5,
        ThemePresetChoice::Red => 6,
        ThemePresetChoice::Custom => 7,
    };
    let rgb = match preset {
        ThemePresetChoice::Custom => custom_rgb,
        p => p.base_rgb(),
    };
    (mode_bit << 28)
        | (preset_idx << 24)
        | ((rgb[0] as u32) << 16)
        | ((rgb[1] as u32) << 8)
        | (rgb[2] as u32)
}

impl ThemePalette {
    #[must_use]
    pub fn from_config(mode: ThemeMode, preset: ThemePresetChoice, custom_rgb: [u8; 3]) -> Self {
        let ladder = ThemeLadder::generate(preset, custom_rgb);
        let theme_key = compute_theme_key(mode, preset, custom_rgb);
        match mode {
            ThemeMode::Dark => Self {
                theme_key,
                bg: DARK_BG,
                panel: DARK_PANEL,
                panel_alt: DARK_PANEL_ALT,
                border: DARK_BORDER,
                text: Color32::from_rgb(238, 238, 238), // #EEEEEE
                muted: Color32::from_rgb(158, 158, 158), // #9E9E9E
                subtle_text: Color32::from_rgb(132, 132, 132), // #848484
                hint_text: Color32::from_rgb(102, 102, 102), // #666666
                input_bg: DARK_BG,
                header_input_bg: DARK_BG,
                widget: DARK_WIDGET,
                widget_hover: DARK_WIDGET_HOVER,
                widget_active: lerp_color(DARK_PANEL, ladder.source, 0.24),
                accent_line: ladder.accent_dark,
                accent_border: ladder.accent_dark,
                accent_soft_fill: ladder.soft_fill_dark,
                accent_pill_text: ladder.pill_text_dark,
                accent_deep: ladder.deep,
                accent_source: ladder.source,
                accent_sweep: ladder.sweep_dark,
                near_status_hue: ladder.near_status_hue,
                status_green: STATUS_GREEN,
                status_red: STATUS_RED,
                status_amber: STATUS_AMBER,
                delta_grew: STATUS_RED,
                no_touch_outline: STATUS_RED,
                pill_green_fill: Color32::from_rgb(16, 46, 32), // #102E20
                pill_amber_fill: Color32::from_rgb(56, 46, 37), // #382E25
                pill_red_fill: Color32::from_rgb(44, 18, 30),   // #2C121E
                scrollbar_thumb: Color32::from_rgb(88, 88, 88), // #585858
                scrollbar_thumb_hover: Color32::from_rgb(124, 124, 124), // #7C7C7C
            },
            ThemeMode::Light => Self {
                theme_key,
                bg: LIGHT_BG,
                panel: LIGHT_PANEL,
                panel_alt: LIGHT_PANEL_ALT,
                border: LIGHT_BORDER,
                text: Color32::from_rgb(30, 30, 30),    // #1E1E1E
                muted: Color32::from_rgb(99, 99, 99),   // #636363
                subtle_text: Color32::from_rgb(120, 120, 120), // #787878
                hint_text: Color32::from_rgb(154, 154, 154),   // #9A9A9A
                input_bg: LIGHT_BG,
                header_input_bg: LIGHT_PANEL,
                widget: LIGHT_WIDGET,
                widget_hover: LIGHT_WIDGET_HOVER,
                widget_active: lerp_color(LIGHT_PANEL, ladder.source, 0.30),
                accent_line: ladder.ink_light,
                accent_border: ladder.accent_light,
                accent_soft_fill: ladder.soft_fill_light,
                accent_pill_text: ladder.ink_light,
                accent_deep: ladder.deep,
                accent_source: ladder.source,
                accent_sweep: ladder.sweep_light,
                near_status_hue: ladder.near_status_hue,
                status_green: Color32::from_rgb(10, 122, 60),  // #0A7A3C
                status_red: Color32::from_rgb(224, 36, 68),    // #E02444
                status_amber: Color32::from_rgb(166, 108, 7),  // #A66C07
                delta_grew: Color32::from_rgb(224, 36, 68),
                no_touch_outline: Color32::from_rgb(224, 36, 68),
                pill_green_fill: Color32::from_rgb(228, 249, 236), // #E4F9EC
                pill_amber_fill: Color32::from_rgb(253, 240, 218), // #FDF0DA
                pill_red_fill: Color32::from_rgb(253, 232, 237),   // #FDE8ED
                scrollbar_thumb: Color32::from_rgb(176, 176, 176), // #B0B0B0
                scrollbar_thumb_hover: Color32::from_rgb(136, 136, 136), // #888888
            },
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn for_mode(mode: ThemeMode) -> Self {
        Self::from_config(mode, ThemePresetChoice::Blue, y_core::config::default_custom_theme_rgb())
    }

    #[inline]
    #[must_use]
    pub fn sample_sweep(&self, t: f32) -> Color32 {
        let stops = &self.accent_sweep;
        let scaled = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
        let idx = (scaled.floor() as usize).min(stops.len() - 2);
        let frac = scaled - idx as f32;
        lerp_color(stops[idx], stops[idx + 1], frac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_blue_and_pink_presets_match_locked_tokens_with_neutral_backgrounds() {
        let blue_dark = ThemePalette::from_config(ThemeMode::Dark, ThemePresetChoice::Blue, [0, 0, 0]);
        let blue_light = ThemePalette::from_config(ThemeMode::Light, ThemePresetChoice::Blue, [0, 0, 0]);
        assert_eq!(blue_dark.accent_line, Color32::from_rgb(0x69, 0xCB, 0xFC));
        assert_eq!(blue_dark.accent_soft_fill, Color32::from_rgb(0x25, 0x31, 0x41));
        assert_eq!(blue_dark.accent_pill_text, Color32::from_rgb(0xAF, 0xE3, 0xFD));
        assert_eq!(blue_light.accent_line, Color32::from_rgb(0x00, 0x90, 0xD9));
        assert_eq!(blue_light.accent_soft_fill, Color32::from_rgb(0xE2, 0xF4, 0xFC));
        assert_eq!(blue_light.accent_pill_text, Color32::from_rgb(0x00, 0x90, 0xD9));

        let pink_dark = ThemePalette::from_config(ThemeMode::Dark, ThemePresetChoice::Pink, [0, 0, 0]);
        let pink_light = ThemePalette::from_config(ThemeMode::Light, ThemePresetChoice::Pink, [0, 0, 0]);
        assert_eq!(pink_dark.accent_line, Color32::from_rgb(0xFA, 0x80, 0xCB));
        assert_eq!(pink_dark.accent_soft_fill, Color32::from_rgb(0x38, 0x2C, 0x3C));
        assert_eq!(pink_dark.accent_pill_text, Color32::from_rgb(0xFB, 0xBB, 0xE3));
        assert_eq!(pink_light.accent_line, Color32::from_rgb(0xFA, 0x29, 0xAA));
        assert_eq!(pink_light.accent_soft_fill, Color32::from_rgb(0xFC, 0xEC, 0xF6));

        // Pure neutral backgrounds in both modes (R == G == B)
        for p in [&blue_dark, &blue_light, &pink_dark, &pink_light] {
            for c in [p.bg, p.panel, p.panel_alt, p.widget, p.widget_hover, p.border, p.text, p.muted] {
                assert_eq!(c.r(), c.g(), "non-neutral R/G in {c:?}");
                assert_eq!(c.g(), c.b(), "non-neutral G/B in {c:?}");
            }
        }
    }

    #[test]
    fn custom_extremes_normalise_for_wcag_contrast_in_both_modes() {
        let extreme_inputs = [
            [0, 0, 0],       // pure black
            [255, 255, 255], // pure white
            [255, 255, 160], // pale yellow
            [18, 10, 48],    // near-black indigo
            [250, 204, 21],  // Yellow preset
            [62, 207, 110],  // Green preset
            [248, 113, 113], // Red preset
            [155, 138, 251], // Purple preset
            [251, 146, 60],  // Orange preset
        ];
        for rgb in extreme_inputs {
            let dark = ThemePalette::from_config(ThemeMode::Dark, ThemePresetChoice::Custom, rgb);
            let light = ThemePalette::from_config(ThemeMode::Light, ThemePresetChoice::Custom, rgb);
            assert!(
                contrast_ratio(dark.accent_line, dark.panel) >= 5.5,
                "dark contrast failed for {rgb:?}: {:?}",
                dark.accent_line
            );
            assert!(
                contrast_ratio(dark.accent_pill_text, dark.accent_soft_fill) >= 6.5,
                "dark pill contrast failed for {rgb:?}"
            );
            assert!(
                contrast_ratio(light.accent_line, light.panel) >= 4.5,
                "light panel contrast failed for {rgb:?}: {:?}",
                light.accent_line
            );
            assert!(
                contrast_ratio(light.accent_line, light.bg) >= 4.5,
                "light bg contrast failed for {rgb:?}: {:?}",
                light.accent_line
            );
            assert!(
                contrast_ratio(light.accent_line, light.accent_soft_fill) >= 4.5,
                "light soft-fill contrast failed for {rgb:?}"
            );
        }
        assert!(is_hue_near_status([62, 207, 110]));
        assert!(is_hue_near_status([250, 204, 21]));
        assert!(is_hue_near_status([248, 113, 113]));
        assert!(!is_hue_near_status([124, 203, 242]));
        assert!(!is_hue_near_status([155, 138, 251]));
    }

    #[test]
    fn hex_parser_and_formatter_round_trip() {
        assert_eq!(parse_hex_rgb("#7CCBF2"), Some([0x7C, 0xCB, 0xF2]));
        assert_eq!(parse_hex_rgb("9b8afb"), Some([0x9B, 0x8A, 0xFB]));
        assert_eq!(parse_hex_rgb("invalid"), None);
        assert_eq!(format_hex_rgb([0x7C, 0xCB, 0xF2]), "#7CCBF2");
    }
}
