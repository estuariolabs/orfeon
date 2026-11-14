// En Windows la app es de ventana: sin consola negra detrás (con --consola se engancha a la
// consola desde la que se la abrió).
#![cfg_attr(windows, windows_subsystem = "windows")]

//! Orfeón independiente: abre la salida de audio por defecto, escucha un teclado MIDI si hay
//! uno y muestra la interfaz (la misma del plugin). Con `--consola` funciona sin ventana,
//! por comandos de texto. No necesita un DAW.
//!
//! Hilos:
//! - "orfeon-audio" es dueño del coro: lo calcula por bloques y lo deja en una cola. Corre con
//!   prioridad de audio. Consola y MIDI le mandan mensajes por un canal.
//! - la placa de sonido sólo copia de la cola. Así una demora del cálculo (un hilo que el
//!   sistema despertó tarde, un cambio de programa) la absorbe el colchón y no se oye.
//! - los ayudantes del coro calculan voces en paralelo, también con prioridad de audio.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use orfeon_core::console::{self, Command, DEMO, HELP};
use orfeon_core::preset::PRESETS;
use orfeon_core::{Choir, ChoirParams, Mode, Quality};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

// En Windows, sin ventana negra de consola detrás de la interfaz.
#[cfg(windows)]
fn attach_console() {
    // SAFETY: si la app se abrió desde una consola, se engancha a ella (modo --consola).
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS);
    }
}
#[cfg(not(windows))]
fn attach_console() {}

/// Muestras por cálculo del coro (un múltiplo del tramo de 128).
const PRODUCE: usize = 512;
/// Colchón de audio por defecto (ms).
const DEFAULT_LATENCY_MS: u32 = 40;
/// Carga máxima que se acepta al medir la PC: deja margen para el sistema y los picos.
const LOAD_TARGET: f64 = 0.55;

/// Lo que la consola y el MIDI le piden al hilo de audio.
enum Msg {
    On(i32, u8, f64),
    Off(i32),
    Bend(f64),
    AllOff,
    Params(ChoirParams),
    Threads(usize),
    Lyrics(orfeon_core::lyrics::Lyrics),
    ResetLyrics,
}

/// Medidores compartidos entre los hilos.
struct Meters {
    /// carga media y máxima del cálculo, en milésimas del tiempo disponible
    load: AtomicU32,
    load_max: AtomicU32,
    voices: AtomicUsize,
    /// pico de salida en milésimas
    peak: AtomicU32,
    /// veces que la placa pidió audio y la cola estaba vacía (cada una es un corte)
    underruns: AtomicU32,
    /// colchón pedido, en muestras
    latency: AtomicUsize,
    quit: AtomicBool,
    /// nivel de cada cantante (bits de f32) y notas sonando, para la interfaz
    levels: [AtomicU32; orfeon_core::MAX_SINGERS],
    held: [AtomicBool; 128],
    threads: AtomicUsize,
}

impl Default for Meters {
    fn default() -> Self {
        Meters {
            load: AtomicU32::new(0),
            load_max: AtomicU32::new(0),
            voices: AtomicUsize::new(0),
            peak: AtomicU32::new(0),
            underruns: AtomicU32::new(0),
            latency: AtomicUsize::new(0),
            quit: AtomicBool::new(false),
            levels: std::array::from_fn(|_| AtomicU32::new(0)),
            held: std::array::from_fn(|_| AtomicBool::new(false)),
            threads: AtomicUsize::new(1),
        }
    }
}

// ------------------------------------------------------------------ prioridad (Windows)

#[cfg(windows)]
fn promote_thread() {
    use windows_sys::Win32::System::Threading::{AvSetMmThreadCharacteristicsW, GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
    let name: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
    let mut index = 0u32;
    // SAFETY: llamadas del sistema sobre el hilo actual, con una cadena terminada en 0.
    unsafe {
        let h = AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut index);
        if h.is_null() {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        }
    }
}

#[cfg(not(windows))]
fn promote_thread() {}

#[cfg(windows)]
fn fine_timer() {
    // SAFETY: pide al sistema un reloj de 1 ms (las esperas cortas dejan de durar 15 ms).
    unsafe {
        windows_sys::Win32::Media::timeBeginPeriod(1);
    }
}

