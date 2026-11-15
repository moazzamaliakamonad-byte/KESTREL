//! The ship's true flight: 6-DoF rigid body (shared equations from
//! `model::dynamics`) plus actuators (engines, flaps, cold-gas RCS), wind
//! gusts, contact with the tower, the arms or the ground, and the outcome.
//! Guidance never reads this directly. Simulation only.

use model::aero::{self, FLAPS};
use model::dynamics::{self, Extra, State, M, Q, V, W};
use model::quat::{self, Vec3};
use model::{Planet, Scenario, VehicleKind};

use crate::raptors::Cluster;

/// Integration substep, s.
pub const SUBSTEP: f64 = 0.01;
/// Flap exposure slew rate, 1/s, and lowest exposure (folded).
pub const FLAP_RATE: f64 = 1.0;
pub const FLAP_MIN: f64 = 0.05;
/// Grid-fin slew rate, rad/s (*illustrative*).
pub const FIN_RATE: f64 = 1.0;
/// RCS fires on an axis when the request exceeds this.
pub const RCS_DEADBAND: f64 = 0.2;
/// Wind gust profile: total length and ramps, s.
pub const GUST_DURATION: f64 = 4.0;
pub const GUST_RAMP: f64 = 0.5;
/// Arms close when the pins are this far above them and the axis this close, m.
pub const ARMS_CLOSE_ABOVE: f64 = 8.0;
pub const ARMS_CLOSE_RADIUS: f64 = 3.0;
/// Catch limits.
pub const CATCH_DOWN: f64 = 2.0;
pub const CATCH_SIDE: f64 = 1.0;
pub const CATCH_TILT_DEG: f64 = 3.0;
pub const CATCH_RATE_DEG: f64 = 2.0;
/// Leg landing limits.
pub const LAND_DOWN: f64 = 2.0;
pub const LAND_SIDE: f64 = 1.0;
pub const LAND_TILT_DEG: f64 = 5.0;

/// What the flight computer asks of the actuators.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Command {
    /// Thrust, body axes, N (zero = no thrust wanted).
    pub thrust_body: Vec3,
    /// Control surfaces. Ship: flap exposures, order of
    /// `model::aero::FLAPS`. Booster: grid-fin deflections, rad, order of
    /// `model::aero::FIN_RADIAL` (the fourth is unused).
    pub flaps: [f64; FLAPS],
    /// Torque request per body axis, each in [−1, 1] (bang-bang RCS).
    pub rcs: Vec3,
}

