//! Orfeón como plugin (CLAP y VST3). Sin interfaz propia: los controles se ven en el editor
//! genérico del anfitrión. Toda la voz está en `orfeon-core`.

use nih_plug::prelude::*;
use orfeon_core::preset::PRESETS;
use orfeon_core::singer::Face;
use orfeon_core::{Choir, ChoirParams};
use std::sync::Arc;

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
enum ModeP {
    #[name = "Coro (una voz por nota)"]
    Coro,
    #[name = "Tutti (todos la misma nota)"]
    Tutti,
    #[name = "Solo (un cantante, ligado)"]
    Solo,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
enum EnsembleP {
    Mixto,
    Femenino,
    Masculino,
    #[name = "Niños"]
    Ninos,
    Sopranos,
    Contraltos,
    Tenores,
    Bajos,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
enum StyleP {
    #[name = "Clásico"]
    Clasico,
    Antiguo,
    Pop,
    Gospel,
    Folk,
    Susurro,
    Belting,
    Rasgado,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
enum RoomP {
    Seco,
    Estudio,
    #[name = "Sala de conciertos"]
    Sala,
    Iglesia,
    Catedral,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
enum QualityP {
    Alta,
    #[name = "Eco (menos CPU)"]
    Eco,
}

#[derive(Params)]
struct OrfeonParams {
    /// 0 = Manual (mandan los controles); 1..36 = programa de fábrica
    #[id = "prog"]
    program: IntParam,
    #[id = "mode"]
    mode: EnumParam<ModeP>,
    #[id = "singers"]
    singers: IntParam,
    #[id = "ens"]
    ensemble: EnumParam<EnsembleP>,
    #[id = "seed"]
    seed: IntParam,
    #[id = "style"]
    style: EnumParam<StyleP>,
    #[id = "vowx"]
    vowel_x: FloatParam,
    #[id = "vowy"]
    vowel_y: FloatParam,
    #[id = "vowmov"]
    vowel_move: FloatParam,
    #[id = "vowrate"]
    vowel_rate: FloatParam,
    #[id = "vowspr"]
    vowel_spread: FloatParam,
    #[id = "human"]
    humanize: FloatParam,
    #[id = "vib"]
    vibrato: FloatParam,
    #[id = "breath"]
    breath: FloatParam,
    #[id = "bright"]
    brightness: FloatParam,
    #[id = "neck"]
    neck: FloatParam,
    #[id = "size"]
    size: FloatParam,
    #[id = "jaw"]
    jaw: FloatParam,
    #[id = "lips"]
    lips: FloatParam,
    #[id = "nose"]
    nose: FloatParam,
    #[id = "age"]
    age: FloatParam,
    #[id = "variat"]
    variation: FloatParam,
    #[id = "width"]
    width: FloatParam,
    #[id = "depth"]
    depth: FloatParam,
    #[id = "room"]
    room: EnumParam<RoomP>,
    #[id = "roommix"]
    room_mix: FloatParam,
    #[id = "mic"]
    mic: FloatParam,
    #[id = "gain"]
    gain: FloatParam,
    #[id = "voices"]
    max_voices: IntParam,
    #[id = "quality"]
    quality: EnumParam<QualityP>,
    #[id = "threads"]
    threads: IntParam,
}

fn unit(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 }).with_value_to_string(formatters::v2s_f32_rounded(2))
}
fn bipolar(name: &str) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }).with_value_to_string(formatters::v2s_f32_rounded(2))
}