#[cfg(not(windows))]
fn fine_timer() {}

// ------------------------------------------------------------------ cálculo del coro

/// El hilo que calcula: atiende mensajes y mantiene la cola llena hasta el colchón pedido.
fn producer(mut choir: Choir, rx: Receiver<Msg>, mut out: rtrb::Producer<f32>, sr: f64, meters: Arc<Meters>) {
    promote_thread();
    let mut l = vec![0f32; PRODUCE];
    let mut r = vec![0f32; PRODUCE];
    let mut avg = 0.0f64;
    let cap = out.buffer().capacity();
    while !meters.quit.load(Ordering::Relaxed) {
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::On(id, key, vel) => {
                    choir.note_on(id, key, vel);
                    meters.held[(key & 127) as usize].store(true, Ordering::Relaxed);
                }
                Msg::Off(id) => {
                    choir.note_off(id);
                    // los id de MIDI y de la pantalla son canal*128 + nota
                    meters.held[id.rem_euclid(128) as usize].store(false, Ordering::Relaxed);
                }
                Msg::Bend(s) => choir.set_pitch_bend(s),
                Msg::AllOff => {
                    choir.all_notes_off();
                    for h in meters.held.iter() {
                        h.store(false, Ordering::Relaxed);
                    }
                }
                Msg::Params(p) => choir.set_params(p),
                Msg::Threads(n) => choir.set_threads(n),
                Msg::Lyrics(bank) => { choir.replace_lyrics(bank); }
                Msg::ResetLyrics => choir.reset_lyrics(),
            }
        }
        let queued = (cap - out.slots()) / 2;
        let target = meters.latency.load(Ordering::Relaxed);
        if queued >= target || out.slots() < PRODUCE * 2 {
            // la cola tiene suficiente: dormir hasta que la placa consuma (ella despierta)
            std::thread::park_timeout(Duration::from_millis(2));
            continue;
        }
        let t0 = Instant::now();
        choir.render(&mut l, &mut r);
        let load = t0.elapsed().as_secs_f64() / (PRODUCE as f64 / sr);
        avg += 0.05 * (load - avg);
        if let Ok(mut chunk) = out.write_chunk_uninit(PRODUCE * 2) {
            let (a, b) = chunk.as_mut_slices();
            for (k, slot) in a.iter_mut().chain(b.iter_mut()).enumerate() {
                let i = k / 2;
                slot.write(if k % 2 == 0 { l[i] } else { r[i] });
            }
            // SAFETY: se escribieron las PRODUCE*2 posiciones pedidas.
            unsafe { chunk.commit_all() };
        }
        meters.load.store((avg * 1000.0) as u32, Ordering::Relaxed);
        meters.load_max.fetch_max((load * 1000.0) as u32, Ordering::Relaxed);
        meters.voices.store(choir.sounding(), Ordering::Relaxed);
        for (a, l) in meters.levels.iter().zip(choir.levels().iter()) {
            a.store(l.to_bits(), Ordering::Relaxed);
        }
        let pk = l.iter().chain(r.iter()).fold(0f32, |a, x| a.max(x.abs()));
        meters.peak.fetch_max((pk * 1000.0) as u32, Ordering::Relaxed);
    }
}

// ------------------------------------------------------------------ salida de audio

struct Output {
    queue: rtrb::Consumer<f32>,
    producer: std::thread::Thread,
    meters: Arc<Meters>,
    started: bool,
}

/// La placa pide `data` (intercalado, `channels` canales): copiar de la cola, nada más.
fn write<T: cpal::SizedSample + cpal::FromSample<f32>>(o: &mut Output, data: &mut [T], channels: usize) {
    let channels = channels.max(1);
    let mut short = false;
    for frame in data.chunks_mut(channels) {
        let (l, r) = if o.queue.slots() >= 2 {
            o.started = true;
            (o.queue.pop().unwrap_or(0.0), o.queue.pop().unwrap_or(0.0))
        } else {
            short = o.started;
            (0.0, 0.0)
        };
        for (c, out) in frame.iter_mut().enumerate() {
            let x = match c {
                0 if channels == 1 => 0.5 * (l + r),
                0 => l,
                1 => r,
                _ => 0.0,
            };
            *out = T::from_sample(x);
        }
    }
    if short {
        o.meters.underruns.fetch_add(1, Ordering::Relaxed);
    }
    o.producer.unpark();
}

fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(device: &cpal::Device, cfg: &cpal::StreamConfig, mut o: Output) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = cfg.channels as usize;
    device.build_output_stream(cfg, move |data: &mut [T], _: &cpal::OutputCallbackInfo| write(&mut o, data, channels), |e| eprintln!("\n[audio] {e}"), None)
}

// ------------------------------------------------------------------ medir la PC

/// Cuántas voces a la vez aguanta esta máquina con esa calidad, dejando margen.
fn measure(sr: f64, threads: usize, base: ChoirParams, quality: Quality) -> usize {
    let mut choir = Choir::with_thread_init(sr, threads, Some(promote_thread));
    let mut l = vec![0f32; PRODUCE];
    let mut r = vec![0f32; PRODUCE];
    let mut best = 1;
    for n in [1usize, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 32, 48, 64] {
        let p = ChoirParams { mode: Mode::Coro, singers: n.max(base.singers).min(64), max_voices: n, quality, humanize: 0.0, ..base };
        choir.set_params(p);
        choir.all_notes_off();
        for i in 0..n {
            choir.note_on(i as i32, 48 + ((i * 5) % 30) as u8, 0.7);
        }
        // que entren todas antes de medir
        for _ in 0..(0.1 * sr) as usize / PRODUCE {
            choir.render(&mut l, &mut r);
        }
        let blocks = ((0.25 * sr) as usize / PRODUCE).max(4);
        let t0 = Instant::now();
        for _ in 0..blocks {
            choir.render(&mut l, &mut r);
        }
        let load = t0.elapsed().as_secs_f64() / (blocks * PRODUCE) as f64 * sr;
        if load > LOAD_TARGET {
            break;
        }
        best = n;
    }
    best
}

// ------------------------------------------------------------------ MIDI

fn midi_ports() -> Vec<String> {
    match midir::MidiInput::new("Orfeon") {
        Ok(input) => input.ports().iter().map(|p| input.port_name(p).unwrap_or_else(|_| "(sin nombre)".into())).collect(),
        Err(_) => Vec::new(),
    }
}

/// Se conecta a la entrada `index` (desde 0). La conexión vive mientras se guarde el valor.
fn midi_connect(index: usize, tx: Sender<Msg>, producer: std::thread::Thread) -> Result<(midir::MidiInputConnection<()>, String), String> {
    let mut input = midir::MidiInput::new("Orfeon").map_err(|e| e.to_string())?;
    input.ignore(midir::Ignore::All);
    let ports = input.ports();
    let port = ports.get(index).ok_or_else(|| format!("no hay una entrada MIDI {}", index + 1))?;
    let name = input.port_name(port).unwrap_or_else(|_| "(sin nombre)".into());
    let monitor = if std::env::var_os("ORFEON_MONITOR_MIDI").is_some() {
        let (log, records) = std::sync::mpsc::sync_channel::<(u64, [u8; 3], usize, u64)>(4096);
        let port_name = name.clone();
        std::thread::Builder::new().name("orfeon-midi-monitor".into()).spawn(move || {
            eprintln!("MIDI monitor: {port_name}");
            let mut notes = [[false; 128]; 16];
            for (stamp, bytes, len, dropped) in records {
                if dropped > 0 {
                    eprintln!("MIDI monitor: {dropped} registros omitidos; contador parcial hasta el próximo CC 120/123");
                }
                let [status, note, velocity] = bytes;
                if len == 3 && note < 128 {
                    match status & 0xF0 {
                        0x80 | 0x90 => {
                            notes[(status & 15) as usize][note as usize] =
                                status & 0xF0 == 0x90 && velocity > 0;
                        }
                        0xB0 if note == 120 || note == 123 => notes = [[false; 128]; 16],
                        _ => {}
                    }
                }
                let held = notes.iter().flatten().filter(|&&held| held).count();
                eprintln!("MIDI {stamp:>12} μs {:02X?}  notas recibidas sostenidas: {held}", &bytes[..len]);
            }
        }).map_err(|e| e.to_string())?;
        Some(log)
    } else {
        None
    };
    let mut lost_records = 0u64;
    let conn = input
        .connect(
            port,
            "orfeon-entrada",
            move |stamp, msg, _| {
                if let Some(log) = &monitor {
                    let mut bytes = [0; 3];
                    let len = msg.len().min(bytes.len());
                    bytes[..len].copy_from_slice(&msg[..len]);
                    if log.try_send((stamp, bytes, len, lost_records)).is_ok() {
                        lost_records = 0;
                    } else {
                        lost_records += 1;
                    }
                }
                if msg.len() < 3 {
                    return;
                }
                let (status, ch) = (msg[0] & 0xF0, (msg[0] & 0x0F) as i32);
                let d1 = msg[1] & 0x7F;
                let d2 = msg[2] & 0x7F;
                let id = ch * 128 + d1 as i32;
                let m = match status {
                    0x90 if d2 > 0 => Some(Msg::On(id, d1, d2 as f64 / 127.0)),
                    0x80 | 0x90 => Some(Msg::Off(id)),
                    // rueda de afinación: 14 bits, ±2 semitonos
                    0xE0 => Some(Msg::Bend((((d2 as i32) << 7 | d1 as i32) - 8192) as f64 / 8192.0 * 2.0)),
                    // CC 120 / 123: todo fuera
                    0xB0 if d1 == 120 || d1 == 123 => Some(Msg::AllOff),
                    _ => None,
                };
                if let Some(m) = m {
                    let _ = tx.send(m);
                    producer.unpark();
                }
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    Ok((conn, name))
}

// ------------------------------------------------------------------ consola

/// Toca notas un rato desde un hilo aparte (para `n` y `demo`).
fn play_later(tx: &Sender<Msg>, start: f64, secs: f64, keys: Vec<u8>, first_id: i32) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs_f64(start));
        for (i, k) in keys.iter().enumerate() {
            let _ = tx.send(Msg::On(first_id + i as i32, *k, 0.7));
        }
        std::thread::sleep(Duration::from_secs_f64(secs));
        for i in 0..keys.len() {
            let _ = tx.send(Msg::Off(first_id + i as i32));
        }
    });
}

