//! Interfaz de Orfeón. La misma pantalla corre dentro del plugin (ventana del DAW) y en la
//! app independiente: no sabe cuál de los dos es. Lee y escribe `ChoirParams`, recibe
//! mediciones (`Telemetry`) y devuelve pedidos (`Event`) como notas del teclado en pantalla.

pub use egui;
pub mod face;
pub mod theme;
pub mod widgets;

use egui::{vec2, Align, Color32, Layout, RichText, ScrollArea, Sense, Slider, Ui};
use orfeon_core::preset::PRESETS;
use orfeon_core::singer::{Ensemble, Face, VoiceType};
use orfeon_core::{ChoirParams, Mode, Quality, RoomKind, Style, MAX_SINGERS};
use theme::{DIM, GOLD, LINE, PANEL, TEXT, WARN};

/// Tamaño de ventana para el que está pensada la interfaz.
pub const WIDTH: u32 = 1180;
pub const HEIGHT: u32 = 760;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Cantante,
    Voz,
    Vocal,
    Escenario,
    Ajustes,
}

/// Lo que la pantalla recuerda entre cuadros.
pub struct Model {
    pub params: ChoirParams,
    /// último programa elegido (para mostrar su nombre)
    pub preset: Option<usize>,
    pub selected: usize,
    pub tab: Tab,
    pub lyrics: [String; 4],
    pub words: [String; 4],
    pub lyrics_status: String,
    /// tecla que el mouse tiene apretada en el teclado de pantalla
    kb_note: Option<(u8, usize)>,
    /// fase del movimiento de la vocal (sólo para dibujar)
    phase: f32,
}

impl Model {
    pub fn new(params: ChoirParams, preset: Option<usize>) -> Model {
        Model { params, preset, selected: 0, tab: Tab::Cantante, lyrics: Default::default(), words: Default::default(), lyrics_status: String::new(), kb_note: None, phase: 0.0 }
    }
}

/// Mediciones que llegan del motor.
#[derive(Clone, Debug)]
pub struct Telemetry {
    /// carga del cálculo: 1 = todo el tiempo disponible
    pub load: f32,
    pub voices: usize,
    pub levels: [f32; MAX_SINGERS],
    /// notas que suenan por MIDI o por el DAW
    pub held: [bool; 128],
    /// cortes de audio (sólo la app independiente)
    pub underruns: Option<u32>,
    pub threads: usize,
}

impl Default for Telemetry {
    fn default() -> Self {
        Telemetry { load: 0.0, voices: 0, levels: [0.0; MAX_SINGERS], held: [false; 128], underruns: None, threads: 1 }
    }
}

/// Lo que sólo la app independiente tiene (en el plugin esto lo maneja el DAW).
#[derive(Clone, Debug, Default)]
pub struct Standalone {
    pub audio: String,
    pub sample_rate: u32,
    pub latency_ms: u32,
    pub midi_ports: Vec<String>,
    pub midi_selected: Option<usize>,
    /// un aviso para mostrar arriba (falta de audio, etc.)
    pub status: Option<String>,
}

/// Pedidos de la pantalla al motor.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    NoteOn(u8, f32, usize),
    NoteOff(u8, usize),
    AllOff,
    Threads(usize),
    Latency(u32),
    Midi(usize),
    ApplyLyrics,
    ResetLyrics,
    SaveQuartet,
    OpenQuartet,
}

fn note_name(k: u8) -> String {
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", N[(k % 12) as usize], k as i32 / 12 - 1)
}

fn short(v: VoiceType) -> &'static str {
    match v {
        VoiceType::Bajo => "Bajo",
        VoiceType::Baritono => "Barít.",
        VoiceType::Tenor => "Tenor",
        VoiceType::Contralto => "Contr.",
        VoiceType::Mezzo => "Mezzo",
        VoiceType::Soprano => "Sopr.",
        VoiceType::Nino => "Niño",
    }
}

