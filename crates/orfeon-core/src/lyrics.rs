//! Preparación de pronunciación y gestos fuera del hilo de audio.
use crate::{ChoirParams, PART_COUNT};
use chimera::anatomy::human::BodyMix;
use chimera::control::score::{ControlKey, Gesture};
use chimera::phonetics::ipa::{skipped_ipa, tokenize_ipa};
use chimera::phonetics::planner::is_vowel;
use chimera::phonetics::sing::{midi_to_hz, plan_note_off, plan_note_on, NoteOnOptions};
use chimera::phonetics::{Segment, Syllable};

pub const MAX_SYLLABLES: usize = 64;
const REFERENCE: f64 = 60.0;

#[derive(Default)]
pub struct Lyrics {
    pub(crate) lines: [Vec<SungSyllable>; PART_COUNT],
}

pub(crate) struct SungSyllable {
    pub(crate) continuation: bool,
    pub(crate) attack: Vec<Gesture>,
    pub(crate) release: Vec<Gesture>,
}

impl Lyrics {
    /// La separación explícita usa espacios o puntos: "kan.ta al sol".
    pub fn prepare(text: &[String; PART_COUNT], params: &ChoirParams) -> Result<Self, String> {
        let mut result = Self::default();
        for (i, line) in text.iter().enumerate() {
            let singer = params.singer(i);
            for token in line.split(|c: char| c.is_whitespace() || c == '.').filter(|x| !x.is_empty()) {
                if result.lines[i].len() >= MAX_SYLLABLES || token.len() > 48 {
                    return Err(format!("Canal {}: máximo {MAX_SYLLABLES} sílabas de hasta 48 bytes", i + 1));
                }
                if token == "-" {
                    let Some(previous) = result.lines[i].last() else {
                        return Err(format!("Canal {}: el melisma necesita una sílaba anterior", i + 1));
                    };
                    let continuation = SungSyllable {
                        continuation: true,
                        attack: previous.attack.clone(),
                        release: previous.release.clone(),
                    };
                    result.lines[i].push(continuation);
                    continue;
                }
                let syl = parse(token, singer.mix).map_err(|e| format!("Canal {} · {token}: {e}", i + 1))?;
                result.lines[i].push(SungSyllable {
                    continuation: false,
                    attack: plan_note_on(&syl, REFERENCE, &NoteOnOptions {
                        body: singer.mix, tag: 1.0, vibrato: 0.0, legato_from: None,
                    }),
                    release: plan_note_off(&syl, REFERENCE, singer.mix),
                });
            }
        }
        Ok(result)
    }
}

fn parse(token: &str, body: BodyMix) -> Result<Syllable, &'static str> {
    if !skipped_ipa(token).is_empty() {
        return Err("hay fonemas o marcas no reconocidos");
    }
    let mut sounds = tokenize_ipa(token);
    let mut vowels = sounds.iter().enumerate().filter(|(_, s)| is_vowel(&s.ipa, body));
    let Some((index, _)) = vowels.next() else { return Err("falta la vocal de la sílaba") };
    if vowels.next().is_some() {
        return Err("separá las sílabas con un punto");
    }
    let coda = sounds.split_off(index + 1);
    let nucleus = sounds.pop().ok_or("falta la vocal de la sílaba")?;
    for Segment { ipa, aspirated, .. } in &mut sounds {
        if aspirated.is_none() && matches!(ipa.as_str(), "p" | "t" | "k") {
            *aspirated = Some(false);
        }
    }
    Ok(Syllable { text: token.into(), onset: sounds, nucleus, coda, ..Default::default() })
}

pub(crate) fn shifted(mut gesture: Gesture, time: f64, key: f64) -> Gesture {
    gesture.t0 += time;
    gesture.t1 += time;
    if let Some(hz) = gesture.targets.get(ControlKey::F0) {
        gesture.targets.set(ControlKey::F0, hz * (midi_to_hz(key) / midi_to_hz(REFERENCE)));
    }
    gesture
}