impl Default for Command {
    fn default() -> Command {
        Command {
            thrust_body: [0.0; 3],
            flaps: [0.5; FLAPS],
            rcs: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashReason {
    HitGround,
    HitTower,
    MissedArms,
    TooFast,
    Tipped,
    OutOfPropellant,
}

impl CrashReason {
    pub fn as_str(self) -> &'static str {
        match self {
            CrashReason::HitGround => "hit the ground",
            CrashReason::HitTower => "hit the tower",
            CrashReason::MissedArms => "missed the arms",
            CrashReason::TooFast => "too fast",
            CrashReason::Tipped => "tipped over",
            CrashReason::OutOfPropellant => "out of propellant",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Caught,
    Landed,
    Crashed(CrashReason),
}

/// Wind blowing along `dir` (unit, horizontal) at `speed` m/s, trapezoidal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindGust {
    pub start: f64,
    pub dir: Vec3,
    pub speed: f64,
}

impl WindGust {
    pub fn wind(&self, t: f64) -> Vec3 {
        let tau = t - self.start;
        if !(0.0..=GUST_DURATION).contains(&tau) {
            return [0.0; 3];
        }
        let ramp = (tau / GUST_RAMP)
            .min((GUST_DURATION - tau) / GUST_RAMP)
            .clamp(0.0, 1.0);
        quat::scale(self.dir, self.speed * ramp)
    }
}

/// Which way each of the six thrusters pushes, for the picture:
/// pitch +/−, yaw +/− (forward group), roll +/− (aft group).
pub const RCS_COUNT: usize = 6;

#[derive(Debug, Clone)]
pub struct ShipSim {
    pub scn: Scenario,
    pub x: State,
    pub t: f64,
    pub cluster: Cluster,
    /// Actual flap exposures.
    pub flaps: [f64; FLAPS],
    pub rcs_firing: [bool; RCS_COUNT],
    pub gusts: Vec<WindGust>,
    /// Steady wind, inertial, m/s (the same at every height).
    pub wind: Vec3,
    pub legs_down: bool,
    pub arms_closed: bool,
    pub outcome: Option<Outcome>,
    /// Velocity when the flight ended.
    pub end_velocity: Option<Vec3>,
    /// The tanks ran dry at some point.
    pub starved: bool,
}

impl ShipSim {
    pub fn new(scn: Scenario, x: State) -> ShipSim {
        let cluster = Cluster::new(&scn.ship);
        let trim = scn.ship.surface_trim();
        ShipSim {
            scn,
            x,
            t: 0.0,
            cluster,
            flaps: [trim; FLAPS],
            rcs_firing: [false; RCS_COUNT],
            gusts: Vec::new(),
            wind: [0.0; 3],
            legs_down: false,
            arms_closed: false,
            outcome: None,
            end_velocity: None,
            starved: false,
        }
    }

    pub fn wind_at(&self, t: f64) -> Vec3 {
        self.gusts
            .iter()
            .fold(self.wind, |acc, g| quat::add(acc, g.wind(t)))
    }

    pub fn propellant(&self) -> f64 {
        (self.x[M] - self.scn.dry_mass).max(0.0)
    }

    /// Advance `dt` seconds with `cmd` held.
    pub fn step(&mut self, cmd: &Command, dt: f64) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }
        let mut left = dt;
        while left > 1e-12 && self.outcome.is_none() {
            let h = left.min(SUBSTEP);
            self.substep(cmd, h);
            left -= h;
        }
        if self.outcome.is_some() {
            // Hold still once caught, landed or crashed.
            self.t += left.max(0.0);
        }
    }

    /// Body torque from the RCS for a request, and which thrusters fire.
    fn rcs(&self, req: Vec3) -> (Vec3, [bool; RCS_COUNT]) {
        let s = &self.scn.ship;
        let arm = [
            s.rcs_thrust * (s.rcs_fore - s.rcs_aft),
            s.rcs_thrust * (s.rcs_fore - s.rcs_aft),
            2.0 * s.rcs_thrust * s.radius,
        ];
        let mut torque = [0.0; 3];
        let mut firing = [false; RCS_COUNT];
        for i in 0..3 {
            if req[i] > RCS_DEADBAND {
                torque[i] = arm[i];
                firing[2 * i] = true;
            } else if req[i] < -RCS_DEADBAND {
                torque[i] = -arm[i];
                firing[2 * i + 1] = true;
            }
        }
        (torque, firing)
    }

    fn substep(&mut self, cmd: &Command, h: f64) {
        // Actuators (zero-order hold over the substep).
        if self.x[M] <= self.scn.dry_mass {
            if self.cluster.lit() > 0 {
                self.starved = true;
            }
            self.cluster.cut();
        }
        self.cluster.step(cmd.thrust_body, h);
        let kind = self.scn.ship.kind;
        let (lo, hi, rate) = match (kind, self.scn.ship.fins) {
            (VehicleKind::Booster, Some(f)) => (-f.max_deflection, f.max_deflection, FIN_RATE),
            _ => (FLAP_MIN, 1.0, FLAP_RATE),
        };
        for i in 0..FLAPS {
            let want = if cmd.flaps[i].is_finite() { cmd.flaps[i].clamp(lo, hi) } else { self.flaps[i] };
            let d = (want - self.flaps[i]).clamp(-rate * h, rate * h);
            self.flaps[i] += d;
        }
        let (rcs_torque, firing) = self.rcs(cmd.rcs);
        self.rcs_firing = firing;

        let thrust = self.cluster.thrust;
        let flaps = self.flaps;
        let t0 = self.t;
        let deriv = |t: f64, x: &State| -> State {
            let wind = self.wind_at(t);
            let q = dynamics::attitude(x);
            let w_air = quat::sub(dynamics::velocity(x), wind);
            let rho = self.scn.density(x[dynamics::R + 2]);
            let (ff, ft) = match kind {
                VehicleKind::Ship => aero::flap_force(&self.scn.ship, rho, q, w_air, flaps),
                VehicleKind::Booster => aero::fin_force(&self.scn.ship, rho, q, w_air, flaps),
            };
            let extra = Extra {
                force: ff,
                torque: quat::add(ft, rcs_torque),
                wind,
            };
            dynamics::f(&self.scn, x, thrust, &extra)
        };
        let x0 = self.x;
        let k1 = deriv(t0, &x0);
        let k2 = deriv(t0 + h / 2.0, &axpy(&x0, &k1, h / 2.0));
        let k3 = deriv(t0 + h / 2.0, &axpy(&x0, &k2, h / 2.0));
        let k4 = deriv(t0 + h, &axpy(&x0, &k3, h));
        let mut x = x0;
        for i in 0..dynamics::STATE {
            x[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        let q = quat::normalize(dynamics::attitude(&x));
        x[Q..Q + 4].copy_from_slice(&q);
        x[M] = x[M].max(self.scn.dry_mass.min(x0[M]));
        self.x = x;
        self.t += h;
        self.contact();
    }

    /// Point at height `s` above the base along the axis, inertial, m.
    pub fn axis_point(&self, s: f64) -> Vec3 {
        let q = dynamics::attitude(&self.x);
        let r = dynamics::position(&self.x);
        quat::add(r, quat::rotate(q, [0.0, 0.0, s - self.scn.ship.cm_height]))
    }

    fn end(&mut self, outcome: Outcome) {
        let reason = match outcome {
            Outcome::Crashed(_) if self.starved => Outcome::Crashed(CrashReason::OutOfPropellant),
            o => o,
        };
        self.outcome = Some(reason);
        self.end_velocity = Some(dynamics::velocity(&self.x));
        self.cluster.cut();
        self.rcs_firing = [false; RCS_COUNT];
        if !matches!(reason, Outcome::Crashed(_)) {
            for i in 0..3 {
                self.x[V + i] = 0.0;
                self.x[W + i] = 0.0;
            }
        }
    }

    fn contact(&mut self) {
        let ship = self.scn.ship.clone();
        let q = dynamics::attitude(&self.x);
        let v = dynamics::velocity(&self.x);
        let w = dynamics::rates(&self.x);
        let tilt = quat::tilt(q);
        let base = self.axis_point(0.0);
        let nose = self.axis_point(ship.length);
        let hull_low = base[2].min(nose[2]) - ship.radius * tilt.sin().abs();

        if let Some(c) = self.scn.catch {
            // The mast: any of several points along the axis within a radius
            // of the box.
            for i in 0..=8 {
                let p = self.axis_point(ship.length * f64::from(i) / 8.0);
                if p[2] > c.tower_top || p[2] < 0.0 {
                    continue;
                }
                let dx = ((p[0] - c.mast_x).abs() - c.mast_half).max(0.0);
                let dy = (p[1].abs() - c.mast_half).max(0.0);
                if (dx * dx + dy * dy).sqrt() < ship.radius {
                    return self.end(Outcome::Crashed(CrashReason::HitTower));
                }
            }
            let pins = self.axis_point(ship.pin_height);
            let off = (pins[0] * pins[0] + pins[1] * pins[1]).sqrt();
            let above = pins[2] - c.arm_z;
            if !self.arms_closed && (0.0..=ARMS_CLOSE_ABOVE).contains(&above) && off <= ARMS_CLOSE_RADIUS {
                self.arms_closed = true;
            }
            if above <= 0.0 && above > -3.0 && off <= ARMS_CLOSE_RADIUS + ship.radius {
                let side = (v[0] * v[0] + v[1] * v[1]).sqrt();
                let outcome = if !self.arms_closed || off > c.capture_radius {
                    Outcome::Crashed(CrashReason::MissedArms)
                } else if -v[2] > CATCH_DOWN || side > CATCH_SIDE {
                    Outcome::Crashed(CrashReason::TooFast)
                } else if tilt > CATCH_TILT_DEG.to_radians() || quat::norm(w) > CATCH_RATE_DEG.to_radians() {
                    Outcome::Crashed(CrashReason::Tipped)
                } else {
                    Outcome::Caught
                };
                return self.end(outcome);
            }
        }

        if self.legs_down && self.scn.planet == Planet::Mars {
            let foot_low = (0..6)
                .map(|k| {
                    let a = f64::from(k) * std::f64::consts::TAU / 6.0;
                    let foot = [
                        ship.leg_radius * a.cos(),
                        ship.leg_radius * a.sin(),
                        -ship.cm_height - ship.leg_drop,
                    ];
                    quat::add(dynamics::position(&self.x), quat::rotate(q, foot))[2]
                })
                .fold(f64::INFINITY, f64::min);
            if foot_low <= 0.0 {
                let side = (v[0] * v[0] + v[1] * v[1]).sqrt();
                let outcome = if -v[2] > LAND_DOWN || side > LAND_SIDE {
                    Outcome::Crashed(CrashReason::TooFast)
                } else if tilt > LAND_TILT_DEG.to_radians() {
                    Outcome::Crashed(CrashReason::Tipped)
                } else {
                    Outcome::Landed
                };
                return self.end(outcome);
            }
        }
        if hull_low <= 0.0 {
            self.end(Outcome::Crashed(CrashReason::HitGround));
        }
    }
}

fn axpy(x: &State, d: &State, s: f64) -> State {
    let mut y = *x;
    for i in 0..dynamics::STATE {
        y[i] += s * d[i];
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::dynamics::state;
    use std::f64::consts::FRAC_PI_2;

    fn belly_down() -> model::Quat {
        quat::from_axis_angle([0.0, 1.0, 0.0], FRAC_PI_2)
    }

    fn upright() -> model::Quat {
        [1.0, 0.0, 0.0, 0.0]
    }

    fn earth_at(r: Vec3, v: Vec3, q: model::Quat) -> ShipSim {
        let s = Scenario::earth();
        let m = s.start_mass;
        ShipSim::new(s, state(m, r, v, q, [0.0; 3]))
    }

    /// Flaps trimmed by a crude pitch hold, so the belly stays down.
    fn hold_belly(sim: &ShipSim) -> Command {
        let q = dynamics::attitude(&sim.x);
        let nose = quat::rotate(q, [0.0, 0.0, 1.0]);
        let pitch = nose[2]; // + = nose up
        let rate = dynamics::rates(&sim.x)[1]; // + = nose down
        let u = (2.0 * pitch - 1.5 * rate).clamp(-0.45, 0.45);
        Command {
            flaps: [0.5 - u, 0.5 - u, 0.5 + u, 0.5 + u],
            ..Command::default()
        }
    }

    #[test]
    fn free_fall_belly_down_reaches_terminal_speed() {
        let mut sim = earth_at([0.0, 0.0, 5_000.0], [0.0, 0.0, -80.0], belly_down());
        for _ in 0..800 {
            let c = hold_belly(&sim);
            sim.step(&c, 0.05);
        }
        let v = dynamics::velocity(&sim.x);
        let z = sim.x[dynamics::R + 2];
        assert!(sim.outcome.is_none(), "{:?} at z {z}", sim.outcome);
        assert!((60.0..100.0).contains(&-v[2]), "speed {} at z {z}", -v[2]);
        assert!(quat::tilt(dynamics::attitude(&sim.x)) > 80f64.to_radians());
    }

    #[test]
    fn long_spin_keeps_unit_quaternion() {
        let s = Scenario::mars();
        let x = state(s.start_mass, [0.0, 0.0, 1e6], [0.0; 3], upright(), [0.3, 0.2, 1.0]);
        let mut sim = ShipSim::new(s, x);
        sim.step(&Command::default(), 300.0);
        let q = dynamics::attitude(&sim.x);
        let n = q.iter().map(|c| c * c).sum::<f64>().sqrt();
        assert!((n - 1.0).abs() < 1e-12);
    }

    #[test]
    fn flaps_fore_exposed_pitches_nose_up() {
        let mut sim = earth_at([0.0, 0.0, 3000.0], [0.0, 0.0, -80.0], belly_down());
        let cmd = Command {
            flaps: [1.0, 1.0, FLAP_MIN, FLAP_MIN],
            ..Command::default()
        };
        sim.flaps = cmd.flaps;
        sim.step(&cmd, 1.0);
        let nose = quat::rotate(dynamics::attitude(&sim.x), [0.0, 0.0, 1.0]);
        assert!(nose[2] > 0.0, "nose {nose:?}");
    }

    #[test]
    fn rcs_roll_torque() {
        let mut sim = earth_at([0.0, 0.0, 100_000.0], [0.0; 3], upright());
        let cmd = Command {
            rcs: [0.0, 0.0, 1.0],
            ..Command::default()
        };
        sim.step(&cmd, 1.0);
        assert!(dynamics::rates(&sim.x)[2] > 0.0);
        assert!(sim.rcs_firing[4] && !sim.rcs_firing[5]);
    }

    #[test]
    fn empty_tanks_shut_engines() {
        let s = Scenario::earth();
        let x = state(s.dry_mass + 50.0, [0.0, 0.0, 400.0], [0.0; 3], upright(), [0.0; 3]);
        let mut sim = ShipSim::new(s, x);
        sim.cluster.ignite_all();
        let cmd = Command {
            thrust_body: [0.0, 0.0, 3e6],
            ..Command::default()
        };
        for _ in 0..2000 {
            sim.step(&cmd, 0.05);
        }
        assert_eq!(sim.cluster.running(), 0);
        assert!(sim.x[M] >= sim.scn.dry_mass - 1e-6);
        assert_eq!(sim.outcome, Some(Outcome::Crashed(CrashReason::OutOfPropellant)));
    }

    #[test]
    fn tower_contact_is_a_crash() {
        // Upright, drifting toward the mast at the height of its middle.
        let mut sim = earth_at([-3.0, 0.0, 60.0], [-5.0, 0.0, 0.0], upright());
        sim.cluster.ignite_all();
        let hover = sim.x[M] * 9.80665;
        let cmd = Command {
            thrust_body: [0.0, 0.0, hover],
            ..Command::default()
        };
        sim.step(&cmd, 5.0);
        assert_eq!(sim.outcome, Some(Outcome::Crashed(CrashReason::HitTower)));
    }

    fn caught_cm(s: &Scenario) -> f64 {
        s.floor_z
    }

    fn descend_onto_arms(offset: f64, speed: f64) -> ShipSim {
        let s = Scenario::earth();
        let z = caught_cm(&s) + 6.0;
        let mut sim = earth_at([offset, 0.0, z], [0.0, 0.0, -speed], upright());
        // Thrust exactly cancelling gravity keeps the descent rate.
        // One engine running, so it can throttle down to hover.
        sim.cluster.ignite_all();
        sim.cluster.step([0.0, 0.0, 1.0], 1.5);
        sim.cluster.engines[1] = crate::EngineState::Shutdown;
        sim.cluster.engines[2] = crate::EngineState::Shutdown;
        for _ in 0..400 {
            let cmd = Command {
                thrust_body: [0.0, 0.0, sim.x[M] * 9.80665],
                ..Command::default()
            };
            sim.step(&cmd, 0.05);
        }
        sim
    }

    #[test]
    fn slow_upright_on_axis_is_caught() {
        let sim = descend_onto_arms(0.3, 1.0);
        assert!(sim.arms_closed);
        assert_eq!(sim.outcome, Some(Outcome::Caught));
    }

    #[test]
    fn fast_on_axis_is_too_fast() {
        let sim = descend_onto_arms(0.0, 4.0);
        assert_eq!(sim.outcome, Some(Outcome::Crashed(CrashReason::TooFast)));
    }

    #[test]
    fn off_axis_misses_the_arms() {
        let sim = descend_onto_arms(2.5, 1.0);
        assert_eq!(sim.outcome, Some(Outcome::Crashed(CrashReason::MissedArms)));
    }

    #[test]
    fn booster_fins_slew_clamp_and_turn_it() {
        let s = Scenario::booster();
        let x = state(s.start_mass, [0.0, 0.0, 8000.0], [0.0, 0.0, -300.0], upright(), [0.0; 3]);
        let mut sim = ShipSim::new(s, x);
        assert_eq!(sim.flaps, [0.0; 4]);
        // δ1 − δ0 > 0 is a + torque about y: the top tips toward +x.
        let cmd = Command {
            flaps: [-9.0, 9.0, 0.0, 0.0],
            ..Command::default()
        };
        sim.step(&cmd, 0.1);
        assert!((sim.flaps[1] - 0.1).abs() < 1e-9, "slews at 1 rad/s: {:?}", sim.flaps);
        sim.step(&cmd, 1.0);
        let lim = sim.scn.ship.fins.unwrap().max_deflection;
        assert!((sim.flaps[1] - lim).abs() < 1e-9 && (sim.flaps[0] + lim).abs() < 1e-9);
        assert!(dynamics::rates(&sim.x)[1] > 0.0);
    }

    #[test]
    fn booster_slow_on_axis_is_caught() {
        let s = Scenario::booster();
        let x = state(s.dry_mass + 30e3, [0.2, 0.0, s.floor_z + 6.0], [0.0, 0.0, -1.0], upright(), [0.0; 3]);
        let mut sim = ShipSim::new(s, x);
        sim.cluster.ignite_all();
        sim.cluster.step([0.0, 0.0, 1.0], 1.5);
        sim.cluster.shutdown_to(3);
        for _ in 0..400 {
            let cmd = Command {
                thrust_body: [0.0, 0.0, sim.x[M] * 9.80665],
                flaps: [0.0; 4],
                ..Command::default()
            };
            sim.step(&cmd, 0.05);
        }
        assert_eq!(sim.outcome, Some(Outcome::Caught));
    }

    #[test]
    fn steady_wind_adds_to_gusts() {
        let mut sim = earth_at([0.0, 0.0, 5000.0], [0.0; 3], upright());
        sim.wind = [3.0, -4.0, 0.0];
        sim.gusts.push(WindGust { start: 0.0, dir: [1.0, 0.0, 0.0], speed: 10.0 });
        assert_eq!(sim.wind_at(2.0), [13.0, -4.0, 0.0]);
        assert_eq!(sim.wind_at(100.0), [3.0, -4.0, 0.0]);
    }

    fn mars_touchdown(q: model::Quat, speed: f64) -> ShipSim {
        let s = Scenario::mars();
        let z = s.floor_z + 0.05;
        let x = state(s.dry_mass + 1000.0, [0.0, 0.0, z], [0.0, 0.0, -speed], q, [0.0; 3]);
        let mut sim = ShipSim::new(s, x);
        sim.legs_down = true;
        sim.step(&Command::default(), 5.0);
        sim
    }

    #[test]
    fn mars_upright_soft_is_landed() {
        let sim = mars_touchdown(upright(), 1.0);
        assert_eq!(sim.outcome, Some(Outcome::Landed));
    }

    #[test]
    fn mars_tilted_tips() {
        let q = quat::from_axis_angle([1.0, 0.0, 0.0], 10f64.to_radians());
        let sim = mars_touchdown(q, 1.0);
        assert_eq!(sim.outcome, Some(Outcome::Crashed(CrashReason::Tipped)));
    }
}
