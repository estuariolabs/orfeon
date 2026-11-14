//! Programas de fábrica: 37 puntos de partida. Son combinaciones elegidas a mano de los
//! mismos parámetros que se pueden mover; ninguno fue ajustado contra grabaciones.

use crate::choir::{ChoirParams, Mode, Quality};
use crate::room::RoomKind;
use crate::singer::{Ensemble, Face};
use crate::style::Style;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub name: &'static str,
    pub params: ChoirParams,
}

const D: ChoirParams = ChoirParams::DEFAULT;
const NO_FACE: Face = Face { neck: 0.0, size: 0.0, jaw: 0.0, lips: 0.0, nose: 0.0, age: 0.0 };
// vocales en la carta
const A: (f64, f64) = (0.45, 1.00);
const E: (f64, f64) = (0.08, 0.40);
const I: (f64, f64) = (0.00, 0.00);
const O: (f64, f64) = (0.93, 0.40);
const U: (f64, f64) = (1.00, 0.00);

const fn p(name: &'static str, mode: Mode, ensemble: Ensemble, singers: usize, style: Style, vowel: (f64, f64), room: RoomKind, mic: f64) -> Preset {
    Preset { name, params: ChoirParams { mode, ensemble, singers, style, vowel_x: vowel.0, vowel_y: vowel.1, room, mic, max_voices: if singers < 8 { singers } else { 8 }, ..D } }
}

