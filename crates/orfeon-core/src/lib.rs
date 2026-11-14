//! Orfeón: coro de cantantes modelados físicamente.
//!
//! Cada cantante es una instancia propia del motor de voz de CHIMERA (pulmones, cuerdas
//! vocales, tracto y nariz simulados), con su cuerpo, su rostro y su manera de cantar. Este
//! crate no hace E/S: recibe notas y entrega audio estéreo. Lo usan el plugin y la línea de
//! comandos.

pub mod choir;
pub mod console;
pub mod lyrics;
pub mod preset;
pub mod rng;
pub mod room;
pub mod singer;
pub mod style;
pub mod vowel;

pub use choir::{Choir, ChoirParams, Mode, Part, Quality, PART_COUNT};
pub use preset::{Preset, PRESETS};
pub use room::RoomKind;
pub use singer::{Ensemble, Face, Singer, VoiceType};
pub use style::Style;

/// Máximo de cantantes definidos en un coro.
pub const MAX_SINGERS: usize = 64;
