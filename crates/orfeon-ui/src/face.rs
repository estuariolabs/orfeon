//! Retrato de un cantante, dibujado a partir de su rostro y su cuerpo: cuello, tamaño,
//! mandíbula, labios, nariz y edad cambian el dibujo igual que cambian la voz. La boca se
//! abre con lo que canta y toma la forma de la vocal.

use crate::theme::{mix, voice_color, GOLD, LINE, PANEL_HI, SING};
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use orfeon_core::singer::{Singer, VoiceType};

const SKIN: [Color32; 5] = [
    Color32::from_rgb(236, 200, 170),
    Color32::from_rgb(214, 170, 132),
    Color32::from_rgb(178, 128, 92),
    Color32::from_rgb(130, 88, 60),
    Color32::from_rgb(96, 64, 46),
];
const HAIR: [Color32; 5] = [
    Color32::from_rgb(40, 30, 26),
    Color32::from_rgb(86, 56, 36),
    Color32::from_rgb(150, 92, 52),
    Color32::from_rgb(196, 160, 96),
    Color32::from_rgb(24, 22, 24),
];

fn ellipse(c: Pos2, r: egui::Vec2, n: usize) -> Vec<Pos2> {
    (0..n).map(|k| {
        let a = std::f32::consts::TAU * k as f32 / n as f32;
        pos2(c.x + r.x * a.cos(), c.y + r.y * a.sin())
    }).collect()
}

fn long_hair(v: VoiceType) -> bool {
    matches!(v, VoiceType::Soprano | VoiceType::Mezzo | VoiceType::Contralto)
}

