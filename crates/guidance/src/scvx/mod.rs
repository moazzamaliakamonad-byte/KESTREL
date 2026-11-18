//! 6-DoF powered-descent guidance for the ship's flip and landing burn by
//! successive convexification (SCvx; Szmuk & Açıkmeşe 2018,
//! arXiv:1802.03827): linearize the nonlinear dynamics around a reference
//! trajectory, solve one convex SOCP with virtual control and a trust
//! region, take its answer as the next reference, repeat.
//!
//! Free final time (`σ`), first-order-hold controls, K nodes. Simulation only.

pub mod discretize;
mod subproblem;
pub use subproblem::{Backend, IPM_ITERATIONS, backend, set_backend};

use model::dynamics::{self, M, Q, State};
use model::quat::{self, Quat, Vec3};
use model::{Scenario, VehicleKind};

use crate::GuidanceError;
use discretize::{NX, Units};

/// Nodes per plan: the ship 16 (each subproblem solves 2.3 times faster
/// than with 24, and 32 closed-loop flights, nominal and engine-out,
/// caught as many: 25 of 32 both; 12 caught 23). The booster keeps 24:
/// with 16 its plans failed when solves took a second (slow-solver test).
pub const NODES: usize = 24;
pub const SHIP_NODES: usize = 16;
/// Interior-point tolerance of each subproblem: the solver's default for
/// the booster, 1e-6 for the ship (about a fifth less time per
/// subproblem, same number of SCvx iterations; looser cost more
/// iterations than it saved). The booster's browser sweep caught one
/// fewer with 1e-6.
pub const SOCP_TOLERANCE: f64 = 1e-8;
pub const SHIP_SOCP_TOLERANCE: f64 = 1e-6;
/// Largest planned body rate, rad/s (40°/s).
pub const OMEGA_MAX: f64 = 0.698_131_700_797_731_8;
/// Most SCvx iterations for a cold start and for a warm re-plan.
pub const COLD_ITERATIONS: usize = 15;
pub const WARM_ITERATIONS: usize = 6;
/// A plan is accepted when re-integrating it ends this close to the gate, m
/// (the terminal controller flies the precise last part).
pub const GATE_TOLERANCE: f64 = 30.0;
/// …but at most this share of the distance from the start to the gate,
/// and never less than GATE_TOLERANCE_NEAR, m.
pub const GATE_FRACTION: f64 = 0.15;
pub const GATE_TOLERANCE_NEAR: f64 = 3.0;
const SUBSTEPS: usize = 6;
const W_VIRTUAL: f64 = 1e3;
const W_TRUST: f64 = 0.5;
/// Trust-region weight on the flight time (kept separate so it can be tuned;
/// lighter values were tried and converged no faster).
const W_TRUST_SIGMA: f64 = 0.5;
/// A solve still needing this much virtual control (non-dimensional,
/// summed) after this many iterations is given up as infeasible.
pub const HOPELESS_AFTER: usize = 7;
pub const HOPELESS_DEFECT: f64 = 1e-2;
/// Converged when the largest change of any non-dimensional variable is below this.
const STEP_TOLERANCE: f64 = 5e-3;

#[derive(Debug, Clone, PartialEq)]
pub struct ShipProblem {
    pub scn: Scenario,
    /// Start state (SI).
    pub x0: State,
    /// Planned thrust magnitude range, N.
    pub thrust_limits: (f64, f64),
    /// From node `switch_node` on, the range with one engine, N.
    pub late_limits: Option<(f64, f64)>,
    pub switch_node: usize,
    /// Fly within [`UPRIGHT_TILT`] of upright from this node on (overrides
    /// the default: from the switch node, else the last fifth).
    pub upright_from: Option<usize>,
    /// Largest planned tilt from vertical, rad (None: free, as the ship's
    /// flip needs). The booster stays near upright through its burn.
    pub max_tilt: Option<f64>,
    /// `max_tilt` applies from this node on (the ship: once its flip is done).
    pub tilt_from: usize,
    /// Approach funnel around the gate: radius at the gate, m, and how
    /// much wider per metre above it.
    pub approach: (f64, f64),
    /// Sideways speed limit: m/s at the gate's height, and more per metre above.
    pub side_speed: (f64, f64),
    pub nodes: usize,
}

/// The booster's largest planned tilt in the landing burn, rad (15°,
/// *illustrative*). Without it the optimizer turns the booster broadside
/// at 400 m/s to brake on air drag: cheap on paper, violent in reality.
pub const BOOSTER_TILT: f64 = 0.261_799_387_799_149_4;

