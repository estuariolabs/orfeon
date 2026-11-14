//! El coro: reparte las notas entre los cantantes, dirige cada voz y mezcla el escenario.
//!
//! Tiempo real: `render`, las notas y los cambios de parámetros no estructurales no reservan
//! memoria. Cambiar el conjunto (cantidad, tipo, semilla, rostros, calidad) reconstruye las
//! voces: eso sí reserva y corta lo que estaba sonando.
//!
//! Cada cantante sonando cuesta una simulación completa del motor de CHIMERA. Por eso las
//! voces se calculan en paralelo (`threads`) y hay un tope de voces simultáneas (`max_voices`).

use crate::rng::Rng;
use crate::room::{Room, RoomKind};
use crate::singer::{Ensemble, Face, Singer};
use crate::style::{Style, StyleDef};
use crate::vowel::VowelSpace;
use crate::MAX_SINGERS;
use chimera::anatomy::human::ArticKey;
use chimera::voice::human_voice::{oversample_for, HumanVoice, SourcePatch};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

/// Muestras por tramo de control (a la frecuencia del anfitrión).
pub const CHUNK: usize = 128;
/// Tramos que cada hilo calcula por vez. Despertar a los hilos ayudantes cuesta (en Windows,
/// mucho y de forma irregular): se los despierta una vez cada BLOCK_CHUNKS tramos, no en cada
/// uno. El resultado es el mismo muestra a muestra.
const BLOCK_CHUNKS: usize = 8;
const BLOCK: usize = CHUNK * BLOCK_CHUNKS;
/// Frecuencia interna del modo Eco: la más baja a la que el motor simula a 2x (ver
/// `oversample_for`). Cuesta ~2/3 de la calidad Alta y llega hasta 15 kHz.
const ECO_RATE: f64 = 30100.0;
const STACK: usize = 16;
/// id de nota que usan Tutti y Solo (todas las voces cantan "la nota de arriba")
const TOP_ID: i32 = -2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// cada nota va al cantante siguiente: polifonía real, una voz por nota
    Coro,
    /// todos cantan la última nota, cada uno en su octava
    Tutti,
    /// un solo cantante, ligado
    Solo,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Coro, Mode::Tutti, Mode::Solo];
    pub fn name(self) -> &'static str {
        match self {
            Mode::Coro => "Coro",
            Mode::Tutti => "Tutti",
            Mode::Solo => "Solo",
        }
    }
    pub fn from_index(i: usize) -> Mode {
        Mode::ALL[i.min(2)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// el motor corre a la frecuencia del anfitrión
    Alta,
    /// el motor corre a ~30 kHz y se remuestrea: menos CPU, sin los agudos más altos
    Eco,
}

/// Todo lo que se puede ajustar. Los campos marcados (estructural) reconstruyen las voces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChoirParams {
    pub mode: Mode,
    /// cantantes definidos, 1..64 (estructural)
    pub singers: usize,
    /// (estructural)
    pub ensemble: Ensemble,
    /// elige a las personas del coro (estructural)
    pub seed: u32,
    pub style: Style,
    /// vocal en la carta: x 0 anterior .. 1 posterior, y 0 cerrada .. 1 abierta
    pub vowel_x: f64,
    pub vowel_y: f64,
    /// movimiento de la vocal alrededor del punto (0..1) y su velocidad (Hz)
    pub vowel_move: f64,
    pub vowel_rate: f64,
    /// cuánto difiere la vocal de cada cantante (0..1)
    pub vowel_spread: f64,
    /// cuánto se apartan entre sí en afinación, entrada e intensidad (0..1)
    pub humanize: f64,
    /// En Coro, favorece el registro central y la cercanía a la nota anterior.
    pub voice_leading: bool,
    /// Separación de ataque de grave a agudo, en milisegundos (0..40).
    pub stagger_ms: f64,
    /// multiplicador del vibrato del estilo (0..2)
    pub vibrato: f64,
    /// aire agregado a la voz (-0.5..0.5)
    pub breath: f64,
    /// brillo agregado (-0.5..0.5)
    pub brightness: f64,
    /// rostro común a todos (estructural)
    pub face: Face,
    /// cuánto difiere cada rostro del común, 0..1 (estructural)
    pub variation: f64,
    /// ancho y profundidad del escenario (0..1)
    pub width: f64,
    pub depth: f64,
    pub room: RoomKind,
    /// cantidad de sala (0..1)
    pub room_mix: f64,
    /// micrófono: 0 pegado al coro .. 1 lejos
    pub mic: f64,
    pub gain_db: f64,
    /// tope de cantantes sonando a la vez (protege la CPU)
    pub max_voices: usize,
    /// (estructural)
    pub quality: Quality,
    /// rostros elegidos a mano, por cantante (None = el que le toca por elenco y variedad).
    /// Cambiar uno reconstruye sólo ese cantante.
    pub custom: [Option<Face>; MAX_SINGERS],
}

