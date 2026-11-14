//! One closed-loop flight: the simulator (truth), the sensors, the tracking
//! controller and the latest plan. Re-planning itself happens elsewhere (in
//! the page's worker, or synchronously in tests): this struct hands out
//! measurements and accepts plans stamped with the measurement time.
//!
//! Every controller step reads the *sensors*, never the truth.

use guidance::track::{self, Envelope, Estimate, Gains};
use guidance::{GuidanceError, Hint, LandingProblem, Mode, Replan, Trajectory, Vec3, Vehicle};
use sim::{Engine, Gust, Measurement, Noise, Params, Sensor, Sim};

/// Intervals per re-plan (the preview before Land uses 50).
pub const REPLAN_STEPS: usize = 40;
/// Controller and sensor rate: one reading per simulator substep.
const CONTROL_STEP: f64 = sim::sim::SUBSTEP;
/// Gust strength as a fraction of the healthy maximum thrust.
const GUST_FRACTION: f64 = 0.15;
/// Guidance plans to rest this far above the pad; the controller's
/// terminal descent law flies the rest. A fuel-optimal plan to the ground
/// ends with a hard, late burn that a lagging engine cannot deliver, m.
pub const TERMINAL_GATE: f64 = 15.0;
/// Guidance plans with this fraction of the available maximum thrust.
pub const THRUST_MARGIN: f64 = 0.85;
/// Guidance never plans to burn this fraction of the tank capacity.
pub const RESERVE_FRACTION: f64 = 0.02;
/// Propellant budget for the terminal descent, in seconds of hovering.
/// From the gate the descent law takes ~11 s (sink at 0.3·z, then 0.5 m/s).
pub const TERMINAL_SECONDS: f64 = 12.0;
/// Stop re-planning when the plan has this little left, s: tiny horizons
/// are near vertical, where the relaxation is not guaranteed tight.
pub const FREEZE: f64 = 2.0;
/// No re-plans below this height above the terminal gate, m.
pub const GATE_MARGIN: f64 = 10.0;
/// Touchdown limits for "landed" rather than "crashed", m/s.
pub const MAX_VERTICAL: f64 = 2.0;
pub const MAX_HORIZONTAL: f64 = 1.0;

/// Re-plan with a thrust margin: first with [`THRUST_MARGIN`] of the
/// maximum, and only if no landing exists that way, with all of it.
///
/// Also retried at full thrust when the margined plan misses the pad: a
/// pinpoint landing is worth more than the margin.
pub fn plan(problem: &LandingProblem, hint: Option<Hint>) -> Result<Replan, GuidanceError> {
    let margined = LandingProblem {
        vehicle: Vehicle {
            max_thrust: problem.vehicle.max_thrust * THRUST_MARGIN,
            ..problem.vehicle.clone()
        },
        ..problem.clone()
    };
    let first = guidance::replan(&margined, hint);
    let spent = match &first {
        Ok(r) if r.miss <= 1.0 => return first,
        Ok(r) => r.solves,
        Err(_) => 0,
    };
    match guidance::replan(problem, hint) {
        Ok(mut full) if first.is_err() || full.miss <= 1.0 => {
            full.solves += spent;
            Ok(full)
        }
        // Full thrust only gets closer, not onto the pad: keep the margin.
        _ => first,
    }
}

#[derive(Debug, Clone)]
pub struct FlightConfig {
    pub position: Vec3,
    pub velocity: Vec3,
    /// The healthy vehicle; `wet_mass` is the start mass.
    pub vehicle: Vehicle,
    pub seed: u64,
    /// Engine lag time constant, s.
    pub lag: f64,
    pub noise: Noise,
}

/// What guidance currently tells the controller to do.
#[derive(Debug, Clone)]
pub enum Guide {
    /// Fly this plan; `t0` is the sim time of the measurement it was solved from.
    Plan {
        trajectory: Trajectory,
        mode: Mode,
        miss: f64,
        t0: f64,
    },
    /// No landing exists: full thrust against the velocity.
    Brake,
}

/// How the flight ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    /// True if touchdown was within [`MAX_VERTICAL`] and [`MAX_HORIZONTAL`].
    pub landed: bool,
    /// Touchdown speeds, m/s.
    pub speed: f64,
    pub vertical: f64,
    pub horizontal: f64,
    /// Horizontal distance from the pad, m.
    pub miss: f64,
    /// Propellant burned, kg.
    pub fuel_used: f64,
}

