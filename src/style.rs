use crate::dwm::sync_window_blur;
use egui::{Color32, Context, CornerRadius, Painter, Rect, Stroke, Vec2, Visuals};

/// Matches Windows 11 `DWMWCP_ROUND` window corner curvature so the egui glass fill aligns cleanly with DWM Acrylic.
pub const DEFAULT_GLASS_CORNER_RADIUS: f32 = 9.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BackgroundMode {
    #[default]
    FrostedGlass,
    Transparent,
}

impl BackgroundMode {
    pub fn label(self) -> &'static str {
        match self {
            BackgroundMode::FrostedGlass => "Frosted Glass",
            BackgroundMode::Transparent => "Transparent Mode",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            BackgroundMode::FrostedGlass => BackgroundMode::Transparent,
            BackgroundMode::Transparent => BackgroundMode::FrostedGlass,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GlassTintPreset {
    #[cfg_attr(
        feature = "serde",
        serde(alias = "MidnightDark", alias = "CharcoalSlate", alias = "OceanBlue")
    )]
    Dark,
    #[cfg_attr(feature = "serde", serde(alias = "FrostLight", alias = "Light"))]
    Light,
    #[default]
    #[cfg_attr(feature = "serde", serde(alias = "FollowWindows", alias = "Auto"))]
    System,
}