impl ShipProblem {
    pub fn new(mut scn: Scenario, x0: State, thrust_limits: (f64, f64)) -> ShipProblem {
        let max_tilt = match scn.ship.kind {
            VehicleKind::Ship => None,
            VehicleKind::Booster => {
                // Plan without crossflow drag. It rewards leaning either
                // way equally (two mirror-image optima), and successive
                // convexification hopped between them instead of
                // converging. The controller absorbs the difference.
                scn.ship.c_n = 0.0;
                Some(BOOSTER_TILT)
            }
        };
        let nodes = match scn.ship.kind {
            VehicleKind::Ship => SHIP_NODES,
            VehicleKind::Booster => NODES,
        };
        ShipProblem {
            scn,
            x0,
            thrust_limits,
            late_limits: None,
            // (No switch: past the last node.)
            switch_node: nodes,
            upright_from: None,
            max_tilt,
            tilt_from: 2,
            approach: (APPROACH_RADIUS, APPROACH_TAN),
            side_speed: (SIDE_SPEED_0, SIDE_SPEED_SLOPE),
            nodes,
        }
    }

    /// Thrust bounds at node `k`, N.
    pub fn bounds(&self, k: usize) -> (f64, f64) {
        match self.late_limits {
            Some(l) if k >= self.switch_node => l,
            _ => self.thrust_limits,
        }
    }
}

/// Approach cone: radius at the gate height, m, and half-angle (45°).
pub const APPROACH_RADIUS: f64 = 10.0;
pub const APPROACH_TAN: f64 = 1.0;
/// Sideways speed limit: `SIDE_SPEED_0 + SIDE_SPEED_SLOPE · height above
/// the gate` (m/s, 1/s): slow and careful next to the tower.
pub const SIDE_SPEED_0: f64 = 2.0;
pub const SIDE_SPEED_SLOPE: f64 = 0.15;
/// The last nodes (this fraction) fly within this tilt of upright, rad (10°).
pub const UPRIGHT_FRACTION: f64 = 0.2;
pub const UPRIGHT_TILT: f64 = 0.174_532_925_199_432_95;

/// A planned flip and landing burn (SI). Time 0 is the start state.
#[derive(Debug, Clone, PartialEq)]
pub struct ShipPlan {
    pub t: Vec<f64>,
    pub x: Vec<State>,
    /// Body-frame thrust at each node, N (linear between nodes).
    pub u: Vec<Vec3>,
    pub final_time: f64,
    pub iterations: usize,
    /// Σ|virtual control| of the last subproblem (non-dimensional).
    pub defect: f64,
    /// Distance from the gate when the plan is re-integrated, m.
    pub gate_error: f64,
    pub converged: bool,
    /// Plan time when the plan drops to one engine, s (None: one throughout).
    pub switch_time: Option<f64>,
}

impl ShipPlan {
    /// Plan time at node `k`, s.
    pub fn node_time(&self, k: usize) -> f64 {
        self.t[k.min(self.t.len() - 1)]
    }

    /// State and thrust at plan time `t` (clamped to the plan).
    pub fn sample(&self, t: f64) -> (State, Vec3) {
        let n = self.t.len();
        let t = t.clamp(0.0, self.final_time);
        let dt = self.final_time / (n - 1) as f64;
        let k = ((t / dt).floor() as usize).min(n - 2);
        let a = ((t - self.t[k]) / dt).clamp(0.0, 1.0);
        let mut x: State = std::array::from_fn(|i| (1.0 - a) * self.x[k][i] + a * self.x[k + 1][i]);
        let q = quat::normalize(dynamics::attitude(&x));
        x[Q..Q + 4].copy_from_slice(&q);
        let u = std::array::from_fn(|i| (1.0 - a) * self.u[k][i] + a * self.u[k + 1][i]);
        (x, u)
    }
}

/// Rotation that removes the tilt of `q` (keeps its heading): the upright
/// attitude closest to `q`.
pub fn upright_like(q: Quat) -> Quat {
    let bz = quat::rotate(q, [0.0, 0.0, 1.0]);
    let axis = quat::cross(bz, [0.0, 0.0, 1.0]);
    let angle = bz[2].clamp(-1.0, 1.0).acos();
    let fix = if quat::norm(axis) < 1e-9 {
        if bz[2] > 0.0 {
            [1.0, 0.0, 0.0, 0.0]
        } else {
            quat::from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::PI)
        }
    } else {
        quat::from_axis_angle(axis, angle)
    };
    quat::normalize(quat::mul(fix, q))
}

fn slerp(a: Quat, b: Quat, s: f64) -> Quat {
    let mut d: f64 = (0..4).map(|i| a[i] * b[i]).sum();
    let b = if d < 0.0 {
        d = -d;
        b.map(|c| -c)
    } else {
        b
    };
    if d > 0.9995 {
        return quat::normalize(std::array::from_fn(|i| a[i] + s * (b[i] - a[i])));
    }
    let th = d.clamp(-1.0, 1.0).acos();
    let (wa, wb) = (((1.0 - s) * th).sin() / th.sin(), (s * th).sin() / th.sin());
    quat::normalize(std::array::from_fn(|i| wa * a[i] + wb * b[i]))
}