pub struct Flight {
    vehicle: Vehicle,
    sim: Sim,
    sensor: Sensor,
    guide: Option<Guide>,
    grabbed: bool,
    /// Last thrust command, N.
    command: Vec3,
    /// The latest sensor reading the controller used.
    nav: Measurement,
    start_mass: f64,
    outcome: Option<Outcome>,
}

impl Flight {
    pub fn new(cfg: FlightConfig) -> Result<Flight, String> {
        let probe = LandingProblem {
            vehicle: cfg.vehicle.clone(),
            position: cfg.position,
            velocity: cfg.velocity,
            steps: REPLAN_STEPS,
        };
        probe.validate_state().map_err(|e| format!("{e:?}"))?;
        if !(cfg.lag.is_finite() && cfg.lag >= 0.0) {
            return Err("lag must be a non-negative number".into());
        }
        let v = &cfg.vehicle;
        let mut engine = Engine::new(v.min_thrust, v.max_thrust);
        engine.lag = cfg.lag;
        let params = Params {
            gravity: v.gravity,
            dry_mass: v.dry_mass,
            alpha: v.alpha(),
        };
        let sim = Sim::new(params, engine, cfg.position, cfg.velocity, v.wet_mass);
        let mut sensor = Sensor::new(cfg.noise, cfg.seed);
        let nav = sensor.measure(&sim.truth);
        Ok(Flight {
            sim,
            sensor,
            nav,
            start_mass: v.wet_mass,
            vehicle: cfg.vehicle,
            guide: None,
            grabbed: false,
            command: [0.0; 3],
            outcome: None,
        })
    }

    /// Sim time, s.
    pub fn time(&self) -> f64 {
        self.sim.truth.t
    }

    /// The vehicle as guidance should see it now: thrust limits reduced
    /// for failed engines (fault detection is assumed perfect).
    pub fn current_vehicle(&self) -> Vehicle {
        let (min_thrust, max_thrust) = self.sim.engine.limits();
        Vehicle {
            min_thrust,
            max_thrust,
            ..self.vehicle.clone()
        }
    }

    /// A sensor reading for guidance.
    pub fn measure(&mut self) -> Measurement {
        self.sensor.measure(&self.sim.truth)
    }

    /// The landing problem guidance should solve for a measurement (solve it
    /// with [`plan`]).
    ///
    /// Plans keep margins the controller can draw on: a fuel-optimal plan
    /// otherwise rides exactly on the thrust limit and empties the tanks,
    /// and the smallest disturbance then makes the next re-plan infeasible.
    /// The propellant reserve is built in here; the thrust margin in [`plan`].
    pub fn problem(&self, m: &Measurement) -> LandingProblem {
        let v = self.current_vehicle();
        let capacity = self.vehicle.wet_mass - v.dry_mass;
        // Spare propellant, plus what the terminal descent below the gate
        // burns (about the vehicle's weight for TERMINAL_SECONDS).
        let terminal = v.alpha() * m.mass * v.gravity * TERMINAL_SECONDS;
        let reserve = (RESERVE_FRACTION * capacity + terminal).min((m.mass - v.dry_mass).max(0.0));
        let v = Vehicle {
            dry_mass: v.dry_mass + reserve,
            ..v
        };
        // Guidance's "pad" is the terminal gate above the real one.
        let [x, y, z] = m.position;
        LandingProblem::from_state(
            v,
            [x, y, z - TERMINAL_GATE],
            m.velocity,
            m.mass,
            REPLAN_STEPS,
        )
    }

    /// The previous plan's landing time as seen from sim time `t`.
    pub fn hint(&self, t: f64) -> Option<Hint> {
        match &self.guide {
            Some(Guide::Plan {
                trajectory,
                mode,
                t0,
                ..
            }) => Some(Hint {
                final_time: trajectory.final_time - (t - t0),
                mode: *mode,
            }),
            _ => None,
        }
    }

