/// PCG32 — deterministic across platforms incl. WASM, no external deps.
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut r = Rng { state: 0, inc: (seed << 1) | 1 };
        r.state = seed.wrapping_add(0x853c49e6748fea9b);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform f32 in [0, 1).
    pub fn gen_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform i32 in [lo, hi). Requires lo < hi.
    pub fn gen_range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u32() % (hi - lo) as u32) as i32
    }

    pub fn gen_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.gen_f32() * (hi - lo)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.gen_f32() < p
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.gen_range(0, i as i32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..100).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 5);
    }

    #[test]
    fn gen_range_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.gen_range(3, 9);
            assert!((3..9).contains(&v));
        }
    }

    #[test]
    fn gen_f32_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.gen_f32();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