/// Lo que queda andando después de arrancar el audio.
struct Running {
    tx: Sender<Msg>,
    producer: std::thread::Thread,
    worker: std::thread::JoinHandle<()>,
    meters: Arc<Meters>,
    stream: Option<cpal::Stream>,
    params: ChoirParams,
    sr: f64,
    threads: usize,
    audio_name: String,
    channels: u16,
    status: Option<String>,
}

/// Abre el audio, mide la PC y arranca el hilo del coro. Si no hay salida de audio, el coro
/// arranca igual (sin sonido) y `status` lo explica.
fn start(verbose: bool) -> Running {
    fine_timer();
    let host = cpal::default_host();
    let device = host.default_output_device();
    let mut status = None;
    let (cfg, format) = match device.as_ref().map(|d| d.default_output_config()) {
        Some(Ok(c)) => {
            let f = c.sample_format();
            (Some(c.into()), Some(f))
        }
        Some(Err(e)) => {
            status = Some(format!("No pude leer la salida de audio: {e}"));
            (None, None)
        }
        None => {
            status = Some("No encontré una salida de audio: revisá que haya parlantes o auriculares activos y volvé a abrir Orfeón.".to_string());
            (None, None)
        }
    };
    let cfg: Option<cpal::StreamConfig> = cfg;
    let sr = cfg.as_ref().map_or(48000.0, |c| c.sample_rate.0 as f64);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    // un núcleo queda para el sistema, la placa de sonido y la ventana
    let threads = cores.saturating_sub(1).clamp(1, 16);

    // ---- cuántas voces aguanta esta PC ----
    if verbose {
        print!("midiendo esta PC ({cores} núcleos)... ");
        let _ = std::io::stdout().flush();
    }
    let mut params = PRESETS[0].params;
    let alta = measure(sr, threads, params, Quality::Alta);
    params.quality = Quality::Alta;
    params.max_voices = alta;
    if alta < 4 {
        let eco = measure(sr, threads, params, Quality::Eco);
        if eco > alta {
            params.quality = Quality::Eco;
            params.max_voices = eco;
        }
    }
    if verbose {
        println!("aguanta {} voces a la vez{}", params.max_voices, if params.quality == Quality::Eco { " en calidad Eco" } else { "" });
    }

    // ---- hilo que calcula + cola hacia la placa ----
    let meters = Arc::new(Meters::default());
    meters.latency.store((sr * DEFAULT_LATENCY_MS as f64 / 1000.0) as usize, Ordering::Relaxed);
    meters.threads.store(threads, Ordering::Relaxed);
    let (queue_in, queue_out) = rtrb::RingBuffer::<f32>::new((sr * 0.5) as usize * 2);
    let (tx, rx) = channel::<Msg>();
    let mut choir = Choir::with_thread_init(sr, threads, Some(promote_thread));
    choir.set_params(params);
    let m2 = meters.clone();
    let worker = std::thread::Builder::new().name("orfeon-audio".into()).spawn(move || producer(choir, rx, queue_in, sr, m2)).expect("no pude crear el hilo de audio");
    let producer_thread = worker.thread().clone();

    let mut stream = None;
    let mut audio_name = "sin salida de audio".to_string();
    let mut channels = 0;
    if let (Some(device), Some(cfg), Some(format)) = (device.as_ref(), cfg.as_ref(), format) {
        let output = Output { queue: queue_out, producer: producer_thread.clone(), meters: meters.clone(), started: false };
        let built = match format {
            cpal::SampleFormat::F32 => build::<f32>(device, cfg, output),
            cpal::SampleFormat::I16 => build::<i16>(device, cfg, output),
            cpal::SampleFormat::U16 => build::<u16>(device, cfg, output),
            cpal::SampleFormat::I32 => build::<i32>(device, cfg, output),
            other => {
                status = Some(format!("La salida de audio usa un formato que no manejo ({other:?})."));
                Err(cpal::BuildStreamError::StreamConfigNotSupported)
            }
        };
        match built {
            Ok(s) => match s.play() {
                Ok(()) => {
                    audio_name = device.name().unwrap_or_else(|_| "salida por defecto".into());
                    channels = cfg.channels;
                    stream = Some(s);
                }
                Err(e) => status = Some(format!("No pude arrancar el audio: {e}")),
            },
            Err(e) => {
                if status.is_none() {
                    status = Some(format!("No pude abrir la salida de audio: {e}"));
                }
            }
        }
    }
    Running { tx, producer: producer_thread, worker, meters, stream, params, sr, threads, audio_name, channels, status }
}