    /// Whether a re-plan is worth asking for now: not after touchdown, not
    /// with under [`FREEZE`] seconds of plan left, and not in the terminal
    /// descent (judged from the latest navigation reading).
    pub fn wants_replan(&self) -> bool {
        self.outcome.is_none()
            && self.nav.position[2] >= TERMINAL_GATE + GATE_MARGIN
            && self.plan_remaining().is_none_or(|left| left > FREEZE)
    }

    /// Seconds left on the current plan, if there is one.
    pub fn plan_remaining(&self) -> Option<f64> {
        self.hint(self.time()).map(|h| h.final_time)
    }

    /// Install a plan solved for [`Flight::problem`] (which is relative to
    /// the terminal gate) from a measurement taken at `t_measured`.
    pub fn set_plan(&mut self, r: Replan, t_measured: f64) {
        let mut trajectory = r.trajectory;
        for p in &mut trajectory.position {
            p[2] += TERMINAL_GATE;
        }
        self.guide = Some(Guide::Plan {
            trajectory,
            mode: r.mode,
            miss: r.miss,
            t0: t_measured,
        });
    }

    /// A re-plan failed. `Infeasible` means no landing exists: brake. Other
    /// failures (a loose relaxation, a noisy reading below ground…) keep the
    /// current plan.
    pub fn plan_failed(&mut self, e: &GuidanceError) {
        if matches!(e, GuidanceError::Infeasible) || self.guide.is_none() {
            self.guide = Some(Guide::Brake);
        }
    }

    pub fn guide(&self) -> Option<&Guide> {
        self.guide.as_ref()
    }

    /// Advance `dt` seconds of sim time: sense, control, simulate, in
    /// 10 ms steps. Frozen while grabbed and after touchdown.
    pub fn step(&mut self, dt: f64) {
        if self.grabbed || self.outcome.is_some() || !(dt.is_finite() && dt > 0.0) {
            return;
        }
        let mut remaining = dt;
        while remaining > 1e-12 && self.outcome.is_none() {
            let h = remaining.min(CONTROL_STEP);
            let m = self.sensor.measure(&self.sim.truth);
            self.command = self.control(&m);
            self.nav = m;
            self.sim.step(self.command, h);
            remaining -= h;
            if self.sim.truth.landed {
                self.outcome = Some(self.judge());
            }
        }
    }

    fn control(&self, m: &Measurement) -> Vec3 {
        let (min_thrust, max_thrust) = self.sim.engine.limits();
        let env = Envelope {
            gravity: self.vehicle.gravity,
            max_tilt: self.vehicle.max_tilt,
            min_thrust,
            max_thrust,
        };
        let est = Estimate {
            position: m.position,
            velocity: m.velocity,
            mass: m.mass,
        };
        match &self.guide {
            Some(Guide::Brake) if m.velocity[2] < 0.0 => track::brake(m.velocity, &env),
            // Braking done (or rising): sink gently like after a plan.
            Some(Guide::Brake) => track::command(None, 0.0, &est, &env, Gains::default()),
            Some(Guide::Plan { trajectory, t0, .. }) => {
                track::command(Some(trajectory), m.t - t0, &est, &env, Gains::default())
            }
            None => track::command(None, 0.0, &est, &env, Gains::default()),
        }
    }

    fn judge(&self) -> Outcome {
        let t = &self.sim.truth;
        let v = t.touchdown_velocity.unwrap_or([0.0; 3]);
        let horizontal = v[0].hypot(v[1]);
        let vertical = v[2].abs();
        Outcome {
            landed: vertical <= MAX_VERTICAL && horizontal <= MAX_HORIZONTAL,
            speed: horizontal.hypot(vertical),
            vertical,
            horizontal,
            miss: t.position[0].hypot(t.position[1]),
            fuel_used: self.start_mass - t.mass,
        }
    }

    /// Push sideways: `+1` toward +x, `-1` toward −x.
    pub fn gust(&mut self, dir: f64) {
        let sign = if dir < 0.0 { -1.0 } else { 1.0 };
        self.sim.gusts.push(Gust {
            start: self.time(),
            dir: [sign, 0.0, 0.0],
            magnitude: GUST_FRACTION * self.vehicle.max_thrust,
        });
    }

    pub fn engine_out(&mut self) -> bool {
        self.sim.engine.fail_one()
    }

    pub fn set_lag(&mut self, lag: f64) {
        self.sim.engine.lag = lag.max(0.0);
    }

