//! Un cantante: tipo de voz, rostro y cuerpo, y sus rasgos personales.
//!
//! El rostro y el cuerpo se traducen a las cuatro medidas anatómicas que el motor de CHIMERA
//! acepta (tamaño, largo de faringe, lengua, labios) más la abertura del velo (nariz). Los
//! rasgos personales (afinación, vibrato, tiempo de reacción) hacen que dos cantantes del mismo
//! tipo no suenen iguales.

use crate::rng::Rng;
use chimera::anatomy::human::{BodyMix, HumanBody};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceType {
    Bajo,
    Baritono,
    Tenor,
    Contralto,
    Mezzo,
    Soprano,
    Nino,
}

impl VoiceType {
    pub const ALL: [VoiceType; 7] = [VoiceType::Bajo, VoiceType::Baritono, VoiceType::Tenor, VoiceType::Contralto, VoiceType::Mezzo, VoiceType::Soprano, VoiceType::Nino];

    pub fn name(self) -> &'static str {
        match self {
            VoiceType::Bajo => "bajo",
            VoiceType::Baritono => "barítono",
            VoiceType::Tenor => "tenor",
            VoiceType::Contralto => "contralto",
            VoiceType::Mezzo => "mezzo",
            VoiceType::Soprano => "soprano",
            VoiceType::Nino => "niño",
        }
    }

    /// Registro cómodo (notas MIDI, extremos incluidos).
    pub fn range(self) -> (u8, u8) {
        match self {
            VoiceType::Bajo => (40, 64),      // E2..E4
            VoiceType::Baritono => (43, 67),  // G2..G4
            VoiceType::Tenor => (48, 72),     // C3..C5
            VoiceType::Contralto => (53, 77), // F3..F5
            VoiceType::Mezzo => (57, 81),     // A3..A5
            VoiceType::Soprano => (60, 84),   // C4..C6
            VoiceType::Nino => (60, 81),      // C4..A5
        }
    }

    /// Cuerpo de partida entre los tres calibrados de CHIMERA, y un ajuste de tamaño.
    fn base(self) -> (BodyMix, f64) {
        match self {
            VoiceType::Bajo => (BodyMix::new(0.0, 0.0), 1.06),
            VoiceType::Baritono => (BodyMix::new(0.0, 0.0), 1.00),
            VoiceType::Tenor => (BodyMix::new(0.18, 0.0), 0.98),
            VoiceType::Contralto => (BodyMix::new(0.85, 0.0), 1.03),
            VoiceType::Mezzo => (BodyMix::new(1.0, 0.0), 1.00),
            VoiceType::Soprano => (BodyMix::new(1.0, 0.0), 0.96),
            VoiceType::Nino => (BodyMix::new(0.0, 1.0), 1.00),
        }
    }

    pub fn is_low(self) -> bool {
        matches!(self, VoiceType::Bajo | VoiceType::Baritono | VoiceType::Tenor)
    }
}

/// Rostro y cuerpo. Todo va de -1 a 1 (0 = promedio), salvo `nose` (0..1).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Face {
    /// cuello: más largo = faringe más larga, voz más oscura
    pub neck: f64,
    /// tamaño general de la cabeza y el tracto
    pub size: f64,
    /// mandíbula: más ancha = más lugar para la lengua
    pub jaw: f64,
    /// labios: más llenos / anchos
    pub lips: f64,
    /// nariz: cuánto resuena (velo un poco abierto)
    pub nose: f64,
    /// edad: -1 infantil .. 0 adulto .. 1 mayor
    pub age: f64,
}

impl Face {
    fn clamped(self) -> Face {
        let c = |x: f64| if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 };
        Face { neck: c(self.neck), size: c(self.size), jaw: c(self.jaw), lips: c(self.lips), nose: c(self.nose).max(0.0), age: c(self.age) }
    }
    /// Suma dos rostros (el global más la variación personal), recortando.
    pub fn plus(self, o: Face) -> Face {
        Face { neck: self.neck + o.neck, size: self.size + o.size, jaw: self.jaw + o.jaw, lips: self.lips + o.lips, nose: self.nose + o.nose, age: self.age + o.age }.clamped()
    }
}

/// Qué tipos de voz forman el coro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ensemble {
    /// soprano, contralto, tenor y bajo por partes iguales
    Mixto,
    /// sopranos, mezzos y contraltos
    Femenino,
    /// tenores, barítonos y bajos
    Masculino,
    /// voces blancas
    Ninos,
    Sopranos,
    Contraltos,
    Tenores,
    Bajos,
}

