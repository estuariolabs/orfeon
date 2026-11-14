//! Orfeón como plugin (CLAP y VST3), con la misma interfaz que la app independiente
//! (`orfeon-ui`). Toda la voz está en `orfeon-core`.
//!
//! Los controles son parámetros del DAW (se automatizan y se guardan con el proyecto). Las
//! caras editadas a mano y el último programa elegido se guardan aparte, también en el
//! proyecto.

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, EguiState};
use orfeon_core::singer::Face;
use orfeon_core::{Choir, ChoirParams, MAX_SINGERS};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
enum ModeP {
    #[name = "Coro (una voz por nota)"]
    Coro,
    #[name = "Tutti (todos la misma nota)"]
    Tutti,
    #[name = "Solo (un cantante, ligado)"]
    Solo,
    Partes,
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
    /// tamaño de la ventana
    #[persist = "editor"]
    editor_state: Arc<EguiState>,
    /// caras editadas a mano (una por cantante; None = la del elenco)
    #[persist = "caras"]
    custom: RwLock<Vec<Option<Face>>>,
    /// último programa elegido (sólo para mostrar su nombre)
    #[persist = "programa"]
    preset: RwLock<Option<usize>>,
    #[persist = "part_ipa_v1"]
    lyrics: RwLock<[String; 4]>,
    #[persist = "part_words_v1"]
    words: RwLock<[String; 4]>,
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
    #[id = "voiceld"]
    voice_leading: BoolParam,
    #[id = "stagger"]
    stagger_ms: FloatParam,
    #[id = "part0_low"]
    part0_low: IntParam,
    #[id = "part0_high"]
    part0_high: IntParam,
    #[id = "part0_style"]
    part0_style: EnumParam<StyleP>,
    #[id = "part1_low"]
    part1_low: IntParam,
    #[id = "part1_high"]
    part1_high: IntParam,
    #[id = "part1_style"]
    part1_style: EnumParam<StyleP>,
    #[id = "part2_low"]
    part2_low: IntParam,
    #[id = "part2_high"]
    part2_high: IntParam,
    #[id = "part2_style"]
    part2_style: EnumParam<StyleP>,
    #[id = "part3_low"]
    part3_low: IntParam,
    #[id = "part3_high"]
    part3_high: IntParam,
    #[id = "part3_style"]
    part3_style: EnumParam<StyleP>,
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
            editor_state: EguiState::from_size(orfeon_ui::WIDTH, orfeon_ui::HEIGHT),
            custom: RwLock::new(vec![None; MAX_SINGERS]),
            preset: RwLock::new(Some(0)),
            lyrics: RwLock::default(),
            words: RwLock::default(),
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
            voice_leading: BoolParam::new("Conducción vocal", d.voice_leading),
            stagger_ms: FloatParam::new("Entradas de grave a agudo", d.stagger_ms as f32, FloatRange::Linear { min: 0.0, max: 40.0 }).with_unit(" ms"),
            part0_low: IntParam::new("Parte 1: Grave", d.parts[0].low as i32, IntRange::Linear { min: 0, max: 127 }),
            part0_high: IntParam::new("Parte 1: Aguda", d.parts[0].high as i32, IntRange::Linear { min: 0, max: 127 }),
            part0_style: EnumParam::new("Parte 1: Estilo", StyleP::from_index(d.parts[0].style.index())),
            part1_low: IntParam::new("Parte 2: Grave", d.parts[1].low as i32, IntRange::Linear { min: 0, max: 127 }),
            part1_high: IntParam::new("Parte 2: Aguda", d.parts[1].high as i32, IntRange::Linear { min: 0, max: 127 }),
            part1_style: EnumParam::new("Parte 2: Estilo", StyleP::from_index(d.parts[1].style.index())),
            part2_low: IntParam::new("Parte 3: Grave", d.parts[2].low as i32, IntRange::Linear { min: 0, max: 127 }),
            part2_high: IntParam::new("Parte 3: Aguda", d.parts[2].high as i32, IntRange::Linear { min: 0, max: 127 }),
            part2_style: EnumParam::new("Parte 3: Estilo", StyleP::from_index(d.parts[2].style.index())),
            part3_low: IntParam::new("Parte 4: Grave", d.parts[3].low as i32, IntRange::Linear { min: 0, max: 127 }),
            part3_high: IntParam::new("Parte 4: Aguda", d.parts[3].high as i32, IntRange::Linear { min: 0, max: 127 }),
            part3_style: EnumParam::new("Parte 4: Estilo", StyleP::from_index(d.parts[3].style.index())),
            threads: IntParam::new("Hilos", cores.clamp(1, 16), IntRange::Linear { min: 1, max: 16 }),
        }
    }
}