impl GlassTintPreset {
    pub fn label(self) -> &'static str {
        match self {
            GlassTintPreset::Dark => "Dark",
            GlassTintPreset::Light => "Light",
            GlassTintPreset::System => "Follow System",
        }
    }

    pub fn is_light_theme(self, windows_is_light: bool) -> bool {
        match self {
            GlassTintPreset::Dark => false,
            GlassTintPreset::Light => true,
            GlassTintPreset::System => windows_is_light,
        }
    }

    pub fn base_rgb(self, windows_is_light: bool) -> [u8; 3] {
        if self.is_light_theme(windows_is_light) {
            [240, 244, 250]
        } else {
            [20, 22, 28]
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassStyle {
    pub tint_preset: GlassTintPreset,
    pub opacity: f32,
    pub windows_is_light: bool,
    pub corner_radius: f32,
}

impl Default for GlassStyle {
    fn default() -> Self {
        Self::new(GlassTintPreset::System, 0.52, false)
    }
}

impl GlassStyle {
    pub fn new(tint_preset: GlassTintPreset, opacity: f32, windows_is_light: bool) -> Self {
        Self {
            tint_preset,
            opacity: opacity.clamp(0.05, 0.98),
            windows_is_light,
            corner_radius: DEFAULT_GLASS_CORNER_RADIUS,
        }
    }

    pub fn with_corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.05, 0.98);
        self
    }

    pub fn is_light(&self) -> bool {
        self.tint_preset.is_light_theme(self.windows_is_light)
    }

    pub fn tint_rgb(&self) -> [u8; 3] {
        self.tint_preset.base_rgb(self.windows_is_light)
    }

    pub fn fill_color(&self) -> Color32 {
        self.fill_color_with_alpha(1.0)
    }

    pub fn fill_color_with_alpha(&self, alpha_scale: f32) -> Color32 {
        let [r, g, b] = self.tint_rgb();
        let base_alpha = self.opacity.clamp(0.05, 0.98);
        let alpha = (base_alpha * alpha_scale.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_unmultiplied(r, g, b, alpha)
    }

    pub fn corner_radius_egui(&self) -> CornerRadius {
        CornerRadius::same(self.corner_radius.round().clamp(0.0, 255.0) as u8)
    }

    pub fn palette(&self) -> GlassPalette {
        GlassPalette::for_theme(self.is_light())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlassPalette {
    pub is_light: bool,
    pub primary_text: Color32,
    pub secondary_text: Color32,
    pub muted_text: Color32,
    pub accent_text: Color32,
    pub divider: Color32,
    pub item_hover_fill: Color32,
    pub item_hover_text: Color32,
    pub item_hover_secondary_text: Color32,
    pub danger_text: Color32,
    pub danger_hover_fill: Color32,
    pub card_fill: Color32,
    pub card_stroke: Color32,
    pub control_fill: Color32,
    pub control_hover_fill: Color32,
    pub control_stroke: Color32,
    pub input_fill: Color32,
    pub accent_fill: Color32,
    pub accent_hover_fill: Color32,
    pub accent_text_on_fill: Color32,
    pub selection_fill: Color32,
    pub success_text: Color32,
    pub segment_track_fill: Color32,
    pub segment_active_fill: Color32,
    pub toggle_off_fill: Color32,
    pub toggle_knob_fill: Color32,
}

impl GlassPalette {
    pub fn for_theme(is_light: bool) -> Self {
        if is_light {
            Self {
                is_light: true,
                primary_text: Color32::from_rgb(20, 24, 34),
                secondary_text: Color32::from_rgb(68, 78, 98),
                muted_text: Color32::from_rgb(98, 108, 128),
                accent_text: Color32::from_rgb(218, 58, 48),
                divider: Color32::from_rgba_unmultiplied(20, 30, 50, 16),
                item_hover_fill: Color32::from_rgba_unmultiplied(42, 112, 232, 210),
                item_hover_text: Color32::from_rgb(250, 252, 255),
                item_hover_secondary_text: Color32::from_rgba_unmultiplied(235, 242, 255, 225),
                danger_text: Color32::from_rgb(208, 42, 42),
                danger_hover_fill: Color32::from_rgba_unmultiplied(220, 52, 52, 215),
                card_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 58),
                card_stroke: Color32::from_rgba_unmultiplied(20, 32, 56, 16),
                control_fill: Color32::from_rgba_unmultiplied(24, 36, 58, 13),
                control_hover_fill: Color32::from_rgba_unmultiplied(24, 36, 58, 24),
                control_stroke: Color32::from_rgba_unmultiplied(20, 32, 56, 22),
                input_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 115),
                accent_fill: Color32::from_rgb(36, 108, 232),
                accent_hover_fill: Color32::from_rgb(28, 94, 214),
                accent_text_on_fill: Color32::from_rgb(252, 253, 255),
                selection_fill: Color32::from_rgba_unmultiplied(42, 112, 232, 90),
                success_text: Color32::from_rgb(22, 134, 74),
                segment_track_fill: Color32::from_rgba_unmultiplied(18, 28, 48, 14),
                segment_active_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 128),
                toggle_off_fill: Color32::from_rgba_unmultiplied(24, 36, 58, 38),
                toggle_knob_fill: Color32::from_rgb(252, 253, 255),
            }
        } else {
            Self {
                is_light: false,
                primary_text: Color32::from_rgb(250, 252, 255),
                secondary_text: Color32::from_rgba_unmultiplied(228, 236, 250, 215),
                muted_text: Color32::from_rgba_unmultiplied(186, 198, 222, 195),
                accent_text: Color32::from_rgb(255, 108, 96),
                divider: Color32::from_rgba_unmultiplied(210, 220, 240, 12),
                item_hover_fill: Color32::from_rgba_unmultiplied(64, 132, 244, 185),
                item_hover_text: Color32::from_rgb(250, 252, 255),
                item_hover_secondary_text: Color32::from_rgba_unmultiplied(235, 242, 255, 225),
                danger_text: Color32::from_rgb(255, 108, 108),
                danger_hover_fill: Color32::from_rgba_unmultiplied(228, 62, 62, 195),
                card_fill: Color32::from_rgba_unmultiplied(10, 12, 18, 44),
                card_stroke: Color32::from_rgba_unmultiplied(210, 220, 240, 11),
                control_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 10),
                control_hover_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 18),
                control_stroke: Color32::from_rgba_unmultiplied(210, 220, 240, 14),
                input_fill: Color32::from_rgba_unmultiplied(8, 10, 16, 92),
                accent_fill: Color32::from_rgb(66, 136, 248),
                accent_hover_fill: Color32::from_rgb(86, 150, 252),
                accent_text_on_fill: Color32::from_rgb(252, 253, 255),
                selection_fill: Color32::from_rgba_unmultiplied(66, 136, 248, 135),
                success_text: Color32::from_rgb(112, 224, 156),
                segment_track_fill: Color32::from_rgba_unmultiplied(6, 8, 14, 72),
                segment_active_fill: Color32::from_rgba_unmultiplied(210, 222, 245, 16),
                toggle_off_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 26),
                toggle_knob_fill: Color32::from_rgb(252, 253, 255),
            }
        }
    }
}