pub const PRESETS: [Preset; 37] = [
    // --- coros mixtos ---
    p("Coro de cámara", Mode::Coro, Ensemble::Mixto, 8, Style::Clasico, A, RoomKind::Sala, 0.35),
    p("Coro sinfónico", Mode::Coro, Ensemble::Mixto, 32, Style::Clasico, A, RoomKind::Sala, 0.55),
    p("Catedral", Mode::Coro, Ensemble::Mixto, 24, Style::Antiguo, U, RoomKind::Catedral, 0.7),
    p("Motete", Mode::Coro, Ensemble::Mixto, 12, Style::Antiguo, O, RoomKind::Iglesia, 0.5),
    p("Cuarteto vocal", Mode::Coro, Ensemble::Mixto, 4, Style::Clasico, A, RoomKind::Estudio, 0.15),
    p("Coro pop", Mode::Coro, Ensemble::Mixto, 8, Style::Pop, A, RoomKind::Estudio, 0.2),
    p("Coro gospel", Mode::Coro, Ensemble::Mixto, 16, Style::Gospel, A, RoomKind::Iglesia, 0.4),
    p("Peña folclórica", Mode::Coro, Ensemble::Mixto, 8, Style::Folk, A, RoomKind::Sala, 0.3),
    // --- voces femeninas ---
    p("Coro femenino", Mode::Coro, Ensemble::Femenino, 12, Style::Clasico, A, RoomKind::Sala, 0.4),
    p("Sopranos lejanas", Mode::Coro, Ensemble::Sopranos, 8, Style::Antiguo, U, RoomKind::Catedral, 0.9),
    p("Contraltos cálidas", Mode::Coro, Ensemble::Contraltos, 6, Style::Clasico, O, RoomKind::Sala, 0.35),
    p("Trío pop femenino", Mode::Coro, Ensemble::Femenino, 3, Style::Pop, A, RoomKind::Estudio, 0.1),
    p("Susurros", Mode::Coro, Ensemble::Femenino, 6, Style::Susurro, A, RoomKind::Estudio, 0.05),
    Preset { name: "Voces búlgaras", params: ChoirParams { brightness: 0.25, vibrato: 0.2, ..p("", Mode::Coro, Ensemble::Femenino, 9, Style::Folk, E, RoomKind::Sala, 0.3).params } },
    // --- voces masculinas ---
    p("Coro masculino", Mode::Coro, Ensemble::Masculino, 12, Style::Clasico, A, RoomKind::Sala, 0.4),
    p("Monjes", Mode::Coro, Ensemble::Bajos, 10, Style::Antiguo, O, RoomKind::Catedral, 0.75),
    p("Tenores", Mode::Coro, Ensemble::Tenores, 6, Style::Clasico, A, RoomKind::Sala, 0.35),
    p("Bajos profundos", Mode::Coro, Ensemble::Bajos, 6, Style::Clasico, U, RoomKind::Iglesia, 0.5),
    p("Cuarteto de barbería", Mode::Coro, Ensemble::Masculino, 4, Style::Pop, A, RoomKind::Estudio, 0.15),
    Preset { name: "Coro de taberna", params: ChoirParams { humanize: 1.0, variation: 1.0, ..p("", Mode::Coro, Ensemble::Masculino, 9, Style::Belting, A, RoomKind::Estudio, 0.3).params } },
    // --- niños ---
    p("Niños cantores", Mode::Coro, Ensemble::Ninos, 12, Style::Antiguo, A, RoomKind::Iglesia, 0.5),
    p("Coro escolar", Mode::Coro, Ensemble::Ninos, 16, Style::Folk, A, RoomKind::Sala, 0.35),
    Preset { name: "Voces blancas lejanas", params: ChoirParams { breath: 0.15, ..p("", Mode::Coro, Ensemble::Ninos, 8, Style::Antiguo, U, RoomKind::Catedral, 1.0).params } },
    // --- tutti: todos al unísono ---
    p("Tutti mixto", Mode::Tutti, Ensemble::Mixto, 8, Style::Clasico, A, RoomKind::Sala, 0.45),
    p("Unísono gregoriano", Mode::Tutti, Ensemble::Masculino, 6, Style::Antiguo, O, RoomKind::Catedral, 0.7),
    p("Unísono femenino", Mode::Tutti, Ensemble::Femenino, 6, Style::Clasico, A, RoomKind::Iglesia, 0.5),
    p("Tutti gospel", Mode::Tutti, Ensemble::Mixto, 8, Style::Gospel, A, RoomKind::Iglesia, 0.4),
    p("Hinchada", Mode::Tutti, Ensemble::Masculino, 8, Style::Belting, O, RoomKind::Sala, 0.6),
    // --- solistas ---
    p("Soprano solista", Mode::Solo, Ensemble::Sopranos, 1, Style::Clasico, A, RoomKind::Sala, 0.25),
    p("Tenor solista", Mode::Solo, Ensemble::Tenores, 1, Style::Clasico, A, RoomKind::Sala, 0.25),
    p("Bajo solista", Mode::Solo, Ensemble::Bajos, 1, Style::Clasico, O, RoomKind::Sala, 0.25),
    p("Contralto solista", Mode::Solo, Ensemble::Contraltos, 1, Style::Clasico, A, RoomKind::Sala, 0.25),
    p("Voz pop", Mode::Solo, Ensemble::Femenino, 1, Style::Pop, A, RoomKind::Estudio, 0.05),
    p("Cantor rasgado", Mode::Solo, Ensemble::Masculino, 1, Style::Rasgado, A, RoomKind::Sala, 0.2),
    // --- con movimiento ---
    Preset { name: "Vocales que giran", params: ChoirParams { vowel_x: 0.5, vowel_y: 0.5, vowel_move: 1.0, vowel_rate: 0.15, ..p("", Mode::Coro, Ensemble::Mixto, 8, Style::Antiguo, A, RoomKind::Iglesia, 0.5).params } },
    Preset { name: "Multitud", params: ChoirParams { humanize: 1.0, variation: 1.0, vowel_spread: 1.0, face: NO_FACE, quality: Quality::Eco, ..p("", Mode::Coro, Ensemble::Mixto, 64, Style::Folk, I, RoomKind::Sala, 0.8).params } },
    Preset { name: "Cuarteto coral", params: ChoirParams {
        voice_leading: true, stagger_ms: 24.0, humanize: 0.35, vowel_spread: 0.2,
        room_mix: 0.18, ..p("", Mode::Coro, Ensemble::Mixto, 4, Style::Clasico, A, RoomKind::Estudio, 0.25).params
    } },
];

/// Busca un programa por nombre (sin distinguir mayúsculas).
pub fn find(name: &str) -> Option<&'static Preset> {
    let n = name.to_lowercase();
    PRESETS.iter().find(|p| p.name.to_lowercase() == n)
}