/// Español rioplatense, con límites de palabra y reglas de grupos consonánticos.
/// Genera IPA editable antes de aplicar la letra.
pub fn spanish_ipa(text: &str) -> String {
    let mut syllables = Vec::new();
    for word in text.to_lowercase().split(|c: char| !c.is_alphabetic()) {
        if word.is_empty() {
            continue;
        }
        let mut letters = word.chars().peekable();
        let mut phones = Vec::new();
        while let Some(c) = letters.next() {
            let next = letters.peek().copied();
            let front = matches!(next, Some('e' | 'i' | 'é' | 'í'));
            let phone = match c {
                'a' | 'á' => "a", 'e' | 'é' => "e", 'i' | 'í' => "i",
                'o' | 'ó' => "o", 'u' | 'ú' | 'ü' => "u",
                'h' => continue,
                'b' | 'v' => "b",
                'c' if next == Some('h') => { letters.next(); "tʃ" }
                'c' if front => "s",
                'c' | 'k' => "k",
                'q' => { if next == Some('u') { letters.next(); } "k" }
                'g' if front => "x",
                'g' => {
                    if next == Some('u') && matches!(letters.clone().nth(1), Some('e' | 'i' | 'é' | 'í')) {
                        letters.next();
                    }
                    "g"
                }
                'j' => "x",
                's' | 'z' => "s",
                'x' if phones.is_empty() => "s",
                'x' => { phones.push(("k", false)); "s" }
                'l' if next == Some('l') => { letters.next(); "ʃ" }
                'y' if next.is_none() => "i",
                'y' => "ʃ",
                'r' if next == Some('r') => { letters.next(); "r" }
                'r' if phones.is_empty() || matches!(phones.last(), Some(("n" | "l" | "s", _))) => "r",
                'r' => "ɾ",
                'w' => "w", 'ñ' => "ɲ", 'd' => "d", 'f' => "f",
                'l' => "l", 'm' => "m", 'n' => "n", 'p' => "p", 't' => "t",
                _ => continue,
            };
            phones.push((phone, matches!(c, 'i' | 'u' | 'ü')));
        }
        let vowel = |p: &str| matches!(p, "a" | "e" | "i" | "o" | "u");
        let glide = |p: &str| if p == "i" { "j" } else { "w" };
        let mut nuclei = Vec::new();
        let mut i = 0;
        while i < phones.len() {
            let (p, weak) = phones[i];
            if !vowel(p) { i += 1; continue; }
            if let Some(&(q, weak_next)) = phones.get(i + 1) {
                if vowel(q) && (weak || weak_next) {
                    if weak { phones[i].0 = glide(p); } else { phones[i + 1].0 = glide(q); }
                    nuclei.push((i, i + 2));
                    i += 2;
                    continue;
                }
            }
            nuclei.push((i, i + 1));
            i += 1;
        }
        let mut start = 0;
        for (i, &(_, end)) in nuclei.iter().enumerate() {
            let split = if let Some(&(next, _)) = nuclei.get(i + 1) {
                let gap = next - end;
                let cluster = gap >= 2 && matches!((phones[next - 2].0, phones[next - 1].0),
                    ("p" | "b" | "f" | "k" | "g" | "t" | "d", "ɾ") |
                    ("p" | "b" | "f" | "k" | "g", "l"));
                next - if cluster { 2 } else { gap.min(1) }
            } else {
                phones.len()
            };
            syllables.push(phones[start..split].iter().map(|p| p.0).collect::<String>());
            start = split;
        }
    }
    syllables.join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convierte_espanol_y_valida_ipa() {
        assert_eq!(spanish_ipa("Canta al sol"), "kan.ta.al.sol");
        assert_eq!(spanish_ipa("guitarra, lluvia, país"), "gi.ta.ra.ʃu.bja.pa.is");
        let params = ChoirParams::DEFAULT;
        assert!(Lyrics::prepare(&["k".into(), "".into(), "".into(), "".into()], &params).is_err());
    }
}