fn style_desc(s: Style) -> &'static str {
    match s {
        Style::Clasico => "lírico, vibrato amplio",
        Style::Antiguo => "recto, casi sin vibrato",
        Style::Pop => "hablado, vibrato tardío",
        Style::Gospel => "apretado, entra desde abajo",
        Style::Folk => "abierto y brillante",
        Style::Susurro => "mucho aire",
        Style::Belting => "pecho arriba, potente",
        Style::Rasgado => "tenso y áspero",
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).color(GOLD).strong());
}

fn slider_f(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, label: &str) -> bool {
    let mut x = *v as f32;
    // sin max_decimals: redondea el valor al dibujar y eso cuenta como un cambio
    let r = ui.add(Slider::new(&mut x, lo as f32..=hi as f32).text(label).custom_formatter(|v, _| format!("{v:.2}")));
    if r.changed() {
        *v = x as f64;
        true
    } else {
        false
    }
}

/// Elige un programa: copia sus valores, conserva lo que depende de la máquina.
fn load_preset(m: &mut Model, i: usize) {
    let cur = m.params;
    m.params = ChoirParams { max_voices: cur.max_voices, quality: cur.quality, gain_db: cur.gain_db, ..PRESETS[i].params };
    m.preset = Some(i);
    m.selected = m.selected.min(m.params.singers.saturating_sub(1));
}