impl OrfeonParams {
    /// Los parámetros del coro, desde los controles. `try_read`: el hilo de audio nunca espera
    /// a la interfaz; si justo está escribiendo las caras, usa las del bloque anterior.
    fn choir(&self, prev: &ChoirParams) -> ChoirParams {
        let mut p = {
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
                voice_leading: self.voice_leading.value(),
                stagger_ms: self.stagger_ms.value() as f64,
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
        p.parts[0].low = self.part0_low.value() as u8;
        p.parts[0].high = self.part0_high.value() as u8;
        p.parts[0].style = orfeon_core::Style::from_index(self.part0_style.value() as usize);
        p.parts[1].low = self.part1_low.value() as u8;
        p.parts[1].high = self.part1_high.value() as u8;
        p.parts[1].style = orfeon_core::Style::from_index(self.part1_style.value() as usize);
        p.parts[2].low = self.part2_low.value() as u8;
        p.parts[2].high = self.part2_high.value() as u8;
        p.parts[2].style = orfeon_core::Style::from_index(self.part2_style.value() as usize);
        p.parts[3].low = self.part3_low.value() as u8;
        p.parts[3].high = self.part3_high.value() as u8;
        p.parts[3].style = orfeon_core::Style::from_index(self.part3_style.value() as usize);
        p.gain_db = self.gain.value() as f64;
        p.max_voices = self.max_voices.value().clamp(1, 64) as usize;
        p.quality = if self.quality.value() == QualityP::Eco { orfeon_core::Quality::Eco } else { orfeon_core::Quality::Alta };
        p.custom = match self.custom.try_read() {
            Ok(c) => {
                let mut a = [None; MAX_SINGERS];
                for (d, s) in a.iter_mut().zip(c.iter()) {
                    *d = *s;
                }
                a
            }
            Err(_) => prev.custom,
        };
        p
    }

    /// Escribe en los parámetros del DAW lo que cambió en la interfaz (con gesto de edición,
    /// así el DAW lo graba como automatización).
    fn write(&self, setter: &ParamSetter, old: &ChoirParams, new: &ChoirParams) {
        fn f(setter: &ParamSetter, p: &FloatParam, a: f64, b: f64) {
            if a != b {
                setter.begin_set_parameter(p);
                setter.set_parameter(p, b as f32);
                setter.end_set_parameter(p);
            }
        }
        fn i(setter: &ParamSetter, p: &IntParam, a: i32, b: i32) {
            if a != b {
                setter.begin_set_parameter(p);
                setter.set_parameter(p, b);
                setter.end_set_parameter(p);
            }
        }
        fn e<T: Enum + PartialEq + 'static>(setter: &ParamSetter, p: &EnumParam<T>, a: usize, b: usize) {
            if a != b {
                setter.begin_set_parameter(p);
                setter.set_parameter_normalized(p, p.preview_normalized(T::from_index(b)));
                setter.end_set_parameter(p);
            }
        }
        let mode = |m: orfeon_core::Mode| orfeon_core::Mode::ALL.iter().position(|x| *x == m).unwrap_or(0);
        e(setter, &self.mode, mode(old.mode), mode(new.mode));
        i(setter, &self.singers, old.singers as i32, new.singers as i32);
        e(setter, &self.ensemble, old.ensemble.index(), new.ensemble.index());
        i(setter, &self.seed, old.seed as i32, new.seed as i32);
        e(setter, &self.style, old.style.index(), new.style.index());
        f(setter, &self.vowel_x, old.vowel_x, new.vowel_x);
        f(setter, &self.vowel_y, old.vowel_y, new.vowel_y);
        f(setter, &self.vowel_move, old.vowel_move, new.vowel_move);
        f(setter, &self.vowel_rate, old.vowel_rate, new.vowel_rate);
        f(setter, &self.vowel_spread, old.vowel_spread, new.vowel_spread);
        f(setter, &self.humanize, old.humanize, new.humanize);
        if old.voice_leading != new.voice_leading {
            setter.begin_set_parameter(&self.voice_leading);
            setter.set_parameter(&self.voice_leading, new.voice_leading);
            setter.end_set_parameter(&self.voice_leading);
        }
        f(setter, &self.stagger_ms, old.stagger_ms, new.stagger_ms);
        i(setter, &self.part0_low, old.parts[0].low as i32, new.parts[0].low as i32);
        i(setter, &self.part0_high, old.parts[0].high as i32, new.parts[0].high as i32);
        e(setter, &self.part0_style, old.parts[0].style.index(), new.parts[0].style.index());
        i(setter, &self.part1_low, old.parts[1].low as i32, new.parts[1].low as i32);
        i(setter, &self.part1_high, old.parts[1].high as i32, new.parts[1].high as i32);
        e(setter, &self.part1_style, old.parts[1].style.index(), new.parts[1].style.index());
        i(setter, &self.part2_low, old.parts[2].low as i32, new.parts[2].low as i32);
        i(setter, &self.part2_high, old.parts[2].high as i32, new.parts[2].high as i32);
        e(setter, &self.part2_style, old.parts[2].style.index(), new.parts[2].style.index());
        i(setter, &self.part3_low, old.parts[3].low as i32, new.parts[3].low as i32);
        i(setter, &self.part3_high, old.parts[3].high as i32, new.parts[3].high as i32);
        e(setter, &self.part3_style, old.parts[3].style.index(), new.parts[3].style.index());
        f(setter, &self.vibrato, old.vibrato, new.vibrato);
        f(setter, &self.breath, old.breath, new.breath);
        f(setter, &self.brightness, old.brightness, new.brightness);
        f(setter, &self.neck, old.face.neck, new.face.neck);
        f(setter, &self.size, old.face.size, new.face.size);
        f(setter, &self.jaw, old.face.jaw, new.face.jaw);
        f(setter, &self.lips, old.face.lips, new.face.lips);
        f(setter, &self.nose, old.face.nose, new.face.nose);
        f(setter, &self.age, old.face.age, new.face.age);
        f(setter, &self.variation, old.variation, new.variation);
        f(setter, &self.width, old.width, new.width);
        f(setter, &self.depth, old.depth, new.depth);
        e(setter, &self.room, old.room.index(), new.room.index());
        f(setter, &self.room_mix, old.room_mix, new.room_mix);
        f(setter, &self.mic, old.mic, new.mic);
        f(setter, &self.gain, old.gain_db, new.gain_db);
        i(setter, &self.max_voices, old.max_voices as i32, new.max_voices as i32);
        let q = |q: orfeon_core::Quality| if q == orfeon_core::Quality::Eco { 1 } else { 0 };
        e(setter, &self.quality, q(old.quality), q(new.quality));
        if old.custom != new.custom {
            if let Ok(mut c) = self.custom.write() {
                *c = new.custom.to_vec();
            }
        }
    }
}