impl ChoirParams {
    pub const DEFAULT: ChoirParams = ChoirParams {
        mode: Mode::Coro,
        singers: 8,
        ensemble: Ensemble::Mixto,
        seed: 1,
        style: Style::Clasico,
        vowel_x: 0.45,
        vowel_y: 1.0,
        vowel_move: 0.0,
        vowel_rate: 0.25,
        vowel_spread: 0.5,
        humanize: 0.6,
        voice_leading: false,
        stagger_ms: 0.0,
        vibrato: 1.0,
        breath: 0.0,
        brightness: 0.0,
        face: Face { neck: 0.0, size: 0.0, jaw: 0.0, lips: 0.0, nose: 0.0, age: 0.0 },
        variation: 0.7,
        width: 0.8,
        depth: 0.6,
        room: RoomKind::Sala,
        room_mix: 0.35,
        mic: 0.35,
        gain_db: 0.0,
        max_voices: 8,
        quality: Quality::Alta,
        custom: [None; MAX_SINGERS],
    };

    /// El cantante `i` tal como lo arma el coro con estos parámetros (sin crear su voz).
    /// Sirve para dibujarlo.
    pub fn singer(&self, i: usize) -> Singer {
        let n = self.singers.clamp(1, MAX_SINGERS);
        let i = i.min(MAX_SINGERS - 1);
        Singer::with_face(self.ensemble, i, n, self.seed, self.face, self.variation, self.custom[i])
    }

    /// ¿Mismos parámetros salvo lo que depende de la máquina (tope de voces, calidad, volumen)?
    pub fn same_sound(&self, o: &ChoirParams) -> bool {
        ChoirParams { max_voices: o.max_voices, quality: o.quality, gain_db: o.gain_db, ..*self } == *o
    }

    fn structural(&self) -> (usize, Ensemble, u32, Face, u64, Quality) {
        (self.singers.clamp(1, MAX_SINGERS), self.ensemble, self.seed, self.face, self.variation.to_bits(), self.quality)
    }
}

impl Default for ChoirParams {
    fn default() -> Self {
        ChoirParams::DEFAULT
    }
}

/// Lo que cada voz necesita saber en un tramo (igual para todas).
#[derive(Clone, Copy)]
struct Ctl {
    style: StyleDef,
    vx: f64,
    vy: f64,
    spread: f64,
    vibrato: f64,
    breath: f64,
    bright: f64,
    humanize: f64,
    bend: f64,
    n: usize,
    dt: f64,
}

/// Un cantante con su voz y el estado de lo que está cantando.
struct Slot {
    singer: Singer,
    voice: HumanVoice,
    space: VowelSpace,
    // remuestreo del modo Eco
    eco: bool,
    ratio: f64,
    frac: f64,
    hist: [f32; 4],
    ebuf: [f32; CHUNK + 8],
    /// salida mono del bloque (BLOCK_CHUNKS tramos)
    out: [f32; BLOCK],
    /// en qué tramos del bloque la voz se calculó (y entra en la mezcla)
    wanted: [bool; BLOCK_CHUNKS],
    // --- nota ---
    note_id: i32,
    /// altura cantada (MIDI, ya llevada a su registro)
    key: f64,
    /// la tecla sigue apretada
    held: bool,
    /// la voz se está calculando (sonando o apagándose)
    active: bool,
    /// muestras que faltan para que entre (-1: ya entró)
    wait: i64,
    /// primer tramo tras entrar desde el silencio: la altura salta, no desliza
    fresh: bool,
    vel: f64,
    mpe_bend: f64,
    mpe_pressure: f64,
    mpe_bright: f64,
    /// segundos desde que entró
    t: f64,
    /// orden de llegada (para robar la más vieja)
    born: u64,
    drift: f64,
    rng: Rng,
    sway: f64,
    quiet: u32,
    /// filtro de distancia (lo usa la mezcla)
    dist_lp: f32,
}

impl Slot {
    fn new(fs: f64, singer: Singer, quality: Quality) -> Slot {
        let eco = quality == Quality::Eco && fs > ECO_RATE * 1.05;
        let efs = if eco { ECO_RATE } else { fs };
        Slot {
            singer,
            voice: HumanVoice::new(efs, singer.body, oversample_for(efs)),
            space: VowelSpace::new(singer.mix),
            eco,
            ratio: efs / fs,
            frac: 0.0,
            hist: [0.0; 4],
            ebuf: [0.0; CHUNK + 8],
            out: [0.0; BLOCK],
            wanted: [false; BLOCK_CHUNKS],
            note_id: -1,
            key: 60.0,
            held: false,
            active: false,
            wait: -1,
            fresh: false,
            vel: 0.7,
            mpe_bend: 0.0,
            mpe_pressure: 1.0,
            mpe_bright: 0.5,
            t: 0.0,
            born: 0,
            drift: 0.0,
            rng: Rng::new(singer.seed),
            sway: singer.sway_phase,
            quiet: 0,
            dist_lp: 0.0,
        }
    }

