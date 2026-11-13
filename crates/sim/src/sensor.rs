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

#[derive(Debug, Clone)]
pub struct Sensor {
    pub noise: Noise,
    rng: Rng,
}

impl Sensor {
    pub fn new(noise: Noise, seed: u64) -> Sensor {
        Sensor {
            noise,
            rng: Rng::new(seed),
        }
    }

    pub fn measure(&mut self, truth: &Truth) -> Measurement {
        let (sp, sv) = (self.noise.pos_sigma, self.noise.vel_sigma);
        let position = truth.position.map(|x| x + sp * self.rng.gaussian());
        let velocity = truth.velocity.map(|x| x + sv * self.rng.gaussian());
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

    #[test]
    fn noise_mean_and_sigma() {
        let mut s = Sensor::new(Noise::HIGH, 9);
        let n = 100_000;
        let errs: Vec<f64> = (0..n)
            .map(|_| s.measure(&truth()).position[0] - 100.0)
            .collect();
        let mean = errs.iter().sum::<f64>() / n as f64;
        let sigma = (errs.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
        assert!(mean.abs() < 0.05 * 8.0, "mean {mean}");
        assert!((sigma / 8.0 - 1.0).abs() < 0.02, "sigma {sigma}");
    }

    #[test]
    fn same_seed_same_noise() {
        let a = Sensor::new(Noise::LOW, 5).measure(&truth());
        let b = Sensor::new(Noise::LOW, 5).measure(&truth());
        assert_eq!(a, b);
        assert_ne!(a.position, truth().position);
    }
}
