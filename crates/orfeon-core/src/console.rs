//! Comandos de texto de la aplicación independiente. Sólo interpreta: no toca audio ni
//! dispositivos, así se puede probar sin ellos.

use crate::choir::{ChoirParams, Mode, Quality};
use crate::preset::{find, PRESETS};
use crate::room::RoomKind;
use crate::singer::Ensemble;
use crate::style::Style;
use crate::vowel;

/// Lo que pide una línea escrita por el usuario.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// nuevos parámetros del coro, y qué decirle al usuario
    Params(ChoirParams, String),
    /// tocar estas notas durante tantos segundos
    Play(Vec<u8>, f64),
    /// tocar la demo de acordes
    Demo,
    AllOff,
    Threads(usize),
    /// listar las entradas MIDI, o conectarse a la número N (desde 1)
    Midi(Option<usize>),
    Presets,
    Info,
    Help,
    Quit,
    /// línea vacía
    Nothing,
}

pub const HELP: &str = "\
comandos (Enter para enviar):
  programas            lista los 36 programas
  p N | p NOMBRE       elige un programa (p 7, p catedral)
  n 60 64 67 [SEG]     toca esas notas MIDI juntas (2 s si no se indica)
  demo                 toca una cadencia de acordes
  fuera                apaga todas las notas
  modo coro|tutti|solo
  vocal a|e|i|o|u      o dos números 0..1: vocal 0.5 0.8
  estilo 1..8          clásico antiguo pop gospel folk susurro belting rasgado
  sala 1..5            seco estudio sala iglesia catedral
  conjunto 1..8        mixto femenino masculino niños sopranos contraltos tenores bajos
  cantantes 1..64      cuántos forman el coro
  elenco N             otra gente (semilla)
  voces 1..64          tope de cantantes sonando a la vez
  eco si|no            calidad económica (menos CPU)
  hilos 1..16          hilos de cálculo
  mic 0..1             distancia del micrófono       vol DB   volumen en dB
  humanizar 0..1       vibrato 0..2                  mover 0..1 (la vocal se mueve sola)
  midi                 lista las entradas MIDI       midi N   usa la entrada N
  info                 estado y carga de CPU
  salir";

fn num(tok: Option<&str>, what: &str, lo: f64, hi: f64) -> Result<f64, String> {
    let t = tok.ok_or_else(|| format!("falta {what}"))?;
    let v: f64 = t.replace(',', ".").parse().map_err(|_| format!("\"{t}\" no es un número"))?;
    if !v.is_finite() || v < lo || v > hi {
        return Err(format!("{what} va de {lo} a {hi}"));
    }
    Ok(v)
}

