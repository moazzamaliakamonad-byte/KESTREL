//! One closed-loop flight of the ship or the booster: the 6-DoF simulator (truth), the sensors,
//! the flight phases and controllers, and the latest SCvx plan. Planning
//! itself happens elsewhere (the page's worker, or synchronously in tests):
//! this hands out measurements and accepts plans stamped with the
//! measurement time. Every control step reads the sensors, never the truth.
//!
//! Phases, as on the real vehicles' returns:
//! 1. **Belly flop** (ship) / **descent** (booster): engines off; flaps or
//!    grid fins fly the vehicle toward the ignition point.
//! 2. **Flip** (ship) / **start-up** (booster): every healthy landing
//!    engine lights at minimum throttle; a scripted attitude controller
//!    brings the vehicle upright (≤ 35°/s).
//! 3. **Landing burn**, in legs with a fixed number of engines each, planned
//!    by SCvx: on the final engines (ship 1, booster 3) straight to the
//!    approach gate if they can stop in time, else on every lit engine
//!    (ship 3, booster 13) to an intermediate gate higher up, then on the
//!    final engines to the approach gate.
//! 4. **Terminal**: slide over the catch axis / pad and settle onto the arms
//!    or the legs.
//!
//! Simulation only.

use guidance::GuidanceError;
use guidance::scvx::{self, ShipPlan, ShipProblem};
use guidance::ship_control::{self, Actuation};
use model::dynamics::{self, M, R, State, V};
use model::quat::{self, Vec3};
use model::{Planet, Scenario, Tuning, VehicleKind};
use sim::ship::{Command, SUBSTEP, ShipSim, WindGust};
use sim::sensor::{Measurement, Noise, Sensor};
use sim::{Rng, Truth};

