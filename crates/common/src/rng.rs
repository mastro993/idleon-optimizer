//! The simulator's seeded random stream.
//!
//! Ported from `createRng` in `js/stats/systems/common/combat-sim-core.js`: a
//! mulberry32 generator with a call counter. Every path that rolls dice shares
//! one stream, so a seed plus a run's inputs reproduce its results exactly.

/// One seeded stream.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u32,
    calls: u64,
}

impl Rng {
    /// The stream `seed` starts, matching the upstream zero fallback.
    pub fn new(seed: u64) -> Self {
        let state = seed as u32;
        Self {
            state: if state == 0 { 0x9e37_79b9 } else { state },
            calls: 0,
        }
    }

    /// How many numbers the stream has produced.
    pub fn calls(&self) -> u64 {
        self.calls
    }

    /// The next number in `[0, 1)`.
    pub fn float(&mut self) -> f64 {
        self.calls += 1;
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let mut value = self.state;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        f64::from(value ^ (value >> 14)) / 4_294_967_296.0
    }

    /// The upstream `randomInt`: an inclusive integer roll with its swapped
    /// bounds and float truncation, so an inverted range still returns a value.
    pub fn int(&mut self, low: i64, high: i64) -> i64 {
        let roll = self.float();
        let (start, span) = if low <= high {
            (low, high - low + 1)
        } else {
            (high, low - high + 1)
        };
        start + (roll * span as f64).floor() as i64
    }

    /// The upstream `randomFloatBetween`, with the same swapped bounds.
    pub fn between(&mut self, low: f64, high: f64) -> f64 {
        let roll = self.float();
        if low <= high {
            low + roll * (high - low)
        } else {
            high + roll * (low - high)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_matches_upstream_mulberry32() {
        // node -e "const {createRng} = await import('./js/stats/systems/common/combat-sim-core.js'); const r = createRng(1); console.log(JSON.stringify([0,1,2,3].map(() => r.randomFloat())))"
        let mut rng = Rng::new(1);
        let values: Vec<f64> = (0..4).map(|_| rng.float()).collect();
        let want = [
            0.627_073_940_588_161_3,
            0.002_735_721_180_215_478,
            0.527_447_039_959_952_2,
            0.981_050_967_471_674_1,
        ];
        for (got, want) in values.iter().zip(want) {
            assert!((got - want).abs() < 1e-12, "{got} != {want}");
        }
        assert_eq!(rng.calls(), 4);
    }

    #[test]
    fn a_zero_seed_falls_back_like_upstream() {
        let mut zero = Rng::new(0);
        let mut fallback = Rng::new(0x9e37_79b9);
        assert_eq!(zero.float(), fallback.float());
    }

    #[test]
    fn an_inverted_range_still_rolls_inside_it() {
        let mut rng = Rng::new(7);
        for _ in 0..32 {
            let value = rng.int(9, 2);
            assert!((2..=9).contains(&value), "{value} outside 2..=9");
        }
    }
}
