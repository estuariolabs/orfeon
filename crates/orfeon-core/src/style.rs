//! Estilos de canto: maneras de usar la misma voz (presión, cierre de las cuerdas, vibrato,
//! ataque). Son puntos de partida elegidos a mano; no salen de mediciones.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// lírico: vibrato amplio, voz plena
    Clasico,
    /// música antigua / coro inglés: casi sin vibrato, voz recta
    Antiguo,
    /// pop: voz hablada, vibrato tardío y chico
    Pop,
    /// gospel: voz apretada, ataque desde abajo, vibrato ancho
    Gospel,
    /// folclórico: voz abierta y brillante, sin adorno
    Folk,
    /// susurrado: mucho aire, poca presión
    Susurro,
    /// belting: pecho arriba, mucha presión y brillo
    Belting,
    /// rasgado: voz tensa y áspera, vibrato lento y ancho
    Rasgado,
}

/// Lo que un estilo le pide a la voz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleDef {
    pub name: &'static str,
    /// presión de pulmones a velocidad media (cmH2O)
    pub pressure: f64,
    /// 0 flojo .. 1 apretado
    pub tension: f64,
    pub breathiness: f64,
    /// 0 oscuro .. 1 brillante
    pub brightness: f64,
    pub roughness: f64,
    /// profundidad del vibrato (cents) y velocidad (Hz)
    pub vibrato_cents: f64,
    pub vibrato_hz: f64,
    /// cuánto tarda en aparecer el vibrato (s)
    pub vibrato_delay: f64,
    /// ataque desde abajo (cents; 0 = entra afinado)
    pub scoop_cents: f64,
    /// deslizamiento entre notas ligadas (s)
    pub glide: f64,
}

impl Style {
    pub const ALL: [Style; 8] = [Style::Clasico, Style::Antiguo, Style::Pop, Style::Gospel, Style::Folk, Style::Susurro, Style::Belting, Style::Rasgado];

    pub fn def(self) -> StyleDef {
        match self {
            Style::Clasico => StyleDef { name: "Clásico", pressure: 9.0, tension: 0.50, breathiness: 0.12, brightness: 0.30, roughness: 0.08, vibrato_cents: 38.0, vibrato_hz: 5.6, vibrato_delay: 0.25, scoop_cents: 0.0, glide: 0.07 },
            Style::Antiguo => StyleDef { name: "Antiguo", pressure: 7.0, tension: 0.42, breathiness: 0.18, brightness: 0.22, roughness: 0.05, vibrato_cents: 4.0, vibrato_hz: 5.2, vibrato_delay: 0.6, scoop_cents: 0.0, glide: 0.05 },
            Style::Pop => StyleDef { name: "Pop", pressure: 8.0, tension: 0.55, breathiness: 0.22, brightness: 0.40, roughness: 0.10, vibrato_cents: 18.0, vibrato_hz: 5.8, vibrato_delay: 0.55, scoop_cents: 25.0, glide: 0.06 },
            Style::Gospel => StyleDef { name: "Gospel", pressure: 11.0, tension: 0.66, breathiness: 0.10, brightness: 0.50, roughness: 0.16, vibrato_cents: 45.0, vibrato_hz: 5.4, vibrato_delay: 0.35, scoop_cents: 60.0, glide: 0.09 },
            Style::Folk => StyleDef { name: "Folk", pressure: 8.5, tension: 0.60, breathiness: 0.10, brightness: 0.48, roughness: 0.10, vibrato_cents: 8.0, vibrato_hz: 5.5, vibrato_delay: 0.5, scoop_cents: 10.0, glide: 0.05 },
            Style::Susurro => StyleDef { name: "Susurro", pressure: 5.0, tension: 0.25, breathiness: 0.75, brightness: 0.15, roughness: 0.06, vibrato_cents: 6.0, vibrato_hz: 5.0, vibrato_delay: 0.7, scoop_cents: 0.0, glide: 0.08 },
            Style::Belting => StyleDef { name: "Belting", pressure: 13.0, tension: 0.74, breathiness: 0.05, brightness: 0.62, roughness: 0.14, vibrato_cents: 28.0, vibrato_hz: 5.9, vibrato_delay: 0.45, scoop_cents: 35.0, glide: 0.06 },
            Style::Rasgado => StyleDef { name: "Rasgado", pressure: 12.0, tension: 0.80, breathiness: 0.08, brightness: 0.55, roughness: 0.42, vibrato_cents: 70.0, vibrato_hz: 4.4, vibrato_delay: 0.2, scoop_cents: 45.0, glide: 0.10 },
        }
    }

    pub fn from_index(i: usize) -> Style {
        Style::ALL[i.min(Style::ALL.len() - 1)]
    }
    pub fn index(self) -> usize {
        Style::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}
