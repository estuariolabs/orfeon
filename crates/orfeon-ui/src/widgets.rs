//! Controles dibujados a mano: carta de vocales, escenario con la sala, teclado.

use crate::theme::{mix, voice_color, BG, DIM, GOLD, LINE, PANEL_HI, SING, TEXT};
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};
use orfeon_core::singer::{Singer, VoiceType};
use orfeon_core::vowel::ANCHORS;
use orfeon_core::{ChoirParams, RoomKind};

// ------------------------------------------------------------------ carta de vocales

/// Carta de vocales (como la de CHIMERA y la del AFI): arriba las cerradas, abajo las
/// abiertas; a la izquierda las anteriores, a la derecha las posteriores. Arrastrar el punto
/// elige la vocal. Devuelve si cambió.
pub fn vowel_pad(ui: &mut Ui, size: egui::Vec2, p: &mut ChoirParams, phase: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, BG);
    let inner = egui::Rect::from_min_max(rect.min + vec2(64.0, 34.0), rect.max - vec2(22.0, 22.0));
    // trapecio vocálico
    let q = |x: f32, y: f32| pos2(inner.left() + inner.width() * (x * (1.0 - 0.28 * y) + 0.28 * y * 0.5 + 0.0), inner.top() + inner.height() * y);
    let tl = q(0.0, 0.0);
    let tr = q(1.0, 0.0);
    let br = q(1.0, 1.0);
    let bl = q(0.0, 1.0);
    painter.add(Shape::closed_line(vec![tl, tr, br, bl], Stroke::new(1.0_f32, LINE)));
    for k in 1..3 {
        let y = k as f32 / 3.0;
        painter.line_segment([q(0.0, y), q(1.0, y)], Stroke::new(0.5_f32, LINE));
    }
    painter.line_segment([q(0.5, 0.0), q(0.5, 1.0)], Stroke::new(0.5_f32, LINE));
    painter.text(pos2(inner.left(), rect.top() + 8.0), Align2::LEFT_TOP, "anterior", FontId::proportional(11.0), DIM);
    painter.text(pos2(inner.right(), rect.top() + 8.0), Align2::RIGHT_TOP, "posterior", FontId::proportional(11.0), DIM);
    painter.text(pos2(rect.left() + 8.0, inner.top()), Align2::LEFT_CENTER, "cerrada", FontId::proportional(11.0), DIM);
    painter.text(pos2(rect.left() + 8.0, inner.bottom()), Align2::LEFT_CENTER, "abierta", FontId::proportional(11.0), DIM);
    // anclas: las vocales calibradas del motor (puntos) y las cinco del español (letras)
    for (_, x, y) in ANCHORS.iter() {
        painter.circle_filled(q(*x as f32, *y as f32), 3.0, LINE);
    }
    for v in ['a', 'e', 'i', 'o', 'u'] {
        let (x, y) = orfeon_core::vowel::spanish(v);
        let c = q(x as f32, y as f32);
        // la letra hacia adentro de la carta, para no tapar el borde
        let off = vec2(if x < 0.3 { 16.0 } else if x > 0.7 { -16.0 } else { 0.0 }, if y > 0.8 { -18.0 } else { 14.0 });
        painter.text(c + off, Align2::CENTER_CENTER, v.to_string(), FontId::proportional(18.0), TEXT);
    }
    // recorrido del movimiento automático
    let mv = p.vowel_move.clamp(0.0, 1.0) as f32 * 0.4;
    let (vx, vy) = (p.vowel_x as f32, p.vowel_y as f32);
    if mv > 0.001 {
        let pts: Vec<Pos2> = (0..48).map(|k| {
            let a = std::f32::consts::TAU * k as f32 / 48.0;
            q((vx + mv * a.cos()).clamp(0.0, 1.0), (vy + mv * a.sin()).clamp(0.0, 1.0))
        }).collect();
        painter.add(Shape::closed_line(pts, Stroke::new(1.0_f32, mix(GOLD, BG, 0.5))));
        let a = std::f32::consts::TAU * phase;
        painter.circle_filled(q((vx + mv * a.cos()).clamp(0.0, 1.0), (vy + mv * a.sin()).clamp(0.0, 1.0)), 4.0, SING);
    }
    // el punto
    let c = q(vx, vy);
    painter.circle_filled(c, 9.0, mix(GOLD, BG, 0.3));
    painter.circle_stroke(c, 9.0, Stroke::new(2.0_f32, GOLD));
    // arrastrar: de pantalla a carta (invierte el trapecio)
    let mut changed = false;
    if let Some(pos) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
            let y = ((pos.y - inner.top()) / inner.height()).clamp(0.0, 1.0);
            let left = q(0.0, y).x;
            let right = q(1.0, y).x;
            let x = ((pos.x - left) / (right - left).max(1.0)).clamp(0.0, 1.0);
            p.vowel_x = x as f64;
            p.vowel_y = y as f64;
            changed = true;
        }
    }
    changed
}

