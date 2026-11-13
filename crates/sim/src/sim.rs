//! Point-mass dynamics of the true vehicle:
//! `ṙ = v`, `v̇ = (T + F_gust)/m − g ẑ`, `ṁ = −α‖T‖`,
//! integrated with classic RK4 in fixed substeps.

use crate::engine::Engine;
use crate::{Vec3, norm};

/// Largest integration substep, s.
pub const SUBSTEP: f64 = 0.01;
/// How long a gust blows, s (including ramps).
pub const GUST_DURATION: f64 = 2.0;
/// Ramp-up and ramp-down time of a gust, s.
pub const GUST_RAMP: f64 = 0.3;

/// Environment and vehicle constants the simulator needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// m/s², acting along −z.
    pub gravity: f64,
    /// kg. The engines stop when the mass reaches it (tanks empty).
    pub dry_mass: f64,
    /// Propellant per newton-second, kg/(N·s).
    pub alpha: f64,
}

/// The true state. Guidance never sees this directly.
#[derive(Debug, Clone, PartialEq)]
pub struct Truth {
    /// Sim time, s.
    pub t: f64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f64,
    /// Latched once the vehicle reaches the ground.
    pub landed: bool,
    /// Velocity at the moment of touchdown.
    pub touchdown_velocity: Option<Vec3>,
}

/// A sideways push: trapezoidal force profile starting at `start`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gust {
    /// Sim time the gust begins, s.
    pub start: f64,
    /// Unit direction of the push.
    pub dir: Vec3,
    /// Peak force, N.
    pub magnitude: f64,
}

impl Gust {
    /// Force at sim time `t`, N.
    pub fn force(&self, t: f64) -> Vec3 {
        let tau = t - self.start;
        if !(0.0..=GUST_DURATION).contains(&tau) {
            return [0.0; 3];
        }
        let ramp = (tau / GUST_RAMP)
            .min((GUST_DURATION - tau) / GUST_RAMP)
            .clamp(0.0, 1.0);
        self.dir.map(|d| d * self.magnitude * ramp)
    }
}

#[derive(Debug, Clone)]
pub struct Sim {
    pub params: Params,
    pub truth: Truth,
    pub engine: Engine,
    pub gusts: Vec<Gust>,
}

impl Sim {
    pub fn new(params: Params, engine: Engine, position: Vec3, velocity: Vec3, mass: f64) -> Sim {
        Sim {
            params,
            truth: Truth {
                t: 0.0,
                position,
                velocity,
                mass,
                landed: false,
                touchdown_velocity: None,
            },
            engine,
            gusts: Vec::new(),
        }
    }

    /// Total gust force at sim time `t`, N.
    pub fn external_force(&self, t: f64) -> Vec3 {
        self.gusts.iter().fold([0.0; 3], |acc, g| {
            let f = g.force(t);
            [acc[0] + f[0], acc[1] + f[1], acc[2] + f[2]]
        })
    }