    /// Empezar (o cambiar a) una nota. `delay`: muestras de demora humana.
    fn start(&mut self, id: i32, key: f64, vel: f64, delay: i64, born: u64) {
        let legato = self.held && self.wait < 0;
        self.note_id = id;
        self.key = key;
        self.vel = vel.clamp(0.0, 1.0);
        self.born = born;
        self.mpe_bend = 0.0;
        self.mpe_pressure = 1.0;
        self.mpe_bright = 0.5;
        self.held = true;
        if !legato {
            // desde el silencio o desde una cola: entra con su demora y ataca de nuevo
            self.wait = delay.max(0);
            self.fresh = !self.active;
        }
    }

    fn release(&mut self) {
        self.held = false;
        self.note_id = -1;
        self.wait = -1;
    }

    /// Los tramos de un bloque, cada uno con su control. Anota en cuáles la voz se calculó.
    fn render_block(&mut self, ctls: &[Ctl]) {
        for (k, c) in ctls.iter().enumerate() {
            let wanted = self.active || self.wait >= 0;
            self.wanted[k] = wanted;
            if wanted {
                self.render_chunk(c, k * CHUNK);
            }
        }
    }

    /// Control + audio de un tramo, escrito en `out[off..]`. Devuelve si la voz sigue activa.
    fn render_chunk(&mut self, c: &Ctl, off: usize) -> bool {
        let n = c.n;
        if self.wait >= 0 {
            if self.wait >= n as i64 {
                self.wait -= n as i64;
            } else {
                self.wait = -1;
                self.t = 0.0;
                self.active = true;
                self.quiet = 0;
            }
        }
        if !self.active {
            self.out[off..off + n].fill(0.0);
            return false;
        }
        let sounding = self.held && self.wait < 0;

        // ---- control ----
        let s = &c.style;
        let sg = self.singer;
        self.t += c.dt;
        // deriva lenta de afinación: paseo al azar que vuelve al centro
        self.drift += self.rng.bipolar() * 0.5 - self.drift * 0.03;
        let scoop = -s.scoop_cents * (-self.t / 0.07).exp();
        let vib_in = ((self.t - s.vibrato_delay) / 0.35).clamp(0.0, 1.0);
        let cents = (sg.detune + self.drift * sg.drift) * c.humanize + scoop;
        let midi = self.key + self.mpe_bend + c.bend + cents / 100.0;
        let f0 = 440.0 * 2f64.powf((midi - 69.0) / 12.0);
        let dynamics = (0.55 + 0.9 * self.vel) * (0.5 + 0.5 * self.mpe_pressure);
        let patch = SourcePatch {
            f0: Some(f0.clamp(40.0, 2200.0)),
            glide: Some(if self.fresh { 0.002 } else { s.glide }),
            pressure: Some((s.pressure * dynamics).clamp(2.0, 18.0)),
            breathiness: Some((s.breathiness + sg.breathiness + c.breath).clamp(0.0, 1.0)),
            tension: Some((s.tension + sg.tension).clamp(0.0, 1.0)),
            fry: None,
            roughness: Some((s.roughness + sg.roughness).clamp(0.0, 1.0)),
            vibrato_depth: Some((s.vibrato_cents * sg.vibrato_depth * c.vibrato * vib_in).max(0.0)),
            vibrato_rate: Some((s.vibrato_hz * sg.vibrato_rate).clamp(3.0, 9.0)),
            drift: Some(2.0),
            voiced: Some(true),
            brightness: Some((s.brightness + sg.brightness + c.bright + 0.4 * (self.mpe_bright - 0.5)).clamp(0.0, 1.0)),
        };
        self.fresh = false;
        self.voice.set_source(&patch);
        let mut artic = self.space.at(c.vx + sg.vowel_dx * c.spread, c.vy + sg.vowel_dy * c.spread);
        if sg.velum > 0.0 {
            artic.set(ArticKey::Velum, -0.15 + 1.8 * sg.velum);
        }
        self.voice.set_articulation(&artic, None);
        self.sway += sg.sway_rate * c.dt;
        if self.sway >= 1.0 {
            self.sway -= 1.0;
        }
        let gate = 1.0 + 0.10 * c.humanize * (std::f64::consts::TAU * self.sway).sin();
        self.voice.set_gate(if sounding { gate } else { 0.0 });

        // ---- audio ----
        if self.eco {
            self.render_eco(off, n);
        } else {
            self.voice.render(&mut self.out[off..off + n]);
        }
        let g = sg.gain as f32;
        let mut peak = 0f32;
        for x in self.out[off..off + n].iter_mut() {
            if !x.is_finite() {
                *x = 0.0; // una voz que se rompe no arrastra al coro
            }
            *x *= g;
            peak = peak.max(x.abs());
        }
        // ---- ¿terminó de apagarse? ----
        if !self.held && self.wait < 0 {
            if peak < 1e-4 && self.voice.gate < 0.01 {
                self.quiet += 1;
            } else {
                self.quiet = 0;
            }
            if self.quiet > 30 {
                self.active = false;
            }
        }
        self.active
    }