/// Lo que comparten el hilo de audio y la interfaz.
enum LyricsUpdate {
    Apply(orfeon_core::lyrics::Lyrics),
    Returned { _bank: orfeon_core::lyrics::Lyrics },
    Reset,
}

struct Shared {
    lyrics: Mutex<Option<LyricsUpdate>>,
    /// carga del cálculo en milésimas, voces sonando, nivel por cantante (bits de f32), notas
    load: AtomicU32,
    voices: AtomicUsize,
    levels: [AtomicU32; MAX_SINGERS],
    held: [AtomicBool; 128],
    /// notas del teclado de pantalla: (apretada, nota, velocidad). 255 = apagar todo.
    screen: Mutex<Option<rtrb::Producer<(bool, u8, f32, usize)>>>,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            lyrics: Mutex::new(None),
            load: AtomicU32::new(0),
            voices: AtomicUsize::new(0),
            levels: std::array::from_fn(|_| AtomicU32::new(0)),
            held: std::array::from_fn(|_| AtomicBool::new(false)),
            screen: Mutex::new(None),
        }
    }
}

/// Estado de la ventana del plugin.
struct EditorState {
    model: orfeon_ui::Model,
    params: Arc<OrfeonParams>,
    shared: Arc<Shared>,
    events: Vec<orfeon_ui::Event>,
}

