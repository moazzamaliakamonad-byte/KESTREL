//! Sensors: what guidance is allowed to know. Position and velocity carry
//! independent Gaussian noise; mass and time are reported exactly.

use crate::Vec3;
use crate::rng::Rng;
use crate::sim::Truth;

/// Standard deviations of the measurement noise, per axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Noise {
    /// m
    pub pos_sigma: f64,
    /// m/s
    pub vel_sigma: f64,
}

impl Noise {
    pub const OFF: Noise = Noise {
        pos_sigma: 0.0,
        vel_sigma: 0.0,
    };
    pub const LOW: Noise = Noise {
        pos_sigma: 2.0,
        vel_sigma: 0.2,
    };
    pub const HIGH: Noise = Noise {
        pos_sigma: 8.0,
        vel_sigma: 0.8,
    };
}

/// One sensor reading.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// Sim time of the reading, s.
    pub t: f64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f64,
}

/// How long the navigation error takes to decorrelate, s.
pub const CORRELATION_TIME: f64 = 2.0;

/// Below this altitude the landing radar takes over, m.
pub const RADAR_RANGE: f64 = 200.0;

/// Error scale at altitude `z`: 1 above [`RADAR_RANGE`], then proportional
/// to range (like a landing radar), never below 10 %.
fn range_scale(z: f64) -> f64 {
    (z / RADAR_RANGE).clamp(0.1, 1.0)
}

/// The navigation error is first-order Gauss–Markov per axis: it wanders
/// slowly with the stated σ, like the output of a real navigation filter,
/// rather than jumping independently every reading (white noise at 100 Hz
/// would mostly be averaged away by the vehicle while saturating the
/// controller).
#[derive(Debug, Clone)]
pub struct Sensor {
    pub noise: Noise,
    rng: Rng,
    /// Unit-variance error states: position xyz, velocity xyz.
    error: Option<[f64; 6]>,
    last_t: f64,
}

impl Sensor {
    pub fn new(noise: Noise, seed: u64) -> Sensor {
        Sensor {
            noise,
            rng: Rng::new(seed),
            error: None,
            last_t: 0.0,
        }
    }

    pub fn measure(&mut self, truth: &Truth) -> Measurement {
        let e = match self.error {
            // First reading: start from the stationary distribution.
            None => [(); 6].map(|_| self.rng.gaussian()),
            Some(mut e) => {
                let dt = (truth.t - self.last_t).max(0.0);
                if dt > 0.0 {
                    let phi = (-dt / CORRELATION_TIME).exp();
                    let kick = (1.0 - phi * phi).sqrt();
                    for x in &mut e {
                        *x = phi * *x + kick * self.rng.gaussian();
                    }
                }
                e
            }
        };
        self.error = Some(e);
        self.last_t = truth.t;
        let near = range_scale(truth.position[2]);
        let (sp, sv) = (near * self.noise.pos_sigma, near * self.noise.vel_sigma);
        let position = [0, 1, 2].map(|i| truth.position[i] + sp * e[i]);
        let velocity = [0, 1, 2].map(|i| truth.velocity[i] + sv * e[3 + i]);
        Measurement {
            t: truth.t,
            position,
            velocity,
            mass: truth.mass,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn truth() -> Truth {
        Truth {
            t: 3.0,
            position: [100.0, 0.0, 500.0],
            velocity: [-5.0, 0.0, -20.0],
            mass: 1800.0,
            landed: false,
            touchdown_velocity: None,
        }
    }

    #[test]
    fn off_is_exact() {
        let mut s = Sensor::new(Noise::OFF, 1);
        let m = s.measure(&truth());
        assert_eq!(
            (m.t, m.position, m.velocity, m.mass),
            (3.0, truth().position, truth().velocity, 1800.0)
        );
    }

    fn at(t: f64) -> Truth {
        Truth { t, ..truth() }
    }

    #[test]
    fn noise_mean_and_sigma() {
        // Readings 10 s apart (5 time constants) are nearly independent.
        let mut s = Sensor::new(Noise::HIGH, 9);
        let n = 100_000;
        let errs: Vec<f64> = (0..n)
            .map(|i| s.measure(&at(10.0 * i as f64)).position[0] - 100.0)
            .collect();
        let mean = errs.iter().sum::<f64>() / n as f64;
        let sigma = (errs.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
        assert!(mean.abs() < 0.05 * 8.0, "mean {mean}");
        assert!((sigma / 8.0 - 1.0).abs() < 0.02, "sigma {sigma}");
    }

    #[test]
    fn error_drifts_slowly() {
        // Gauss–Markov: over 10 ms the error changes by σ·√(1 − φ²) ≈ 0.1σ.
        let mut s = Sensor::new(Noise::HIGH, 2);
        let mut steps = Vec::new();
        let mut prev = s.measure(&at(0.0)).position[0];
        for i in 1..=20_000 {
            let e = s.measure(&at(0.01 * i as f64)).position[0];
            steps.push(e - prev);
            prev = e;
        }
        let rms = (steps.iter().map(|d| d * d).sum::<f64>() / steps.len() as f64).sqrt();
        let expected = 8.0 * (1.0 - (-2.0 * 0.01 / CORRELATION_TIME).exp()).sqrt();
        assert!(
            (rms / expected - 1.0).abs() < 0.05,
            "rms step {rms} vs {expected}"
        );
    }

    #[test]
    fn error_shrinks_near_the_ground() {
        // Landing radar: error proportional to range below 200 m, floor 10 %.
        let low = |z: f64| Truth {
            position: [0.0, 0.0, z],
            ..at(5.0)
        };
        for (z, scale) in [
            (1000.0, 1.0),
            (200.0, 1.0),
            (100.0, 0.5),
            (5.0, 0.1),
            (0.0, 0.1),
        ] {
            let mut a = Sensor::new(Noise::HIGH, 11);
            let mut b = Sensor::new(Noise::HIGH, 11);
            let vx = truth().velocity[0];
            let ea = a.measure(&low(z)).velocity[0] - vx;
            let eb = b.measure(&low(1000.0)).velocity[0] - vx;
            assert!(
                (ea - scale * eb).abs() < 1e-12,
                "z {z}: {ea} vs {}",
                scale * eb
            );
        }
    }

    #[test]
    fn same_time_same_reading() {
        let mut s = Sensor::new(Noise::HIGH, 4);
        let a = s.measure(&at(1.0));
        let b = s.measure(&at(1.0));
        assert_eq!(a, b, "guidance and controller see one navigation solution");
    }

    #[test]
    fn same_seed_same_noise() {
        let a = Sensor::new(Noise::LOW, 5).measure(&truth());
        let b = Sensor::new(Noise::LOW, 5).measure(&truth());
        assert_eq!(a, b);
        assert_ne!(a.position, truth().position);
    }
}