impl Ensemble {
    pub const ALL: [Ensemble; 8] = [Ensemble::Mixto, Ensemble::Femenino, Ensemble::Masculino, Ensemble::Ninos, Ensemble::Sopranos, Ensemble::Contraltos, Ensemble::Tenores, Ensemble::Bajos];

    pub fn name(self) -> &'static str {
        match self {
            Ensemble::Mixto => "Mixto",
            Ensemble::Femenino => "Femenino",
            Ensemble::Masculino => "Masculino",
            Ensemble::Ninos => "Niños",
            Ensemble::Sopranos => "Sopranos",
            Ensemble::Contraltos => "Contraltos",
            Ensemble::Tenores => "Tenores",
            Ensemble::Bajos => "Bajos",
        }
    }
    pub fn from_index(i: usize) -> Ensemble {
        Ensemble::ALL[i.min(Ensemble::ALL.len() - 1)]
    }
    pub fn index(self) -> usize {
        Ensemble::ALL.iter().position(|e| *e == self).unwrap_or(0)
    }

    /// Tipo de voz del cantante `i`. El orden reparte las cuerdas desde el primero, así un
    /// coro chico ya las tiene todas.
    pub fn voice_type(self, i: usize) -> VoiceType {
        match self {
            Ensemble::Mixto => [VoiceType::Soprano, VoiceType::Bajo, VoiceType::Contralto, VoiceType::Tenor][i % 4],
            Ensemble::Femenino => [VoiceType::Soprano, VoiceType::Contralto, VoiceType::Mezzo][i % 3],
            Ensemble::Masculino => [VoiceType::Tenor, VoiceType::Bajo, VoiceType::Baritono][i % 3],
            Ensemble::Ninos => VoiceType::Nino,
            Ensemble::Sopranos => VoiceType::Soprano,
            Ensemble::Contraltos => VoiceType::Contralto,
            Ensemble::Tenores => VoiceType::Tenor,
            Ensemble::Bajos => VoiceType::Bajo,
        }
    }
}

/// Un cantante ya resuelto: lo que hace falta para construir y dirigir su voz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Singer {
    pub voice_type: VoiceType,
    pub face: Face,
    /// anatomía para el motor
    pub body: HumanBody,
    /// cuerpo calibrado más cercano (para las posturas de las vocales)
    pub mix: BodyMix,
    /// cuánto baja el velo (resonancia nasal), en unidades del articulador
    pub velum: f64,
    /// lugar en el escenario: -1 izquierda .. 1 derecha, 0 adelante .. 1 al fondo
    pub pan: f64,
    pub depth: f64,
    // --- rasgos personales (se suman a lo que pide el estilo) ---
    pub tension: f64,
    pub breathiness: f64,
    pub brightness: f64,
    pub roughness: f64,
    pub vibrato_rate: f64,
    pub vibrato_depth: f64,
    /// desafinación fija (cents) y cuánto deriva
    pub detune: f64,
    pub drift: f64,
    /// 0..1: cuánto tarda en entrar respecto del más rápido
    pub lateness: f64,
    /// su vocal, corrida un poco en la carta
    pub vowel_dx: f64,
    pub vowel_dy: f64,
    /// balanceo lento de la intensidad (Hz) y fase inicial
    pub sway_rate: f64,
    pub sway_phase: f64,
    /// nivelado: los cuerpos chicos suenan más fuerte en el modelo
    pub gain: f64,
    /// semilla propia (deriva de afinación)
    pub seed: u64,
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

impl Singer {
    /// El cantante `index` de un coro. `face` es el rostro común a todos y `variation` (0..1)
    /// cuánto se aparta cada uno de él. Determinista para una misma semilla.
    pub fn new(ensemble: Ensemble, index: usize, count: usize, seed: u32, face: Face, variation: f64) -> Singer {
        Singer::with_face(ensemble, index, count, seed, face, variation, None)
    }

