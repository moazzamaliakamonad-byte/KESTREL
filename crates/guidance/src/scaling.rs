//! Non-dimensional scaling.
//!
//! Conic solvers struggle when numbers span many orders of magnitude
//! (positions of 1000 m next to masses of 2000 kg next to α ≈ 0.0005).
//! We pick reference units so the values the solver sees are near 1.

/// Reference units for length (m), time (s) and mass (kg).
///
/// Derived units: velocity `L/T`, acceleration `L/T²`, force `M·L/T²`.
/// `time = sqrt(length / gravity)` makes non-dimensional gravity exactly 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaling {
    pub length: f64,
    pub time: f64,
    pub mass: f64,
}

impl Scaling {
    pub fn new(length: f64, gravity: f64, mass: f64) -> Scaling {
        Scaling {
            length,
            time: (length / gravity).sqrt(),
            mass,
        }
    }

    fn velocity_unit(&self) -> f64 {
        self.length / self.time
    }

    fn accel_unit(&self) -> f64 {
        self.length / (self.time * self.time)
    }

    fn force_unit(&self) -> f64 {
        self.mass * self.accel_unit()
    }

    pub fn to_nd_length(&self, x: f64) -> f64 {
        x / self.length
    }
    pub fn from_nd_length(&self, x: f64) -> f64 {
        x * self.length
    }
    pub fn to_nd_velocity(&self, x: f64) -> f64 {
        x / self.velocity_unit()
    }
    pub fn from_nd_velocity(&self, x: f64) -> f64 {
        x * self.velocity_unit()
    }
    pub fn to_nd_accel(&self, x: f64) -> f64 {
        x / self.accel_unit()
    }
    pub fn from_nd_accel(&self, x: f64) -> f64 {
        x * self.accel_unit()
    }
    pub fn to_nd_time(&self, x: f64) -> f64 {
        x / self.time
    }
    pub fn from_nd_time(&self, x: f64) -> f64 {
        x * self.time
    }
    pub fn to_nd_force(&self, x: f64) -> f64 {
        x / self.force_unit()
    }
    pub fn from_nd_force(&self, x: f64) -> f64 {
        x * self.force_unit()
    }

    /// Non-dimensional α' for the log-mass equation `dz/dt' = −α'·σ'`.
    ///
    /// From `dz/dt = −α·σ`: `dz/dt' = T·dz/dt = −α·T·(L/T²)·σ' = −(α·L/T)·σ'`.
    pub fn nd_alpha(&self, alpha: f64) -> f64 {
        alpha * self.length / self.time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel_close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * a.abs().max(b.abs())
    }

    fn example() -> Scaling {
        Scaling::new(2500.0, 3.7114, 1905.0)
    }

    #[test]
    fn gravity_is_one_nd() {
        let s = example();
        assert!((s.to_nd_accel(3.7114) - 1.0).abs() <= 1e-12);
    }

    #[test]
    fn round_trip_each_quantity() {
        let s = example();
        let x = 1234.5;
        let pairs: [(&str, f64); 5] = [
            ("length", s.from_nd_length(s.to_nd_length(x))),
            ("velocity", s.from_nd_velocity(s.to_nd_velocity(x))),
            ("accel", s.from_nd_accel(s.to_nd_accel(x))),
            ("time", s.from_nd_time(s.to_nd_time(x))),
            ("force", s.from_nd_force(s.to_nd_force(x))),
        ];
        for (name, back) in pairs {
            assert!(rel_close(back, x, 1e-12), "{name}: {back} != {x}");
        }
    }

    #[test]
    fn nd_alpha_preserves_mass_decay() {
        // dz/dt = −α·σ in SI (1/s) must equal the non-dimensional rate
        // −α'·σ' converted back to per-second (divide by the time unit).
        let s = example();
        let alpha = 1.0 / (225.0 * 9.80665);
        let sigma = 10_000.0 / 1800.0; // Γ/m, m/s²
        let si_rate = alpha * sigma;
        let nd_rate = s.nd_alpha(alpha) * s.to_nd_accel(sigma) / s.time;
        assert!(rel_close(si_rate, nd_rate, 1e-12), "{si_rate} vs {nd_rate}");
    }
}