/// Dibuja la interfaz completa. Devuelve `true` si cambió algún parámetro.
pub fn show(ctx: &egui::Context, m: &mut Model, tel: &Telemetry, host: Option<&Standalone>, events: &mut Vec<Event>) -> bool {
    let before = m.params;
    let mut panic = ctx.input(|input| input.key_pressed(egui::Key::Escape));
    // fondo de toda la ventana: entre paneles no queda negro
    ctx.layer_painter(egui::LayerId::background()).rect_filled(ctx.screen_rect(), 0.0, PANEL);
    m.phase = (m.phase + m.params.vowel_rate.clamp(0.0, 20.0) as f32 / 30.0).fract();
    ctx.request_repaint_after(std::time::Duration::from_millis(33));

    // ---------------- barra de arriba ----------------
    egui::TopBottomPanel::top("orfeon-barra").exact_height(48.0).frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(12, 8))).show(ctx, |ui| {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("ORFEÓN").size(22.0).color(GOLD).strong());
            panic |= ui.button("Pánico").on_hover_text("Silenciar todo (Esc)").clicked();
            ui.add_space(14.0);
            // programas
            if ui.button("◀").on_hover_text("programa anterior").clicked() {
                let i = m.preset.map_or(PRESETS.len() - 1, |i| (i + PRESETS.len() - 1) % PRESETS.len());
                load_preset(m, i);
            }
            let modified = m.preset.map_or(false, |i| !PRESETS[i].params.same_sound(&ChoirParams { custom: [None; MAX_SINGERS], ..m.params }) || m.params.custom.iter().any(|c| c.is_some()));
            let name = match m.preset {
                Some(i) => format!("{}{}", PRESETS[i].name, if modified { " *" } else { "" }),
                None => "Manual".to_string(),
            };
            let mut pick = None;
            egui::ComboBox::from_id_salt("orfeon-programa").width(190.0).selected_text(RichText::new(name).strong()).height(520.0).show_ui(ui, |ui| {
                for (i, p) in PRESETS.iter().enumerate() {
                    let label = format!("{:2}  {}   ·  {} {}", i + 1, p.name, p.params.singers, if p.params.singers == 1 { "voz" } else { "voces" });
                    if ui.selectable_label(m.preset == Some(i), label).clicked() {
                        pick = Some(i);
                    }
                }
            });
            if let Some(i) = pick {
                load_preset(m, i);
            }
            if ui.button("▶").on_hover_text("programa siguiente").clicked() {
                let i = m.preset.map_or(0, |i| (i + 1) % PRESETS.len());
                load_preset(m, i);
            }
            ui.add_space(14.0);
            for (mode, label, tip) in [(Mode::Coro, "Coro", "cada nota va a otro cantante"), (Mode::Tutti, "Tutti", "todos cantan la última nota, cada uno en su octava"), (Mode::Solo, "Solo", "un cantante, ligado"), (Mode::Partes, "Cuarteto", "cuatro cantantes con entradas independientes")] {
                if ui.selectable_label(m.params.mode == mode, label).on_hover_text(tip).clicked() && m.params.mode != mode {
                    m.params.mode = mode;
                    m.selected = 0;
                    m.tab = Tab::Cantante;
                    events.push(Event::AllOff);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().slider_width = 92.0;
                let mut db = m.params.gain_db as f32;
                if ui.add(Slider::new(&mut db, -36.0..=12.0).suffix(" dB").custom_formatter(|v, _| format!("{v:.1}"))).on_hover_text("volumen").changed() {
                    m.params.gain_db = db as f64;
                }
                ui.label(RichText::new("Vol").color(DIM));
                ui.add_space(8.0);
                widgets::meter(ui, 88.0, tel.load, &format!("CPU {:.0} %", tel.load * 100.0));
                if let Some(u) = tel.underruns {
                    if u > 0 {
                        ui.label(RichText::new(format!("{u} cortes")).color(WARN)).on_hover_text("veces que el audio no llegó a tiempo: ver Ajustes");
                    }
                }
            });
        });
    });
    if let Some(msg) = host.and_then(|h| h.status.clone()) {
        egui::TopBottomPanel::top("orfeon-aviso").show(ctx, |ui| {
            ui.label(RichText::new(msg).color(WARN));
        });
    }

    // ---------------- teclado ----------------
    egui::TopBottomPanel::bottom("orfeon-teclado").exact_height(112.0).frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(12, 8))).show(ctx, |ui| {
        let mut types: Vec<VoiceType> = Vec::new();
        let (ensemble, count) = m.params.elenco();
        let part_mode = m.params.mode == Mode::Partes;
        for i in 0..count {
            if part_mode && i != m.selected.min(3) { continue; }
            let t = ensemble.voice_type(i);
            if !types.contains(&t) {
                types.push(t);
            }
        }
        types.sort_by_key(|t| t.range().0);
        let w = ui.available_width();
        let now = widgets::keyboard(ui, vec2(w, 96.0), &tel.held, &types)
            .map(|key| (key, if part_mode { m.selected.min(3) } else { 0 }));
        if now != m.kb_note {
            if let Some((k, part)) = m.kb_note {
                events.push(Event::NoteOff(k, part));
            }
            if let Some((k, part)) = now {
                events.push(Event::NoteOn(k, 0.75, part));
            }
            m.kb_note = now;
        }
    });

    // ---------------- elenco (izquierda) ----------------
    egui::SidePanel::left("orfeon-elenco").exact_width(356.0).resizable(false).frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(12))).show(ctx, |ui| {
        if m.params.mode == Mode::Partes {
            ui.heading("Cuarteto");
            ui.add_space(12.0);
            for (i, name) in PART_NAMES.iter().enumerate() {
                let part = m.params.parts[i];
                let label = format!("{}     {}–{}\nCanal {} · {}", name, note_name(part.low), note_name(part.high), i + 1, part.style.def().name);
                if ui.add_sized([320.0, 52.0], egui::Button::new(label).selected(i == m.selected)).clicked() {
                    m.selected = i;
                    m.tab = Tab::Cantante;
                }
            }
            return;
        }
        const INNER: f32 = 332.0;
        // el marco del panel se pinta alrededor del contenido: que ocupe todo el ancho
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.heading("Elenco");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("🎲 Otro elenco").on_hover_text("otras personas, mismo conjunto").clicked() {
                    m.params.seed = m.params.seed % 999 + 1;
                    m.params.custom = [None; MAX_SINGERS];
                }
            });
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("orfeon-conjunto").width(120.0).selected_text(m.params.ensemble.name()).show_ui(ui, |ui| {
                for e in Ensemble::ALL {
                    ui.selectable_value(&mut m.params.ensemble, e, e.name());
                }
            });
            let mut n = m.params.singers as u32;
            ui.spacing_mut().slider_width = 110.0;
            if ui.add(Slider::new(&mut n, 1..=64).text("cantantes")).changed() {
                m.params.singers = n as usize;
            }
        });
        let n = m.params.singers.clamp(1, MAX_SINGERS);
        m.selected = m.selected.min(n - 1);
        ui.add_space(4.0);
        // galería de caras
        let cols = 4;
        let cell = vec2((INNER - 14.0 - 6.0 * (cols - 1) as f32) / cols as f32, 92.0);
        let vowel = (m.params.vowel_x as f32, m.params.vowel_y as f32);
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("orfeon-caras").spacing(vec2(6.0, 6.0)).show(ui, |ui| {
                for i in 0..n {
                    let s = m.params.singer(i);
                    let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
                    let face_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.max.y - 14.0));
                    face::draw(ui.painter(), face_rect, &s, tel.levels[i] * 2.5, vowel, i == m.selected);
                    let edited = m.params.custom[i].is_some();
                    ui.painter().text(egui::pos2(rect.center().x, rect.bottom() - 6.0), egui::Align2::CENTER_CENTER, format!("{} {}{}", i + 1, short(s.voice_type), if edited { " *" } else { "" }), egui::FontId::proportional(11.0), if i == m.selected { GOLD } else { DIM });
                    if resp.clicked() {
                        m.selected = i;
                        m.tab = Tab::Cantante;
                    }
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
        });
    });

    // ---------------- centro: pestañas ----------------
    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::PANEL_HI).inner_margin(egui::Margin::same(14))).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (t, label) in [(Tab::Cantante, "Cantante"), (Tab::Voz, "Voz"), (Tab::Vocal, "Vocal"), (Tab::Escenario, "Escenario y sala"), (Tab::Ajustes, "Ajustes")] {
                if ui.selectable_label(m.tab == t, RichText::new(label).size(15.0)).clicked() {
                    m.tab = t;
                }
            }
        });
        ui.separator();
        match m.tab {
            Tab::Cantante if m.params.mode == Mode::Partes => tab_part(ui, m, tel, events, host.is_some()),
            Tab::Cantante => tab_singer(ui, m, tel),
            Tab::Voz => tab_voice(ui, m),
            Tab::Vocal => tab_vowel(ui, m),
            Tab::Escenario => tab_stage(ui, m, tel),
            Tab::Ajustes => tab_settings(ui, m, tel, host, events),
        }
    });

    if panic {
        events.push(Event::AllOff);
    }
    m.params != before
}

