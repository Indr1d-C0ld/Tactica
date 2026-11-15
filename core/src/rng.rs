//! PRNG deterministico (SplitMix64). Solo aritmetica intera: stesso seed,
//! stessa sequenza su ogni piattaforma, quindi server, client e replay concordano.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn state(&self) -> u64 {
        self.state
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Intero uniforme in `0..n`.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "Rng::below(0)");
        (((self.next_u64() >> 32) * n as u64) >> 32) as u32
    }

    /// Intero uniforme in `lo..=hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        debug_assert!(lo <= hi);
        lo + self.below((hi - lo + 1) as u32) as i32
    }

    /// Vero con probabilità `pct`%.
    pub fn percent(&mut self, pct: i32) -> bool {
        (self.below(100) as i32) < pct
    }

    /// Generatore indipendente derivato da questo (per sottosistemi).
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            v.swap(i, j);
        }
    }
}