// ------------------------------------------------------------------ escenario

/// Escenario visto desde la platea: los cantantes en su lugar (cuerdas de izquierda a
/// derecha, graves atrás), la sala de fondo y el micrófono. Click en un cantante lo elige.
pub fn stage(ui: &mut Ui, size: egui::Vec2, p: &ChoirParams, levels: &[f32], selected: &mut usize) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let room = p.room;
    let wall = match room {
        RoomKind::Seco => Color32::from_rgb(22, 23, 28),
        RoomKind::Estudio => Color32::from_rgb(34, 36, 44),
        RoomKind::Sala => Color32::from_rgb(52, 34, 32),
        RoomKind::Iglesia => Color32::from_rgb(58, 52, 44),
        RoomKind::Catedral => Color32::from_rgb(46, 46, 54),
    };
    painter.rect_filled(rect, 8.0, wall);
    let horizon = rect.top() + rect.height() * 0.42;
    let front = rect.bottom() - rect.height() * 0.06;
    // fondo de la sala
    match room {
        RoomKind::Seco => {}
        RoomKind::Estudio => {
            for k in 0..6 {
                let x = rect.left() + rect.width() * (0.12 + 0.13 * k as f32);
                painter.rect_filled(Rect::from_min_size(pos2(x, rect.top() + 18.0), vec2(rect.width() * 0.09, horizon - rect.top() - 30.0)), 3.0, Color32::from_rgb(44, 48, 60));
            }
        }
        RoomKind::Sala => {
            for k in 0..3 {
                let y = rect.top() + 16.0 + k as f32 * 22.0;
                painter.line_segment([pos2(rect.left() + 10.0, y), pos2(rect.right() - 10.0, y)], Stroke::new(2.0_f32, Color32::from_rgb(110, 70, 52)));
            }
            painter.rect_filled(Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(rect.left() + 16.0, horizon)), 0.0, Color32::from_rgb(120, 30, 36));
            painter.rect_filled(Rect::from_min_max(pos2(rect.right() - 16.0, rect.top()), pos2(rect.right(), horizon)), 0.0, Color32::from_rgb(120, 30, 36));
        }
        RoomKind::Iglesia | RoomKind::Catedral => {
            let tall = room == RoomKind::Catedral;
            let n = if tall { 5 } else { 3 };
            for k in 0..n {
                let w = rect.width() / (n as f32 + 1.0) * 0.45;
                let x = rect.left() + rect.width() * (k as f32 + 1.0) / (n as f32 + 1.0);
                let top = rect.top() + if tall { 14.0 } else { 34.0 };
                let bottom = horizon - 6.0;
                // ventana en arco ojival
                let mut pts = vec![pos2(x - w / 2.0, bottom), pos2(x - w / 2.0, top + w)];
                for j in 0..=10 {
                    let a = j as f32 / 10.0;
                    pts.push(pos2(x - w / 2.0 + w * a, top + w - (1.0 - (2.0 * a - 1.0).powi(2)).sqrt() * w));
                }
                pts.push(pos2(x + w / 2.0, bottom));
                painter.add(Shape::closed_line(pts, Stroke::new(1.5_f32, Color32::from_rgb(150, 136, 110))));
            }
            if tall {
                let c = pos2(rect.center().x, rect.top() + 44.0);
                painter.circle_stroke(c, 22.0, Stroke::new(1.5_f32, Color32::from_rgb(150, 136, 110)));
                for k in 0..8 {
                    let a = std::f32::consts::TAU * k as f32 / 8.0;
                    painter.line_segment([c, c + vec2(a.cos(), a.sin()) * 22.0], Stroke::new(0.8_f32, Color32::from_rgb(150, 136, 110)));
                }
            }
        }
    }
    // piso en perspectiva
    let floor = vec![pos2(rect.left() + rect.width() * 0.22, horizon), pos2(rect.right() - rect.width() * 0.22, horizon), pos2(rect.right(), front + 20.0), pos2(rect.left(), front + 20.0)];
    painter.add(Shape::convex_polygon(floor, mix(wall, Color32::BLACK, 0.35), Stroke::NONE));

    // cantantes: atrás primero
    let n = p.singers.clamp(1, 64);
    let width = p.width.clamp(0.0, 1.0) as f32 * (1.0 - 0.45 * p.mic.clamp(0.0, 1.0) as f32);
    let mut order: Vec<(usize, Singer)> = (0..n).map(|i| (i, p.singer(i))).collect();
    order.sort_by(|a, b| b.1.depth.partial_cmp(&a.1.depth).unwrap_or(std::cmp::Ordering::Equal));
    let place = |s: &Singer| {
        let d = (s.depth as f32 * p.depth.clamp(0.0, 1.0) as f32).clamp(0.0, 1.0);
        let y = front - (front - horizon) * (0.15 + 0.75 * d);
        let half = rect.width() * (0.5 - 0.22 * (0.15 + 0.75 * d)) * 0.92;
        let x = rect.center().x + s.pan as f32 * width * half;
        let scale = 1.0 - 0.55 * (0.15 + 0.75 * d);
        (pos2(x, y), scale)
    };
    let mut hit: Option<usize> = None;
    let click = resp.interact_pointer_pos().filter(|_| resp.clicked());
    for (i, s) in order.iter() {
        let (pos, scale) = place(s);
        let h = (rect.height() * 0.26) * scale * s.body.scale as f32;
        let w = h * 0.36;
        let lvl = levels.get(*i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let robe = voice_color(s.voice_type);
        if lvl > 0.01 {
            painter.circle_filled(pos - vec2(0.0, h * 0.55), h * 0.55, Color32::from_rgba_unmultiplied(SING.r(), SING.g(), SING.b(), (lvl * 70.0) as u8));
        }
        painter.add(Shape::convex_polygon(vec![pos2(pos.x - w * 0.35, pos.y - h * 0.78), pos2(pos.x + w * 0.35, pos.y - h * 0.78), pos2(pos.x + w * 0.55, pos.y), pos2(pos.x - w * 0.55, pos.y)], robe, Stroke::NONE));
        painter.circle_filled(pos2(pos.x, pos.y - h * 0.9), h * 0.13, Color32::from_rgb(222, 186, 150));
        if *i == *selected {
            painter.rect_stroke(Rect::from_min_max(pos2(pos.x - w * 0.7, pos.y - h * 1.08), pos2(pos.x + w * 0.7, pos.y + 2.0)), 3.0, Stroke::new(1.5_f32, GOLD), StrokeKind::Outside);
        }
        if let Some(c) = click {
            if (c.x - pos.x).abs() < w * 0.6 && c.y > pos.y - h * 1.05 && c.y < pos.y + 2.0 {
                hit = Some(*i);
            }
        }
    }
    if let Some(i) = hit {
        *selected = i;
    }
    // micrófono: pegado al coro (0) o lejos, cerca del público (1)
    let m = p.mic.clamp(0.0, 1.0) as f32;
    let my = front - (front - horizon) * 0.05 + 0.0 * m;
    let mx = rect.center().x;
    let mic_y = my - (1.0 - m) * (front - horizon) * 0.05 + m * 10.0;
    let ms = 6.0 + 6.0 * m;
    painter.line_segment([pos2(mx, mic_y), pos2(mx, rect.bottom() - 2.0)], Stroke::new(1.5_f32, Color32::from_rgb(160, 160, 166)));
    painter.add(Shape::convex_polygon(super::widgets::capsule(pos2(mx, mic_y - ms), ms * 0.55, ms), Color32::from_rgb(190, 190, 196), Stroke::NONE));
    painter.text(pos2(rect.left() + 8.0, rect.bottom() - 6.0), Align2::LEFT_BOTTOM, room.name(), FontId::proportional(12.0), TEXT);
    resp
}