const PART_NAMES: [&str; 4] = ["Soprano", "Bajo", "Contralto", "Tenor"];

fn tab_part(ui: &mut Ui, m: &mut Model, tel: &Telemetry, events: &mut Vec<Event>, standalone: bool) {
    let i = m.selected.min(PART_NAMES.len() - 1);
    let part = &mut m.params.parts[i];
    ui.heading(PART_NAMES[i]);
    ui.label(format!("Entrada MIDI por canal {} · teclado de pantalla", i + 1));
    widgets::meter(ui, 160.0, tel.levels[i], "Nivel");
    ui.add_space(12.0);
    section(ui, "Registro");
    ui.horizontal(|ui| {
        ui.label("Desde");
        ui.add(egui::DragValue::new(&mut part.low).range(0..=part.high).custom_formatter(|v, _| note_name(v as u8)).custom_parser(parse_note_name));
        ui.label("Hasta");
        ui.add(egui::DragValue::new(&mut part.high).range(part.low..=127).custom_formatter(|v, _| note_name(v as u8)).custom_parser(parse_note_name));
    });
    section(ui, "Preset vocal");
    egui::ComboBox::from_id_salt("orfeon-part-style").selected_text(part.style.def().name).show_ui(ui, |ui| {
        for style in Style::ALL {
            ui.selectable_value(&mut part.style, style, style.def().name);
        }
    });
    ui.add_space(14.0);
    section(ui, "Letra de este cantante");
    ui.add(egui::TextEdit::singleline(&mut m.words[i]).hint_text("Escribí en español, por ejemplo Canta al sol").desired_width(f32::INFINITY).char_limit(512));
    if ui.button("Convertir a sílabas").clicked() {
        m.lyrics[i] = orfeon_core::lyrics::spanish_ipa(&m.words[i]);
    }
    ui.label("Pronunciación IPA · puntos entre sílabas, – para prolongar (escribí -)");
    ui.add(egui::TextEdit::multiline(&mut m.lyrics[i]).desired_rows(3).desired_width(f32::INFINITY).char_limit(2048));
    ui.horizontal(|ui| {
        if ui.button("Aplicar letras").clicked() { events.push(Event::ApplyLyrics); }
        if ui.button("Reiniciar cursores").clicked() { events.push(Event::ResetLyrics); }
        if standalone {
            if ui.button("Guardar cuarteto").clicked() { events.push(Event::SaveQuartet); }
            if ui.button("Abrir cuarteto").clicked() { events.push(Event::OpenQuartet); }
        }
    });
    if !m.lyrics_status.is_empty() {
        ui.label(&m.lyrics_status);
    }
}

