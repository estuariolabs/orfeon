//! Carta de vocales: un punto (x, y) elige la postura de la boca.
//! x: 0 anterior (i, e) .. 1 posterior (u, o).  y: 0 cerrada (i, u) .. 1 abierta (a).
//! La postura sale de mezclar las vocales calibradas de CHIMERA más cercanas al punto.

use chimera::anatomy::human::{ArticPatch, BodyMix};
use chimera::phonetics::planner::vowel_artic;

/// Vocales de anclaje y su lugar en la carta.
pub const ANCHORS: [(&str, f64, f64); 9] = [
    ("i", 0.00, 0.00),
    ("e", 0.08, 0.40),
    ("ɛ", 0.16, 0.70),
    ("a", 0.45, 1.00),
    ("ɑ", 0.78, 1.00),
    ("ɔ", 0.88, 0.68),
    ("o", 0.93, 0.40),
    ("u", 1.00, 0.00),
    ("ə", 0.50, 0.50),
];

/// Punto de la carta para las cinco vocales del español.
pub fn spanish(vowel: char) -> (f64, f64) {
    match vowel.to_ascii_lowercase() {
        'e' => (0.08, 0.40),
        'i' => (0.00, 0.00),
        'o' => (0.93, 0.40),
        'u' => (1.00, 0.00),
        _ => (0.45, 1.00),
    }
}

/// Las posturas de anclaje de un cuerpo, calculadas una vez al crear el cantante.
#[derive(Clone, Copy, Debug)]
pub struct VowelSpace {
    patches: [ArticPatch; 9],
}

impl VowelSpace {
    pub fn new(body: BodyMix) -> VowelSpace {
        let mut patches = [ArticPatch::default(); 9];
        for (i, (ipa, _, _)) in ANCHORS.iter().enumerate() {
            patches[i] = vowel_artic(ipa, body).unwrap_or_default();
        }
        VowelSpace { patches }
    }

    /// Postura en el punto (x, y). No reserva memoria.
    pub fn at(&self, x: f64, y: f64) -> ArticPatch {
        let x = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.5 };
        let y = if y.is_finite() { y.clamp(0.0, 1.0) } else { 0.5 };
        let mut parts = [(0.0, ArticPatch::default()); 9];
        for (i, (_, ax, ay)) in ANCHORS.iter().enumerate() {
            let d2 = (x - ax) * (x - ax) + (y - ay) * (y - ay);
            // peso que cae rápido: sobre un ancla manda esa vocal, entre dos se mezclan
            let w = 1.0 / (d2 * d2 + 1e-5);
            parts[i] = (w, self.patches[i]);
        }
        ArticPatch::blend(&parts)
    }
}