fn stop(r: Running) {
    r.meters.quit.store(true, Ordering::Relaxed);
    r.producer.unpark();
    drop(r.stream);
    let _ = r.worker.join();
}

// ------------------------------------------------------------------ ventana

/// id de las notas del teclado de pantalla: el "canal 16" (así id % 128 = nota).
const SCREEN_ID: i32 = 16 * 128;

#[derive(serde::Serialize, serde::Deserialize)]
struct QuartetProject {
    version: u8,
    parts: [(u8, u8, usize); 4],
    words: [String; 4],
    lyrics: [String; 4],
}

impl QuartetProject {
    fn path() -> std::io::Result<std::path::PathBuf> {
        let root = std::env::var_os("ORFEON_PROJECT");
        if let Some(path) = root { return Ok(path.into()); }
        Ok(std::env::current_dir()?.join("Orfeon-Cuarteto.json"))
    }

    fn save(model: &orfeon_ui::Model) -> Result<String, Box<dyn std::error::Error>> {
        let path = Self::path()?;
        let project = Self {
            version: 1,
            parts: model.params.parts.map(|p| (p.low, p.high, p.style.index())),
            words: model.words.clone(),
            lyrics: model.lyrics.clone(),
        };
        orfeon_core::lyrics::Lyrics::prepare(&project.lyrics, &model.params)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&project)?)?;
        std::fs::rename(tmp, &path)?;
        Ok(format!("Cuarteto guardado en {}", path.display()))
    }

    fn load(model: &mut orfeon_ui::Model) -> Result<orfeon_core::lyrics::Lyrics, Box<dyn std::error::Error>> {
        let path = Self::path()?;
        let file = std::fs::File::open(&path)?;
        if file.metadata()?.len() > 64 * 1024 {
            return Err("El archivo del cuarteto supera 64 KiB.".into());
        }
        let project: Self = serde_json::from_reader(file)?;
        if project.version != 1 || project.parts.iter().any(|&(lo, hi, style)| lo > hi || hi > 127 || style >= orfeon_core::Style::ALL.len()) {
            return Err("Formato de cuarteto inválido.".into());
        }
        let mut params = model.params;
        params.mode = Mode::Partes;
        params.parts = project.parts.map(|(low, high, style)| orfeon_core::Part { low, high, style: orfeon_core::Style::from_index(style) });
        let bank = orfeon_core::lyrics::Lyrics::prepare(&project.lyrics, &params)?;
        model.params = params;
        model.words = project.words;
        model.lyrics = project.lyrics;
        model.preset = None;
        model.lyrics_status = format!("Cuarteto abierto desde {}", path.display());
        Ok(bank)
    }
}