/// Interpreta una línea. `cur` son los parámetros vigentes (los comandos cambian uno solo).
pub fn parse(line: &str, cur: &ChoirParams) -> Result<Command, String> {
    let mut it = line.split_whitespace();
    let Some(cmd) = it.next() else { return Ok(Command::Nothing) };
    let cmd = cmd.to_lowercase();
    let mut p = *cur;
    let ok = |p: ChoirParams, msg: String| Ok(Command::Params(p, msg));
    match cmd.as_str() {
        "ayuda" | "help" | "?" => Ok(Command::Help),
        "salir" | "q" | "quit" | "exit" => Ok(Command::Quit),
        "programas" | "presets" => Ok(Command::Presets),
        "info" => Ok(Command::Info),
        "demo" => Ok(Command::Demo),
        "fuera" | "off" => Ok(Command::AllOff),
        "p" | "programa" => {
            let rest: Vec<&str> = it.collect();
            if rest.is_empty() {
                return Err("falta el número o el nombre del programa".into());
            }
            let name = rest.join(" ");
            let pr = match name.parse::<usize>() {
                Ok(n) if n >= 1 && n <= PRESETS.len() => Some(&PRESETS[n - 1]),
                Ok(_) => None,
                Err(_) => find(&name).or_else(|| {
                    let n = name.to_lowercase();
                    PRESETS.iter().find(|x| x.name.to_lowercase().contains(&n))
                }),
            };
            let pr = pr.ok_or_else(|| format!("no existe el programa \"{name}\" (ver: programas)"))?;
            // lo que depende de la máquina no lo pisa el programa
            let q = ChoirParams { max_voices: cur.max_voices, quality: cur.quality, gain_db: cur.gain_db, ..pr.params };
            ok(q, format!("programa: {} ({}, {} cantantes, {})", pr.name, q.mode.name(), q.singers, q.style.def().name))
        }
        "n" | "nota" | "notas" => {
            let vals: Vec<f64> = it.map(|t| t.replace(',', ".").parse::<f64>().map_err(|_| format!("\"{t}\" no es un número"))).collect::<Result<_, _>>()?;
            // las notas son enteros 0..127; un último valor con decimales o chico es la duración
            let mut keys = Vec::new();
            let mut secs = 2.0;
            for (i, v) in vals.iter().enumerate() {
                let last = i + 1 == vals.len();
                if last && vals.len() > 1 && (v.fract() != 0.0 || *v < 12.0) {
                    secs = v.clamp(0.05, 60.0);
                } else if v.fract() == 0.0 && (0.0..=127.0).contains(v) {
                    keys.push(*v as u8);
                } else {
                    return Err(format!("{v} no es una nota MIDI (0..127)"));
                }
            }
            if keys.is_empty() {
                return Err("faltan las notas (ej: n 60 64 67)".into());
            }
            if keys.len() > 16 {
                return Err("como mucho 16 notas a la vez".into());
            }
            Ok(Command::Play(keys, secs))
        }
        "modo" => {
            p.mode = match it.next().map(|s| s.to_lowercase()).as_deref() {
                Some("coro") => Mode::Coro,
                Some("tutti") => Mode::Tutti,
                Some("solo") => Mode::Solo,
                _ => return Err("modo coro | tutti | solo".into()),
            };
            ok(p, format!("modo: {}", p.mode.name()))
        }
        "vocal" => {
            let a = it.next().ok_or("vocal a|e|i|o|u, o dos números 0..1")?;
            let b = it.next();
            let (x, y) = match (a.replace(',', ".").parse::<f64>(), b) {
                (Ok(_), Some(_)) => (num(Some(a), "x", 0.0, 1.0)?, num(b, "y", 0.0, 1.0)?),
                _ => {
                    let c = a.chars().next().unwrap_or('a').to_ascii_lowercase();
                    if !"aeiou".contains(c) || a.chars().count() != 1 {
                        return Err("vocal a|e|i|o|u, o dos números 0..1".into());
                    }
                    vowel::spanish(c)
                }
            };
            p.vowel_x = x;
            p.vowel_y = y;
            ok(p, format!("vocal: x {x:.2}  y {y:.2}"))
        }
        "estilo" => {
            p.style = Style::from_index(num(it.next(), "el estilo", 1.0, 8.0)? as usize - 1);
            ok(p, format!("estilo: {}", p.style.def().name))
        }
        "sala" => {
            p.room = RoomKind::from_index(num(it.next(), "la sala", 1.0, 5.0)? as usize - 1);
            ok(p, format!("sala: {}", p.room.name()))
        }
        "conjunto" => {
            p.ensemble = Ensemble::from_index(num(it.next(), "el conjunto", 1.0, 8.0)? as usize - 1);
            ok(p, format!("conjunto: {} (se reconstruyen las voces)", p.ensemble.name()))
        }
        "cantantes" => {
            p.singers = num(it.next(), "la cantidad", 1.0, 64.0)? as usize;
            ok(p, format!("cantantes: {} (se reconstruyen las voces)", p.singers))
        }
        "elenco" => {
            p.seed = num(it.next(), "el elenco", 1.0, 999.0)? as u32;
            ok(p, format!("elenco: {} (se reconstruyen las voces)", p.seed))
        }
        "voces" => {
            p.max_voices = num(it.next(), "el tope", 1.0, 64.0)? as usize;
            ok(p, format!("voces a la vez: {}", p.max_voices))
        }
        "eco" => {
            p.quality = match it.next().map(|s| s.to_lowercase()).as_deref() {
                Some("si") | Some("sí") | Some("on") | Some("1") => Quality::Eco,
                Some("no") | Some("off") | Some("0") => Quality::Alta,
                _ => return Err("eco si | no".into()),
            };
            ok(p, format!("calidad: {:?} (se reconstruyen las voces)", p.quality))
        }
        "mic" => {
            p.mic = num(it.next(), "la distancia", 0.0, 1.0)?;
            ok(p, format!("micrófono: {:.2}", p.mic))
        }
        "vol" => {
            p.gain_db = num(it.next(), "el volumen", -36.0, 12.0)?;
            ok(p, format!("volumen: {:+.1} dB", p.gain_db))
        }
        "humanizar" => {
            p.humanize = num(it.next(), "humanizar", 0.0, 1.0)?;
            ok(p, format!("humanizar: {:.2}", p.humanize))
        }
        "vibrato" => {
            p.vibrato = num(it.next(), "el vibrato", 0.0, 2.0)?;
            ok(p, format!("vibrato: {:.2}", p.vibrato))
        }
        "mover" => {
            p.vowel_move = num(it.next(), "el movimiento", 0.0, 1.0)?;
            ok(p, format!("movimiento de la vocal: {:.2}", p.vowel_move))
        }
        "hilos" => Ok(Command::Threads(num(it.next(), "los hilos", 1.0, 16.0)? as usize)),
        "midi" => match it.next() {
            None => Ok(Command::Midi(None)),
            Some(t) => Ok(Command::Midi(Some(num(Some(t), "la entrada", 1.0, 64.0)? as usize))),
        },
        other => Err(format!("no conozco \"{other}\" (escribí: ayuda)")),
    }
}

