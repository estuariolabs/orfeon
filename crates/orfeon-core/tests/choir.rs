//! Tests del coro: que cante afinado, que cada cantante sea distinto, que los modos repartan
//! bien las notas, que termine en silencio y que el resultado no dependa de los hilos.

use orfeon_core::preset::PRESETS;
use orfeon_core::singer::{Ensemble, Face, Singer, VoiceType};
use orfeon_core::{Choir, ChoirParams, Mode, Quality, RoomKind, Style};

const FS: f64 = 48000.0;

fn render(c: &mut Choir, secs: f64) -> (Vec<f32>, Vec<f32>) {
    let n = (secs * FS) as usize;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        c.render(a, b);
    }
    (l, r)
}
fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}
fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |a, v| a.max(v.abs()))
}
fn finite(x: &[f32]) -> bool {
    x.iter().all(|v| v.is_finite())
}
/// Energía en una frecuencia (Goertzel).
fn tone(x: &[f32], hz: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / FS;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0, 0.0);
    for v in x {
        let s0 = *v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - c * s1 * s2) / (x.len() as f64).powi(2)
}
fn hz(midi: f64) -> f64 {
    440.0 * 2f64.powf((midi - 69.0) / 12.0)
}
/// Parámetros secos y sin humanizar: lo que se mide es la voz.
fn dry() -> ChoirParams {
    ChoirParams { room: RoomKind::Seco, room_mix: 0.0, mic: 0.0, humanize: 0.0, vowel_spread: 0.0, variation: 0.0, style: Style::Antiguo, ..ChoirParams::DEFAULT }
}

#[test]
fn canta_afinado_y_termina_en_silencio() {
    let mut c = Choir::new(FS, 1);
    c.set_params(ChoirParams { mode: Mode::Solo, ensemble: Ensemble::Tenores, singers: 1, ..dry() });
    c.note_on(1, 57, 0.7); // A3
    let (l, r) = render(&mut c, 1.5);
    assert!(finite(&l) && finite(&r));
    let tail = &l[36000..];
    assert!(rms(tail) > 0.01, "no suena: rms {}", rms(tail));
    assert!(peak(&l) < 0.95, "satura: pico {}", peak(&l));
    let on = tone(tail, hz(57.0));
    assert!(on > 30.0 * tone(tail, hz(57.0) * 1.06), "la fundamental no está en A3");
    assert!(on > 30.0 * tone(tail, hz(57.0) * 0.94), "la fundamental no está en A3");
    assert_eq!(c.sounding(), 1);
    c.note_off(1);
    let (l, _) = render(&mut c, 1.5);
    assert!(peak(&l[60000..]) == 0.0, "no termina en silencio exacto: {}", peak(&l[60000..]));
    assert_eq!(c.sounding(), 0, "la voz sigue calculándose después de apagarse");
}

#[test]
fn modo_coro_una_voz_por_nota_y_en_su_registro() {
    let mut c = Choir::new(FS, 1);
    c.set_params(ChoirParams { singers: 8, max_voices: 8, ..dry() });
    // bajo, tenor, contralto, soprano
    for (id, key) in [(1, 43u8), (2, 55), (3, 64), (4, 76)] {
        c.note_on(id, key, 0.7);
    }
    let (l, r) = render(&mut c, 1.2);
    assert_eq!(c.sounding(), 4, "cuatro notas tienen que ser cuatro cantantes");
    assert!(finite(&l) && finite(&r));
    let tail = &l[30000..];
    let tr = &r[30000..];
    for key in [43.0, 55.0, 64.0, 76.0] {
        let e = tone(tail, hz(key)) + tone(tr, hz(key));
        let off = tone(tail, hz(key) * 1.059) + tone(tr, hz(key) * 1.059);
        assert!(e > 5.0 * off, "falta la nota {key} en el acorde");
    }
    // soltar una sola nota deja las otras tres
    c.note_off(2);
    render(&mut c, 1.0);
    assert_eq!(c.sounding(), 3);
    c.all_notes_off();
    render(&mut c, 1.0);
    assert_eq!(c.sounding(), 0);
}