struct Gui {
    model: orfeon_ui::Model,
    tx: Sender<Msg>,
    producer: std::thread::Thread,
    meters: Arc<Meters>,
    standalone: orfeon_ui::Standalone,
    midi: Option<midir::MidiInputConnection<()>>,
    sr: f64,
    events: Vec<orfeon_ui::Event>,
}

impl Gui {
    fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
        self.producer.unpark();
    }

    fn frame(&mut self, ctx: &orfeon_ui::egui::Context) {
        let mut tel = orfeon_ui::Telemetry::default();
        tel.load = self.meters.load.load(Ordering::Relaxed) as f32 / 1000.0;
        tel.voices = self.meters.voices.load(Ordering::Relaxed);
        tel.underruns = Some(self.meters.underruns.load(Ordering::Relaxed));
        tel.threads = self.meters.threads.load(Ordering::Relaxed);
        for (t, a) in tel.levels.iter_mut().zip(self.meters.levels.iter()) {
            *t = f32::from_bits(a.load(Ordering::Relaxed));
        }
        for (t, a) in tel.held.iter_mut().zip(self.meters.held.iter()) {
            *t = a.load(Ordering::Relaxed);
        }
        self.standalone.latency_ms = (self.meters.latency.load(Ordering::Relaxed) as f64 * 1000.0 / self.sr).round() as u32;
        self.events.clear();
        let mut events = std::mem::take(&mut self.events);
        let changed = orfeon_ui::show(ctx, &mut self.model, &tel, Some(&self.standalone), &mut events);
        if changed {
            self.send(Msg::Params(self.model.params));
        }
        for e in events.drain(..) {
            match e {
                orfeon_ui::Event::NoteOn(k, v, part) => self.send(Msg::On(SCREEN_ID + part as i32 * 128 + k as i32, k, v as f64)),
                orfeon_ui::Event::NoteOff(k, part) => self.send(Msg::Off(SCREEN_ID + part as i32 * 128 + k as i32)),
                orfeon_ui::Event::AllOff => self.send(Msg::AllOff),
                orfeon_ui::Event::ResetLyrics => self.send(Msg::ResetLyrics),
                orfeon_ui::Event::SaveQuartet => {
                    self.model.lyrics_status = QuartetProject::save(&self.model).unwrap_or_else(|error| error.to_string());
                }
                orfeon_ui::Event::OpenQuartet => match QuartetProject::load(&mut self.model) {
                    Ok(bank) => {
                        self.send(Msg::Params(self.model.params));
                        self.send(Msg::Lyrics(bank));
                    }
                    Err(error) => self.model.lyrics_status = error.to_string(),
                },
                orfeon_ui::Event::ApplyLyrics => {
                    self.model.lyrics_status = match orfeon_core::lyrics::Lyrics::prepare(&self.model.lyrics, &self.model.params) {
                        Ok(bank) => {
                            self.send(Msg::Lyrics(bank));
                            "Letras aplicadas. Los cursores empiezan desde el inicio.".into()
                        }
                        Err(error) => error,
                    };
                }
                orfeon_ui::Event::Threads(n) => {
                    self.meters.threads.store(n, Ordering::Relaxed);
                    self.send(Msg::Threads(n));
                }
                orfeon_ui::Event::Latency(ms) => self.meters.latency.store((self.sr * ms as f64 / 1000.0) as usize, Ordering::Relaxed),
                orfeon_ui::Event::Midi(i) => {
                    self.midi = None;
                    match midi_connect(i, self.tx.clone(), self.producer.clone()) {
                        Ok((c, _)) => {
                            self.midi = Some(c);
                            self.standalone.midi_selected = Some(i);
                            self.standalone.status = None;
                        }
                        Err(e) => {
                            self.standalone.midi_selected = None;
                            self.standalone.status = Some(format!("MIDI: {e}"));
                        }
                    }
                }
            }
        }
        self.events = events;
    }
}