struct Orfeon {
    params: Arc<OrfeonParams>,
    choir: Option<Choir>,
    applied: ChoirParams,
    threads: usize,
    shared: Arc<Shared>,
    screen: Option<rtrb::Consumer<(bool, u8, f32, usize)>>,
    load: f64,
}

impl Default for Orfeon {
    fn default() -> Self {
        Orfeon { params: Arc::new(OrfeonParams::default()), choir: None, applied: ChoirParams::DEFAULT, threads: 1, shared: Arc::new(Shared::default()), screen: None, load: 0.0 }
    }
}

/// id de las notas del teclado de pantalla: el "canal 16"
const SCREEN_ID: i32 = 16 * 128;

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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let start = params.choir(&ChoirParams::DEFAULT);
        let preset = params.preset.read().ok().and_then(|p| *p);
        let mut model = orfeon_ui::Model::new(start, preset);
        if let Ok(lyrics) = params.lyrics.read() { model.lyrics.clone_from(&lyrics); }
        if let Ok(words) = params.words.read() { model.words.clone_from(&words); }
        let state = EditorState { model, params: params.clone(), shared: self.shared.clone(), events: Vec::new() };
        create_egui_editor(
            params.editor_state.clone(),
            state,
            |ctx, _st| orfeon_ui::theme::apply(ctx),
            |ctx, setter, st| {
                // el DAW puede haber movido algo (automatización): la interfaz parte de ahí
                let cur = st.params.choir(&st.model.params);
                st.model.params = cur;
                let mut tel = orfeon_ui::Telemetry::default();
                tel.load = st.shared.load.load(Ordering::Relaxed) as f32 / 1000.0;
                tel.voices = st.shared.voices.load(Ordering::Relaxed);
                tel.threads = st.params.threads.value() as usize;
                for (t, a) in tel.levels.iter_mut().zip(st.shared.levels.iter()) {
                    *t = f32::from_bits(a.load(Ordering::Relaxed));
                }
                for (t, a) in tel.held.iter_mut().zip(st.shared.held.iter()) {
                    *t = a.load(Ordering::Relaxed);
                }
                let mut events = std::mem::take(&mut st.events);
                events.clear();
                if orfeon_ui::show(ctx, &mut st.model, &tel, None, &mut events) {
                    st.params.write(setter, &cur, &st.model.params);
                }
                if let Ok(mut p) = st.params.preset.write() {
                    *p = st.model.preset;
                }
                for ev in events.drain(..) {
                    let msg = match ev {
                        orfeon_ui::Event::NoteOn(k, v, part) => Some((true, k, v, part)),
                        orfeon_ui::Event::NoteOff(k, part) => Some((false, k, 0.0, part)),
                        orfeon_ui::Event::AllOff => Some((false, 255, 0.0, 0)),
                        orfeon_ui::Event::Threads(n) => {
                            setter.begin_set_parameter(&st.params.threads);
                            setter.set_parameter(&st.params.threads, n as i32);
                            setter.end_set_parameter(&st.params.threads);
                            None
                        }
                        orfeon_ui::Event::ApplyLyrics => {
                            st.model.lyrics_status = match orfeon_core::lyrics::Lyrics::prepare(&st.model.lyrics, &st.model.params) {
                                Ok(bank) => {
                                    if let Ok(mut lyrics) = st.params.lyrics.write() { lyrics.clone_from(&st.model.lyrics); }
                                    if let Ok(mut words) = st.params.words.write() { words.clone_from(&st.model.words); }
                                    if let Ok(mut pending) = st.shared.lyrics.lock() { *pending = Some(LyricsUpdate::Apply(bank)); }
                                    "Letras aplicadas. Los cursores empiezan desde el inicio.".into()
                                }
                                Err(error) => error,
                            };
                            None
                        }
                        orfeon_ui::Event::ResetLyrics => {
                            if let Ok(mut pending) = st.shared.lyrics.lock() {
                                if !matches!(*pending, Some(LyricsUpdate::Apply(_))) { *pending = Some(LyricsUpdate::Reset); }
                            }
                            None
                        }
                        orfeon_ui::Event::Latency(_) | orfeon_ui::Event::Midi(_) | orfeon_ui::Event::SaveQuartet | orfeon_ui::Event::OpenQuartet => None,
                    };
                    if let Some(m) = msg {
                        if let Ok(mut q) = st.shared.screen.lock() {
                            if let Some(q) = q.as_mut() {
                                let _ = q.push(m);
                            }
                        }
                    }
                }
                st.events = events;
            },
        )
    }

    fn initialize(&mut self, _layout: &AudioIOLayout, config: &BufferConfig, _context: &mut impl InitContext<Self>) -> bool {
        // construir el coro acá (reserva memoria y arranca los hilos), no en el hilo de audio
        self.threads = self.params.threads.value().clamp(1, 16) as usize;
        let mut choir = Choir::new(config.sample_rate as f64, self.threads);
        self.applied = self.params.choir(&ChoirParams::DEFAULT);
        let (tx, rx) = rtrb::RingBuffer::new(256);
        if let Ok(mut q) = self.shared.screen.lock() {
            *q = Some(tx);
        }
        self.screen = Some(rx);
        choir.set_params(self.applied);
        if let Ok(lyrics) = self.params.lyrics.read() {
            if let Ok(bank) = orfeon_core::lyrics::Lyrics::prepare(&lyrics, &self.applied) {
                choir.replace_lyrics(bank);
            }
        }
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

        let t0 = Instant::now();
        // ---- teclado de pantalla ----
        if let Some(rx) = self.screen.as_mut() {
            while let Ok((on, key, vel, part)) = rx.pop() {
                let id = SCREEN_ID + (part as i32) * 128 + key as i32;
                if key == 255 {
                    choir.all_notes_off();
                    for h in self.shared.held.iter() {
                        h.store(false, Ordering::Relaxed);
                    }
                } else if on {
                    choir.note_on(id, key, vel as f64);
                    self.shared.held[key as usize & 127].store(true, Ordering::Relaxed);
                } else {
                    choir.note_off(id);
                    self.shared.held[key as usize & 127].store(false, Ordering::Relaxed);
                }
            }
        }
        // ---- controles ----
        let want = self.params.choir(&self.applied);
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

        if let Ok(mut update) = self.shared.lyrics.try_lock() {
            *update = match update.take() {
                Some(LyricsUpdate::Apply(bank)) => Some(LyricsUpdate::Returned { _bank: choir.replace_lyrics(bank) }),
                Some(LyricsUpdate::Reset) => { choir.reset_lyrics(); None }
                other => other,
            };
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
                            NoteEvent::NoteOn { channel, note, velocity, .. } => {
                                choir.note_on(note_id(channel, note), note, velocity as f64);
                                self.shared.held[note as usize & 127].store(true, Ordering::Relaxed);
                            }
                            NoteEvent::NoteOff { channel, note, .. } | NoteEvent::Choke { channel, note, .. } => {
                                choir.note_off(note_id(channel, note));
                                self.shared.held[note as usize & 127].store(false, Ordering::Relaxed);
                            }
                            NoteEvent::PolyTuning { channel, note, tuning, .. } => choir.note_pitch_bend(note_id(channel, note), tuning as f64),
                            NoteEvent::PolyPressure { channel, note, pressure, .. } => choir.note_pressure(note_id(channel, note), pressure as f64),
                            NoteEvent::PolyBrightness { channel, note, brightness, .. } => choir.note_brightness(note_id(channel, note), brightness as f64),
                            // rueda de afinación: ±2 semitonos
                            NoteEvent::MidiPitchBend { value, .. } => choir.set_pitch_bend((value as f64 - 0.5) * 4.0),
                            // CC 120 / 123: mismo alcance global que en la app.
                            NoteEvent::MidiCC { cc: 120 | 123, .. } => {
                                choir.all_notes_off();
                                for h in &self.shared.held {
                                    h.store(false, Ordering::Relaxed);
                                }
                            }
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

        // ---- mediciones para la interfaz ----
        let block = total as f64 / choir.sample_rate();
        if block > 0.0 {
            self.load += 0.1 * (t0.elapsed().as_secs_f64() / block - self.load);
        }
        self.shared.load.store((self.load * 1000.0) as u32, Ordering::Relaxed);
        self.shared.voices.store(choir.sounding(), Ordering::Relaxed);
        for (a, l) in self.shared.levels.iter().zip(choir.levels().iter()) {
            a.store(l.to_bits(), Ordering::Relaxed);
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