/// Contorno de una cápsula vertical (el micrófono).
pub fn capsule(c: Pos2, rx: f32, ry: f32) -> Vec<Pos2> {
    (0..20).map(|k| {
        let a = std::f32::consts::TAU * k as f32 / 20.0;
        pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
    }).collect()
}

// ------------------------------------------------------------------ teclado

pub const KEY_LO: u8 = 36; // C2
pub const KEY_HI: u8 = 96; // C7

fn is_black(k: u8) -> bool {
    matches!(k % 12, 1 | 3 | 6 | 8 | 10)
}

/// Teclado de C2 a C7. Arriba, bandas con el registro de cada tipo de voz del coro. Devuelve
/// la nota que el mouse aprieta ahora (si alguna).
pub fn keyboard(ui: &mut Ui, size: egui::Vec2, held: &[bool; 128], types: &[VoiceType]) -> Option<u8> {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, BG);
    let band_h = 4.0 * types.len().max(1) as f32 + 6.0;
    let keys_rect = Rect::from_min_max(pos2(rect.left(), rect.top() + band_h), rect.max);
    let whites: Vec<u8> = (KEY_LO..=KEY_HI).filter(|k| !is_black(*k)).collect();
    let ww = keys_rect.width() / whites.len() as f32;
    let white_x = |k: u8| whites.iter().position(|w| *w == k).map(|i| keys_rect.left() + i as f32 * ww);
    let key_rect = |k: u8| -> Rect {
        if is_black(k) {
            let x = white_x(k - 1).unwrap_or(keys_rect.left()) + ww * 0.68;
            Rect::from_min_size(pos2(x, keys_rect.top()), vec2(ww * 0.64, keys_rect.height() * 0.6))
        } else {
            Rect::from_min_size(pos2(white_x(k).unwrap_or(keys_rect.left()), keys_rect.top()), vec2(ww, keys_rect.height()))
        }
    };
    // registros
    for (j, t) in types.iter().enumerate() {
        let (lo, hi) = t.range();
        let a = key_rect(lo.max(KEY_LO)).left();
        let b = key_rect(hi.min(KEY_HI)).right();
        let y = rect.top() + 3.0 + j as f32 * 4.0;
        painter.rect_filled(Rect::from_min_max(pos2(a, y), pos2(b, y + 3.0)), 1.0, voice_color(*t));
    }
    // qué tecla está bajo el mouse (las negras primero, están arriba)
    let pointer = resp.interact_pointer_pos().filter(|_| resp.is_pointer_button_down_on());
    let mut pressed = None;
    if let Some(pos) = pointer {
        for k in KEY_LO..=KEY_HI {
            if is_black(k) && key_rect(k).contains(pos) {
                pressed = Some(k);
            }
        }
        if pressed.is_none() {
            for k in KEY_LO..=KEY_HI {
                if !is_black(k) && key_rect(k).contains(pos) {
                    pressed = Some(k);
                }
            }
        }
    }
    let on = |k: u8| held[k as usize] || pressed == Some(k);
    for k in (KEY_LO..=KEY_HI).filter(|k| !is_black(*k)) {
        let r = key_rect(k).shrink2(vec2(0.5, 0.0));
        painter.rect_filled(r, 2.0, if on(k) { GOLD } else { Color32::from_rgb(232, 230, 224) });
        if k % 12 == 0 {
            painter.text(pos2(r.center().x, r.bottom() - 4.0), Align2::CENTER_BOTTOM, format!("C{}", k as i32 / 12 - 1), FontId::proportional(10.0), Color32::from_rgb(110, 110, 116));
        }
    }
    for k in (KEY_LO..=KEY_HI).filter(|k| is_black(*k)) {
        painter.rect_filled(key_rect(k), 2.0, if on(k) { mix(GOLD, Color32::BLACK, 0.2) } else { Color32::from_rgb(26, 26, 30) });
    }
    painter.rect_stroke(keys_rect, 2.0, Stroke::new(1.0_f32, PANEL_HI), StrokeKind::Inside);
    pressed
}

/// Barra de nivel horizontal (carga de CPU).
pub fn meter(ui: &mut Ui, w: f32, value: f32, label: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 16.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, BG);
    let v = value.clamp(0.0, 1.0);
    let c = if value > 0.9 { crate::theme::WARN } else if value > 0.7 { GOLD } else { SING };
    p.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width() * v, rect.height())), 3.0, mix(c, BG, 0.35));
    p.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.0), TEXT);
}