fn parse_note_name(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().or_else(|| {
        (0u8..=127).find(|&n| text.eq_ignore_ascii_case(&note_name(n))).map(f64::from)
    })
}

fn face_sliders(ui: &mut Ui, f: &mut Face) -> bool {
    let mut ch = false;
    ch |= slider_f(ui, &mut f.neck, -1.0, 1.0, "cuello (faringe)");
    ch |= slider_f(ui, &mut f.size, -1.0, 1.0, "tamaño");
    ch |= slider_f(ui, &mut f.jaw, -1.0, 1.0, "mandíbula (lengua)");
    ch |= slider_f(ui, &mut f.lips, -1.0, 1.0, "labios");
    ch |= slider_f(ui, &mut f.nose, 0.0, 1.0, "nariz (resonancia nasal)");
    ch |= slider_f(ui, &mut f.age, -1.0, 1.0, "edad (niño ↔ mayor)");
    ch
}

fn tab_singer(ui: &mut Ui, m: &mut Model, tel: &Telemetry) {
    let n = m.params.singers.clamp(1, MAX_SINGERS);
    let i = m.selected.min(n - 1);
    let s = m.params.singer(i);
    ui.horizontal_top(|ui| {
        // retrato grande
        let (rect, _) = ui.allocate_exact_size(vec2(250.0, 300.0), Sense::hover());
        face::draw(ui.painter(), rect, &s, tel.levels[i] * 2.5, (m.params.vowel_x as f32, m.params.vowel_y as f32), false);
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if ui.button("‹").clicked() {
                    m.selected = (i + n - 1) % n;
                }
                ui.label(RichText::new(format!("Cantante {} de {}", i + 1, n)).size(18.0).strong());
                if ui.button("›").clicked() {
                    m.selected = (i + 1) % n;
                }
            });
            let (lo, hi) = s.voice_type.range();
            ui.label(RichText::new(format!("{} · registro {}–{} · {:+.0} cents", s.voice_type.name(), note_name(lo), note_name(hi), s.detune)).color(DIM));
            ui.label(RichText::new(format!("tracto ×{:.2} · faringe ×{:.2} · lengua {:.2} cm · labios {:.2} cm", s.body.scale, s.body.pharynx, s.body.tongue_radius, s.body.lip_width)).color(DIM).small());
            section(ui, "Rostro y cuerpo de este cantante");
            let mut f = m.params.custom[i].unwrap_or(s.face);
            if face_sliders(ui, &mut f) {
                m.params.custom[i] = Some(f);
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(m.params.custom[i].is_some(), egui::Button::new("Volver al del elenco")).clicked() {
                    m.params.custom[i] = None;
                }
                if ui.button("Borrar todas las caras editadas").clicked() {
                    m.params.custom = [None; MAX_SINGERS];
                }
            });
        });
    });
    ui.add_space(6.0);
    ui.separator();
    section(ui, "Rostro común a todo el coro");
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            face_sliders(ui, &mut m.params.face);
        });
        ui.vertical(|ui| {
            slider_f(ui, &mut m.params.variation, 0.0, 1.0, "variedad entre cantantes");
            let mut seed = m.params.seed;
            if ui.add(Slider::new(&mut seed, 1..=999).text("elenco")).changed() {
                m.params.seed = seed;
            }
            ui.label(RichText::new("Las caras editadas a mano no cambian con estos controles.").color(DIM).small());
        });
    });
}