impl Default for OrfeonParams {
    fn default() -> Self {
        let d = ChoirParams::DEFAULT;
        let cores = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(2);
        OrfeonParams {
            program: IntParam::new("Programa", 0, IntRange::Linear { min: 0, max: PRESETS.len() as i32 })
                .with_value_to_string(Arc::new(|v| if v <= 0 { "Manual".to_string() } else { PRESETS[(v as usize - 1).min(PRESETS.len() - 1)].name.to_string() })),
            mode: EnumParam::new("Modo", ModeP::Coro),
            singers: IntParam::new("Cantantes", d.singers as i32, IntRange::Linear { min: 1, max: 64 }),
            ensemble: EnumParam::new("Conjunto", EnsembleP::Mixto),
            seed: IntParam::new("Elenco (semilla)", d.seed as i32, IntRange::Linear { min: 1, max: 999 }),
            style: EnumParam::new("Estilo", StyleP::Clasico),
            vowel_x: unit("Vocal: anterior-posterior", d.vowel_x as f32),
            vowel_y: unit("Vocal: cerrada-abierta", d.vowel_y as f32),
            vowel_move: unit("Vocal: movimiento", d.vowel_move as f32),
            vowel_rate: FloatParam::new("Vocal: velocidad", d.vowel_rate as f32, FloatRange::Skewed { min: 0.02, max: 4.0, factor: FloatRange::skew_factor(-1.5) }).with_unit(" Hz").with_value_to_string(formatters::v2s_f32_rounded(2)),
            vowel_spread: unit("Vocal: diferencia entre cantantes", d.vowel_spread as f32),
            humanize: unit("Humanizar", d.humanize as f32),
            vibrato: FloatParam::new("Vibrato", d.vibrato as f32, FloatRange::Linear { min: 0.0, max: 2.0 }).with_value_to_string(formatters::v2s_f32_rounded(2)),
            breath: FloatParam::new("Aire", 0.0, FloatRange::Linear { min: -0.5, max: 0.5 }).with_value_to_string(formatters::v2s_f32_rounded(2)),
            brightness: FloatParam::new("Brillo", 0.0, FloatRange::Linear { min: -0.5, max: 0.5 }).with_value_to_string(formatters::v2s_f32_rounded(2)),
            neck: bipolar("Rostro: cuello"),
            size: bipolar("Rostro: tamaño"),
            jaw: bipolar("Rostro: mandíbula"),
            lips: bipolar("Rostro: labios"),
            nose: unit("Rostro: nariz", 0.0),
            age: bipolar("Rostro: edad"),
            variation: unit("Rostro: variedad", d.variation as f32),
            width: unit("Escenario: ancho", d.width as f32),
            depth: unit("Escenario: fondo", d.depth as f32),
            room: EnumParam::new("Sala", RoomP::Sala),
            room_mix: unit("Sala: cantidad", d.room_mix as f32),
            mic: unit("Micrófono: distancia", d.mic as f32),
            gain: FloatParam::new("Volumen", 0.0, FloatRange::Linear { min: -36.0, max: 12.0 }).with_unit(" dB").with_value_to_string(formatters::v2s_f32_rounded(1)),
            max_voices: IntParam::new("Voces a la vez (tope)", d.max_voices as i32, IntRange::Linear { min: 1, max: 64 }),
            quality: EnumParam::new("Calidad", QualityP::Alta),
            threads: IntParam::new("Hilos", cores.clamp(1, 16), IntRange::Linear { min: 1, max: 16 }),
        }
    }
}

impl OrfeonParams {
    /// Los parámetros del coro: los de un programa de fábrica, o los controles.
    /// Volumen, tope de voces y calidad siempre salen de los controles (dependen de la máquina).
    fn choir(&self) -> ChoirParams {
        let prog = self.program.value();
        let mut p = if prog >= 1 {
            PRESETS[(prog as usize - 1).min(PRESETS.len() - 1)].params
        } else {
            ChoirParams {
                mode: orfeon_core::Mode::from_index(self.mode.value() as usize),
                singers: self.singers.value().clamp(1, 64) as usize,
                ensemble: orfeon_core::Ensemble::from_index(self.ensemble.value() as usize),
                seed: self.seed.value().max(1) as u32,
                style: orfeon_core::Style::from_index(self.style.value() as usize),
                vowel_x: self.vowel_x.value() as f64,
                vowel_y: self.vowel_y.value() as f64,
                vowel_move: self.vowel_move.value() as f64,
                vowel_rate: self.vowel_rate.value() as f64,
                vowel_spread: self.vowel_spread.value() as f64,
                humanize: self.humanize.value() as f64,
                vibrato: self.vibrato.value() as f64,
                breath: self.breath.value() as f64,
                brightness: self.brightness.value() as f64,
                face: Face { neck: self.neck.value() as f64, size: self.size.value() as f64, jaw: self.jaw.value() as f64, lips: self.lips.value() as f64, nose: self.nose.value() as f64, age: self.age.value() as f64 },
                variation: self.variation.value() as f64,
                width: self.width.value() as f64,
                depth: self.depth.value() as f64,
                room: orfeon_core::RoomKind::from_index(self.room.value() as usize),
                room_mix: self.room_mix.value() as f64,
                mic: self.mic.value() as f64,
                ..ChoirParams::DEFAULT
            }
        };
        p.gain_db = self.gain.value() as f64;
        p.max_voices = self.max_voices.value().clamp(1, 64) as usize;
        p.quality = if self.quality.value() == QualityP::Eco { orfeon_core::Quality::Eco } else { orfeon_core::Quality::Alta };
        p
    }
}

struct Orfeon {
    params: Arc<OrfeonParams>,
    choir: Option<Choir>,
    applied: ChoirParams,
    threads: usize,
}

