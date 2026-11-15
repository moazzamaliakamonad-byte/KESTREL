//! The ship's three sea-level engines: ignition, a shared gimbal that slews
//! at a limited rate, throttle with an optional first-order lag, an engine
//! manager that shuts engines down when fewer are enough, and failures.
//!
//! All lit engines share one thrust direction and throttle (a simplification:
//! real engines gimbal individually and sit off the axis).

use model::quat::{self, Vec3};
use model::Ship;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineState {
    Off,
    /// Lit and spinning up: seconds until thrust.
    Igniting(f64),
    Running,
    /// Shut down by the engine manager; stays off for the rest of the flight.
    Shutdown,
    Failed,
}

impl EngineState {
    pub fn as_str(self) -> &'static str {
        match self {
            EngineState::Off => "off",
            EngineState::Igniting(_) => "igniting",
            EngineState::Running => "running",
            EngineState::Shutdown => "shutdown",
            EngineState::Failed => "failed",
        }
    }
}

/// At most this many engines may fail ("engine out" button).
pub const MAX_FAILED: usize = 2;
/// The manager shuts one engine down when the command stays below this
/// fraction of what one engine fewer could give…
pub const SHUTDOWN_FRACTION: f64 = 0.75;
/// …for this long, s.
pub const SHUTDOWN_HOLD: f64 = 1.0;

#[derive(Debug, Clone)]
pub struct Cluster {
    pub engines: [EngineState; 3],
    /// Throttle lag time constant, s (0 = instant).
    pub lag: f64,
    /// Thrust now, body axes, N.
    pub thrust: Vec3,
    /// Unit thrust direction now (body axes).
    pub direction: Vec3,
    /// Time the command has been low enough to drop an engine, s.
    low_for: f64,
    engine_max: f64,
    throttle_min: f64,
    gimbal_max: f64,
    gimbal_rate: f64,
    ignition_time: f64,
}

impl Cluster {
    pub fn new(ship: &Ship) -> Cluster {
        Cluster {
            engines: [EngineState::Off; 3],
            lag: 0.0,
            thrust: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
            low_for: 0.0,
            engine_max: ship.engine_max,
            throttle_min: ship.throttle_min,
            gimbal_max: ship.gimbal_max,
            gimbal_rate: ship.gimbal_rate,
            ignition_time: ship.ignition_time,
        }
    }

    pub fn running(&self) -> u8 {
        self.engines.iter().filter(|e| **e == EngineState::Running).count() as u8
    }

    /// Engines that can still give thrust (running, igniting or never lit).
    pub fn healthy(&self) -> u8 {
        self.engines
            .iter()
            .filter(|e| matches!(e, EngineState::Off | EngineState::Igniting(_) | EngineState::Running))
            .count() as u8
    }

    /// Engines running or about to be.
    pub fn lit(&self) -> u8 {
        self.engines
            .iter()
            .filter(|e| matches!(e, EngineState::Igniting(_) | EngineState::Running))
            .count() as u8
    }

    pub fn failed(&self) -> usize {
        self.engines.iter().filter(|e| **e == EngineState::Failed).count()
    }

    /// `(min, max)` thrust of the engines running now, N.
    pub fn limits(&self) -> (f64, f64) {
        let n = f64::from(self.running());
        (n * self.engine_max * self.throttle_min, n * self.engine_max)
    }

    /// Light every engine that is off.
    pub fn ignite_all(&mut self) {
        for e in &mut self.engines {
            if *e == EngineState::Off {
                *e = EngineState::Igniting(self.ignition_time);
            }
        }
    }

    /// Fail one engine (a running one first). False if the limit is reached.
    pub fn fail_next(&mut self) -> bool {
        if self.failed() >= MAX_FAILED {
            return false;
        }
        let pick = [
            EngineState::Running,
            EngineState::Igniting(0.0),
            EngineState::Off,
            EngineState::Shutdown,
        ]
        .iter()
        .find_map(|want| {
            self.engines
                .iter()
                .position(|e| std::mem::discriminant(e) == std::mem::discriminant(want))
        });
        match pick {
            Some(i) => {
                self.engines[i] = EngineState::Failed;
                true
            }
            None => false,
        }
    }

    /// Stop every engine (tanks empty, or the flight is over).
    pub fn cut(&mut self) {
        for e in &mut self.engines {
            if matches!(e, EngineState::Igniting(_) | EngineState::Running) {
                *e = EngineState::Shutdown;
            }
        }
        self.thrust = [0.0; 3];
    }

    /// Advance `h` seconds toward the body-frame thrust command `cmd`, N.
    pub fn step(&mut self, cmd: Vec3, h: f64) {
        for e in &mut self.engines {
            if let EngineState::Igniting(left) = e {
                *e = if *left - h <= 0.0 {
                    EngineState::Running
                } else {
                    EngineState::Igniting(*left - h)
                };
            }
        }
        self.manage(quat::norm(cmd), h);

        let n = self.running();
        if n == 0 {
            self.thrust = [0.0; 3];
            return;
        }
        // Direction: clamp to the gimbal cone, then slew at the rate limit.
        let want = clamp_cone(cmd, self.gimbal_max).unwrap_or(self.direction);
        self.direction = slew(self.direction, want, self.gimbal_rate * h);

        let (min, max) = self.limits();
        let target = quat::norm(cmd).clamp(min, max);
        let now = quat::norm(self.thrust);
        let mag = if self.lag > 0.0 {
            now + (target - now) * (h / self.lag).min(1.0)
        } else {
            target
        };
        // Spinning up from zero may pass below the minimum; never above max.
        self.thrust = quat::scale(self.direction, mag.min(max));
    }

