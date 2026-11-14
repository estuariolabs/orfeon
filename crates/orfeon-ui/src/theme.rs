//! Colores y estilo de Orfeón: fondo carbón, acento dorado.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use orfeon_core::singer::VoiceType;

pub const BG: Color32 = Color32::from_rgb(18, 19, 23);
pub const PANEL: Color32 = Color32::from_rgb(27, 29, 35);
pub const PANEL_HI: Color32 = Color32::from_rgb(38, 41, 50);
pub const LINE: Color32 = Color32::from_rgb(58, 62, 74);
pub const TEXT: Color32 = Color32::from_rgb(222, 220, 212);
pub const DIM: Color32 = Color32::from_rgb(140, 140, 150);
pub const GOLD: Color32 = Color32::from_rgb(222, 170, 72);
pub const GOLD_DIM: Color32 = Color32::from_rgb(120, 92, 42);
pub const SING: Color32 = Color32::from_rgb(110, 210, 190);
pub const WARN: Color32 = Color32::from_rgb(230, 110, 90);

/// Color de cada tipo de voz (togas, marcas del teclado).
pub fn voice_color(v: VoiceType) -> Color32 {
    match v {
        VoiceType::Soprano => Color32::from_rgb(196, 86, 112),
        VoiceType::Mezzo => Color32::from_rgb(206, 120, 80),
        VoiceType::Contralto => Color32::from_rgb(196, 156, 70),
        VoiceType::Tenor => Color32::from_rgb(70, 160, 150),
        VoiceType::Baritono => Color32::from_rgb(76, 120, 186),
        VoiceType::Bajo => Color32::from_rgb(110, 88, 176),
        VoiceType::Nino => Color32::from_rgb(110, 170, 90),
    }
}

/// Mezcla lineal de dos colores.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = PANEL_HI;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = GOLD_DIM;
    v.selection.stroke = Stroke::new(1.0_f32, GOLD);
    v.hyperlink_color = GOLD;
    let r = CornerRadius::same(4);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = r;
    }
    // bg_fill es también el riel de los deslizadores: tiene que verse sobre los paneles
    v.widgets.inactive.bg_fill = Color32::from_rgb(64, 68, 82);
    v.widgets.inactive.weak_bg_fill = PANEL_HI;
    v.widgets.hovered.bg_fill = Color32::from_rgb(52, 56, 68);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(52, 56, 68);
    v.widgets.active.bg_fill = GOLD_DIM;
    v.widgets.active.weak_bg_fill = GOLD_DIM;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.slider_width = 180.0;
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.text_styles.insert(TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(13.5, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
    });
}
