//! Orfeón independiente: abre la salida de audio por defecto, escucha un teclado MIDI si hay
//! uno, y recibe comandos de texto. No necesita un DAW.
//!
//! Hilos: el de audio (el que llama la placa de sonido) es dueño del coro; la consola y el
//! MIDI le mandan mensajes por un canal. Las voces se calculan con los hilos del propio coro.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use orfeon_core::console::{self, Command, DEMO, HELP};
use orfeon_core::preset::PRESETS;
use orfeon_core::{Choir, ChoirParams};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Lo que la consola y el MIDI le piden al hilo de audio.
enum Msg {
    On(i32, u8, f64),
    Off(i32),
    Bend(f64),
    AllOff,
    Params(ChoirParams),
    Threads(usize),
}

/// Medidores que el hilo de audio deja para la consola.
#[derive(Default)]
struct Meters {
    /// carga media y máxima, en milésimas del tiempo disponible
    load: AtomicU32,
    load_max: AtomicU32,
    voices: AtomicUsize,
    /// pico de salida en milésimas
    peak: AtomicU32,
}

struct Engine {
    choir: Choir,
    rx: Receiver<Msg>,
    l: Vec<f32>,
    r: Vec<f32>,
    sr: f64,
    avg: f64,
    meters: Arc<Meters>,
}

const SCRATCH: usize = 4096;

impl Engine {
    /// Atiende los mensajes pendientes y rinde `frames` muestras en `l` / `r`.
    fn fill(&mut self, frames: usize) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::On(id, key, vel) => self.choir.note_on(id, key, vel),
                Msg::Off(id) => self.choir.note_off(id),
                Msg::Bend(s) => self.choir.set_pitch_bend(s),
                Msg::AllOff => self.choir.all_notes_off(),
                Msg::Params(p) => self.choir.set_params(p),
                Msg::Threads(n) => self.choir.set_threads(n),
            }
        }
        let t0 = Instant::now();
        self.choir.render(&mut self.l[..frames], &mut self.r[..frames]);
        let load = t0.elapsed().as_secs_f64() / (frames as f64 / self.sr);
        self.avg += 0.05 * (load - self.avg);
        let m = &self.meters;
        m.load.store((self.avg * 1000.0) as u32, Ordering::Relaxed);
        m.load_max.fetch_max((load * 1000.0) as u32, Ordering::Relaxed);
        m.voices.store(self.choir.sounding(), Ordering::Relaxed);
        let pk = self.l[..frames].iter().chain(self.r[..frames].iter()).fold(0f32, |a, x| a.max(x.abs()));
        m.peak.fetch_max((pk * 1000.0) as u32, Ordering::Relaxed);
    }
}

/// Llena el buffer de la placa (intercalado, `channels` canales) en el formato que pida.
fn write<T: cpal::SizedSample + cpal::FromSample<f32>>(eng: &mut Engine, data: &mut [T], channels: usize) {
    let channels = channels.max(1);
    for block in data.chunks_mut(SCRATCH * channels) {
        let frames = block.len() / channels;
        eng.fill(frames);
        for (i, frame) in block.chunks_mut(channels).enumerate() {
            for (c, out) in frame.iter_mut().enumerate() {
                let x = match c {
                    0 if channels == 1 => 0.5 * (eng.l[i] + eng.r[i]),
                    0 => eng.l[i],
                    1 => eng.r[i],
                    _ => 0.0,
                };
                *out = T::from_sample(x);
            }
        }
    }
}

fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(device: &cpal::Device, cfg: &cpal::StreamConfig, mut eng: Engine) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = cfg.channels as usize;
    device.build_output_stream(cfg, move |data: &mut [T], _: &cpal::OutputCallbackInfo| write(&mut eng, data, channels), |e| eprintln!("\n[audio] {e}"), None)
}

// ------------------------------------------------------------------ MIDI

fn midi_ports() -> Vec<String> {
    match midir::MidiInput::new("Orfeon") {
        Ok(input) => input.ports().iter().map(|p| input.port_name(p).unwrap_or_else(|_| "(sin nombre)".into())).collect(),
        Err(_) => Vec::new(),
    }
}