    /// Modo Eco: el motor rinde a ~30 kHz y se lleva a la frecuencia del anfitrión con una
    /// interpolación cúbica (Hermite de 4 puntos).
    fn render_eco(&mut self, off: usize, n: usize) {
        let need = (self.frac + n as f64 * self.ratio).floor() as usize;
        let need = need.min(CHUNK + 8);
        self.voice.render(&mut self.ebuf[..need]);
        let mut k = 0;
        for i in 0..n {
            self.frac += self.ratio;
            while self.frac >= 1.0 {
                self.frac -= 1.0;
                self.hist = [self.hist[1], self.hist[2], self.hist[3], if k < need { self.ebuf[k] } else { self.hist[3] }];
                k += 1;
            }
            let [y0, y1, y2, y3] = self.hist;
            let t = self.frac as f32;
            let c1 = 0.5 * (y2 - y0);
            let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
            self.out[off + i] = ((c3 * t + c2) * t + c1) * t + y1;
        }
    }
}

// ------------------------------------------------------------------ hilos

struct Job {
    gen: u64,
    quit: bool,
}

struct Shared {
    slots: Vec<Mutex<Option<Slot>>>,
    job: Mutex<Job>,
    cv: Condvar,
    /// controles de cada tramo del bloque y cuántos tramos tiene
    ctl: Mutex<([Ctl; BLOCK_CHUNKS], usize)>,
    list: Mutex<[usize; MAX_SINGERS]>,
    next: AtomicUsize,
    total: AtomicUsize,
    done: AtomicUsize,
    busy: AtomicUsize,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
    /// Toma voces de la lista hasta que no queden. Lo corren los ayudantes y el hilo de audio.
    fn work(&self) {
        let (ctls, nc) = *lock(&self.ctl);
        loop {
            let i = self.next.fetch_add(1, Ordering::AcqRel);
            if i >= self.total.load(Ordering::Acquire) {
                break;
            }
            let idx = lock(&self.list)[i];
            if let Some(slot) = lock(&self.slots[idx]).as_mut() {
                slot.render_block(&ctls[..nc]);
            }
            self.done.fetch_add(1, Ordering::AcqRel);
        }
    }
}

fn worker(shared: Arc<Shared>, init: Option<fn()>) {
    if let Some(f) = init {
        f();
    }
    let mut seen = 0u64;
    loop {
        {
            let mut j = lock(&shared.job);
            while j.gen == seen && !j.quit {
                j = shared.cv.wait(j).unwrap_or_else(|e| e.into_inner());
            }
            if j.quit {
                return;
            }
            seen = j.gen;
        }
        shared.work();
        shared.busy.fetch_sub(1, Ordering::AcqRel);
    }
}

// ------------------------------------------------------------------ coro

pub struct Choir {
    fs: f64,
    params: ChoirParams,
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
    room: Room,
    /// cantantes construidos
    count: usize,
    order: u64,
    rr: usize,
    /// teclas apretadas (Tutti y Solo): id, tecla, velocidad
    stack: [(i32, u8, f64); STACK],
    stack_len: usize,
    bend: f64,
    vowel_phase: f64,
    active: [usize; MAX_SINGERS],
    n_active: usize,
    thread_init: Option<fn()>,
    /// nivel reciente de cada cantante
    levels: [f32; MAX_SINGERS],
}

impl Choir {
    /// `threads`: hilos para calcular voces (1 = todo en el hilo de audio).
    pub fn new(sample_rate: f64, threads: usize) -> Choir {
        Choir::with_thread_init(sample_rate, threads, None)
    }

    /// Como `new`, pero cada hilo ayudante corre `init` al arrancar (por ejemplo, para pedirle
    /// al sistema prioridad de audio). El núcleo no sabe nada del sistema operativo.
    pub fn with_thread_init(sample_rate: f64, threads: usize, init: Option<fn()>) -> Choir {
        chimera::init();
        let ctl = Ctl { style: Style::Clasico.def(), vx: 0.45, vy: 1.0, spread: 0.0, vibrato: 1.0, breath: 0.0, bright: 0.0, humanize: 0.0, bend: 0.0, n: 0, dt: 0.0 };
        let shared = Arc::new(Shared {
            slots: (0..MAX_SINGERS).map(|_| Mutex::new(None)).collect(),
            job: Mutex::new(Job { gen: 0, quit: false }),
            cv: Condvar::new(),
            ctl: Mutex::new(([ctl; BLOCK_CHUNKS], 0)),
            list: Mutex::new([0; MAX_SINGERS]),
            next: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            done: AtomicUsize::new(0),
            busy: AtomicUsize::new(0),
        });
        let mut choir = Choir {
            fs: sample_rate,
            params: ChoirParams::DEFAULT,
            shared,
            workers: Vec::new(),
            room: Room::new(sample_rate),
            count: 0,
            order: 0,
            rr: 0,
            stack: [(0, 0, 0.0); STACK],
            stack_len: 0,
            bend: 0.0,
            vowel_phase: 0.0,
            active: [0; MAX_SINGERS],
            n_active: 0,
            thread_init: init,
            levels: [0.0; MAX_SINGERS],
        };
        choir.set_threads(threads);
        choir.rebuild();
        choir.room.set_kind(choir.params.room);
        choir
    }

