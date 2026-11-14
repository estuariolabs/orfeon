//! Prototipo fuera de línea: un cantante toca «Canta al sol» con y sin consonantes.
use chimera::anatomy::human::BodyMix;
use chimera::control::score::Gesture;
use chimera::engine::Engine;
use chimera::phonetics::planner::is_vowel;
use chimera::phonetics::sing::{plan_note_off, plan_note_on, NoteOnOptions};
use chimera::phonetics::{Segment, Syllable};
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

const FS: usize = 48_000;
const BLOCK: usize = 128;
const BEAT: usize = 240 * BLOCK;
const HOLD: usize = 165 * BLOCK;
const SAMPLES: usize = 4 * BEAT + 60 * BLOCK;

struct Note {
    on: Vec<Gesture>,
    off: Vec<Gesture>,
}

fn plan(body: BodyMix, consonants: bool) -> Vec<Note> {
    // Transcripción manual para aislar la síntesis del futuro silabeador.
    let phrase = [
        ("can", "k", "a", "n", 57.0),
        ("ta", "t", "a", "", 60.0),
        ("al", "", "a", "l", 62.0),
        ("sol", "s", "o", "l", 57.0),
    ];
    phrase.iter().enumerate().map(|(i, &(word, onset, nucleus, coda, midi))| {
        assert!(is_vowel(nucleus, body), "vocal fuera del inventario: {nucleus}");
        let mut syl = Syllable {
            text: word.into(),
            nucleus: Segment::new(nucleus),
            ..Default::default()
        };
        if consonants {
            if !onset.is_empty() {
                let mut segment = Segment::new(onset);
                if matches!(onset, "k" | "t") {
                    segment.aspirated = Some(false);
                }
                syl.onset.push(segment);
            }
            if !coda.is_empty() {
                syl.coda.push(Segment::new(coda));
            }
        }
        let options = NoteOnOptions { body, vibrato: 20.0, tag: tag(i), legato_from: None };
        Note {
            on: plan_note_on(&syl, midi, &options),
            off: plan_note_off(&syl, midi, body),
        }
    }).collect()
}

fn tag(note: usize) -> f64 {
    (2 * note + 1) as f64
}

fn render(body: BodyMix, notes: &[Note]) -> (Vec<f32>, Duration) {
    let mut engine = Engine::new(FS as f64);
    engine.set_body(body);
    let mut audio = vec![0.0; SAMPLES];
    let start = Instant::now();
    for (block, dst) in audio.chunks_mut(BLOCK).enumerate() {
        let pos = block * BLOCK;
        let index = pos / BEAT;
        if let Some(note) = notes.get(index) {
            match pos % BEAT {
                0 => { engine.schedule(&note.on, 0.0, true); }
                HOLD => {
                    engine.release(tag(index));
                    engine.release(tag(index) + 1.0);
                    engine.schedule(&note.off, 0.0, false);
                }
                _ => {}
            }
        }
        engine.render(dst);
    }
    (audio, start.elapsed())
}

fn wav(path: &Path, audio: &[f32]) -> io::Result<()> {
    let mut out = io::BufWriter::new(std::fs::File::create(path)?);
    let bytes = (audio.len() * 2) as u32;
    out.write_all(b"RIFF")?;
    out.write_all(&(bytes + 36).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&(FS as u32).to_le_bytes())?;
    out.write_all(&(FS as u32 * 2).to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for &x in audio {
        out.write_all(&((x.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())?;
    }
    out.flush()
}

fn main() -> io::Result<()> {
    let target = std::env::args_os().nth(1).unwrap_or_else(|| ".".into());
    let target = Path::new(&target);
    let body: BodyMix = "male".into();
    let plans = [plan(body, false), plan(body, true)];
    let mut timings = [Vec::new(), Vec::new()];
    let duration = SAMPLES as f64 / FS as f64;
    for round in 0..=7 {
        for j in 0..2 {
            let i = (round + j) % 2;
            let (audio, time) = render(body, &plans[i]);
            if round == 0 {
                assert!(audio.iter().all(|x| x.is_finite()));
                let rms = (audio.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / SAMPLES as f64).sqrt();
                assert!(rms > 0.001);
                let filename = ["solo-vocales.wav", "canta-al-sol.wav"][i];
                wav(&target.join(filename), &audio)?;
                println!("{filename}: {duration:.3} s, RMS {rms:.4}");
            } else {
                timings[i].push(time);
            }
        }
    }
    for (label, times) in ["Solo vocales", "Con letra"].iter().zip(&mut timings) {
        times.sort_unstable();
        let seconds = times[3].as_secs_f64();
        println!("{label}: {:.1} ms, {:.2}% del tiempo real (mediana de 7)", seconds * 1000.0, 100.0 * seconds / duration);
    }
    Ok(())
}