/// Se conecta a la entrada `index` (desde 0). La conexión vive mientras se guarde el valor.
fn midi_connect(index: usize, tx: Sender<Msg>) -> Result<(midir::MidiInputConnection<()>, String), String> {
    let mut input = midir::MidiInput::new("Orfeon").map_err(|e| e.to_string())?;
    input.ignore(midir::Ignore::All);
    let ports = input.ports();
    let port = ports.get(index).ok_or_else(|| format!("no hay una entrada MIDI {}", index + 1))?;
    let name = input.port_name(port).unwrap_or_else(|_| "(sin nombre)".into());
    let conn = input
        .connect(
            port,
            "orfeon-entrada",
            move |_stamp, msg, _| {
                if msg.len() < 2 {
                    return;
                }
                let (status, ch) = (msg[0] & 0xF0, (msg[0] & 0x0F) as i32);
                let d1 = msg[1] & 0x7F;
                let d2 = msg.get(2).copied().unwrap_or(0) & 0x7F;
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

fn main() {
    println!("Orfeón {} — coro de cantantes modelados físicamente", env!("CARGO_PKG_VERSION"));
    println!("Motor de voz: CHIMERA (Jameson Hodge, MIT). Software libre, GPL-3.0-or-later.\n");

    // ---- salida de audio ----
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        eprintln!("No encontré una salida de audio. Revisá que haya parlantes o auriculares activos.");
        wait_and_exit(1);
    };
    let supported = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("No pude leer la configuración de la salida de audio: {e}");
            wait_and_exit(1);
        }
    };
    let format = supported.sample_format();
    let cfg: cpal::StreamConfig = supported.into();
    let sr = cfg.sample_rate.0 as f64;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let mut threads = cores.clamp(1, 16);

    let (tx, rx) = channel::<Msg>();
    let meters = Arc::new(Meters::default());
    let mut params = PRESETS[0].params;
    params.max_voices = params.max_voices.min((cores * 2).max(2));
    let mut choir = Choir::new(sr, threads);
    choir.set_params(params);
    let eng = Engine { choir, rx, l: vec![0.0; SCRATCH], r: vec![0.0; SCRATCH], sr, avg: 0.0, meters: meters.clone() };

    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &cfg, eng),
        cpal::SampleFormat::I16 => build::<i16>(&device, &cfg, eng),
        cpal::SampleFormat::U16 => build::<u16>(&device, &cfg, eng),
        cpal::SampleFormat::I32 => build::<i32>(&device, &cfg, eng),
        other => {
            eprintln!("La salida de audio usa un formato que no manejo ({other:?}).");
            wait_and_exit(1);
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            eprintln!("No pude abrir la salida de audio: {e}");
            wait_and_exit(1);
        }
    };
    if let Err(e) = stream.play() {
        eprintln!("No pude arrancar el audio: {e}");
        wait_and_exit(1);
    }
    println!("audio:  {} — {} Hz, {} canales", device.name().unwrap_or_else(|_| "salida por defecto".into()), sr as u32, cfg.channels);

    // ---- MIDI: la primera entrada, si hay ----
    let mut midi: Option<midir::MidiInputConnection<()>> = None;
    let ports = midi_ports();
    if ports.is_empty() {
        println!("midi:   no hay teclados MIDI conectados (se puede tocar con el comando n)");
    } else {
        match midi_connect(0, tx.clone()) {
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
                let _ = tx.send(Msg::Params(p));
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
            Ok(Command::AllOff) => {
                let _ = tx.send(Msg::AllOff);
            }
            Ok(Command::Threads(n)) => {
                threads = n;
                let _ = tx.send(Msg::Threads(n));
                println!("  hilos: {n}");
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
                match midi_connect(n - 1, tx.clone()) {
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
                println!("  carga: {load:.0} % (máximo desde el último info: {max:.0} %)   voces sonando: {}   pico: {peak:.2}", meters.voices.load(Ordering::Relaxed));
                println!("  {} · {} cantantes · {} · {} · {} · tope {} voces · {:?} · {threads} hilos", params.mode.name(), params.singers, params.ensemble.name(), params.style.def().name, params.room.name(), params.max_voices, params.quality);
                if max > 90.0 {
                    println!("  la carga pasó el 90 %: si se entrecorta, probá  voces {}  o  eco si", (params.max_voices / 2).max(1));
                }
            }
        }
    }
    drop(midi);
    drop(stream);
}

/// En Windows la ventana se cierra sola al terminar: deja leer el error antes.
fn wait_and_exit(code: i32) -> ! {
    eprintln!("\n(Enter para cerrar)");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    std::process::exit(code)
}
