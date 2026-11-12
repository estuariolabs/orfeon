//! Generador determinista chico (xorshift): el mismo coro suena igual en cada render.

#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // mezcla la semilla para que 1, 2, 3... den secuencias sin relación
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Rng((z ^ (z >> 31)) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// uniforme en 0..1
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// uniforme en -1..1
    pub fn bipolar(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
    /// campana aproximada en -1..1 (suma de tres uniformes)
    pub fn bell(&mut self) -> f64 {
        (self.bipolar() + self.bipolar() + self.bipolar()) / 3.0
    }
}