    pub fn set_noise(&mut self, noise: Noise) {
        self.sensor.noise = noise;
    }

    /// Hold the vehicle at `position` (time stops while held).
    pub fn grab(&mut self, position: Vec3) {
        if self.outcome.is_some() || !position.iter().all(|c| c.is_finite()) {
            return;
        }
        self.grabbed = true;
        // Keep it at least a metre up, or it would touch down on release.
        self.sim.truth.position = [position[0], position[1], position[2].max(1.0)];
    }

    pub fn release(&mut self) {
        self.grabbed = false;
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// The latest sensor reading (what guidance believes).
    pub fn nav(&self) -> &Measurement {
        &self.nav
    }

    pub fn command(&self) -> Vec3 {
        self.command
    }

    pub fn noise(&self) -> Noise {
        self.sensor.noise
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> FlightConfig {
        let p = LandingProblem::example();
        FlightConfig {
            position: p.position,
            velocity: p.velocity,
            vehicle: p.vehicle,
            seed: 3,
            lag: 0.0,
            noise: Noise::OFF,
        }
    }

    #[test]
    fn rejects_bad_config() {
        let mut c = config();
        c.position[2] = -1.0;
        assert!(Flight::new(c).is_err());
        let mut c = config();
        c.vehicle.dry_mass = 5000.0;
        assert!(Flight::new(c).is_err());
    }

    #[test]
    fn engine_out_shrinks_guidance_limits() {
        let mut f = Flight::new(config()).unwrap();
        let full = f.current_vehicle().max_thrust;
        assert!(f.engine_out());
        assert!((f.current_vehicle().max_thrust - full * 5.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn problem_uses_measured_mass() {
        let mut f = Flight::new(config()).unwrap();
        f.step(1.0);
        let m = f.measure();
        let p = f.problem(&m);
        assert_eq!(p.vehicle.wet_mass, m.mass);
        assert_eq!(p.steps, REPLAN_STEPS);
    }

    #[test]
    fn grab_freezes_time_and_moves() {
        let mut f = Flight::new(config()).unwrap();
        f.grab([500.0, 0.0, 800.0]);
        f.step(1.0);
        assert_eq!(f.time(), 0.0);
        assert_eq!(f.sim().truth.position, [500.0, 0.0, 800.0]);
        f.release();
        f.step(0.5);
        assert!((f.time() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn grab_cannot_go_underground() {
        let mut f = Flight::new(config()).unwrap();
        f.grab([0.0, 0.0, -50.0]);
        assert!(f.sim().truth.position[2] >= 1.0);
    }

    #[test]
    fn hint_counts_down() {
        let mut f = Flight::new(config()).unwrap();
        let m = f.measure();
        let r = guidance::replan(&f.problem(&m), None).unwrap();
        let tf = r.trajectory.final_time;
        f.set_plan(r, 0.0);
        f.step(2.0);
        let h = f.hint(2.0).unwrap();
        assert!((h.final_time - (tf - 2.0)).abs() < 1e-9);
        assert_eq!(h.mode, Mode::Pinpoint);
        assert!((f.plan_remaining().unwrap() - (tf - 2.0)).abs() < 1e-9);
    }

    #[test]
    fn no_replans_near_the_end_or_below_the_gate() {
        let mut f = Flight::new(config()).unwrap();
        assert!(f.wants_replan(), "no plan yet");
        let m = f.measure();
        f.set_plan(guidance::replan(&f.problem(&m), None).unwrap(), 0.0);
        assert!(f.wants_replan());
        let tf = f.plan_remaining().unwrap();
        f.step(tf - 1.5);
        assert!(!f.wants_replan(), "under 2 s left");

        let mut low = config();
        low.position = [0.0, 0.0, TERMINAL_GATE + 5.0];
        low.velocity = [0.0; 3];
        let f = Flight::new(low).unwrap();
        assert!(!f.wants_replan(), "terminal descent");
    }

    #[test]
    fn plans_end_at_the_terminal_gate() {
        let mut f = Flight::new(config()).unwrap();
        let m = f.measure();
        let p = f.problem(&m);
        assert_eq!(p.position[2], m.position[2] - TERMINAL_GATE);
        f.set_plan(guidance::replan(&p, None).unwrap(), 0.0);
        let Some(Guide::Plan { trajectory, .. }) = f.guide() else {
            panic!()
        };
        let end = trajectory.position[trajectory.position.len() - 1];
        assert!((end[2] - 15.0).abs() < 1e-6, "{end:?}");
        assert!((trajectory.position[0][2] - m.position[2]).abs() < 1e-6);
    }

    #[test]
    fn problem_keeps_margins() {
        let mut f = Flight::new(config()).unwrap();
        let v = config().vehicle;
        let m = f.measure();
        let p = f.problem(&m);
        assert_eq!(
            p.vehicle.max_thrust, v.max_thrust,
            "margin is applied by plan()"
        );
        // 2 % of 400 kg, plus 12 s of hovering at the measured mass.
        let hover = v.alpha() * m.mass * v.gravity * 12.0;
        let want = v.dry_mass + 8.0 + hover;
        assert!(
            (p.vehicle.dry_mass - want).abs() < 1e-9,
            "{} vs {want}",
            p.vehicle.dry_mass
        );
        // Nearly empty: everything left is reserve, so guidance plans
        // nothing new (and the controller keeps flying the last plan).
        let low = Measurement {
            mass: v.dry_mass + 4.0,
            ..m
        };
        let p = f.problem(&low);
        assert_eq!(p.vehicle.dry_mass, low.mass);
        assert!(guidance::replan(&p, None).is_err());
    }

    #[test]
    fn plan_uses_full_thrust_only_when_the_margin_is_infeasible() {
        let v = config().vehicle;
        let five = Vehicle {
            max_thrust: v.max_thrust * 5.0 / 6.0,
            min_thrust: v.min_thrust * 5.0 / 6.0,
            ..v.clone()
        };
        let p =
            LandingProblem::from_state(five, [300.0, 0.0, 1000.0], [-20.0, 0.0, -60.0], 1850.0, 40);
        let margined = LandingProblem {
            vehicle: Vehicle {
                max_thrust: p.vehicle.max_thrust * THRUST_MARGIN,
                ..p.vehicle.clone()
            },
            ..p.clone()
        };
        assert!(guidance::replan(&margined, None).is_err(), "precondition");
        let r = plan(&p, None).expect("full thrust can still land");
        let peak = r
            .trajectory
            .thrust
            .iter()
            .map(|t| t[0].hypot(t[1]).hypot(t[2]))
            .fold(0.0, f64::max);
        assert!(peak > margined.vehicle.max_thrust, "used the margin");

        // When the margin is affordable, it is kept.
        let easy = LandingProblem::example();
        let r = plan(&easy, None).unwrap();
        let peak = r
            .trajectory
            .thrust
            .iter()
            .map(|t| t[0].hypot(t[1]).hypot(t[2]))
            .fold(0.0, f64::max);
        assert!(peak <= easy.vehicle.max_thrust * THRUST_MARGIN * (1.0 + 1e-6));
    }

    #[test]
    fn brake_never_climbs() {
        let mut c = config();
        c.velocity = [0.0, 0.0, 5.0];
        let mut f = Flight::new(c).unwrap();
        f.plan_failed(&GuidanceError::Infeasible);
        f.step(0.01);
        assert!(
            f.command()[2] < f.current_vehicle().max_thrust,
            "rising: no full thrust"
        );
    }

    #[test]
    fn infeasible_brakes_other_errors_keep_plan() {
        let mut f = Flight::new(config()).unwrap();
        let m = f.measure();
        f.set_plan(guidance::replan(&f.problem(&m), None).unwrap(), 0.0);
        f.plan_failed(&GuidanceError::SolverFailed("x".into()));
        assert!(matches!(f.guide(), Some(Guide::Plan { .. })));
        f.plan_failed(&GuidanceError::Infeasible);
        assert!(matches!(f.guide(), Some(Guide::Brake)));
    }

    #[test]
    fn gust_is_fifteen_percent_of_max_thrust() {
        let mut f = Flight::new(config()).unwrap();
        f.gust(-1.0);
        let g = f.sim().gusts[0];
        assert_eq!(g.dir, [-1.0, 0.0, 0.0]);
        assert!((g.magnitude - 0.15 * config().vehicle.max_thrust).abs() < 1e-9);
    }
}