/// Applies shared translucent glass visuals and spacing to an egui viewport `Context`.
pub fn apply_glass_visuals(ctx: &Context, style: &GlassStyle) {
    let palette = style.palette();
    let mut visuals = if style.is_light() {
        Visuals::light()
    } else {
        Visuals::dark()
    };

    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = style.fill_color();
    visuals.extreme_bg_color = palette.input_fill;
    visuals.faint_bg_color = palette.card_fill;
    visuals.override_text_color = Some(palette.primary_text);
    visuals.slider_trailing_fill = true;

    visuals.selection.bg_fill = palette.selection_fill;
    visuals.selection.stroke = Stroke::new(1.5_f32, palette.accent_fill);

    let radius = CornerRadius::same(8);

    visuals.widgets.noninteractive.bg_fill = palette.card_fill;
    visuals.widgets.noninteractive.weak_bg_fill = palette.card_fill;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.card_stroke);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.secondary_text);
    visuals.widgets.noninteractive.corner_radius = radius;

    visuals.widgets.inactive.bg_fill = palette.control_fill;
    visuals.widgets.inactive.weak_bg_fill = palette.control_fill;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.control_stroke);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.secondary_text);
    visuals.widgets.inactive.corner_radius = radius;

    visuals.widgets.hovered.bg_fill = palette.control_hover_fill;
    visuals.widgets.hovered.weak_bg_fill = palette.control_hover_fill;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.accent_fill);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, palette.primary_text);
    visuals.widgets.hovered.corner_radius = radius;

    visuals.widgets.active.bg_fill = palette.accent_fill;
    visuals.widgets.active.weak_bg_fill = palette.accent_fill;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent_fill);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, palette.accent_text_on_fill);
    visuals.widgets.active.corner_radius = radius;

    visuals.widgets.open.bg_fill = palette.control_hover_fill;
    visuals.widgets.open.weak_bg_fill = palette.control_hover_fill;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.control_stroke);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.primary_text);
    visuals.widgets.open.corner_radius = radius;

    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 7.0);
        s.spacing.button_padding = Vec2::new(11.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.scroll = egui::style::ScrollStyle::floating();
    });
}

/// Draws a translucent tinted glass fill layered over a Windows DWM Acrylic blur backdrop,
/// scaled by `alpha_scale` (`0.0..=1.0`).
pub fn draw_glass_surface_with_alpha(
    painter: &Painter,
    rect: Rect,
    style: &GlassStyle,
    alpha_scale: f32,
) {
    painter.rect_filled(
        rect,
        style.corner_radius_egui(),
        style.fill_color_with_alpha(alpha_scale),
    );
}

/// Draws a translucent tinted glass fill layered over a Windows DWM Acrylic blur backdrop.
pub fn draw_glass_surface(painter: &Painter, rect: Rect, style: &GlassStyle) {
    draw_glass_surface_with_alpha(painter, rect, style, 1.0);
}

/// Draws the glass surface when `mode` is `FrostedGlass`, skipping completely in `Transparent` mode.
pub fn draw_glass_background(
    painter: &Painter,
    rect: Rect,
    mode: BackgroundMode,
    style: &GlassStyle,
) {
    if mode == BackgroundMode::Transparent {
        return;
    }
    draw_glass_surface(painter, rect, style);
}

/// Tracks and deduplicates Windows DWM Acrylic backdrop synchronization for an individual native window.
#[derive(Debug, Default, Clone, Copy)]
pub struct GlassWindowBackdrop {
    last_hwnd: Option<isize>,
    last_mode: Option<BackgroundMode>,
    last_is_light: Option<bool>,
}