/// Formato de OpenGL. ORFEON_GL_SIMPLE=1 pide uno sin alfa ni sRGB: lo necesitan algunas
/// pantallas virtuales y escritorios remotos (con el de siempre, la ventana no abre).
fn gl_config() -> baseview::gl::GlConfig {
    let mut c = baseview::gl::GlConfig::default();
    if std::env::var_os("ORFEON_GL_SIMPLE").is_some() {
        c.alpha_bits = 0;
        c.srgb = false;
        c.stencil_bits = 0;
    }
    c
}

fn run_gui() {
    let r = start(false);
    let mut standalone = orfeon_ui::Standalone {
        audio: r.audio_name.clone(),
        sample_rate: r.sr as u32,
        latency_ms: DEFAULT_LATENCY_MS,
        midi_ports: midi_ports(),
        midi_selected: None,
        status: r.status.clone(),
    };
    let mut midi = None;
    if !standalone.midi_ports.is_empty() {
        if let Ok((c, _)) = midi_connect(0, r.tx.clone(), r.producer.clone()) {
            midi = Some(c);
            standalone.midi_selected = Some(0);
        }
    }
    let _ = r.channels;
    let gui = Gui { model: orfeon_ui::Model::new(r.params, Some(0)), tx: r.tx.clone(), producer: r.producer.clone(), meters: r.meters.clone(), standalone, midi, sr: r.sr, events: Vec::new() };
    let settings = baseview::WindowOpenOptions {
        title: "Orfeón".to_string(),
        size: baseview::Size::new(orfeon_ui::WIDTH as f64, orfeon_ui::HEIGHT as f64),
        scale: baseview::WindowScalePolicy::SystemScaleFactor,
        gl_config: Some(gl_config()),
    };
    egui_baseview::EguiWindow::open_blocking(
        settings,
        egui_baseview::GraphicsConfig::default(),
        gui,
        |ctx, _queue, _gui: &mut Gui| orfeon_ui::theme::apply(ctx),
        |ctx, _queue, gui: &mut Gui| gui.frame(ctx),
    );
    stop(r);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--consola") {
        attach_console();
        run_console();
    } else {
        run_gui();
    }
}