/// Dibuja al cantante en `rect`. `level` 0..1 (cuánto canta), `vowel` = punto de la carta.
pub fn draw(p: &Painter, rect: Rect, s: &Singer, level: f32, vowel: (f32, f32), selected: bool) {
    let f = s.face;
    let u = rect.height() / 100.0;
    let cx = rect.center().x;
    let robe = voice_color(s.voice_type);
    let skin = SKIN[(s.seed % 5) as usize];
    let age = f.age as f32;
    let old = age.max(0.0);
    let child = (-age).max(0.0) + if s.voice_type == VoiceType::Nino { 0.6 } else { 0.0 };
    let hair = mix(HAIR[((s.seed >> 8) % 5) as usize], Color32::from_rgb(214, 214, 210), (old - 0.25).max(0.0) * 1.4);

    // fondo de la tarjeta: brilla con lo que canta
    let bg = mix(PANEL_HI, mix(PANEL_HI, SING, 0.45), level.clamp(0.0, 1.0));
    p.rect_filled(rect, 6.0, bg);

    // --- cuerpo (toga con cuello blanco) ---
    let shoulders = rect.top() + 78.0 * u;
    let sw = (34.0 + 6.0 * f.size as f32 - 6.0 * child) * u;
    p.add(Shape::convex_polygon(
        vec![pos2(cx - sw, shoulders + 6.0 * u), pos2(cx - sw * 0.7, shoulders), pos2(cx + sw * 0.7, shoulders), pos2(cx + sw, shoulders + 6.0 * u), pos2(cx + sw * 1.05, rect.bottom()), pos2(cx - sw * 1.05, rect.bottom())],
        mix(robe, Color32::BLACK, 0.15),
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(vec![pos2(cx - 9.0 * u, shoulders), pos2(cx + 9.0 * u, shoulders), pos2(cx, shoulders + 9.0 * u)], Color32::from_rgb(236, 234, 228), Stroke::NONE));

    // --- cuello: más largo, más arriba la cabeza ---
    let neck_len = (7.0 + 5.0 * f.neck as f32 - 2.0 * child) * u;
    let neck_w = (9.0 + 1.5 * f.size as f32 - 1.5 * child) * u;
    p.rect_filled(Rect::from_min_max(pos2(cx - neck_w, shoulders - neck_len - 4.0 * u), pos2(cx + neck_w, shoulders + 1.0 * u)), 2.0, mix(skin, Color32::BLACK, 0.12));

    // --- cabeza ---
    let rx = (16.0 + 2.0 * f.size as f32 + 1.8 * f.jaw as f32 + 2.0 * child) * u;
    let ry = (20.0 + 2.0 * f.size as f32 + 1.0 * child) * u;
    let hc = pos2(cx, shoulders - neck_len - ry + 2.0 * u);
    // pelo de atrás (largo en las voces femeninas)
    if long_hair(s.voice_type) {
        let hr = Rect::from_min_max(pos2(hc.x - rx * 1.18, hc.y - ry * 0.9), pos2(hc.x + rx * 1.18, hc.y + ry * 1.25));
        p.rect_filled(hr, egui::CornerRadius::same((rx * 0.9) as u8), hair);
    }
    // mandíbula: la parte de abajo de la cara se ensancha con `jaw`
    let jaw = 1.0 + 0.12 * f.jaw as f32;
    let mut head = Vec::new();
    for k in 0..40 {
        let a = std::f32::consts::TAU * k as f32 / 40.0;
        let lower = a.sin() > 0.0;
        let w = if lower { rx * (1.0 - (1.0 - jaw) * a.sin()) } else { rx };
        head.push(pos2(hc.x + w * a.cos(), hc.y + ry * a.sin()));
    }
    p.add(Shape::convex_polygon(head, skin, Stroke::new(1.0_f32, mix(skin, Color32::BLACK, 0.3))));
    // orejas
    for side in [-1.0f32, 1.0] {
        p.add(Shape::convex_polygon(ellipse(pos2(hc.x + side * rx * 0.98, hc.y + 1.0 * u), vec2(2.6 * u, 4.5 * u), 12), mix(skin, Color32::BLACK, 0.08), Stroke::NONE));
    }
    // pelo de arriba
    let top = ellipse(pos2(hc.x, hc.y - ry * 0.55), vec2(rx * 1.04, ry * 0.55), 28).into_iter().filter(|q| q.y <= hc.y - ry * 0.35).collect::<Vec<_>>();
    if top.len() > 2 {
        p.add(Shape::convex_polygon(top, hair, Stroke::NONE));
    }

    // --- ojos y cejas (más grandes en un niño) ---
    let eye_y = hc.y - 1.5 * u;
    let eye_dx = rx * 0.42;
    let er = (1.9 + 0.9 * child) * u;
    for side in [-1.0f32, 1.0] {
        let e = pos2(hc.x + side * eye_dx, eye_y);
        p.circle_filled(e, er * 1.5, Color32::from_rgb(245, 243, 238));
        p.circle_filled(e, er, Color32::from_rgb(52, 40, 32));
        p.line_segment([pos2(e.x - 3.5 * u, e.y - 4.5 * u - side * 0.0), pos2(e.x + 3.5 * u, e.y - 5.2 * u)], Stroke::new(1.4 * u, mix(hair, Color32::BLACK, 0.2)));
    }
    // arrugas de la edad
    if old > 0.35 {
        let w = Stroke::new(0.8 * u, mix(skin, Color32::BLACK, 0.35));
        for k in 0..2 {
            let y = hc.y - ry * 0.48 + k as f32 * 2.5 * u;
            p.line_segment([pos2(hc.x - rx * 0.4, y), pos2(hc.x + rx * 0.4, y)], w);
        }
    }

    // --- nariz: más larga y ancha con `nose` ---
    let nl = (4.5 + 3.5 * f.nose as f32) * u;
    let nw = (2.2 + 1.6 * f.nose as f32) * u;
    p.add(Shape::convex_polygon(vec![pos2(hc.x, eye_y + 1.0 * u), pos2(hc.x + nw, eye_y + 2.0 * u + nl), pos2(hc.x - nw, eye_y + 2.0 * u + nl)], mix(skin, Color32::BLACK, 0.16), Stroke::NONE));

    // --- boca: se abre con el canto y toma la forma de la vocal ---
    let my = eye_y + nl + 7.0 * u;
    let (vx, vy) = vowel;
    let open = level.clamp(0.0, 1.0).sqrt();
    let width = (5.0 + 3.5 * (1.0 - vx) * (1.0 - vy) + 2.0 * vy - 2.5 * vx * (1.0 - vy)) * u * (1.0 + 0.2 * f.lips as f32);
    let height = open * (1.5 + 6.5 * vy + 2.5 * vx * (1.0 - vy)) * u;
    let lip = mix(Color32::from_rgb(170, 80, 80), skin, 0.35);
    let lip_w = (1.0 + 0.8 * (f.lips as f32 + 1.0)) * u;
    if height > 0.6 * u {
        p.add(Shape::convex_polygon(ellipse(pos2(hc.x, my), vec2(width, height), 24), Color32::from_rgb(70, 24, 28), Stroke::new(lip_w, lip)));
    } else {
        p.line_segment([pos2(hc.x - width, my), pos2(hc.x + width, my)], Stroke::new(lip_w, lip));
    }

    // marco del seleccionado
    if selected {
        p.rect_stroke(rect, 6.0, Stroke::new(2.0_f32, GOLD), StrokeKind::Inside);
    } else {
        p.rect_stroke(rect, 6.0, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    }
}