    /// Cambia la cantidad de hilos (reserva: no llamar mientras suena).
    pub fn set_threads(&mut self, threads: usize) {
        let want = threads.clamp(1, 32) - 1;
        if want == self.workers.len() {
            return;
        }
        self.stop_workers();
        for i in 0..want {
            let sh = self.shared.clone();
            let init = self.thread_init;
            if let Ok(h) = std::thread::Builder::new().name(format!("orfeon-voz-{i}")).spawn(move || worker(sh, init)) {
                self.workers.push(h);
            }
        }
    }

    pub fn threads(&self) -> usize {
        self.workers.len() + 1
    }

    fn stop_workers(&mut self) {
        if self.workers.is_empty() {
            return;
        }
        lock(&self.shared.job).quit = true;
        self.shared.cv.notify_all();
        for h in self.workers.drain(..) {
            let _ = h.join();
        }
        lock(&self.shared.job).quit = false;
    }

    pub fn sample_rate(&self) -> f64 {
        self.fs
    }
    pub fn params(&self) -> &ChoirParams {
        &self.params
    }
    /// Cantantes construidos.
    pub fn singers(&self) -> usize {
        self.count
    }
    pub fn singer(&self, i: usize) -> Option<Singer> {
        if i >= self.count {
            return None;
        }
        lock(&self.shared.slots[i]).as_ref().map(|s| s.singer)
    }
    /// Voces que se calcularon en el último tramo.
    pub fn sounding(&self) -> usize {
        self.n_active
    }

    /// Aplica parámetros. Si cambió algo estructural reconstruye las voces (reserva memoria y
    /// corta las notas).
    pub fn set_params(&mut self, p: ChoirParams) {
        let rebuild = p.structural() != self.params.structural();
        let faces = p.custom != self.params.custom;
        let mode_changed = p.mode != self.params.mode;
        self.params = p;
        self.room.set_kind(p.room);
        if rebuild {
            self.rebuild();
        } else if faces {
            self.rebuild_changed();
        } else if mode_changed {
            self.all_notes_off();
        }
    }

    fn rebuild(&mut self) {
        let p = self.params;
        let n = p.singers.clamp(1, MAX_SINGERS);
        for i in 0..MAX_SINGERS {
            let slot = if i < n { Some(Slot::new(self.fs, p.singer(i), p.quality)) } else { None };
            *lock(&self.shared.slots[i]) = slot;
        }
        self.count = n;
        self.stack_len = 0;
        self.rr = 0;
        self.n_active = 0;
        self.levels = [0.0; MAX_SINGERS];
    }

    /// Sólo un rostro cambió: se reconstruyen los cantantes que cambiaron; los demás siguen
    /// cantando sin cortes.
    fn rebuild_changed(&mut self) {
        let p = self.params;
        for i in 0..self.count {
            let s = p.singer(i);
            let mut g = lock(&self.shared.slots[i]);
            if g.as_ref().map_or(true, |old| old.singer != s) {
                *g = Some(Slot::new(self.fs, s, p.quality));
                self.levels[i] = 0.0;
            }
        }
    }

    /// Nivel reciente de cada cantante (0..1 aprox.), para animar una interfaz.
    pub fn levels(&self) -> &[f32; MAX_SINGERS] {
        &self.levels
    }

