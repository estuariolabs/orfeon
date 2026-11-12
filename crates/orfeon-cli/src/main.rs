//! Orfeón por línea de comandos. Sirve para escuchar el coro sin un DAW y para medir cuántas
//! voces aguanta una máquina.

use orfeon_core::preset::{find, PRESETS};
use orfeon_core::{Choir, ChoirParams, Mode, Quality, RoomKind, Style};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

const FS: f64 = 48000.0;

/// Una nota de la partitura: inicio y duración en segundos, nota MIDI, velocidad 0..1.
#[derive(Clone, Copy)]
struct Note {
    t: f64,
    dur: f64,
    key: u8,
    vel: f64,
}

fn usage() -> ! {
    eprintln!(
        "Orfeón: coro de cantantes modelados físicamente\n\n\
         uso:\n  \
         orfeon presets                          lista los programas\n  \
         orfeon singers [opciones]               muestra los cantantes del coro\n  \
         orfeon render SALIDA.wav [opciones]     rinde una demo o una partitura\n  \
         orfeon bench [opciones]                 mide la carga por cantidad de voces\n\n\
         opciones:\n  \
         --preset N|NOMBRE    programa de fábrica (número desde 1, o nombre)\n  \
         --demo NOMBRE        acordes | escala | melodia | vocales   (por defecto: acordes)\n  \
         --score ARCHIVO      partitura de texto: una nota por línea, \"inicio duración nota [velocidad]\"\n  \
         --mode coro|tutti|solo   --singers N   --style 1..8   --room 1..5\n  \
         --vowel a|e|i|o|u    --voices N (tope simultáneo)   --eco (calidad económica)\n  \
         --threads N          hilos de cálculo (por defecto: los de la máquina)\n  \
         --tail SEG           segundos de cola al final (por defecto 3)"
    );
    std::process::exit(2);
}

struct Opts {
    params: ChoirParams,
    preset_name: String,
    demo: String,
    score: Option<String>,
    threads: usize,
    tail: f64,
    out: Option<String>,
}

fn parse(args: &[String]) -> Opts {
    let mut o = Opts { params: PRESETS[0].params, preset_name: PRESETS[0].name.to_string(), demo: "acordes".into(), score: None, threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1), tail: 3.0, out: None };
    // el programa primero, así las demás opciones lo modifican
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--preset" && i + 1 < args.len() {
            let v = &args[i + 1];
            let p = match v.parse::<usize>() {
                Ok(n) if n >= 1 && n <= PRESETS.len() => Some(&PRESETS[n - 1]),
                Ok(_) => None,
                Err(_) => find(v),
            };
            match p {
                Some(p) => {
                    o.params = p.params;
                    o.preset_name = p.name.to_string();
                }
                None => {
                    eprintln!("no existe el programa \"{v}\" (ver: orfeon presets)");
                    std::process::exit(2);
                }
            }
        }
        i += 1;
    }
    let mut i = 0;
    let num = |s: &str| -> f64 {
        s.parse::<f64>().unwrap_or_else(|_| {
            eprintln!("\"{s}\" no es un número");
            std::process::exit(2)
        })
    };
    while i < args.len() {
        let a = args[i].as_str();
        let val = |i: usize| -> &str {
            args.get(i + 1).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("a {a} le falta un valor");
                std::process::exit(2)
            })
        };
        match a {
            "--preset" => i += 1,
            "--demo" => {
                o.demo = val(i).to_string();
                i += 1;
            }
            "--score" => {
                o.score = Some(val(i).to_string());
                i += 1;
            }
            "--mode" => {
                o.params.mode = match val(i) {
                    "coro" => Mode::Coro,
                    "tutti" => Mode::Tutti,
                    "solo" => Mode::Solo,
                    _ => usage(),
                };
                i += 1;
            }
            "--singers" => {
                o.params.singers = (num(val(i)) as usize).clamp(1, 64);
                i += 1;
            }
            "--voices" => {
                o.params.max_voices = (num(val(i)) as usize).clamp(1, 64);
                i += 1;
            }
            "--style" => {
                o.params.style = Style::from_index((num(val(i)) as usize).saturating_sub(1));
                i += 1;
            }
            "--room" => {
                o.params.room = RoomKind::from_index((num(val(i)) as usize).saturating_sub(1));
                i += 1;
            }
            "--vowel" => {
                let (x, y) = orfeon_core::vowel::spanish(val(i).chars().next().unwrap_or('a'));
                o.params.vowel_x = x;
                o.params.vowel_y = y;
                i += 1;
            }
            "--eco" => o.params.quality = Quality::Eco,
            "--threads" => {
                o.threads = (num(val(i)) as usize).clamp(1, 32);
                i += 1;
            }
            "--tail" => {
                o.tail = num(val(i)).clamp(0.0, 30.0);
                i += 1;
            }
            s if s.starts_with("--") => usage(),
            s => o.out = Some(s.to_string()),
        }
        i += 1;
    }
    o
}