    /// Advance `dt` seconds with thrust command `cmd` (N) held throughout.
    /// Engines are off once the tanks are empty or after touchdown.
    pub fn step(&mut self, cmd: Vec3, dt: f64) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }
        let mut remaining = dt;
        while remaining > 1e-12 {
            let h = remaining.min(SUBSTEP);
            self.substep(cmd, h);
            remaining -= h;
        }
    }

    fn substep(&mut self, cmd: Vec3, h: f64) {
        let p = self.params;
        if self.truth.landed {
            self.engine.thrust = [0.0; 3];
            self.truth.t += h;
            return;
        }
        let empty = self.truth.mass <= p.dry_mass;
        self.engine.step(if empty { [0.0; 3] } else { cmd }, h);
        let mut thrust = self.engine.thrust;

        // If this substep would burn past empty, burn only what is left:
        // the engines run for that fraction of the substep (on average).
        let burn = p.alpha * norm(thrust) * h;
        let left = self.truth.mass - p.dry_mass;
        if burn > left {
            let f = if burn > 0.0 {
                (left / burn).max(0.0)
            } else {
                0.0
            };
            thrust = thrust.map(|c| c * f);
        }
        let mdot = -p.alpha * norm(thrust);

        // RK4 on (r, v, m); thrust held, gust force evaluated at each stage.
        let t0 = self.truth.t;
        let deriv = |t: f64, v: Vec3, m: f64| -> (Vec3, Vec3, f64) {
            let f = self.external_force(t);
            let a = [0, 1, 2].map(|i| {
                let g = if i == 2 { p.gravity } else { 0.0 };
                (thrust[i] + f[i]) / m - g
            });
            (v, a, mdot)
        };
        let add = |x: Vec3, d: Vec3, s: f64| [x[0] + s * d[0], x[1] + s * d[1], x[2] + s * d[2]];
        let (r0, v0, m0) = (self.truth.position, self.truth.velocity, self.truth.mass);
        let (k1r, k1v, k1m) = deriv(t0, v0, m0);
        let (k2r, k2v, k2m) = deriv(t0 + h / 2.0, add(v0, k1v, h / 2.0), m0 + h / 2.0 * k1m);
        let (k3r, k3v, k3m) = deriv(t0 + h / 2.0, add(v0, k2v, h / 2.0), m0 + h / 2.0 * k2m);
        let (k4r, k4v, k4m) = deriv(t0 + h, add(v0, k3v, h), m0 + h * k3m);
        let combine = |x: Vec3, a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
            [0, 1, 2].map(|i| x[i] + h / 6.0 * (a[i] + 2.0 * b[i] + 2.0 * c[i] + d[i]))
        };
        let r1 = combine(r0, k1r, k2r, k3r, k4r);
        let v1 = combine(v0, k1v, k2v, k3v, k4v);
        let m1 = m0 + h / 6.0 * (k1m + 2.0 * k2m + 2.0 * k3m + k4m);

        self.truth.t = t0 + h;
        self.truth.position = r1;
        self.truth.velocity = v1;
        self.truth.mass = m1.max(p.dry_mass);
        if r1[2] <= 0.0 {
            self.truth.landed = true;
            self.truth.touchdown_velocity = Some(v1);
            self.truth.position[2] = 0.0;
            self.truth.velocity = [0.0; 3];
            self.engine.thrust = [0.0; 3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: f64 = 3.7114;

    fn params(alpha: f64) -> Params {
        Params {
            gravity: G,
            dry_mass: 1500.0,
            alpha,
        }
    }

    fn sim(alpha: f64, r: Vec3, v: Vec3) -> Sim {
        Sim::new(params(alpha), Engine::new(1000.0, 20_000.0), r, v, 1900.0)
    }

    #[test]
    fn free_fall_matches_formula() {
        let mut s = sim(1e-4, [0.0, 0.0, 1000.0], [3.0, 0.0, -10.0]);
        for _ in 0..50 {
            s.step([0.0; 3], 0.1);
        }
        let t = 5.0;
        let z = 1000.0 - 10.0 * t - 0.5 * G * t * t;
        assert!((s.truth.t - t).abs() < 1e-9);
        assert!(
            (s.truth.position[2] - z).abs() < 1e-6,
            "{}",
            s.truth.position[2]
        );
        assert!((s.truth.position[0] - 15.0).abs() < 1e-9);
        assert_eq!(s.truth.mass, 1900.0);
    }

    #[test]
    fn hover_holds_still() {
        let mut s = sim(0.0, [0.0, 0.0, 100.0], [0.0; 3]);
        for _ in 0..100 {
            s.step([0.0, 0.0, 1900.0 * G], 0.1);
        }
        assert!(norm(s.truth.velocity) < 1e-9);
        assert!((s.truth.position[2] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn mass_follows_rocket_equation() {
        // Zero gravity, constant thrust along x: Δv = v_e·ln(m0/m1).
        let alpha = 1.0 / (225.0 * 9.80665);
        let mut s = Sim::new(
            Params {
                gravity: 0.0,
                dry_mass: 1500.0,
                alpha,
            },
            Engine::new(1000.0, 20_000.0),
            [0.0, 0.0, 1000.0],
            [0.0; 3],
            1900.0,
        );
        for _ in 0..200 {
            s.step([10_000.0, 0.0, 0.0], 0.1);
        }
        let m1 = s.truth.mass;
        assert!((m1 - (1900.0 - alpha * 10_000.0 * 20.0)).abs() < 1e-6);
        let dv = (1.0 / alpha) * (1900.0 / m1).ln();
        assert!(
            (s.truth.velocity[0] - dv).abs() / dv < 1e-6,
            "{} vs {dv}",
            s.truth.velocity[0]
        );
    }

    #[test]
    fn gust_impulse() {
        let g = Gust {
            start: 1.0,
            dir: [1.0, 0.0, 0.0],
            magnitude: 2000.0,
        };
        let dt = 1e-4;
        let impulse: f64 = (0..50_000).map(|i| g.force(i as f64 * dt)[0] * dt).sum();
        let expected = 2000.0 * (GUST_DURATION - GUST_RAMP);
        assert!((impulse - expected).abs() / expected < 0.01, "{impulse}");
        assert_eq!(g.force(0.5), [0.0; 3]);
        assert_eq!(g.force(3.5), [0.0; 3]);
    }

    #[test]
    fn gust_pushes_the_vehicle() {
        let mut s = sim(0.0, [0.0, 0.0, 1000.0], [0.0; 3]);
        s.gusts.push(Gust {
            start: 0.0,
            dir: [1.0, 0.0, 0.0],
            magnitude: 1900.0,
        });
        for _ in 0..30 {
            s.step([0.0, 0.0, 1900.0 * G], 0.1);
        }
        // Impulse 1900·1.7 N·s on 1900 kg → 1.7 m/s.
        assert!(
            (s.truth.velocity[0] - 1.7).abs() < 1e-3,
            "{}",
            s.truth.velocity[0]
        );
    }

    #[test]
    fn empty_tank_engine_off() {
        let mut s = sim(1e-3, [0.0, 0.0, 5000.0], [0.0; 3]);
        // 20 kN at α = 1e-3 burns 20 kg/s: 400 kg gone after 20 s, climbing
        // at ~170 m/s; 80 s of coasting later it is falling.
        for _ in 0..1000 {
            s.step([0.0, 0.0, 20_000.0], 0.1);
        }
        assert!((s.truth.mass - 1500.0).abs() < 1e-9, "{}", s.truth.mass);
        assert_eq!(s.engine.thrust, [0.0; 3]);
        assert!(s.truth.velocity[2] < 0.0, "falls once empty");
    }

    #[test]
    fn touchdown_latches() {
        let mut s = sim(0.0, [0.0, 0.0, 10.0], [1.0, 0.0, -5.0]);
        for _ in 0..50 {
            s.step([0.0; 3], 0.1);
        }
        assert!(s.truth.landed);
        assert_eq!(s.truth.position[2], 0.0);
        assert_eq!(s.truth.velocity, [0.0; 3]);
        let v = s.truth.touchdown_velocity.unwrap();
        assert!(v[2] < -5.0 && (v[0] - 1.0).abs() < 1e-9);
        let t = s.truth.t;
        s.step([0.0, 0.0, 20_000.0], 1.0);
        assert_eq!(s.truth.position[2], 0.0, "stays down");
        assert!(s.truth.t > t, "time still runs");
    }
}