impl Default for Orfeon {
    fn default() -> Self {
        Orfeon { params: Arc::new(OrfeonParams::default()), choir: None, applied: ChoirParams::DEFAULT, threads: 1 }
    }
}

fn note_id(channel: u8, note: u8) -> i32 {
    channel as i32 * 128 + note as i32
}

/// Segundos de cola de cada sala (con margen), para avisarle al anfitrión.
fn room_tail(room: orfeon_core::RoomKind) -> f64 {
    match room {
        orfeon_core::RoomKind::Seco => 0.5,
        orfeon_core::RoomKind::Estudio => 1.5,
        orfeon_core::RoomKind::Sala => 4.0,
        orfeon_core::RoomKind::Iglesia => 6.0,
        orfeon_core::RoomKind::Catedral => 12.0,
    }
}

impl Plugin for Orfeon {
    const NAME: &'static str = "Orfeon";
    const VENDOR: &'static str = "Estuario Labs";
    const URL: &'static str = "https://estuariolabs.com.ar";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout { main_input_channels: None, main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() }];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(&mut self, _layout: &AudioIOLayout, config: &BufferConfig, _context: &mut impl InitContext<Self>) -> bool {
        // construir el coro acá (reserva memoria y arranca los hilos), no en el hilo de audio
        self.threads = self.params.threads.value().clamp(1, 16) as usize;
        let mut choir = Choir::new(config.sample_rate as f64, self.threads);
        self.applied = self.params.choir();
        choir.set_params(self.applied);
        self.choir = Some(choir);
        true
    }

    fn reset(&mut self) {
        if let Some(c) = self.choir.as_mut() {
            c.all_notes_off();
            c.set_pitch_bend(0.0);
        }
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, context: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let Some(choir) = self.choir.as_mut() else { return ProcessStatus::Normal };

        // ---- controles ----
        let want = self.params.choir();
        if want != self.applied {
            // un cambio de conjunto, rostros o calidad reconstruye las voces (corta las notas)
            choir.set_params(want);
            self.applied = want;
        }
        let threads = self.params.threads.value().clamp(1, 16) as usize;
        if threads != self.threads {
            choir.set_threads(threads);
            self.threads = threads;
        }

        // ---- audio, cortando el bloque en cada evento ----
        let total = buffer.samples();
        let channels = buffer.as_slice();
        if channels.len() < 2 {
            return ProcessStatus::Normal;
        }
        let (a, b) = channels.split_at_mut(1);
        let (left, right) = (&mut *a[0], &mut *b[0]);
        let mut next = context.next_event();
        let mut pos = 0usize;
        while pos < total {
            let mut end = total;
            loop {
                match next {
                    Some(ev) if (ev.timing() as usize) <= pos => {
                        match ev {
                            NoteEvent::NoteOn { channel, note, velocity, .. } => choir.note_on(note_id(channel, note), note, velocity as f64),
                            NoteEvent::NoteOff { channel, note, .. } | NoteEvent::Choke { channel, note, .. } => choir.note_off(note_id(channel, note)),
                            NoteEvent::PolyTuning { channel, note, tuning, .. } => choir.note_pitch_bend(note_id(channel, note), tuning as f64),
                            NoteEvent::PolyPressure { channel, note, pressure, .. } => choir.note_pressure(note_id(channel, note), pressure as f64),
                            NoteEvent::PolyBrightness { channel, note, brightness, .. } => choir.note_brightness(note_id(channel, note), brightness as f64),
                            // rueda de afinación: ±2 semitonos
                            NoteEvent::MidiPitchBend { value, .. } => choir.set_pitch_bend((value as f64 - 0.5) * 4.0),
                            // CC 123: todas las notas fuera
                            NoteEvent::MidiCC { cc: 123, .. } => choir.all_notes_off(),
                            _ => {}
                        }
                        next = context.next_event();
                    }
                    Some(ev) if (ev.timing() as usize) < end => {
                        end = ev.timing() as usize;
                        break;
                    }
                    _ => break,
                }
            }
            choir.render(&mut left[pos..end], &mut right[pos..end]);
            pos = end;
        }

        // mientras haya voces el anfitrión tiene que seguir llamando; después, la cola de la sala
        if choir.sounding() > 0 {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Tail((room_tail(self.applied.room) * choir.sample_rate()) as u32)
        }
    }
}

impl ClapPlugin for Orfeon {
    const CLAP_ID: &'static str = "ar.com.estuariolabs.orfeon";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Coro de cantantes modelados físicamente");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for Orfeon {
    const VST3_CLASS_ID: [u8; 16] = *b"OrfeonEstuarioLb";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth, Vst3SubCategory::Stereo];
}

nih_export_clap!(Orfeon);
nih_export_vst3!(Orfeon);