#[test]
fn el_tope_de_voces_se_respeta() {
    let mut c = Choir::new(FS, 1);
    c.set_params(ChoirParams { singers: 16, max_voices: 3, ..dry() });
    for i in 0..10 {
        c.note_on(i, 50 + i as u8 * 2, 0.6);
    }
    let (l, _) = render(&mut c, 0.6);
    assert!(finite(&l));
    // sostenidas: como mucho 3 (puede haber colas de las robadas apagándose)
    c.all_notes_off();
    render(&mut c, 1.0);
    c.note_on(100, 60, 0.6);
    c.note_on(101, 64, 0.6);
    c.note_on(102, 67, 0.6);
    c.note_on(103, 72, 0.6);
    render(&mut c, 0.3);
    assert!(c.sounding() <= 4 && c.sounding() >= 3, "voces sonando: {}", c.sounding());
}

#[test]
fn tutti_todos_cantan_la_misma_nota_en_su_octava() {
    let mut c = Choir::new(FS, 1);
    c.set_params(ChoirParams { mode: Mode::Tutti, singers: 4, max_voices: 4, ..dry() });
    c.note_on(1, 60, 0.7);
    let (l, r) = render(&mut c, 1.2);
    assert_eq!(c.sounding(), 4);
    let m: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a + b).collect();
    let tail = &m[30000..];
    // el do central y su octava de abajo (el bajo lo lleva a su registro)
    assert!(tone(tail, hz(60.0)) > 5.0 * tone(tail, hz(60.0) * 1.059));
    // otra tecla: siguen siendo los mismos cuatro, ligando
    c.note_on(2, 64, 0.7);
    render(&mut c, 0.5);
    assert_eq!(c.sounding(), 4);
    // soltar la de arriba vuelve a la anterior; soltar todo calla
    c.note_off(2);
    render(&mut c, 0.3);
    assert_eq!(c.sounding(), 4);
    c.note_off(1);
    render(&mut c, 1.2);
    assert_eq!(c.sounding(), 0);
}

#[test]
fn solo_es_un_cantante_y_liga() {
    let mut c = Choir::new(FS, 1);
    c.set_params(ChoirParams { mode: Mode::Solo, singers: 8, ..dry() });
    c.note_on(1, 60, 0.7);
    render(&mut c, 0.5);
    c.note_on(2, 67, 0.7);
    let (l, _) = render(&mut c, 0.8);
    assert_eq!(c.sounding(), 1);
    let tail = &l[20000..];
    assert!(tone(tail, hz(67.0)) > 5.0 * tone(tail, hz(60.0)), "no pasó a la nota nueva");
    // ligado: sin hueco de silencio entre las dos notas
    let gap = l[..9600].chunks(480).map(rms).fold(f64::MAX, f64::min);
    assert!(gap > 0.002, "hubo un silencio en el ligado ({gap})");
}

#[test]
fn cada_cantante_es_distinto() {
    // mismos tipos de voz, misma nota: dos personas distintas no dan la misma onda
    let take = |seed: u32| {
        let mut c = Choir::new(FS, 1);
        c.set_params(ChoirParams { mode: Mode::Solo, ensemble: Ensemble::Sopranos, singers: 1, seed, variation: 1.0, ..dry() });
        c.note_on(1, 69, 0.7);
        render(&mut c, 0.8).0
    };
    let (a, b, a2) = (take(1), take(2), take(1));
    assert_eq!(a, a2, "la misma semilla tiene que dar el mismo cantante");
    let diff = a.iter().zip(&b).map(|(x, y)| (x - y).abs() as f64).sum::<f64>() / a.len() as f64;
    assert!(diff > 0.2 * rms(&a), "dos semillas dan casi la misma voz ({diff})");

    // los cuerpos salen de los rostros, dentro de límites que el motor aguanta
    let base = Face::default();
    let mut scales = Vec::new();
    for i in 0..64 {
        let s = Singer::new(Ensemble::Mixto, i, 64, 7, base, 1.0);
        assert!((0.66..=1.15).contains(&s.body.scale) && (0.72..=1.12).contains(&s.body.pharynx));
        assert!((-1.0..=1.0).contains(&s.pan) && (0.0..=1.0).contains(&s.depth));
        scales.push(s.body.scale);
    }
    scales.sort_by(|a, b| a.partial_cmp(b).unwrap());
    scales.dedup();
    assert!(scales.len() > 40, "los 64 cuerpos se repiten ({} distintos)", scales.len());
    // un cuello más largo alarga la faringe; una cara más chica achica el tracto
    let long = Singer::new(Ensemble::Tenores, 0, 1, 1, Face { neck: 1.0, ..base }, 0.0);
    let short = Singer::new(Ensemble::Tenores, 0, 1, 1, Face { neck: -1.0, ..base }, 0.0);
    assert!(long.body.pharynx > short.body.pharynx);
    let kid = Singer::new(Ensemble::Tenores, 0, 1, 1, Face { age: -1.0, ..base }, 0.0);
    let adult = Singer::new(Ensemble::Tenores, 0, 1, 1, base, 0.0);
    assert!(kid.body.scale < adult.body.scale);
    // los tipos de voz ordenan los registros
    assert!(VoiceType::Bajo.range().0 < VoiceType::Tenor.range().0 && VoiceType::Tenor.range().0 < VoiceType::Soprano.range().0);
}