/// The cold start's guess of the flight time, s: fall, then brake at 70 %
/// of the deceleration available (the shape a fuel-optimal descent has),
/// or a steady descent if that is quicker. A poor guess is costly: the
/// trust region moves the time only a little per iteration.
pub fn cold_time_guess(p: &ShipProblem) -> f64 {
    let g = p.scn.planet.gravity();
    let h = (p.x0[dynamics::R + 2] - p.scn.gate[2]).max(10.0);
    let v0 = (-p.x0[dynamics::V + 2]).max(0.0);
    let a = (0.7 * (p.bounds(0).1 / p.x0[M] - g)).max(0.5);
    let v1 = ((v0 * v0 + 2.0 * g * h) * a / (a + g)).sqrt().max(v0);
    let braking = (v1 - v0) / g + v1 / a;
    let steady = 2.0 * h / v0.max(1.0);
    braking.min(steady).clamp(4.0, 120.0)
}

/// A simple first guess (SI): straight lines in position and velocity,
/// the flip done in the first third, thrust along the body axis.
fn cold_reference(p: &ShipProblem) -> (Vec<State>, Vec<Vec3>, f64) {
    let scn = &p.scn;
    let x0 = p.x0;
    let g = scn.planet.gravity();
    let r0 = dynamics::position(&x0);
    let v0 = dynamics::velocity(&x0);
    let tf = cold_time_guess(p);
    let q0 = dynamics::attitude(&x0);
    let qf = upright_like(q0);
    let k = p.nodes;
    let mut xs = Vec::with_capacity(k);
    let mut us = Vec::with_capacity(k);
    for n in 0..k {
        let s = n as f64 / (k - 1) as f64;
        let m = (x0[M] - s * tf * scn.ship.alpha() * 1.3 * x0[M] * g).max(scn.dry_mass);
        let r = std::array::from_fn(|i| r0[i] + s * (scn.gate[i] - r0[i]));
        let v = std::array::from_fn(|i| v0[i] + s * (scn.gate_velocity[i] - v0[i]));
        let q = slerp(q0, qf, (3.0 * s).min(1.0));
        xs.push(dynamics::state(m, r, v, q, [0.0; 3]));
        let (tmin, tmax) = p.bounds(n);
        us.push([0.0, 0.0, (1.3 * m * g).clamp(tmin, tmax)]);
    }
    (xs, us, tf)
}

/// Previous plan, re-timed to start `elapsed` seconds in, from `x0`.
fn warm_reference(
    p: &ShipProblem,
    prev: &ShipPlan,
    elapsed: f64,
) -> Option<(Vec<State>, Vec<Vec3>, f64)> {
    let left = prev.final_time - elapsed;
    if left < 2.0 {
        return None;
    }
    let k = p.nodes;
    let mut xs = Vec::with_capacity(k);
    let mut us = Vec::with_capacity(k);
    for n in 0..k {
        let t = elapsed + left * n as f64 / (k - 1) as f64;
        let (x, u) = prev.sample(t);
        xs.push(x);
        us.push(u);
    }
    xs[0] = p.x0;
    Some((xs, us, left))
}