/// Sim time between re-plan requests, s.
pub const REPLAN_PERIOD: f64 = 0.5;
/// No re-plans when the plan has less than this left, s: short-horizon
/// re-plans converge poorly and the ship tilted chasing them.
pub const FREEZE: f64 = 4.0;
/// Re-plan only when the ship has left its plan by this much (m, m/s) or
/// the plan is this old (s). Re-planning every period made each fresh,
/// partly converged plan pull a different way: the ship swung ±40 m.
pub const REPLAN_POSITION_ERROR: f64 = 5.0;
pub const REPLAN_VELOCITY_ERROR: f64 = 2.0;
pub const REPLAN_MAX_AGE: f64 = 3.0;
/// A plan the ship is this far from (m, m/s) is dropped when a re-plan fails.
pub const STALE_POSITION: f64 = 25.0;
pub const STALE_VELOCITY: f64 = 8.0;
/// A leg ends this close to its gate (m, m/s).
pub const GATE_POSITION: f64 = 8.0;
pub const GATE_VELOCITY: f64 = 3.0;
/// The flip is over below this tilt (rad, 8°) and rate (rad/s, 10°/s).
pub const FLIP_DONE_TILT: f64 = 0.139_626_340_159_546_36;
pub const FLIP_DONE_RATE: f64 = 0.174_532_925_199_432_95;
/// Guidance plans with this share of the maximum thrust (room to correct)…
pub const THRUST_MARGIN: f64 = 0.9;
/// …and on the last engine with this share (thrust-to-weight about 1.6 at
/// Earth landing mass, near the 1.6 Everyday Astronaut found optimal).
pub const LATE_MARGIN: f64 = 0.75;
/// In the final approach an engine is shut down when the lit engines'
/// minimum thrust exceeds this share of the weight (too light to hover).
pub const HOVER_SHARE: f64 = 0.97;
/// Default wind gust speed, m/s.
pub const GUST_SPEED: f64 = 15.0;
/// Attitude sensor error σ (rad) and rate error σ (rad/s), high noise.
pub const ATT_SIGMA_HIGH: (f64, f64) = (0.003_490_658_503_988_659, 0.000_872_664_625_997_164_9);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    BellyFlop,
    Flip,
    Burn,
    Terminal,
    Done,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::BellyFlop => "belly-flop",
            Phase::Flip => "flip",
            Phase::Burn => "landing-burn",
            Phase::Terminal => "terminal",
            Phase::Done => "done",
        }
    }

    /// The phase's name for a vehicle: the booster's unpowered fall and
    /// engine start-up are not a belly flop and a flip.
    pub fn label(self, kind: Kind) -> &'static str {
        match (kind, self) {
            (Kind::Booster, Phase::BellyFlop) => "descent",
            (Kind::Booster, Phase::Flip) => "startup",
            _ => self.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The ship back to the tower on Earth.
    Earth,
    /// The ship onto its legs on Mars.
    Mars,
    /// The booster back to the tower on Earth.
    Booster,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "earth" => Some(Kind::Earth),
            "mars" => Some(Kind::Mars),
            "booster" => Some(Kind::Booster),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Earth => "earth",
            Kind::Mars => "mars",
            Kind::Booster => "booster",
        }
    }
    pub fn scenario(self) -> Scenario {
        match self {
            Kind::Earth => Scenario::earth(),
            Kind::Mars => Scenario::mars(),
            Kind::Booster => Scenario::booster(),
        }
    }
    /// Default start: (position, velocity).
    pub fn default_start(self) -> (Vec3, Vec3) {
        match self {
            Kind::Earth => ([700.0, 0.0, 5000.0], [-25.0, 0.0, -75.0]),
            Kind::Mars => ([1500.0, 0.0, 14_000.0], [-40.0, 0.0, -450.0]),
            Kind::Booster => ([900.0, 0.0, 14_000.0], [-30.0, 0.0, -420.0]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShipFlightConfig {
    pub kind: Kind,
    pub position: Vec3,
    pub velocity: Vec3,
    pub seed: u64,
    pub lag: f64,
    pub noise: Noise,
    /// Physics options (None = the scenario's own).
    pub tuning: Option<Tuning>,
    /// Landing propellant at the start, kg (None = the scenario's own).
    pub propellant: Option<f64>,
    /// Steady wind, m/s (x, y).
    pub wind: [f64; 2],
    /// Speed of the gusts the page can throw, m/s.
    pub gust_speed: f64,
    /// Landing engines that never light (indices).
    pub failed: Vec<usize>,
}

impl ShipFlightConfig {
    /// The scenario's default start with nothing changed.
    pub fn new(kind: Kind) -> ShipFlightConfig {
        let (position, velocity) = kind.default_start();
        ShipFlightConfig {
            kind,
            position,
            velocity,
            seed: 1,
            lag: 0.0,
            noise: Noise::OFF,
            tuning: None,
            propellant: None,
            wind: [0.0; 2],
            gust_speed: GUST_SPEED,
            failed: Vec::new(),
        }
    }

    /// The scenario this configuration flies (options applied).
    pub fn scenario(&self) -> Scenario {
        let mut scn = self.kind.scenario();
        if let Some(t) = &self.tuning {
            scn = scn.tuned(t);
        }
        if let Some(p) = self.propellant.filter(|p| p.is_finite()) {
            let standard = scn.start_mass - scn.dry_mass;
            scn.start_mass = scn.dry_mass + p.clamp(1e3, 3.0 * standard);
        }
        scn
    }
}

/// The start attitude. Ship: belly down, nose toward the tower (−x).
/// Booster: engines first, its axis along the fall.
pub fn start_state(scn: &Scenario, r: Vec3, v: Vec3) -> State {
    let q = match scn.ship.kind {
        VehicleKind::Ship => ship_control::belly_attitude(0.0),
        VehicleKind::Booster => ship_control::descent_attitude(v),
    };
    dynamics::state(scn.start_mass, r, v, q, [0.0; 3])
}

/// One leg of the landing burn: this many engines, to this gate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    pub engines: u8,
    pub gate: Vec3,
    pub gate_velocity: Vec3,
    /// The leg to the approach gate (after it, the terminal phase).
    pub last: bool,
}

/// What guidance currently gives the controller.
#[derive(Debug, Clone)]
pub enum Guide {
    None,
    /// Fly this plan; `t0` = sim time of the measurement it was solved from.
    Plan { plan: ShipPlan, t0: f64 },
    /// No plan could be found: brake upright toward the axis.
    Brake,
}

pub struct ShipFlight {
    pub kind: Kind,
    sim: ShipSim,
    sensor: Sensor,
    att_rng: Rng,
    /// Unit-variance attitude/rate errors (Gauss–Markov like the sensor's).
    att_err: [f64; 6],
    noise: Noise,
    phase: Phase,
    guide: Guide,
    leg: Option<Leg>,
    grabbed: bool,
    last: Actuation,
    est: State,
    ignition_z: f64,
    gust_speed: f64,
}

impl ShipFlight {
    pub fn new(c: ShipFlightConfig) -> Result<ShipFlight, &'static str> {
        let scn = c.scenario();
        let ok = c.position.iter().chain(&c.velocity).chain(&c.wind).all(|v| v.is_finite());
        if !ok || c.position[2] < 1500.0 || !(0.0..=5.0).contains(&c.lag) || !c.gust_speed.is_finite() {
            return Err("bad flight configuration");
        }
        let x = start_state(&scn, c.position, c.velocity);
        let trim = scn.ship.surface_trim();
        let mut sim = ShipSim::new(scn, x);
        sim.cluster.lag = c.lag;
        sim.wind = [c.wind[0].clamp(-40.0, 40.0), c.wind[1].clamp(-40.0, 40.0), 0.0];
        for i in &c.failed {
            sim.cluster.fail(*i);
        }
        Ok(ShipFlight {
            kind: c.kind,
            sim,
            sensor: Sensor::new(c.noise, c.seed),
            att_rng: Rng::new(c.seed ^ 0x9e37_79b9_7f4a_7c15),
            att_err: [0.0; 6],
            noise: c.noise,
            phase: Phase::BellyFlop,
            guide: Guide::None,
            leg: None,
            grabbed: false,
            last: Actuation {
                thrust_body: [0.0; 3],
                flaps: [trim; 4],
                rcs: [0.0; 3],
            },
            est: x,
            ignition_z: 0.0,
            gust_speed: c.gust_speed.clamp(0.0, 60.0),
        })
    }

    pub fn sim(&self) -> &ShipSim {
        &self.sim
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn guide(&self) -> &Guide {
        &self.guide
    }
    pub fn leg(&self) -> Option<Leg> {
        self.leg
    }
    pub fn estimate(&self) -> &State {
        &self.est
    }
    pub fn last_command(&self) -> &Actuation {
        &self.last
    }
    pub fn ignition_z(&self) -> f64 {
        self.ignition_z
    }

    fn att_sigma(&self) -> (f64, f64) {
        // Scale with the position noise level (off / low / high).
        let k = self.noise.pos_sigma / Noise::HIGH.pos_sigma;
        (ATT_SIGMA_HIGH.0 * k, ATT_SIGMA_HIGH.1 * k)
    }

    /// A sensor reading of the full state.
    pub fn measure(&mut self) -> State {
        let x = self.sim.x;
        // Height above the rest position sets how good the "radar" is.
        let truth = Truth {
            t: self.sim.t,
            position: {
                let mut p = dynamics::position(&x);
                p[2] -= self.sim.scn.floor_z;
                p
            },
            velocity: dynamics::velocity(&x),
            mass: x[M],
            landed: self.sim.outcome.is_some(),
            touchdown_velocity: None,
        };
        let m: Measurement = self.sensor.measure(&truth);
        let phi = (-SUBSTEP / sim::sensor::CORRELATION_TIME).exp();
        let kick = (1.0 - phi * phi).sqrt();
        for e in &mut self.att_err {
            *e = phi * *e + kick * self.att_rng.gaussian();
        }
        let (sa, sw) = self.att_sigma();
        let axis = [self.att_err[0], self.att_err[1], self.att_err[2]];
        let dq = quat::from_axis_angle(axis, sa * quat::norm(axis));
        let q = quat::normalize(quat::mul(dynamics::attitude(&x), dq));
        let w = dynamics::rates(&x);
        let w = [
            w[0] + sw * self.att_err[3],
            w[1] + sw * self.att_err[4],
            w[2] + sw * self.att_err[5],
        ];
        let mut pos = m.position;
        pos[2] += self.sim.scn.floor_z;
        dynamics::state(x[M], pos, m.velocity, q, w)
    }

    /// Thrust range the controller may command now.
    fn command_limits(&self) -> (f64, f64) {
        self.sim.scn.ship.thrust_limits(self.sim.cluster.lit().max(1))
    }

    /// Engines flown to the end: the vehicle's final count, or fewer if
    /// fewer are left.
    fn final_engines(&self) -> u8 {
        self.sim.scn.ship.final_engines.min(self.sim.cluster.healthy()).max(1)
    }

    /// Planned thrust range for a leg on `n` engines, N.
    pub fn leg_limits(&self, n: u8) -> (f64, f64) {
        let (lo, hi) = self.sim.scn.ship.thrust_limits(n);
        let k = if n <= self.final_engines() { LATE_MARGIN } else { THRUST_MARGIN };
        (lo, (hi * k).max(lo * 1.05))
    }

    /// The last leg: the final engines to the approach gate.
    fn final_leg(&self) -> Leg {
        Leg {
            engines: self.final_engines(),
            gate: self.sim.scn.gate,
            gate_velocity: self.sim.scn.gate_velocity,
            last: true,
        }
    }

    /// An intermediate gate above the approach gate, from which the final
    /// engines (with their margin) stop the vehicle with room to spare.
    fn mid_leg(&self, x: &State, engines: u8) -> Leg {
        let scn = &self.sim.scn;
        let g = scn.planet.gravity();
        let decel = (self.leg_limits(self.final_engines()).1 / x[M] - g).max(0.5);
        let v = scn.mid_gate_speed;
        let z = scn.gate[2] + 1.5 * v * v / (2.0 * decel) + 50.0;
        Leg {
            engines,
            gate: [scn.gate[0], scn.gate[1], z],
            gate_velocity: [0.0, 0.0, -v],
            last: false,
        }
    }

    /// Advance `dt` seconds of sim time (control every substep).
    pub fn step(&mut self, dt: f64) {
        if self.grabbed || self.phase == Phase::Done || !(dt.is_finite() && dt > 0.0) {
            return;
        }
        let mut left = dt;
        while left > 1e-9 && self.phase != Phase::Done {
            let h = left.min(SUBSTEP);
            self.control_step(h);
            left -= h;
        }
    }

    fn near(x: &State, gate: Vec3, gate_velocity: Vec3, tol: (f64, f64)) -> bool {
        quat::norm(quat::sub(dynamics::position(x), gate)) < tol.0
            && quat::norm(quat::sub(dynamics::velocity(x), gate_velocity)) < tol.1
    }

    fn control_step(&mut self, h: f64) {
        let est = self.measure();
        self.est = est;
        let scn = self.sim.scn.clone();
        let t = self.sim.t;
        let act = match self.phase {
            Phase::BellyFlop => {
                let healthy = f64::from(self.sim.cluster.healthy()) * scn.ship.engine_max;
                self.ignition_z = ship_control::ignition_altitude(&scn, &est, healthy);
                let lead = -est[V + 2].min(0.0) * scn.ship.ignition_time;
                if est[R + 2] <= self.ignition_z + lead {
                    self.sim.cluster.ignite_all();
                    if scn.planet == Planet::Mars {
                        self.sim.legs_down = true;
                    }
                    self.phase = Phase::Flip;
                }
                ship_control::unpowered(&scn, &est, self.ignition_z)
            }
            Phase::Flip => {
                let limits = self.command_limits();
                if self.sim.cluster.running() == 0 {
                    // Spinning up: keep the flaps flying, ask for thrust.
                    let mut a = ship_control::unpowered(&scn, &est, self.ignition_z);
                    a.thrust_body = [0.0, 0.0, limits.0];
                    a
                } else {
                    let done = quat::tilt(dynamics::attitude(&est)) < FLIP_DONE_TILT
                        && quat::norm(dynamics::rates(&est)) < FLIP_DONE_RATE;
                    if done {
                        self.phase = Phase::Burn;
                        self.guide = Guide::None;
                    }
                    ship_control::flip(&scn, &est, limits)
                }
            }
            Phase::Burn => self.burn(&scn, &est, t),
            Phase::Terminal => {
                self.sim.cluster.shutdown_to(self.final_engines());
                // Too light to hover on what is lit: drop one more.
                let lit = self.sim.cluster.lit();
                let weight = est[M] * scn.planet.gravity();
                if lit > 1 && self.command_limits().0 > HOVER_SHARE * weight {
                    self.sim.cluster.shutdown_to(lit - 1);
                }
                ship_control::terminal(&scn, &est, self.command_limits())
            }
            Phase::Done => self.last,
        };
        self.last = act;
        let cmd = Command {
            thrust_body: act.thrust_body,
            flaps: act.flaps,
            rcs: act.rcs,
        };
        self.sim.step(&cmd, h);
        if self.sim.outcome.is_some() {
            self.phase = Phase::Done;
        }
    }

    fn burn(&mut self, scn: &Scenario, est: &State, t: f64) -> Actuation {
        match (&self.guide, self.leg) {
            (Guide::Plan { plan, t0 }, Some(leg)) => {
                let tau = t - t0;
                let tol = if leg.last {
                    (GATE_POSITION, GATE_VELOCITY)
                } else {
                    (2.0 * GATE_POSITION, 2.0 * GATE_VELOCITY)
                };
                if tau >= plan.final_time || Self::near(est, leg.gate, leg.gate_velocity, tol) {
                    self.sim.cluster.shutdown_to(self.final_engines());
                    if leg.last {
                        self.phase = Phase::Terminal;
                    } else {
                        self.leg = Some(self.final_leg());
                        self.guide = Guide::None;
                    }
                    return ship_control::terminal(scn, est, self.command_limits());
                }
                let (xr, ur) = plan.sample(tau);
                ship_control::powered(scn, est, (&xr, ur), self.command_limits())
            }
            _ => {
                // No plan (yet): brake upright toward the axis. Several lit
                // engines at minimum throttle out-push the weight, so once
                // the fall is mostly arrested keep only one.
                if self.sim.cluster.lit() > self.final_engines() && est[V + 2] > -10.0 {
                    self.sim.cluster.shutdown_to(self.final_engines());
                }
                ship_control::terminal(scn, est, self.command_limits())
            }
        }
    }

    /// Re-planning is useful now: no plan yet, or the ship has drifted off
    /// it, or it is old, and it is not about to end.
    pub fn wants_replan(&self) -> bool {
        if self.phase != Phase::Burn {
            return false;
        }
        match &self.guide {
            Guide::None | Guide::Brake => true,
            Guide::Plan { plan, t0 } => {
                let age = self.sim.t - t0;
                if plan.final_time - age <= FREEZE {
                    return false;
                }
                let (xr, _) = plan.sample(age);
                let dr = quat::norm(quat::sub(dynamics::position(&xr), dynamics::position(&self.est)));
                let dv = quat::norm(quat::sub(dynamics::velocity(&xr), dynamics::velocity(&self.est)));
                age >= REPLAN_MAX_AGE || dr > REPLAN_POSITION_ERROR || dv > REPLAN_VELOCITY_ERROR
            }
        }
    }

    /// The problems to try, in order, from a measurement taken now:
    /// `(t, [(leg, problem)])`. Without a leg yet: one engine to the approach
    /// gate first, else every lit engine to an intermediate gate.
    pub fn problem(&mut self) -> (f64, Vec<(Leg, ShipProblem)>) {
        let x = self.measure();
        let t = self.sim.t;
        let lit = self.sim.cluster.lit().max(1);
        let legs = match self.leg {
            Some(leg) => vec![leg],
            None if lit > self.final_engines() => vec![self.final_leg(), self.mid_leg(&x, lit)],
            None => vec![self.final_leg()],
        };
        let options = legs
            .into_iter()
            .map(|leg| {
                let mut scn = self.sim.scn.clone();
                scn.gate = leg.gate;
                scn.gate_velocity = leg.gate_velocity;
                (leg, ShipProblem::new(scn, x, self.leg_limits(leg.engines)))
            })
            .collect();
        (t, options)
    }

    /// Install a plan for `leg`, solved from the measurement at sim time `t0`.
    pub fn set_plan(&mut self, plan: ShipPlan, t0: f64, leg: Leg) {
        if self.phase != Phase::Burn {
            return;
        }
        self.sim.cluster.shutdown_to(leg.engines);
        self.leg = Some(leg);
        self.guide = Guide::Plan { plan, t0 };
    }

    /// A re-plan failed: keep the current plan if the ship is still near it;
    /// with none, or far off it (a stale plan leads astray), brake.
    pub fn plan_failed(&mut self, _e: &GuidanceError) {
        if self.phase != Phase::Burn {
            return;
        }
        let stale = match &self.guide {
            Guide::Plan { plan, t0 } => {
                let (xr, _) = plan.sample(self.sim.t - t0);
                quat::norm(quat::sub(dynamics::position(&xr), dynamics::position(&self.est))) > STALE_POSITION
                    || quat::norm(quat::sub(dynamics::velocity(&xr), dynamics::velocity(&self.est))) > STALE_VELOCITY
            }
            _ => true,
        };
        if stale {
            self.guide = Guide::Brake;
        }
    }

    /// A gust along ±x (the tower's line), at the configured speed.
    pub fn gust(&mut self, dir: f64) {
        self.gust_toward(if dir < 0.0 { -1.0 } else { 1.0 }, 0.0);
    }

    /// A gust blowing toward the horizontal direction `(dx, dy)`.
    pub fn gust_toward(&mut self, dx: f64, dy: f64) {
        let n = dx.hypot(dy);
        if !n.is_finite() || n < 1e-9 {
            return;
        }
        self.sim.gusts.push(WindGust {
            start: self.sim.t,
            dir: [dx / n, dy / n, 0.0],
            speed: self.gust_speed,
        });
    }

    pub fn set_gust_speed(&mut self, speed: f64) {
        if speed.is_finite() {
            self.gust_speed = speed.clamp(0.0, 60.0);
        }
    }

    /// Steady wind, m/s (x, y).
    pub fn set_wind(&mut self, x: f64, y: f64) {
        if x.is_finite() && y.is_finite() {
            self.sim.wind = [x.clamp(-40.0, 40.0), y.clamp(-40.0, 40.0), 0.0];
        }
    }

    pub fn engine_out(&mut self) -> bool {
        self.sim.cluster.fail_next()
    }

    /// Fail landing engine `i`.
    pub fn fail_engine(&mut self, i: usize) -> bool {
        self.sim.cluster.fail(i)
    }

    pub fn set_lag(&mut self, seconds: f64) {
        self.sim.cluster.lag = seconds.clamp(0.0, 5.0);
    }

    pub fn set_noise(&mut self, n: Noise) {
        self.noise = n;
        self.sensor.noise = n;
    }

    /// Hold the ship at `p` (sim time stops while held).
    pub fn grab(&mut self, p: Vec3) {
        if self.phase == Phase::Done || !p.iter().all(|c| c.is_finite()) {
            return;
        }
        self.grabbed = true;
        let z = p[2].max(self.sim.scn.floor_z + 30.0);
        self.sim.x[R..R + 3].copy_from_slice(&[p[0], p[1], z]);
    }

    pub fn release(&mut self) {
        self.grabbed = false;
    }

    pub fn grabbed(&self) -> bool {
        self.grabbed
    }

    /// Engine states as text, for the panels.
    pub fn engine_states(&self) -> Vec<&'static str> {
        self.sim.cluster.engines.iter().map(|e| e.as_str()).collect()
    }
}

/// Solve the options in order as the worker would; the first that plans
/// wins: `(index, plan)`. Always from a fresh first guess: warm starts from
/// the previous plan were tried and kept inheriting its shape after it had
/// stopped fitting (flight times creeping from 13 s to 55 s).
pub fn plan(options: &[(Leg, ShipProblem)]) -> Result<(usize, ShipPlan), GuidanceError> {
    let mut last = GuidanceError::Infeasible;
    for (i, (_, p)) in options.iter().enumerate() {
        match scvx::plan_ship(p, None, scvx::COLD_ITERATIONS) {
            Ok(plan) => return Ok((i, plan)),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Predicted belly flop from a start (noise off) until ignition: the glide
/// path (every 0.5 s) and the state at ignition.
pub fn predict_glide(kind: Kind, position: Vec3, velocity: Vec3) -> Option<(Vec<Vec3>, State)> {
    let mut f = ShipFlight::new(ShipFlightConfig {
        position,
        velocity,
        ..ShipFlightConfig::new(kind)
    })
    .ok()?;
    let mut path = vec![position];
    let mut since = 0.0;
    while f.phase == Phase::BellyFlop {
        f.step(0.05);
        since += 0.05;
        if since >= 0.5 {
            path.push(dynamics::position(&f.sim.x));
            since = 0.0;
        }
        if f.sim.t > 600.0 {
            return None;
        }
    }
    path.push(dynamics::position(&f.sim.x));
    Some((path, f.sim.x))
}