fn tab_voice(ui: &mut Ui, m: &mut Model) {
    section(ui, "Estilo de canto");
    egui::Grid::new("orfeon-estilos").spacing(vec2(8.0, 8.0)).show(ui, |ui| {
        for (k, st) in Style::ALL.iter().enumerate() {
            let sel = m.params.style == *st;
            let b = egui::Button::new(RichText::new(format!("{}\n{}", st.def().name, style_desc(*st))).size(13.0)).selected(sel);
            if ui.add_sized([180.0, 50.0], b).clicked() {
                m.params.style = *st;
            }
            if k % 4 == 3 {
                ui.end_row();
            }
        }
    });
    ui.add_space(8.0);
    section(ui, "Expresión");
    slider_f(ui, &mut m.params.vibrato, 0.0, 2.0, "vibrato (× el del estilo)");
    slider_f(ui, &mut m.params.breath, -0.5, 0.5, "aire");
    slider_f(ui, &mut m.params.brightness, -0.5, 0.5, "brillo");
    slider_f(ui, &mut m.params.humanize, 0.0, 1.0, "humanizar (afinación, entradas, balanceo)");
    if m.params.mode == Mode::Coro {
        ui.checkbox(&mut m.params.voice_leading, "Conducción vocal")
            .on_hover_text("Favorece movimientos cortos de cada cantante al llegar notas nuevas");
        let can_group = m.params.voice_leading && m.params.ensemble == orfeon_core::Ensemble::Mixto
            && m.params.singers == 4 && m.params.max_voices >= 4;
        if can_group {
            ui.checkbox(&mut m.params.satb, "Asignar acordes en conjunto (SATB)")
                .on_hover_text("Busca un reparto para todo el acorde y conserva las notas sostenidas");
        }
        if can_group && m.params.satb {
            slider_f(ui, &mut m.params.phrase_ms, 0.0, 120.0, "Escalonado total de entradas y salidas (ms)");
        } else {
            slider_f(ui, &mut m.params.stagger_ms, 0.0, 40.0, "separación de entradas (ms)");
        }
    }
    ui.label(RichText::new("Cada cantante deriva, respira y entra un poco a su manera; humanizar en 0 los alinea a todos.").color(DIM).small());
}

fn tab_vowel(ui: &mut Ui, m: &mut Model) {
    ui.horizontal_top(|ui| {
        let w = (ui.available_width() - 340.0).clamp(300.0, 560.0);
        if widgets::vowel_pad(ui, vec2(w, 340.0), &mut m.params, m.phase) {}
        ui.add_space(12.0);
        ui.vertical(|ui| {
            section(ui, "Vocales del español");
            ui.horizontal(|ui| {
                for v in ['a', 'e', 'i', 'o', 'u'] {
                    let (x, y) = orfeon_core::vowel::spanish(v);
                    let sel = (m.params.vowel_x - x).abs() < 0.02 && (m.params.vowel_y - y).abs() < 0.02;
                    if ui.add(egui::Button::new(RichText::new(v.to_ascii_uppercase().to_string()).size(18.0)).min_size(vec2(34.0, 34.0)).selected(sel)).clicked() {
                        m.params.vowel_x = x;
                        m.params.vowel_y = y;
                    }
                }
            });
            section(ui, "Movimiento");
            ui.spacing_mut().slider_width = 110.0;
            slider_f(ui, &mut m.params.vowel_move, 0.0, 1.0, "cuánto se mueve");
            let mut r = m.params.vowel_rate as f32;
            if ui.add(Slider::new(&mut r, 0.02..=4.0).logarithmic(true).suffix(" Hz").text("velocidad")).changed() {
                m.params.vowel_rate = r as f64;
            }
            slider_f(ui, &mut m.params.vowel_spread, 0.0, 1.0, "cada uno distinto");
            ui.label(RichText::new("Arrastrá el punto por la carta.\nLas posturas salen de las vocales\ncalibradas de CHIMERA.").color(DIM).small());
        });
    });
}