    fn slot(&self, i: usize) -> MutexGuard<'_, Option<Slot>> {
        lock(&self.shared.slots[i])
    }

    fn voices_limit(&self) -> usize {
        self.params.max_voices.clamp(1, MAX_SINGERS).min(self.count)
    }

    // ---------------------------------------------------------------- notas

    /// Tecla apretada. `id` identifica la nota para soltarla y para la expresión por nota
    /// (MPE); `velocity` 0..1.
    pub fn note_on(&mut self, id: i32, key: u8, velocity: f64) {
        let id = id.max(0);
        match self.params.mode {
            Mode::Coro => self.note_on_choir(id, key, velocity),
            Mode::Tutti | Mode::Solo => {
                self.stack_remove(id);
                if self.stack_len == STACK {
                    self.stack.copy_within(1.., 0);
                    self.stack_len -= 1;
                }
                self.stack[self.stack_len] = (id, key, velocity);
                self.stack_len += 1;
                self.sing_top();
            }
        }
    }

    pub fn note_off(&mut self, id: i32) {
        let id = id.max(0);
        match self.params.mode {
            Mode::Coro => {
                for i in 0..self.count {
                    if let Some(s) = self.slot(i).as_mut() {
                        if s.note_id == id && s.held {
                            s.release();
                        }
                    }
                }
            }
            Mode::Tutti | Mode::Solo => {
                self.stack_remove(id);
                self.sing_top();
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.stack_len = 0;
        for i in 0..self.count {
            if let Some(s) = self.slot(i).as_mut() {
                if s.held {
                    s.release();
                }
            }
        }
    }

    fn stack_remove(&mut self, id: i32) {
        if let Some(k) = self.stack[..self.stack_len].iter().position(|e| e.0 == id) {
            self.stack.copy_within(k + 1..self.stack_len, k);
            self.stack_len -= 1;
        }
    }

    /// Tutti / Solo: todos (o el solista) cantan la última tecla apretada.
    fn sing_top(&mut self) {
        let n = if self.params.mode == Mode::Solo { 1 } else { self.voices_limit() };
        if self.stack_len == 0 {
            for i in 0..self.count {
                if let Some(s) = self.slot(i).as_mut() {
                    if s.held {
                        s.release();
                    }
                }
            }
            return;
        }
        let (_, key, vel) = self.stack[self.stack_len - 1];
        self.order += 1;
        let order = self.order;
        for i in 0..n {
            let mut g = self.slot(i);
            if let Some(s) = g.as_mut() {
                let delay = (s.singer.lateness * self.params.humanize * 0.035 * self.fs) as i64;
                let k = s.singer.fold(key as f64);
                s.start(TOP_ID, k, vel, delay, order);
            }
        }
    }

    /// Coro: la nota va al cantante siguiente que esté libre, prefiriendo a quien le quede en
    /// registro. Si no hay libres (o se llegó al tope), toma el lugar de la nota más vieja.
    fn note_on_choir(&mut self, id: i32, key: u8, vel: f64) {
        let n = self.count;
        let limit = self.voices_limit();
        let mut held = 0;
        let mut repeated = None;
        let mut oldest: Option<(usize, u64)> = None;
        let mut best: Option<(usize, f64)> = None;
        for j in 0..n {
            let i = (self.rr + j) % n;
            let g = self.slot(i);
            let Some(s) = g.as_ref() else { continue };
            if s.held {
                if s.note_id == id {
                    repeated = Some(i);
                    break;
                }
                held += 1;
                if oldest.map_or(true, |(_, b)| s.born < b) {
                    oldest = Some((i, s.born));
                }
                continue;
            }
            let (lo, hi) = s.singer.voice_type.range();
            let out = if key < lo { (lo - key) as f64 } else if key > hi { (key - hi) as f64 } else { 0.0 };
            // en registro primero; entre iguales, el que no está apagando una cola, y el turno
            let mut score = out * 10.0 + if s.active { 1.0 } else { 0.0 } + j as f64 * 0.001;
            if self.params.voice_leading {
                let center = (f64::from(lo) + f64::from(hi)) * 0.5;
                let target = s.singer.fold(f64::from(key));
                let previous = if s.born > 0 { s.key } else { center };
                score += (target - previous).abs() + 0.25 * (target - center).abs();
            }
            if best.map_or(true, |(_, b)| score < b) {
                best = Some((i, score));
            }
        }
        let pick = repeated.or_else(|| {
            if held >= limit { oldest.map(|o| o.0) } else { best.map(|b| b.0).or(oldest.map(|o| o.0)) }
        });
        let Some(i) = pick else { return };
        self.order += 1;
        let order = self.order;
        let mut g = self.slot(i);
        if let Some(s) = g.as_mut() {
            let human_delay = s.singer.lateness * self.params.humanize * 0.035;
            let (low, _) = s.singer.voice_type.range();
            // Bajo a soprano ocupa tres separaciones de ataque (E2..C4).
            let register = f64::from(low.saturating_sub(40).min(20)) * 3.0 / 20.0;
            let stagger_ms = self.params.stagger_ms;
            let stagger = if stagger_ms.is_finite() { stagger_ms.clamp(0.0, 40.0) } else { 0.0 };
            let delay = ((human_delay + register * stagger / 1000.0) * self.fs) as i64;
            let k = s.singer.fold(key as f64);
            if s.held {
                s.release(); // repetida o robada: ataca de nuevo
            }
            s.start(id, k, vel, delay, order);
        }
        drop(g);
        if repeated.is_none() {
            self.rr = (i + 1) % n;
        }
    }

    // ---------------------------------------------------------------- expresión

    /// Rueda de afinación global (semitonos).
    pub fn set_pitch_bend(&mut self, semitones: f64) {
        self.bend = if semitones.is_finite() { semitones.clamp(-48.0, 48.0) } else { 0.0 };
    }

    fn for_note(&mut self, id: i32, f: impl Fn(&mut Slot)) {
        let id = id.max(0);
        let mono = self.params.mode != Mode::Coro;
        for i in 0..self.count {
            if let Some(s) = self.slot(i).as_mut() {
                if s.held && (s.note_id == id || (mono && s.note_id == TOP_ID)) {
                    f(s);
                }
            }
        }
    }

    /// Afinación de una nota (MPE), en semitonos.
    pub fn note_pitch_bend(&mut self, id: i32, semitones: f64) {
        if semitones.is_finite() {
            self.for_note(id, |s| s.mpe_bend = semitones.clamp(-48.0, 48.0));
        }
    }
    /// Presión de una nota (MPE), 0..1: intensidad.
    pub fn note_pressure(&mut self, id: i32, pressure: f64) {
        if pressure.is_finite() {
            self.for_note(id, |s| s.mpe_pressure = pressure.clamp(0.0, 1.0));
        }
    }
    /// Timbre de una nota (MPE), 0..1: brillo.
    pub fn note_brightness(&mut self, id: i32, brightness: f64) {
        if brightness.is_finite() {
            self.for_note(id, |s| s.mpe_bright = brightness.clamp(0.0, 1.0));
        }
    }

    // ---------------------------------------------------------------- audio

    /// Rinde `left.len()` muestras estéreo (los dos deben medir lo mismo).
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let total = left.len().min(right.len());
        let mut pos = 0;
        while pos < total {
            let n = (total - pos).min(BLOCK);
            self.render_block(&mut left[pos..pos + n], &mut right[pos..pos + n]);
            pos += n;
        }
    }

    /// Un bloque: hasta BLOCK_CHUNKS tramos de control. Las voces se calculan todas juntas
    /// (en paralelo) y después se mezclan tramo por tramo.
    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let total = left.len();
        let p = self.params;

        // ---- los controles de cada tramo (la vocal se mueve tramo a tramo) ----
        let mut ctls = [Ctl { style: p.style.def(), vx: 0.0, vy: 0.0, spread: 0.0, vibrato: 0.0, breath: 0.0, bright: 0.0, humanize: 0.0, bend: 0.0, n: 0, dt: 0.0 }; BLOCK_CHUNKS];
        let mut nc = 0;
        let mut pos = 0;
        while pos < total {
            let n = (total - pos).min(CHUNK);
            let dt = n as f64 / self.fs;
            self.vowel_phase += p.vowel_rate.clamp(0.0, 20.0) * dt;
            if self.vowel_phase >= 1.0 {
                self.vowel_phase -= self.vowel_phase.floor();
            }
            let a = std::f64::consts::TAU * self.vowel_phase;
            let mv = p.vowel_move.clamp(0.0, 1.0) * 0.4;
            ctls[nc] = Ctl {
                style: p.style.def(),
                vx: p.vowel_x + mv * a.cos(),
                vy: p.vowel_y + mv * a.sin(),
                spread: p.vowel_spread.clamp(0.0, 1.0),
                vibrato: p.vibrato.clamp(0.0, 2.0),
                breath: p.breath.clamp(-0.5, 0.5),
                bright: p.brightness.clamp(-0.5, 0.5),
                humanize: p.humanize.clamp(0.0, 1.0),
                bend: self.bend,
                n,
                dt,
            };
            nc += 1;
            pos += n;
        }

        // ---- qué voces hay que calcular (dentro del bloque no llegan notas nuevas) ----
        let mut k = 0;
        for i in 0..self.count {
            let wanted = lock(&self.shared.slots[i]).as_ref().map_or(false, |s| s.active || s.wait >= 0);
            if wanted {
                self.active[k] = i;
                k += 1;
            }
        }

        // ---- calcularlas: una sola vez por bloque se despierta a los ayudantes ----
        if k > 0 {
            let sh = &self.shared;
            *lock(&sh.ctl) = (ctls, nc);
            *lock(&sh.list) = self.active;
            sh.done.store(0, Ordering::Release);
            sh.next.store(0, Ordering::Release);
            sh.total.store(k, Ordering::Release);
            let helpers = self.workers.len();
            if helpers > 0 && k > 1 {
                sh.busy.store(helpers, Ordering::Release);
                lock(&sh.job).gen += 1;
                sh.cv.notify_all();
                sh.work();
                // este hilo también trabajó; espera a que los ayudantes suelten lo suyo
                let mut spins = 0u32;
                while sh.done.load(Ordering::Acquire) < k || sh.busy.load(Ordering::Acquire) > 0 {
                    spins += 1;
                    if spins < 2000 {
                        std::hint::spin_loop();
                    } else {
                        std::thread::yield_now();
                    }
                }
            } else {
                sh.work();
            }
            sh.total.store(0, Ordering::Release);
        }

        // ---- mezcla, tramo por tramo ----
        for l in self.levels.iter_mut() {
            *l *= 0.8;
        }
        let mut off = 0;
        for c in 0..nc {
            let n = ctls[c].n;
            self.n_active = self.mix_chunk(c, k, &mut left[off..off + n], &mut right[off..off + n]);
            off += n;
        }
    }

    /// Mezcla el tramo `c` del bloque: las voces que se calcularon en él, en su lugar del
    /// escenario, más la sala. Devuelve cuántas voces sonaron en el tramo.
    fn mix_chunk(&mut self, c: usize, listed: usize, left: &mut [f32], right: &mut [f32]) -> usize {
        let n = left.len();
        let p = self.params;
        let base = c * CHUNK;
        let mut sounding = 0;
        // ---- escenario: cada voz en su lugar, y lo que le llega a la sala ----
        const SCR: usize = CHUNK;
        let mut dry_l = [0f32; SCR];
        let mut dry_r = [0f32; SCR];
        let mut snd_l = [0f32; SCR];
        let mut snd_r = [0f32; SCR];
        let mic = p.mic.clamp(0.0, 1.0);
        let width = p.width.clamp(0.0, 1.0) * (1.0 - 0.45 * mic);
        for a in 0..listed {
            let idx = self.active[a];
            let shared = self.shared.clone();
            let mut g = lock(&shared.slots[idx]);
            let Some(s) = g.as_mut() else { continue };
            if !s.wanted[c] {
                continue;
            }
            sounding += 1;
            let mut lv = 0f32;
            for x in &s.out[base..base + n] {
                lv = lv.max(x.abs());
            }
            if lv > self.levels[idx] {
                self.levels[idx] = lv;
            }
            let dist = (0.5 * mic + 0.6 * s.singer.depth * p.depth.clamp(0.0, 1.0)).clamp(0.0, 1.1);
            let pan = (s.singer.pan * width).clamp(-1.0, 1.0);
            let ang = (pan + 1.0) * std::f64::consts::FRAC_PI_4;
            let direct = 1.0 / (1.0 + 1.2 * dist);
            let gl = (ang.cos() * direct * std::f64::consts::SQRT_2) as f32;
            let gr = (ang.sin() * direct * std::f64::consts::SQRT_2) as f32;
            let send = (0.25 + 0.9 * dist) as f32;
            // lejos se pierden agudos
            let lp = (1.0 - 0.55 * dist) as f32;
            let mut z = s.dist_lp;
            for i in 0..n {
                z += lp * (s.out[base + i] - z);
                dry_l[i] += z * gl;
                dry_r[i] += z * gr;
                snd_l[i] += z * send * (ang.cos() as f32);
                snd_r[i] += z * send * (ang.sin() as f32);
            }
            s.dist_lp = if z.abs() < 1e-20 { 0.0 } else { z };
        }

        // ---- sala, micrófono y salida ----
        let wet = (p.room_mix.clamp(0.0, 1.0) * (0.5 + 1.1 * mic)) as f32;
        // de lejos llega menos sonido directo: se compensa para que alejar el micrófono cambie
        // el carácter y no el volumen
        let master = (10f64.powf(p.gain_db.clamp(-48.0, 24.0) / 20.0) * (1.0 + 1.1 * mic)) as f32;
        // en Tutti cantan todos a la vez la misma nota: se baja para no saturar
        let master = if p.mode == Mode::Tutti { master * 0.6 } else { master };
        for i in 0..n {
            let (wl, wr) = self.room.tick(snd_l[i], snd_r[i]);
            left[i] = soft((dry_l[i] + wl * wet) * master);
            right[i] = soft((dry_r[i] + wr * wet) * master);
        }
        sounding
    }
}