/// Demos incorporadas.
fn demo(name: &str) -> Vec<Note> {
    let mut v = Vec::new();
    let mut chord = |t: f64, dur: f64, keys: &[u8]| {
        for k in keys {
            v.push(Note { t, dur, key: *k, vel: 0.7 });
        }
    };
    match name {
        // I - vi - IV - V - I a cuatro voces (bajo, tenor, contralto, soprano)
        "acordes" => {
            chord(0.0, 2.6, &[48, 55, 64, 72]);
            chord(3.0, 2.6, &[45, 57, 64, 72]);
            chord(6.0, 2.6, &[41, 57, 65, 72]);
            chord(9.0, 2.6, &[43, 55, 62, 71]);
            chord(12.0, 4.0, &[48, 55, 64, 72]);
        }
        "escala" => {
            for (i, k) in [48u8, 50, 52, 53, 55, 57, 59, 60, 62, 64, 65, 67, 69, 71, 72].iter().enumerate() {
                chord(i as f64 * 0.7, 0.6, &[*k]);
            }
        }
        // melodía ligada (las notas se pisan): para Solo y Tutti
        "melodia" => {
            for (i, k) in [60u8, 62, 64, 67, 65, 64, 62, 60, 59, 60].iter().enumerate() {
                chord(i as f64 * 0.8, 0.9, &[*k]);
            }
        }
        "vocales" => chord(0.0, 10.0, &[48, 55, 64, 72]),
        _ => {
            eprintln!("demo desconocida \"{name}\": acordes | escala | melodia | vocales");
            std::process::exit(2);
        }
    }
    v
}

fn read_score(path: &str) -> Vec<Note> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("no pude leer {path}: {e}");
        std::process::exit(1)
    });
    let mut v = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<f64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if f.len() < 3 || !(0.0..=127.0).contains(&f[2]) || f[1] <= 0.0 || f[0] < 0.0 {
            eprintln!("{path}:{}: se esperaba \"inicio duración nota [velocidad]\"", ln + 1);
            std::process::exit(1);
        }
        v.push(Note { t: f[0], dur: f[1], key: f[2] as u8, vel: f.get(3).copied().unwrap_or(0.7).clamp(0.0, 1.0) });
    }
    v
}