/// Plan the flip and landing burn from `p.x0` to the scenario's gate.
///
/// `warm` = (previous plan, seconds since its start) to re-plan from it.
pub fn plan_ship(
    p: &ShipProblem,
    warm: Option<(&ShipPlan, f64)>,
    max_iter: usize,
) -> Result<ShipPlan, GuidanceError> {
    let scn = &p.scn;
    if p.nodes < 3 || !p.x0.iter().all(|c| c.is_finite()) || p.x0[M] <= scn.dry_mass {
        return Err(GuidanceError::InvalidProblem("bad ship start state"));
    }
    let units = Units::new(scn, p.x0[M]);
    let (xs_si, us_si, tf) = warm
        .and_then(|(prev, e)| warm_reference(p, prev, e))
        .unwrap_or_else(|| cold_reference(p));
    let mut xs: Vec<State> = xs_si.iter().map(|x| units.x_nd(x)).collect();
    let mut us: Vec<Vec3> = us_si.iter().map(|u| units.u_nd(*u)).collect();
    let mut sigma = tf / units.t;
    let x0 = units.x_nd(&p.x0);

    let lv = units.l / units.t;
    let gate_r = scn.gate.map(|c| c / units.l);
    let gate_v = scn.gate_velocity.map(|c| c / lv);
    let f = units.force();
    let t_bounds: Vec<(f64, f64)> = (0..p.nodes)
        .map(|k| {
            let (a, b) = p.bounds(k);
            (a / f, b / f)
        })
        .collect();
    let mut defect = f64::INFINITY;
    let mut iterations = 0;
    let mut converged = false;
    for it in 1..=max_iter {
        iterations = it;
        let ivs = discretize::discretize(scn, &units, &xs, &us, sigma, SUBSTEPS);
        let sub = subproblem::solve(&subproblem::Sub {
            xs: &xs,
            us: &us,
            sigma,
            ivs: &ivs,
            x0,
            gate_r,
            gate_v,
            m_dry: scn.dry_mass / units.m,
            t_bounds: &t_bounds,
            tan_gimbal: scn.ship.gimbal_max.tan(),
            w_max: OMEGA_MAX * units.t,
            floor_z: scn.floor_z / units.l,
            clearance_x: scn.clearance_x.map(|c| c / units.l),
            approach: (p.approach.0 / units.l, p.approach.1),
            // Upright before dropping to one engine (the flip is done on all
            // of them), else for the last part of the burn.
            upright_from: p.upright_from.unwrap_or_else(|| {
                let tail = p.nodes - 1 - ((p.nodes - 1) as f64 * UPRIGHT_FRACTION).round() as usize;
                if p.late_limits.is_some() {
                    p.switch_node.min(tail)
                } else {
                    tail
                }
            }),
            upright_sin_half: (UPRIGHT_TILT / 2.0).sin(),
            tilt_sin_half: p.max_tilt.map(|t| (t / 2.0).sin()),
            tilt_from: p.tilt_from.max(2),
            side_speed: (p.side_speed.0 / lv, p.side_speed.1 * units.t),
            sigma_bounds: (1.0 / units.t, 200.0 / units.t),
            w_virtual: W_VIRTUAL,
            w_trust: W_TRUST,
            w_trust_sigma: W_TRUST_SIGMA,
            tolerance: match scn.ship.kind {
                VehicleKind::Ship => SHIP_SOCP_TOLERANCE,
                VehicleKind::Booster => SOCP_TOLERANCE,
            },
        })?;
        let mut step: f64 = (sub.sigma - sigma).abs();
        for (sx, x) in sub.xs.iter().zip(&xs) {
            for (a, b) in sx[..NX].iter().zip(&x[..NX]) {
                step = step.max((a - b).abs());
            }
        }
        for (su, u) in sub.us.iter().zip(&us) {
            for (a, b) in su.iter().zip(u) {
                step = step.max((a - b).abs());
            }
        }
        xs = sub.xs;
        us = sub.us;
        sigma = sub.sigma;
        defect = sub.virtual_l1;
        if std::env::var_os("SCVX_TRACE").is_some() {
            eprintln!(
                "it {it}: step {step:.2e} defect {defect:.2e} tf {:.2}",
                sigma * units.t
            );
        }
        if defect < 1e-4 && step < STEP_TOLERANCE {
            converged = true;
            break;
        }
        // The ship: still needing virtual control this late means there is
        // no solution from here. Say so now, not after every iteration (a
        // failed solve took over a second and held up the next option).
        // Not the booster: at long solve latencies its plans needed more
        // iterations and did converge (browser sweep 31 of 33 with the
        // abort, 33 of 33 without).
        if scn.ship.kind == VehicleKind::Ship && it >= HOPELESS_AFTER && defect > HOPELESS_DEFECT {
            return Err(GuidanceError::Infeasible);
        }
    }

    // Accept only what the nonlinear dynamics agree with.
    let flown = discretize::propagate(scn, &units, &x0, &us, sigma, 10);
    let end = units.x_si(flown.last().expect("nodes"));
    let gate_error = quat::norm(quat::sub(dynamics::position(&end), scn.gate));
    // Near the gate a miss of tens of metres is no plan at all: the
    // tolerance shrinks with the distance still to fly.
    let to_go = quat::norm(quat::sub(dynamics::position(&p.x0), scn.gate));
    let tolerance = (GATE_FRACTION * to_go).clamp(GATE_TOLERANCE_NEAR, GATE_TOLERANCE);
    if !gate_error.is_finite() || gate_error > tolerance {
        return Err(GuidanceError::Infeasible);
    }
    let final_time = sigma * units.t;
    let k = xs.len();
    Ok(ShipPlan {
        t: (0..k)
            .map(|n| final_time * n as f64 / (k - 1) as f64)
            .collect(),
        x: xs.iter().map(|x| units.x_si(x)).collect(),
        u: us.iter().map(|u| units.u_si(*u)).collect(),
        final_time,
        iterations,
        defect,
        gate_error,
        converged,
        switch_time: p
            .late_limits
            .map(|_| final_time * p.switch_node.min(k - 1) as f64 / (k - 1) as f64),
    })
}

/// Indices re-exported for tests and callers that read plan states.
pub mod index {
    pub use model::dynamics::{M, Q, R, V, W};
}

#[cfg(test)]
mod tests;
