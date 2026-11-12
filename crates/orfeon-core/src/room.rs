//! Sala: reverberación estéreo (red de retardos realimentados, 8 líneas). Sin E/S y sin
//! reservar memoria después de crearla.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Seco,
    Estudio,
    Sala,
    Iglesia,
    Catedral,
}

impl RoomKind {
    pub const ALL: [RoomKind; 5] = [RoomKind::Seco, RoomKind::Estudio, RoomKind::Sala, RoomKind::Iglesia, RoomKind::Catedral];

    pub fn name(self) -> &'static str {
        match self {
            RoomKind::Seco => "Seco",
            RoomKind::Estudio => "Estudio",
            RoomKind::Sala => "Sala de conciertos",
            RoomKind::Iglesia => "Iglesia",
            RoomKind::Catedral => "Catedral",
        }
    }
    pub fn from_index(i: usize) -> RoomKind {
        RoomKind::ALL[i.min(RoomKind::ALL.len() - 1)]
    }
    pub fn index(self) -> usize {
        RoomKind::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }

    /// (tamaño relativo, segundos de reverberación, amortiguación de agudos 0..1, pre-retardo ms)
    fn def(self) -> (f64, f64, f64, f64) {
        match self {
            RoomKind::Seco => (0.5, 0.0, 0.5, 0.0),
            RoomKind::Estudio => (0.45, 0.5, 0.55, 4.0),
            RoomKind::Sala => (0.85, 1.9, 0.40, 18.0),
            RoomKind::Iglesia => (1.15, 3.2, 0.48, 28.0),
            RoomKind::Catedral => (1.55, 6.5, 0.55, 45.0),
        }
    }
}

const LINES: usize = 8;
/// largos de base a 48 kHz y tamaño 1 (primos, sin divisores comunes)
const BASE: [f64; LINES] = [1423.0, 1637.0, 1801.0, 1973.0, 2143.0, 2311.0, 2503.0, 2687.0];
const MAX_SCALE: f64 = 1.6;

pub struct Room {
    fs: f64,
    kind: RoomKind,
    buf: Vec<Vec<f32>>,
    len: [usize; LINES],
    pos: [usize; LINES],
    gain: [f32; LINES],
    lp: [f32; LINES],
    damp: f32,
    pre_l: Vec<f32>,
    pre_r: Vec<f32>,
    pre_len: usize,
    pre_pos: usize,
    active: bool,
}

impl Room {
    pub fn new(fs: f64) -> Room {
        let cap = |b: f64| (b * MAX_SCALE * fs / 48000.0).ceil() as usize + 8;
        let pre_cap = (0.06 * fs) as usize + 8;
        let mut r = Room {
            fs,
            kind: RoomKind::Seco,
            buf: BASE.iter().map(|b| vec![0.0; cap(*b)]).collect(),
            len: [1; LINES],
            pos: [0; LINES],
            gain: [0.0; LINES],
            lp: [0.0; LINES],
            damp: 0.5,
            pre_l: vec![0.0; pre_cap],
            pre_r: vec![0.0; pre_cap],
            pre_len: 1,
            pre_pos: 0,
            active: false,
        };
        r.set_kind(RoomKind::Sala);
        r
    }

    pub fn kind(&self) -> RoomKind {
        self.kind
    }

    /// Cambia de sala (vacía la cola anterior). No reserva memoria.
    pub fn set_kind(&mut self, kind: RoomKind) {
        if kind == self.kind && self.len[0] > 1 {
            return;
        }
        self.kind = kind;
        let (size, rt60, damp, pre_ms) = kind.def();
        self.active = rt60 > 0.0;
        for i in 0..LINES {
            let n = ((BASE[i] * size * self.fs / 48000.0) as usize).clamp(8, self.buf[i].len() - 1);
            self.len[i] = n;
            self.pos[i] = 0;
            // cada línea pierde lo justo para que el conjunto caiga 60 dB en rt60
            self.gain[i] = if rt60 > 0.0 { 10f64.powf(-3.0 * n as f64 / (rt60 * self.fs)) as f32 } else { 0.0 };
            self.lp[i] = 0.0;
            for x in self.buf[i].iter_mut() {
                *x = 0.0;
            }
        }
        self.damp = damp as f32;
        self.pre_len = ((pre_ms * 0.001 * self.fs) as usize).clamp(1, self.pre_l.len() - 1);
        self.pre_pos = 0;
        for x in self.pre_l.iter_mut().chain(self.pre_r.iter_mut()) {
            *x = 0.0;
        }
    }

    /// Una muestra: entra la mezcla que va a la sala, sale sólo la reverberación.
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }
        // pre-retardo: la sala contesta un rato después del sonido directo
        let (xl, xr) = (self.pre_l[self.pre_pos], self.pre_r[self.pre_pos]);
        self.pre_l[self.pre_pos] = in_l;
        self.pre_r[self.pre_pos] = in_r;
        self.pre_pos += 1;
        if self.pre_pos >= self.pre_len {
            self.pre_pos = 0;
        }
        let mut out = [0f32; LINES];
        let mut sum = 0f32;
        for i in 0..LINES {
            let y = self.buf[i][self.pos[i]];
            // los agudos se apagan antes que los graves
            self.lp[i] += (1.0 - self.damp) * (y - self.lp[i]);
            out[i] = self.lp[i] * self.gain[i];
            sum += out[i];
        }
        let mix = sum * (2.0 / LINES as f32);
        for i in 0..LINES {
            let inp = if i % 2 == 0 { xl } else { xr };
            // matriz de Householder: reparte la energía entre líneas sin amplificarla
            let mut v = out[i] - mix + inp * 0.5;
            if v.abs() < 1e-20 {
                v = 0.0; // sin denormales en la cola
            }
            self.buf[i][self.pos[i]] = v;
            self.pos[i] += 1;
            if self.pos[i] >= self.len[i] {
                self.pos[i] = 0;
            }
        }
        (0.5 * (out[0] + out[2] + out[4] + out[6]), 0.5 * (out[1] + out[3] + out[5] + out[7]))
    }

    pub fn clear(&mut self) {
        let k = self.kind;
        self.len[0] = 1;
        self.set_kind(k);
    }
}