impl GlassWindowBackdrop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self) {
        self.last_mode = None;
        self.last_is_light = None;
    }

    pub fn reset(&mut self) {
        self.last_hwnd = None;
        self.last_mode = None;
        self.last_is_light = None;
    }

    pub fn sync(&mut self, hwnd: Option<isize>, mode: BackgroundMode, is_light: bool) {
        let Some(raw) = hwnd else {
            return;
        };
        let style_dirty = crate::dwm::needs_caption_suppression(Some(raw));
        if style_dirty
            || self.last_hwnd != Some(raw)
            || self.last_mode != Some(mode)
            || self.last_is_light != Some(is_light)
        {
            sync_window_blur(Some(raw), mode, is_light);
            self.last_hwnd = Some(raw);
            self.last_mode = Some(mode);
            self.last_is_light = Some(is_light);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glass_style_opacity_clamping_and_fill() {
        let low = GlassStyle::new(GlassTintPreset::Dark, 0.0, false);
        assert_eq!(low.opacity, 0.05);
        assert_eq!(low.fill_color(), Color32::from_rgba_unmultiplied(20, 22, 28, 13));

        let high = low.with_opacity(2.5);
        assert_eq!(high.opacity, 0.98);
        assert_eq!(
            high.fill_color(),
            Color32::from_rgba_unmultiplied(20, 22, 28, 250)
        );
    }

    #[test]
    fn test_glass_style_theme_resolution() {
        assert!(!GlassStyle::new(GlassTintPreset::Dark, 0.5, true).is_light());
        assert!(GlassStyle::new(GlassTintPreset::Light, 0.5, false).is_light());
        assert!(!GlassStyle::new(GlassTintPreset::System, 0.5, false).is_light());

        let sys_light = GlassStyle::new(GlassTintPreset::System, 0.5, true);
        assert!(sys_light.is_light());
        assert_eq!(
            sys_light.fill_color(),
            Color32::from_rgba_unmultiplied(240, 244, 250, 128)
        );
    }

    #[test]
    fn test_glass_palette_light_vs_dark() {
        let light = GlassPalette::for_theme(true);
        let dark = GlassPalette::for_theme(false);

        assert!(light.is_light);
        assert!(!dark.is_light);
        assert_ne!(light.primary_text, dark.primary_text);
        assert_ne!(light.secondary_text, dark.secondary_text);
        assert_ne!(light.muted_text, dark.muted_text);
        assert_ne!(light.danger_text, dark.danger_text);
        assert_ne!(light.divider, dark.divider);
        assert_ne!(light.card_fill, dark.card_fill);
        assert_ne!(light.card_stroke, dark.card_stroke);
        assert_ne!(light.control_fill, dark.control_fill);
        assert_ne!(light.input_fill, dark.input_fill);
        assert_ne!(light.segment_track_fill, dark.segment_track_fill);
        assert_ne!(light.segment_active_fill, dark.segment_active_fill);
        assert_ne!(light.selection_fill, dark.selection_fill);
        assert_ne!(light.success_text, dark.success_text);
    }

    #[test]
    fn test_glass_style_fill_color_with_alpha() {
        let style = GlassStyle::new(GlassTintPreset::Dark, 0.5, false);

        assert_eq!(
            style.fill_color_with_alpha(0.0),
            Color32::from_rgba_unmultiplied(20, 22, 28, 0)
        );
        assert_eq!(style.fill_color_with_alpha(1.0), style.fill_color());
        assert_eq!(
            style.fill_color_with_alpha(0.5),
            Color32::from_rgba_unmultiplied(20, 22, 28, 64)
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_aliases_and_roundtrip() {
        let dark: GlassTintPreset = serde_json::from_str(r#""MidnightDark""#).unwrap();
        assert_eq!(dark, GlassTintPreset::Dark);
        let dark2: GlassTintPreset = serde_json::from_str(r#""CharcoalSlate""#).unwrap();
        assert_eq!(dark2, GlassTintPreset::Dark);
        let dark3: GlassTintPreset = serde_json::from_str(r#""OceanBlue""#).unwrap();
        assert_eq!(dark3, GlassTintPreset::Dark);

        let light: GlassTintPreset = serde_json::from_str(r#""FrostLight""#).unwrap();
        assert_eq!(light, GlassTintPreset::Light);

        let sys: GlassTintPreset = serde_json::from_str(r#""FollowWindows""#).unwrap();
        assert_eq!(sys, GlassTintPreset::System);
        let sys2: GlassTintPreset = serde_json::from_str(r#""Auto""#).unwrap();
        assert_eq!(sys2, GlassTintPreset::System);

        let bg_json = serde_json::to_string(&BackgroundMode::Transparent).unwrap();
        let bg: BackgroundMode = serde_json::from_str(&bg_json).unwrap();
        assert_eq!(bg, BackgroundMode::Transparent);
    }
}