#[test]
fn la_carta_de_vocales_cambia_el_timbre() {
    let band = |x: &[f32], lo: f64, hi: f64| {
        let mut s = 0.0;
        let mut f = lo;
        while f <= hi {
            s += tone(x, f);
            f += 20.0;
        }
        s
    };
    let take = |vx: f64, vy: f64| {
        let mut c = Choir::new(FS, 1);
        c.set_params(ChoirParams { mode: Mode::Solo, ensemble: Ensemble::Bajos, singers: 1, vowel_x: vx, vowel_y: vy, ..dry() });
        c.note_on(1, 45, 0.7);
        let l = render(&mut c, 1.2).0;
        l[30000..].to_vec()
    };
    let (a, i, u) = (take(0.45, 1.0), take(0.0, 0.0), take(1.0, 0.0));
    let open = |x: &[f32]| band(x, 600.0, 900.0) / band(x, 100.0, 4000.0);
    let front = |x: &[f32]| band(x, 1900.0, 3200.0) / band(x, 100.0, 4000.0);
    assert!(open(&a) > 3.0 * open(&i) && open(&a) > 3.0 * open(&u), "la A no es más abierta que I y U");
    assert!(front(&i) > 3.0 * front(&u), "la I no es más anterior que la U");
}

#[test]
fn los_hilos_no_cambian_el_resultado() {
    let take = |threads: usize| {
        let mut c = Choir::new(FS, threads);
        c.set_params(ChoirParams { singers: 8, max_voices: 8, ..ChoirParams::DEFAULT });
        for (id, key) in [(1, 43u8), (2, 55), (3, 64), (4, 72), (5, 60), (6, 67)] {
            c.note_on(id, key, 0.7);
        }
        let mut out = render(&mut c, 0.5);
        c.all_notes_off();
        let t = render(&mut c, 0.3);
        out.0.extend(t.0);
        out.1.extend(t.1);
        out
    };
    let (a, b, c) = (take(1), take(3), take(4));
    assert!(a == b && a == c, "el audio depende de la cantidad de hilos");
}

#[test]
fn el_tamano_de_bloque_no_cambia_el_resultado_si_es_multiplo_del_tramo() {
    let take = |block: usize| {
        let mut c = Choir::new(FS, 1);
        c.set_params(ChoirParams { singers: 4, max_voices: 4, ..ChoirParams::DEFAULT });
        c.note_on(1, 60, 0.7);
        c.note_on(2, 67, 0.7);
        let n = 19200;
        let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
        for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
            c.render(a, b);
        }
        l
    };
    assert!(take(128) == take(1024));
}

#[test]
fn todos_los_programas_suenan() {
    for p in PRESETS.iter() {
        let mut c = Choir::new(FS, 4);
        // el tope de voces acota el tiempo del test, no cambia lo que se prueba
        c.set_params(ChoirParams { max_voices: p.params.max_voices.min(4), ..p.params });
        assert_eq!(c.singers(), p.params.singers, "{}", p.name);
        let mid = c.singer(0).unwrap().voice_type.range();
        let key = (mid.0 + mid.1) / 2;
        c.note_on(1, key, 0.7);
        c.note_on(2, key + 4, 0.7);
        let (l, r) = render(&mut c, 0.7);
        assert!(finite(&l) && finite(&r), "{}: NaN", p.name);
        assert!(rms(&l[16000..]) + rms(&r[16000..]) > 0.004, "{}: no suena", p.name);
        assert!(peak(&l).max(peak(&r)) <= 1.0, "{}: fuera de rango", p.name);
    }
    let mut names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 36, "hay programas con el mismo nombre o sin nombre");
    assert!(PRESETS.iter().all(|p| !p.name.is_empty()));
}

