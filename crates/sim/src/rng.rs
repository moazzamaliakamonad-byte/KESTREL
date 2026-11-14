//! A tiny seeded random number generator, so every run is reproducible.
//!
//! SplitMix64 (Steele, Lea & Flood 2014) for uniform bits, Box–Muller for
//! Gaussian samples. Not cryptographic; plenty for sensor noise.

#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
    /// Box–Muller makes two samples at a time; the second waits here.
    spare: Option<f64>,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: seed,
            spare: None,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        // The top 53 bits fill an f64 mantissa exactly.
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard normal (mean 0, standard deviation 1).
    pub fn gaussian(&mut self) -> f64 {
        if let Some(x) = self.spare.take() {
            return x;
        }
        // 1 − u is in (0, 1], so the logarithm is finite.
        let u1 = 1.0 - self.uniform();
        let u2 = self.uniform();
        let radius = (-2.0 * u1.ln()).sqrt();
        let angle = std::f64::consts::TAU * u2;
        self.spare = Some(radius * angle.sin());
        radius * angle.cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.gaussian().to_bits(), b.gaussian().to_bits());
        }
        let mut c = Rng::new(43);
        assert_ne!(Rng::new(42).next_u64(), c.next_u64());
    }

    #[test]
    fn uniform_is_in_unit_interval() {
        let mut r = Rng::new(1);
        for _ in 0..10_000 {
            let u = r.uniform();
            assert!((0.0..1.0).contains(&u), "{u}");
        }
    }

    #[test]
    fn gaussian_mean_and_sigma() {
        let mut r = Rng::new(7);
        let n = 100_000;
        let xs: Vec<f64> = (0..n).map(|_| r.gaussian()).collect();
        let mean = xs.iter().sum::<f64>() / n as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var.sqrt() - 1.0).abs() < 0.02, "sigma {}", var.sqrt());
        assert!(xs.iter().all(|x| x.is_finite()));
    }
}