    /// Engine manager: drop one engine when fewer are enough for a while.
    fn manage(&mut self, cmd: f64, h: f64) {
        let n = self.running();
        if n <= 1 || self.lit() != n {
            self.low_for = 0.0;
            return;
        }
        let fewer = f64::from(n - 1) * self.engine_max;
        if cmd < SHUTDOWN_FRACTION * fewer {
            self.low_for += h;
        } else {
            self.low_for = 0.0;
        }
        if self.low_for >= SHUTDOWN_HOLD {
            if let Some(i) = self.engines.iter().rposition(|e| *e == EngineState::Running) {
                self.engines[i] = EngineState::Shutdown;
            }
            self.low_for = 0.0;
        }
    }
}

/// `v` as a unit vector inside the cone of half-angle `max` around +z.
fn clamp_cone(v: Vec3, max: f64) -> Option<Vec3> {
    let n = quat::norm(v);
    if n < 1e-9 {
        return None;
    }
    let u = quat::scale(v, 1.0 / n);
    let angle = u[2].clamp(-1.0, 1.0).acos();
    if angle <= max {
        return Some(u);
    }
    let h = (u[0] * u[0] + u[1] * u[1]).sqrt();
    if h < 1e-12 {
        return Some([0.0, 0.0, 1.0]);
    }
    let (s, c) = max.sin_cos();
    Some([s * u[0] / h, s * u[1] / h, c])
}

/// Rotate unit vector `from` toward `to` by at most `step` radians.
fn slew(from: Vec3, to: Vec3, step: f64) -> Vec3 {
    let angle = quat::dot(from, to).clamp(-1.0, 1.0).acos();
    if angle <= step || angle < 1e-12 {
        return to;
    }
    let axis = quat::cross(from, to);
    let q = quat::from_axis_angle(axis, step);
    quat::rotate(q, from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cluster() -> Cluster {
        Cluster::new(&Ship::starship_class())
    }

    #[test]
    fn ignition_takes_one_second() {
        let mut c = cluster();
        c.ignite_all();
        let cmd = [0.0, 0.0, 5e6];
        for _ in 0..99 {
            c.step(cmd, 0.01);
        }
        assert_eq!(c.running(), 0);
        c.step(cmd, 0.01);
        c.step(cmd, 0.01);
        assert_eq!(c.running(), 3);
        assert!((quat::norm(c.thrust) - 5e6).abs() < 1.0);
    }

    #[test]
    fn failed_engine_reduces_limits() {
        let mut c = cluster();
        c.ignite_all();
        c.step([0.0, 0.0, 7e6], 1.5);
        assert!(c.fail_next());
        assert_eq!(c.running(), 2);
        c.step([0.0, 0.0, 7e6], 0.01);
        assert!((quat::norm(c.thrust) - 2.0 * 2.45e6).abs() < 1.0);
        assert!(c.fail_next());
        assert!(!c.fail_next(), "at most two failures");
    }

    #[test]
    fn gimbal_slews_at_limit_and_stays_in_cone() {
        let mut c = cluster();
        c.ignite_all();
        c.step([0.0, 0.0, 5e6], 1.5);
        let sideways = [5e6, 0.0, 0.0];
        c.step(sideways, 0.1);
        let angle = c.direction[2].acos();
        assert!((angle - 3f64.to_radians()).abs() < 1e-6, "{}", angle.to_degrees());
        for _ in 0..100 {
            c.step(sideways, 0.01);
        }
        assert!((c.direction[2].acos() - 15f64.to_radians()).abs() < 1e-9);
    }

    #[test]
    fn manager_drops_engines_when_fewer_are_enough() {
        let mut c = cluster();
        c.ignite_all();
        c.step([0.0, 0.0, 6e6], 1.5);
        assert_eq!(c.running(), 3);
        // 1.2 MN: below 75 % of two engines and of one engine's 2.45 MN.
        for _ in 0..105 {
            c.step([0.0, 0.0, 1.2e6], 0.01);
        }
        assert_eq!(c.running(), 2);
        for _ in 0..105 {
            c.step([0.0, 0.0, 1.2e6], 0.01);
        }
        assert_eq!(c.running(), 1);
        assert!((quat::norm(c.thrust) - 1.2e6).abs() < 1.0);
    }

    #[test]
    fn lag_reaches_63_percent_at_tau() {
        let mut c = cluster();
        c.ignite_all();
        c.step([0.0, 0.0, 4e6], 1.5);
        c.lag = 1.0;
        for _ in 0..1000 {
            c.step([0.0, 0.0, 6e6], 0.001);
        }
        let got = quat::norm(c.thrust);
        let want = 4e6 + 2e6 * (1.0 - (-1.0f64).exp());
        assert!((got - want).abs() / want < 1e-3, "{got} vs {want}");
    }
}