#[test]
fn eco_suena_parecido_a_alta() {
    let take = |q: Quality| {
        let mut c = Choir::new(FS, 1);
        c.set_params(ChoirParams { mode: Mode::Solo, ensemble: Ensemble::Tenores, singers: 1, quality: q, ..dry() });
        c.note_on(1, 57, 0.7);
        let l = render(&mut c, 1.2).0;
        l[30000..].to_vec()
    };
    let (hi, eco) = (take(Quality::Alta), take(Quality::Eco));
    assert!(finite(&eco));
    let (a, b) = (rms(&hi), rms(&eco));
    assert!((a - b).abs() < 0.35 * a, "nivel distinto: alta {a} eco {b}");
    assert!(tone(&eco, hz(57.0)) > 30.0 * tone(&eco, hz(57.0) * 1.06), "eco desafina");
}

#[test]
fn expresion_por_nota_y_entradas_hostiles() {
    let mut c = Choir::new(FS, 2);
    c.set_params(ChoirParams { singers: 4, max_voices: 4, ..dry() });
    c.note_on(7, 60, 0.7);
    render(&mut c, 0.6);
    // subir un tono esa sola nota (MPE)
    c.note_pitch_bend(7, 2.0);
    let (l, _) = render(&mut c, 0.8);
    let tail = &l[20000..];
    assert!(tone(tail, hz(62.0)) > 5.0 * tone(tail, hz(60.0)), "la nota no siguió su afinación");
    // valores absurdos: no rompen nada
    c.note_pitch_bend(7, f64::NAN);
    c.note_pressure(7, f64::INFINITY);
    c.note_brightness(999, -5.0);
    c.set_pitch_bend(f64::NAN);
    c.note_off(12345);
    c.note_on(-9, 200, 9.0);
    c.set_params(ChoirParams { vowel_x: f64::NAN, vowel_y: 99.0, vowel_move: 50.0, vowel_rate: -3.0, humanize: 7.0, vibrato: -1.0, breath: 9.0, gain_db: 900.0, width: -4.0, depth: 44.0, room_mix: 3.0, mic: -2.0, max_voices: 0, ..*c.params() });
    let (l, r) = render(&mut c, 0.4);
    assert!(l.iter().chain(r.iter()).all(|v| v.is_finite() && v.abs() <= 1.0));
    // cambio de conjunto en caliente: reconstruye y sigue andando
    c.set_params(ChoirParams { singers: 64, ensemble: Ensemble::Ninos, face: Face { neck: 5.0, size: -5.0, jaw: f64::NAN, lips: 1.0, nose: 3.0, age: -2.0 }, ..ChoirParams::DEFAULT });
    assert_eq!(c.singers(), 64);
    c.note_on(1, 67, 0.7);
    let (l, _) = render(&mut c, 0.5);
    assert!(finite(&l) && rms(&l[12000..]) > 0.002);
}

#[test]
fn editar_la_cara_de_un_cantante_no_corta_a_los_demas() {
    let mut c = Choir::new(FS, 1);
    let base = ChoirParams { singers: 4, max_voices: 4, ..dry() };
    c.set_params(base);
    c.note_on(1, 48, 0.7);
    c.note_on(2, 60, 0.7);
    render(&mut c, 0.6);
    let before: Vec<_> = (0..4).map(|i| c.singer(i).unwrap()).collect();
    // la cara del cantante 3 (que no está cantando) cambia a mano
    let mut p = base;
    p.custom[3] = Some(Face { neck: 1.0, size: 1.0, jaw: 0.5, lips: -0.5, nose: 0.8, age: 0.6 });
    c.set_params(p);
    assert_eq!(c.sounding(), 2, "las dos notas siguen después de editar otra cara");
    let (l, _) = render(&mut c, 0.3);
    assert!(rms(&l) > 0.01, "el coro se calló al editar una cara");
    assert_eq!(c.singer(0).unwrap(), before[0]);
    assert_eq!(c.singer(1).unwrap(), before[1]);
    let s3 = c.singer(3).unwrap();
    assert!(s3 != before[3] && s3.body.pharynx > before[3].body.pharynx, "la cara nueva no cambió el cuerpo");
    // misma persona: sólo cambia la anatomía, no la afinación ni el lugar
    assert_eq!((s3.detune, s3.pan), (before[3].detune, before[3].pan));
    // lo que dibuja la interfaz es lo mismo que suena
    assert_eq!(p.singer(3), s3);
    // volver a "sin editar" devuelve al cantante original
    c.set_params(base);
    assert_eq!(c.singer(3).unwrap(), before[3]);
    // nivel por cantante: los que cantan tienen nivel, los otros no
    render(&mut c, 0.2);
    let lv = c.levels();
    assert!(lv.iter().filter(|x| **x > 1e-3).count() >= 2);
}