fn write_wav(path: &str, l: &[f32], r: &[f32]) -> std::io::Result<()> {
    let mut f = BufWriter::new(File::create(path)?);
    let bytes = (l.len() * 4) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&(FS as u32).to_le_bytes())?;
    f.write_all(&(FS as u32 * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&bytes.to_le_bytes())?;
    for i in 0..l.len() {
        for x in [l[i], r[i]] {
            f.write_all(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
    }
    f.flush()
}

/// Rinde la partitura. Devuelve (L, R, segundos de cálculo, máximo de voces a la vez).
fn render(o: &Opts, notes: &[Note]) -> (Vec<f32>, Vec<f32>, f64, usize) {
    let mut choir = Choir::new(FS, o.threads);
    choir.set_params(o.params);
    // eventos ordenados: (muestra, es_on, id, nota, velocidad)
    let mut ev: Vec<(usize, bool, i32, u8, f64)> = Vec::new();
    for (id, n) in notes.iter().enumerate() {
        ev.push(((n.t * FS) as usize, true, id as i32, n.key, n.vel));
        ev.push((((n.t + n.dur) * FS) as usize, false, id as i32, n.key, 0.0));
    }
    // a igual tiempo, primero los note-on (así una melodía ligada desliza)
    ev.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let end = ev.last().map_or(0, |e| e.0) + (o.tail * FS) as usize;
    let mut l = vec![0f32; end];
    let mut r = vec![0f32; end];
    let moving = o.demo == "vocales" && o.score.is_none();
    let t0 = Instant::now();
    let (mut pos, mut k, mut peak) = (0usize, 0usize, 0usize);
    while pos < end {
        while k < ev.len() && ev[k].0 <= pos {
            let (_, on, id, key, vel) = ev[k];
            if on {
                choir.note_on(id, key, vel);
            } else {
                choir.note_off(id);
            }
            k += 1;
        }
        if moving {
            // demo de vocales: a - e - i - o - u, dos segundos cada una
            let v = ['a', 'e', 'i', 'o', 'u'][((pos as f64 / FS / 2.0) as usize).min(4)];
            let (x, y) = orfeon_core::vowel::spanish(v);
            let mut p = *choir.params();
            p.vowel_x = x;
            p.vowel_y = y;
            choir.set_params(p);
        }
        let n = 128.min(end - pos);
        choir.render(&mut l[pos..pos + n], &mut r[pos..pos + n]);
        peak = peak.max(choir.sounding());
        pos += n;
    }
    (l, r, t0.elapsed().as_secs_f64(), peak)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    let o = parse(&args[1..]);
    match cmd.as_str() {
        "presets" => {
            for (i, p) in PRESETS.iter().enumerate() {
                let q = &p.params;
                println!("{:2}  {:24} {:6} {:2} cantantes  {:10} {:8} {}", i + 1, p.name, q.mode.name(), q.singers, q.ensemble.name(), q.style.def().name, q.room.name());
            }
        }
        "singers" => {
            let mut choir = Choir::new(FS, 1);
            choir.set_params(o.params);
            println!("programa: {}  ({} cantantes)", o.preset_name, choir.singers());
            println!(" #  tipo       tamaño faringe lengua labios nariz  lugar  fondo  desaf.");
            for i in 0..choir.singers() {
                let s = choir.singer(i).unwrap();
                println!("{:2}  {:10} {:5.2}  {:5.2}   {:5.2}  {:5.2}  {:4.2}  {:+5.2}  {:4.2}  {:+5.1}c", i + 1, s.voice_type.name(), s.body.scale, s.body.pharynx, s.body.tongue_radius, s.body.lip_width, s.face.nose, s.pan, s.depth, s.detune);
            }
        }
        "render" => {
            let Some(out) = o.out.clone() else { usage() };
            let notes = match &o.score {
                Some(p) => read_score(p),
                None => demo(&o.demo),
            };
            let (l, r, secs, peak) = render(&o, &notes);
            let dur = l.len() as f64 / FS;
            let rms = (l.iter().chain(r.iter()).map(|x| (*x as f64).powi(2)).sum::<f64>() / (2 * l.len()).max(1) as f64).sqrt();
            let pk = l.iter().chain(r.iter()).fold(0f32, |a, x| a.max(x.abs()));
            if let Err(e) = write_wav(&out, &l, &r) {
                eprintln!("no pude escribir {out}: {e}");
                std::process::exit(1);
            }
            println!("{out}: {dur:.1} s de audio en {secs:.1} s ({:.0} % de tiempo real, {} hilos), hasta {peak} voces, rms {rms:.3}, pico {pk:.2}", 100.0 * secs / dur, o.threads);
        }
        "bench" => {
            println!("programa: {}  calidad: {:?}  hilos: {}", o.preset_name, o.params.quality, o.threads);
            println!("voces  carga (100 % = tiempo real)");
            for voices in [1usize, 2, 4, 8, 16, 32, 64] {
                let mut p = o.params;
                p.mode = Mode::Coro;
                p.singers = voices.max(p.singers).min(64);
                p.max_voices = voices;
                let mut choir = Choir::new(FS, o.threads);
                choir.set_params(p);
                for i in 0..voices {
                    choir.note_on(i as i32, 48 + ((i * 5) % 30) as u8, 0.7);
                }
                let secs = 2.0;
                let mut l = vec![0f32; 128];
                let mut r = vec![0f32; 128];
                let t0 = Instant::now();
                for _ in 0..(secs * FS / 128.0) as usize {
                    choir.render(&mut l, &mut r);
                }
                let load = 100.0 * t0.elapsed().as_secs_f64() / secs;
                println!("{voices:5}  {load:6.0} %{}", if load < 70.0 { "" } else if load < 100.0 { "   (justo)" } else { "   (no llega en tiempo real)" });
                if load > 400.0 {
                    break;
                }
            }
        }
        _ => usage(),
    }
}