fn tab_stage(ui: &mut Ui, m: &mut Model, tel: &Telemetry) {
    let w = ui.available_width();
    widgets::stage(ui, vec2(w, 300.0), &m.params, &tel.levels, &mut m.selected);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Sala").color(GOLD).strong());
        for r in RoomKind::ALL {
            if ui.selectable_label(m.params.room == r, r.name()).clicked() {
                m.params.room = r;
            }
        }
    });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            slider_f(ui, &mut m.params.room_mix, 0.0, 1.0, "cantidad de sala");
            slider_f(ui, &mut m.params.mic, 0.0, 1.0, "micrófono: cerca ↔ lejos");
        });
        ui.vertical(|ui| {
            slider_f(ui, &mut m.params.width, 0.0, 1.0, "ancho del escenario");
            slider_f(ui, &mut m.params.depth, 0.0, 1.0, "fondo del escenario");
        });
    });
}

fn tab_settings(ui: &mut Ui, m: &mut Model, tel: &Telemetry, host: Option<&Standalone>, events: &mut Vec<Event>) {
    section(ui, "Rendimiento");
    ui.horizontal(|ui| {
        ui.label("Calidad");
        ui.selectable_value(&mut m.params.quality, Quality::Alta, "Alta");
        ui.selectable_value(&mut m.params.quality, Quality::Eco, "Eco (menos CPU, sin agudos sobre 15 kHz)");
    });
    let mut v = m.params.max_voices as u32;
    if ui.add(Slider::new(&mut v, 1..=64).text("voces a la vez (tope)")).changed() {
        m.params.max_voices = v as usize;
    }
    let mut th = tel.threads as u32;
    if ui.add(Slider::new(&mut th, 1..=16).text("hilos de cálculo")).changed() {
        events.push(Event::Threads(th as usize));
    }
    ui.label(RichText::new(format!("Carga actual {:.0} %. Cada cantante que suena es una simulación completa; si el audio se corta, bajá el tope de voces o pasá a Eco.", tel.load * 100.0)).color(DIM).small());
    if let Some(h) = host {
        ui.add_space(6.0);
        section(ui, "Audio y MIDI");
        ui.label(format!("Salida: {} — {} Hz", h.audio, h.sample_rate));
        let mut lat = h.latency_ms;
        if ui.add(Slider::new(&mut lat, 10..=200).suffix(" ms").text("colchón de audio")).changed() {
            events.push(Event::Latency(lat));
        }
        if let Some(u) = tel.underruns {
            ui.label(RichText::new(format!("Cortes desde que se abrió: {u}")).color(if u > 0 { WARN } else { DIM }));
        }
        ui.horizontal(|ui| {
            ui.label("Teclado MIDI");
            let current = h.midi_selected.and_then(|i| h.midi_ports.get(i).cloned()).unwrap_or_else(|| if h.midi_ports.is_empty() { "no hay entradas".into() } else { "ninguno".into() });
            egui::ComboBox::from_id_salt("orfeon-midi").width(260.0).selected_text(current).show_ui(ui, |ui| {
                for (i, name) in h.midi_ports.iter().enumerate() {
                    if ui.selectable_label(h.midi_selected == Some(i), name).clicked() {
                        events.push(Event::Midi(i));
                    }
                }
            });
        });
    }
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Orfeón — software libre (GPL-3.0-or-later). Motor de voz: CHIMERA, de Jameson Hodge (MIT). Sin samples: cada cantante es una simulación de pulmones, cuerdas vocales, garganta, boca y nariz.").color(DIM).small());
    let _ = (LINE, TEXT, Color32::WHITE);
}