    /// Como `new`, pero con un rostro elegido a mano para este cantante (`custom`): reemplaza
    /// al rostro común más su variación. El resto de sus rasgos (afinación, vibrato, lugar)
    /// sigue saliendo de la semilla, así editar la cara no cambia a la persona.
    pub fn with_face(ensemble: Ensemble, index: usize, count: usize, seed: u32, face: Face, variation: f64, custom: Option<Face>) -> Singer {
        let variation = if variation.is_finite() { variation.clamp(0.0, 1.0) } else { 0.0 };
        let mut rng = Rng::new((seed as u64) << 16 | (index as u64 + 1) << 4 | ensemble.index() as u64);
        let voice_type = ensemble.voice_type(index);
        let own = Face {
            neck: rng.bell() * 0.9 * variation,
            size: rng.bell() * 0.9 * variation,
            jaw: rng.bell() * 0.9 * variation,
            lips: rng.bell() * 0.9 * variation,
            nose: rng.unit() * 0.5 * variation,
            age: rng.bell() * 0.6 * variation,
        };
        let face = match custom {
            Some(c) => c.plus(Face::default()),
            None => face.plus(own),
        };

        // --- cuerpo ---
        let (mut mix, type_scale) = voice_type.base();
        if face.age < 0.0 {
            mix = BodyMix::new(mix.female, mix.child + (-face.age) * 0.7);
        }
        let b = mix.body();
        let body = HumanBody {
            name: b.name,
            scale: (b.scale * type_scale * (1.0 + 0.08 * face.size)).clamp(0.66, 1.15),
            pharynx: (b.pharynx * (1.0 + 0.10 * face.neck)).clamp(0.72, 1.12),
            tongue_radius: (b.tongue_radius * (1.0 + 0.06 * face.jaw)).clamp(1.95, 2.45),
            lip_width: (b.lip_width * (1.0 + 0.10 * face.lips)).clamp(3.5, 5.1),
        };
        let old = face.age.max(0.0);

        // --- lugar en el escenario: cuerdas de izquierda a derecha, graves atrás ---
        let section = match voice_type {
            VoiceType::Soprano | VoiceType::Nino => -0.72,
            VoiceType::Mezzo => -0.40,
            VoiceType::Contralto => -0.24,
            VoiceType::Tenor => 0.24,
            VoiceType::Baritono => 0.48,
            VoiceType::Bajo => 0.72,
        };
        let single_section = matches!(ensemble, Ensemble::Ninos | Ensemble::Sopranos | Ensemble::Contraltos | Ensemble::Tenores | Ensemble::Bajos);
        let pan = if count <= 1 {
            0.0
        } else if single_section {
            lerp(-0.9, 0.9, (index as f64 + 0.5) / count as f64)
        } else {
            (section + rng.bipolar() * 0.22).clamp(-1.0, 1.0)
        };
        let depth = if count <= 1 { 0.0 } else { ((if voice_type.is_low() { 0.62 } else { 0.30 }) + rng.bipolar() * 0.22).clamp(0.0, 1.0) };

        Singer {
            voice_type,
            face,
            body,
            mix,
            velum: 0.22 * face.nose,
            pan,
            depth,
            tension: rng.bell() * 0.10 * variation - 0.05 * old,
            breathiness: rng.bell() * 0.08 * variation + 0.10 * old,
            brightness: rng.bell() * 0.10 * variation,
            roughness: rng.unit() * 0.06 * variation + 0.15 * old,
            vibrato_rate: (1.0 + rng.bell() * 0.12 * variation) * (1.0 - 0.15 * old),
            vibrato_depth: 1.0 + rng.bell() * 0.35 * variation + 0.3 * old,
            detune: rng.bell() * 9.0 * variation,
            drift: 1.0 + rng.unit() * variation,
            lateness: rng.unit(),
            vowel_dx: rng.bell() * 0.10,
            vowel_dy: rng.bell() * 0.10,
            sway_rate: 0.11 + rng.unit() * 0.17,
            sway_phase: rng.unit(),
            gain: 0.32 * body.scale.powf(1.8),
            seed: rng.next_u64(),
        }
    }

    /// La altura que este cantante canta para una tecla: la misma, o llevada por octavas a su
    /// registro si queda a más de una quinta por fuera.
    pub fn fold(&self, key: f64) -> f64 {
        let (lo, hi) = self.voice_type.range();
        let (lo, hi) = (lo as f64 - 7.0, hi as f64 + 7.0);
        let mut k = key;
        while k < lo {
            k += 12.0;
        }
        while k > hi {
            k -= 12.0;
        }
        k
    }

    /// ¿La tecla cae en su registro cómodo?
    pub fn fits(&self, key: u8) -> bool {
        let (lo, hi) = self.voice_type.range();
        key >= lo && key <= hi
    }
}