/// La cadencia de la demo: (segundo de inicio, duración, notas).
pub const DEMO: [(f64, f64, [u8; 4]); 5] = [(0.0, 2.6, [48, 55, 64, 72]), (3.0, 2.6, [45, 57, 64, 72]), (6.0, 2.6, [41, 57, 65, 72]), (9.0, 2.6, [43, 55, 62, 71]), (12.0, 4.0, [48, 55, 64, 72])];

#[cfg(test)]
mod tests {
    use super::*;
    const D: ChoirParams = ChoirParams::DEFAULT;

    #[test]
    fn interpreta_los_comandos() {
        assert_eq!(parse("", &D), Ok(Command::Nothing));
        assert_eq!(parse("   ayuda ", &D), Ok(Command::Help));
        assert_eq!(parse("SALIR", &D), Ok(Command::Quit));
        assert_eq!(parse("n 60 64 67", &D), Ok(Command::Play(vec![60, 64, 67], 2.0)));
        assert_eq!(parse("n 60 64 67 3.5", &D), Ok(Command::Play(vec![60, 64, 67], 3.5)));
        assert_eq!(parse("n 60 64 4", &D), Ok(Command::Play(vec![60, 64], 4.0)));
        assert_eq!(parse("n 60", &D), Ok(Command::Play(vec![60], 2.0)));
        assert_eq!(parse("hilos 4", &D), Ok(Command::Threads(4)));
        assert_eq!(parse("midi", &D), Ok(Command::Midi(None)));
        assert_eq!(parse("midi 2", &D), Ok(Command::Midi(Some(2))));
        let Ok(Command::Params(p, _)) = parse("modo tutti", &D) else { panic!() };
        assert_eq!(p.mode, Mode::Tutti);
        let Ok(Command::Params(p, _)) = parse("vocal u", &D) else { panic!() };
        assert_eq!((p.vowel_x, p.vowel_y), (1.0, 0.0));
        let Ok(Command::Params(p, _)) = parse("vocal 0,5 0.25", &D) else { panic!() };
        assert_eq!((p.vowel_x, p.vowel_y), (0.5, 0.25));
        let Ok(Command::Params(p, _)) = parse("estilo 4", &D) else { panic!() };
        assert_eq!(p.style, Style::Gospel);
        let Ok(Command::Params(p, _)) = parse("eco si", &D) else { panic!() };
        assert_eq!(p.quality, Quality::Eco);
    }

    #[test]
    fn un_programa_no_pisa_lo_que_depende_de_la_maquina() {
        let cur = ChoirParams { max_voices: 3, quality: Quality::Eco, gain_db: -6.0, ..D };
        let Ok(Command::Params(p, msg)) = parse("p 3", &cur) else { panic!() };
        assert_eq!((p.max_voices, p.quality, p.gain_db), (3, Quality::Eco, -6.0));
        assert_eq!(p.room, RoomKind::Catedral);
        assert!(msg.contains("Catedral"));
        // por nombre, completo o por una parte
        let Ok(Command::Params(p, _)) = parse("p monjes", &D) else { panic!() };
        assert_eq!(p.ensemble, Ensemble::Bajos);
        let Ok(Command::Params(p, _)) = parse("p coro gos", &D) else { panic!() };
        assert_eq!(p.style, Style::Gospel);
    }

    #[test]
    fn rechaza_lo_que_no_entiende_sin_romperse() {
        for bad in ["zzz", "p", "p 0", "p 99", "p noexiste", "n", "n hola", "n 200", "n 60.5 61", "modo", "modo x", "vocal", "vocal zz", "vocal 3 4", "estilo 9", "estilo", "sala 0", "voces 0", "voces 999", "eco", "eco quizas", "hilos 0", "vol 99", "mic -1", "midi x", "cantantes nan", "vibrato inf", "n 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19"] {
            assert!(parse(bad, &D).is_err(), "\"{bad}\" tendría que dar error");
        }
    }
}