fn run_console() {
    println!("Orfeón {} — coro de cantantes modelados físicamente", env!("CARGO_PKG_VERSION"));
    println!("Motor de voz: CHIMERA (Jameson Hodge, MIT). Software libre, GPL-3.0-or-later.\n");
    let r = start(true);
    if let Some(msg) = &r.status {
        eprintln!("{msg}");
        stop(r);
        wait_and_exit(1);
    }
    println!("audio:  {} — {} Hz, {} canales, colchón {DEFAULT_LATENCY_MS} ms", r.audio_name, r.sr as u32, r.channels);
    let (tx, meters, sr, producer_thread) = (r.tx.clone(), r.meters.clone(), r.sr, r.producer.clone());
    let mut params = r.params;
    let mut threads = r.threads;
    let notify = |tx: &Sender<Msg>, m: Msg| {
        let _ = tx.send(m);
        producer_thread.unpark();
    };
    // ---- MIDI: la primera entrada, si hay ----
    let mut midi: Option<midir::MidiInputConnection<()>> = None;
    let ports = midi_ports();
    if ports.is_empty() {
        println!("midi:   no hay teclados MIDI conectados (se puede tocar con el comando n)");
    } else {
        match midi_connect(0, tx.clone(), producer_thread.clone()) {
            Ok((c, name)) => {
                println!("midi:   {name}{}", if ports.len() > 1 { format!("  (hay {}; ver: midi)", ports.len()) } else { String::new() });
                midi = Some(c);
            }
            Err(e) => println!("midi:   no pude abrir la entrada: {e}"),
        }
    }
    println!("coro:   {} ({} cantantes, hasta {} a la vez, {} hilos)", PRESETS[0].name, params.singers, params.max_voices, threads);
    println!("\nProbá:  n 48 55 64 72     demo     p catedral     ayuda\n");

    // ---- comandos ----
    let stdin = std::io::stdin();
    let mut next_id = 1_000_000i32;
    let mut seen_underruns = 0u32;
    loop {
        print!("> ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break, // se cerró la entrada
            Ok(_) => {}
        }
        match console::parse(&line, &params) {
            Err(e) => println!("  {e}"),
            Ok(Command::Nothing) => {}
            Ok(Command::Quit) => break,
            Ok(Command::Help) => println!("{HELP}"),
            Ok(Command::Presets) => {
                for (i, p) in PRESETS.iter().enumerate() {
                    println!("  {:2}  {:24} {:6} {:2} cantantes  {:8} {}", i + 1, p.name, p.params.mode.name(), p.params.singers, p.params.style.def().name, p.params.room.name());
                }
            }
            Ok(Command::Params(p, msg)) => {
                params = p;
                notify(&tx, Msg::Params(p));
                println!("  {msg}");
            }
            Ok(Command::Play(keys, secs)) => {
                let n = keys.len() as i32;
                play_later(&tx, 0.0, secs, keys, next_id);
                next_id += n;
            }
            Ok(Command::Demo) => {
                for (start, dur, keys) in DEMO {
                    play_later(&tx, start, dur, keys.to_vec(), next_id);
                    next_id += 4;
                }
                println!("  demo: 16 segundos");
            }
            Ok(Command::AllOff) => notify(&tx, Msg::AllOff),
            Ok(Command::Threads(n)) => {
                threads = n;
                notify(&tx, Msg::Threads(n));
                println!("  hilos: {n}");
            }
            Ok(Command::Latency(ms)) => {
                meters.latency.store((sr * ms as f64 / 1000.0) as usize, Ordering::Relaxed);
                println!("  colchón de audio: {ms} ms");
            }
            Ok(Command::Midi(None)) => {
                let ports = midi_ports();
                if ports.is_empty() {
                    println!("  no hay entradas MIDI");
                }
                for (i, name) in ports.iter().enumerate() {
                    println!("  {}  {name}", i + 1);
                }
            }
            Ok(Command::Midi(Some(n))) => {
                midi = None; // suelta la anterior antes de abrir otra
                match midi_connect(n - 1, tx.clone(), producer_thread.clone()) {
                    Ok((c, name)) => {
                        println!("  midi: {name}");
                        midi = Some(c);
                    }
                    Err(e) => println!("  {e}"),
                }
            }
            Ok(Command::Info) => {
                let load = meters.load.load(Ordering::Relaxed) as f64 / 10.0;
                let max = meters.load_max.swap(0, Ordering::Relaxed) as f64 / 10.0;
                let peak = meters.peak.swap(0, Ordering::Relaxed) as f64 / 1000.0;
                let under = meters.underruns.load(Ordering::Relaxed);
                let latency_ms = meters.latency.load(Ordering::Relaxed) as f64 * 1000.0 / sr;
                println!("  carga: {load:.0} % (máximo desde el último info: {max:.0} %)   voces sonando: {}   pico: {peak:.2}", meters.voices.load(Ordering::Relaxed));
                println!("  cortes: {} desde el último info ({under} en total)   colchón: {latency_ms:.0} ms", under - seen_underruns);
                println!("  {} · {} cantantes · {} · {} · {} · tope {} voces · {:?} · {threads} hilos", params.mode.name(), params.singers, params.ensemble.name(), params.style.def().name, params.room.name(), params.max_voices, params.quality);
                if under > seen_underruns {
                    if max > 100.0 {
                        println!("  el cálculo no llega a tiempo: probá  voces {}  o  eco si", (params.max_voices * 2 / 3).max(1));
                    } else {
                        println!("  el cálculo llega, pero el sistema lo demora: probá  latencia {}", (latency_ms * 2.0).min(200.0) as u32);
                    }
                }
                seen_underruns = under;
            }
        }
    }
    drop(midi);
    stop(r);
}

/// En Windows la ventana se cierra sola al terminar: deja leer el error antes.
fn wait_and_exit(code: i32) -> ! {
    eprintln!("\n(Enter para cerrar)");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    std::process::exit(code)
}
