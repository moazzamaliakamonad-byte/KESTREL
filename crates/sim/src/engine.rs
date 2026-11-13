//! The engine cluster: six engines that follow the commanded thrust vector
//! with an optional first-order lag, within limits that shrink as engines
//! fail.

use crate::{Vec3, norm};

/// Engines in the cluster.
pub const ENGINES: u8 = 6;
/// At most this many may fail (the visitor's "engine out" button).
pub const MAX_FAILED: u8 = 3;

#[derive(Debug, Clone)]
pub struct Engine {
    /// Healthy-cluster minimum thrust while lit, N.
    pub min_thrust: f64,
    /// Healthy-cluster maximum thrust, N.
    pub max_thrust: f64,
    /// Engines out, 0..=MAX_FAILED.
    pub failed: u8,
    /// First-order lag time constant, s (0 = instant).
    pub lag: f64,
    /// Thrust the engines are producing now, N.
    pub thrust: Vec3,
}

impl Engine {
    pub fn new(min_thrust: f64, max_thrust: f64) -> Engine {
        Engine {
            min_thrust,
            max_thrust,
            failed: 0,
            lag: 0.0,
            thrust: [0.0; 3],
        }
    }

    /// `(min, max)` thrust magnitude available with the engines still working.
    pub fn limits(&self) -> (f64, f64) {
        let working = f64::from(ENGINES - self.failed) / f64::from(ENGINES);
        (self.min_thrust * working, self.max_thrust * working)
    }

    /// Fail one more engine. Returns false (and changes nothing) once
    /// `MAX_FAILED` engines are already out.
    pub fn fail_one(&mut self) -> bool {
        if self.failed >= MAX_FAILED {
            return false;
        }
        self.failed += 1;
        true
    }

    /// Advance `dt` seconds toward the command. A zero command shuts the
    /// engines down at once; otherwise the thrust vector lags toward the
    /// command and its magnitude is clamped to [`Engine::limits`].
    pub fn step(&mut self, cmd: Vec3, dt: f64) {
        let cmd_mag = norm(cmd);
        if !(cmd_mag.is_finite() && cmd_mag > 0.0) {
            self.thrust = [0.0; 3];
            return;
        }
        // Exact solution of ẋ = (cmd − x)/lag over dt with cmd held.
        let k = if self.lag > 0.0 {
            1.0 - (-dt / self.lag).exp()
        } else {
            1.0
        };
        let mut t = [0, 1, 2].map(|i| self.thrust[i] + k * (cmd[i] - self.thrust[i]));
        let (lo, hi) = self.limits();
        let mag = norm(t);
        if mag == 0.0 {
            // Lagged exactly back to zero: point along the command instead.
            t = cmd.map(|c| c / cmd_mag * lo);
        } else if mag < lo {
            t = t.map(|c| c / mag * lo);
        } else if mag > hi {
            t = t.map(|c| c / mag * hi);
        }
        self.thrust = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        Engine::new(3000.0, 12000.0)
    }

    #[test]
    fn instant_without_lag() {
        let mut e = engine();
        e.step([0.0, 0.0, 8000.0], 0.01);
        assert_eq!(e.thrust, [0.0, 0.0, 8000.0]);
    }

    #[test]
    fn lag_step_reaches_63_percent_at_tau() {
        let mut e = engine();
        e.lag = 1.0;
        // Start lit at 4000 N, step the command to 10000 N.
        e.thrust = [0.0, 0.0, 4000.0];
        for _ in 0..100 {
            e.step([0.0, 0.0, 10000.0], 0.01);
        }
        let expected = 4000.0 + 6000.0 * (1.0 - (-1.0f64).exp());
        assert!((e.thrust[2] - expected).abs() < 1e-6, "{}", e.thrust[2]);
    }

    #[test]
    fn magnitude_is_clamped_both_ways() {
        let mut e = engine();
        e.step([0.0, 0.0, 50_000.0], 0.01);
        assert!((norm(e.thrust) - 12000.0).abs() < 1e-9);
        e.step([0.0, 100.0, 0.0], 0.01);
        assert!((norm(e.thrust) - 3000.0).abs() < 1e-9);
        assert!(e.thrust[1] > 0.0);
    }

    #[test]
    fn zero_command_shuts_down() {
        let mut e = engine();
        e.lag = 1.0;
        e.thrust = [0.0, 0.0, 9000.0];
        e.step([0.0; 3], 0.01);
        assert_eq!(e.thrust, [0.0; 3]);
    }

    #[test]
    fn engine_out_scales_limits() {
        let mut e = engine();
        for k in 1..=3u8 {
            assert!(e.fail_one());
            let f = f64::from(ENGINES - k) / f64::from(ENGINES);
            let (lo, hi) = e.limits();
            assert!((lo - 3000.0 * f).abs() < 1e-9 && (hi - 12000.0 * f).abs() < 1e-9);
        }
        assert!(!e.fail_one());
        assert_eq!(e.failed, MAX_FAILED);
    }
}