/// Limitador suave: lineal hasta 0.75 y de ahí se curva hacia 1.
#[inline]
fn soft(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.75 {
        x
    } else {
        x.signum() * (0.75 + 0.25 * ((a - 0.75) / 0.25).tanh())
    }
}

impl Drop for Choir {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

#[cfg(test)]
mod phrasing_tests {
    use super::*;

    #[test]
    fn cuarteto_conserva_tono_comun_y_prefiere_el_movimiento_corto() {
        let mut choir = Choir::new(48_000.0, 1);
        let preset = crate::preset::find("Cuarteto coral").unwrap();
        choir.set_params(ChoirParams { humanize: 0.0, ..preset.params });
        for key in [48, 55, 64, 72] {
            choir.note_on(i32::from(key), key, 0.75);
        }
        // Mixto ordena soprano, bajo, contralto y tenor.
        let state = |choir: &Choir, i| {
            let slot = choir.slot(i);
            let slot = slot.as_ref().unwrap();
            (slot.note_id, slot.born, slot.wait)
        };
        assert_eq!(state(&choir, 1).0, 48);
        assert_eq!(state(&choir, 3).0, 55);
        assert_eq!(state(&choir, 2).0, 64);
        assert_eq!(state(&choir, 0).0, 72);
        assert!(state(&choir, 1).2 < state(&choir, 3).2);
        assert!(state(&choir, 3).2 < state(&choir, 2).2);
        assert!(state(&choir, 2).2 < state(&choir, 0).2);

        let soprano = state(&choir, 0);
        choir.note_off(55);
        choir.note_off(64);
        choir.note_on(63, 63, 0.75);
        assert_eq!(state(&choir, 2).0, 63);
        assert_eq!(state(&choir, 0), soprano);
        choir.all_notes_off();
        assert!((0..4).all(|i| state(&choir, i).0 == -1));
    }
}
