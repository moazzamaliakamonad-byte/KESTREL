//! The final approach onto the arms, planned (M4.2, D-047).
//!
//! From the burn's gate to the caught position the vehicle flies a plan
//! that is re-made several times a second, instead of a scripted slide and
//! sink. A three-degree-of-freedom point mass (the centre of mass) over
//! [`NODES`] intervals of a fixed final time, posed as a second-order cone
//! program and solved by Kestrel's own solver (`crates/socp`), which is set
//! up once (pattern analysed, memory allocated) and only updated after.
//!
//! What keeps it off the tower: the vehicle's axis tilts with its thrust,
//! so braking sideways swings the base toward the mast (D-033). The base
//! and the nose are kept clear with the tilt linearised at hover
//! (`x ∓ l · a_x / g ≥ keep_x`), and the thrust is held within [`TILT`] of
//! vertical. Simulation only.

use crate::problem::GuidanceError;
use model::quat::Vec3;
use socp::{Cone, Csc, Settings, Solver, Status};

/// Intervals of an approach plan.
pub const NODES: usize = 20;
/// Largest planned tilt of the thrust from vertical, rad (6°; the catch
/// allows 5° at contact, the funnel keeps the end upright).
pub const TILT: f64 = 0.104_719_755_119_659_77;
/// The plan arrives sinking this fast, m/s (the catch allows 2).
pub const SINK_AT_CONTACT: f64 = 1.0;
/// Over the axis: horizontal distance ≤ FUNNEL.0 + FUNNEL.1 · height above
/// the caught position, m.
pub const FUNNEL: (f64, f64) = (0.5, 1.2);
/// Sink rate ≤ SINK_CONE.0 + SINK_CONE.1 · height, m/s (it slows as it
/// comes down).
pub const SINK_CONE: (f64, f64) = (1.0, 0.4);
/// Sideways speed ≤ SIDE_CONE.0 + SIDE_CONE.1 · height, m/s.
pub const SIDE_CONE: (f64, f64) = (1.0, 0.2);
/// Weight of the change in acceleration between intervals (smoothness).
pub const SMOOTH: f64 = 2.0;
/// Final times the first plan tries, s.
pub const FINAL_TIMES: [f64; 5] = [5.0, 7.0, 9.0, 12.0, 15.0];

/// One approach plan's inputs. Inertial frame, the catch axis at x = y = 0,
/// the mast on the −x side.
#[derive(Debug, Clone, PartialEq)]
pub struct ApproachProblem {
    /// Start: the navigation estimate's position and velocity.
    pub r0: Vec3,
    pub v0: Vec3,
    /// Height of the centre of mass when caught, m.
    pub z_catch: f64,
    /// Final time, s.
    pub t_f: f64,
    /// Acceleration the engines can give, m/s²: least (straight up) and most.
    pub a_min: f64,
    pub a_max: f64,
    /// The axis must stay at `x ≥ keep_x` along its length, m.
    pub keep_x: f64,
    /// Centre of mass to base and to nose along the axis, m.
    pub base_arm: f64,
    pub nose_arm: f64,
    pub g: f64,
}

/// A planned approach: positions and velocities at `NODES + 1` nodes, the
/// acceleration (thrust / mass) on each interval.
#[derive(Debug, Clone, PartialEq)]
pub struct ApproachPlan {
    pub t_f: f64,
    pub r: Vec<Vec3>,
    pub v: Vec<Vec3>,
    pub a: Vec<Vec3>,
    /// Gravity it was planned with, m/s².
    pub g: f64,
    /// The objective's value (propellant plus smoothness).
    pub cost: f64,
    /// Interior-point iterations of the solve.
    pub iterations: usize,
}

impl ApproachPlan {
    /// Position, velocity and commanded acceleration `tau` seconds in
    /// (exact for the plan's piecewise-constant acceleration); after the
    /// end, the end.
    pub fn sample(&self, tau: f64) -> (Vec3, Vec3, Vec3) {
        let n = self.a.len();
        let dt = self.t_f / n as f64;
        if tau.is_nan() || tau <= 0.0 {
            return (self.r[0], self.v[0], self.a[0]);
        }
        if tau >= self.t_f {
            return (self.r[n], self.v[n], self.a[n - 1]);
        }
        let k = ((tau / dt) as usize).min(n - 1);
        let s = tau - k as f64 * dt;
        let (r, v, a) = (self.r[k], self.v[k], self.a[k]);
        let acc = [a[0], a[1], a[2] - self.g];
        (
            std::array::from_fn(|i| r[i] + v[i] * s + 0.5 * acc[i] * s * s),
            std::array::from_fn(|i| v[i] + acc[i] * s),
            a,
        )
    }
}

/// Plans approaches, keeping one set-up solver (the problem's pattern never
/// changes, only its numbers).
pub struct ApproachPlanner {
    solver: Option<Solver>,
    setups: usize,
}

impl Default for ApproachPlanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Variable layout: positions and velocities at every node, then the
/// acceleration and its bound per interval, then the change of
/// acceleration between intervals.
struct Layout;
impl Layout {
    fn r(k: usize) -> usize {
        6 * k
    }
    fn v(k: usize) -> usize {
        6 * k + 3
    }
    fn a(k: usize) -> usize {
        6 * (NODES + 1) + 4 * k
    }
    fn sigma(k: usize) -> usize {
        6 * (NODES + 1) + 4 * k + 3
    }
    fn j(k: usize) -> usize {
        6 * (NODES + 1) + 4 * NODES + 3 * k
    }
    fn n() -> usize {
        6 * (NODES + 1) + 4 * NODES + 3 * (NODES - 1)
    }
}

/// Rows of `A x + s = b`, built in cone order.
struct Rows {
    ri: Vec<usize>,
    ci: Vec<usize>,
    vals: Vec<f64>,
    b: Vec<f64>,
}

impl Rows {
    fn row(&mut self, entries: &[(usize, f64)], rhs: f64) {
        let r = self.b.len();
        for &(c, v) in entries {
            self.ri.push(r);
            self.ci.push(c);
            self.vals.push(v);
        }
        self.b.push(rhs);
    }
}

type Assembled = (Vec<f64>, Vec<f64>, Csc, Vec<f64>, Vec<Cone>);

fn assemble(p: &ApproachProblem) -> Assembled {
    let n = NODES;
    let dt = p.t_f / n as f64;
    let mut rows = Rows {
        ri: Vec::new(),
        ci: Vec::new(),
        vals: Vec::new(),
        b: Vec::new(),
    };
    // Equalities: the start, the dynamics, the end, the change of acceleration.
    for i in 0..3 {
        rows.row(&[(Layout::r(0) + i, 1.0)], p.r0[i]);
        rows.row(&[(Layout::v(0) + i, 1.0)], p.v0[i]);
    }
    for k in 0..n {
        for i in 0..3 {
            let gz = if i == 2 { p.g } else { 0.0 };
            rows.row(
                &[
                    (Layout::r(k + 1) + i, 1.0),
                    (Layout::r(k) + i, -1.0),
                    (Layout::v(k) + i, -dt),
                    (Layout::a(k) + i, -0.5 * dt * dt),
                ],
                -0.5 * gz * dt * dt,
            );
            rows.row(
                &[
                    (Layout::v(k + 1) + i, 1.0),
                    (Layout::v(k) + i, -1.0),
                    (Layout::a(k) + i, -dt),
                ],
                -gz * dt,
            );
        }
    }
    let end_r = [0.0, 0.0, p.z_catch];
    let end_v = [0.0, 0.0, -SINK_AT_CONTACT];
    for i in 0..3 {
        rows.row(&[(Layout::r(n) + i, 1.0)], end_r[i]);
        rows.row(&[(Layout::v(n) + i, 1.0)], end_v[i]);
    }
    for k in 0..n - 1 {
        for i in 0..3 {
            rows.row(
                &[
                    (Layout::j(k) + i, 1.0),
                    (Layout::a(k + 1) + i, -1.0),
                    (Layout::a(k) + i, 1.0),
                ],
                0.0,
            );
        }
    }
    let zero = rows.b.len();
    // Inequalities (s = b − A x ≥ 0).
    for k in 0..n {
        rows.row(&[(Layout::sigma(k), 1.0)], p.a_max);
        rows.row(&[(Layout::a(k) + 2, -1.0)], -p.a_min);
        // Base and nose off the mast: x − l_b a_x / g ≥ keep, x + l_n a_x / g ≥ keep.
        rows.row(
            &[(Layout::r(k), -1.0), (Layout::a(k), p.base_arm / p.g)],
            -p.keep_x,
        );
        rows.row(
            &[(Layout::r(k), -1.0), (Layout::a(k), -p.nose_arm / p.g)],
            -p.keep_x,
        );
    }
    for k in 1..=n {
        // v_z ≥ −(c0 + c1 (z − z_c)).
        rows.row(
            &[(Layout::v(k) + 2, -1.0), (Layout::r(k) + 2, -SINK_CONE.1)],
            SINK_CONE.0 - SINK_CONE.1 * p.z_catch,
        );
    }
    let nonneg = rows.b.len() - zero;
    let mut cones = vec![Cone::Zero(zero), Cone::Nonneg(nonneg)];
    let tan = TILT.tan();
    for k in 0..n {
        // σ ≥ ‖a‖.
        rows.row(&[(Layout::sigma(k), -1.0)], 0.0);
        for i in 0..3 {
            rows.row(&[(Layout::a(k) + i, -1.0)], 0.0);
        }
        cones.push(Cone::Soc(4));
        // tan θ · a_z ≥ ‖(a_x, a_y)‖.
        rows.row(&[(Layout::a(k) + 2, -tan)], 0.0);
        rows.row(&[(Layout::a(k), -1.0)], 0.0);
        rows.row(&[(Layout::a(k) + 1, -1.0)], 0.0);
        cones.push(Cone::Soc(3));
    }
    for k in 1..=n {
        // The funnel onto the axis.
        rows.row(
            &[(Layout::r(k) + 2, -FUNNEL.1)],
            FUNNEL.0 - FUNNEL.1 * p.z_catch,
        );
        rows.row(&[(Layout::r(k), -1.0)], 0.0);
        rows.row(&[(Layout::r(k) + 1, -1.0)], 0.0);
        cones.push(Cone::Soc(3));
        // The side-speed cone.
        rows.row(
            &[(Layout::r(k) + 2, -SIDE_CONE.1)],
            SIDE_CONE.0 - SIDE_CONE.1 * p.z_catch,
        );
        rows.row(&[(Layout::v(k), -1.0)], 0.0);
        rows.row(&[(Layout::v(k) + 1, -1.0)], 0.0);
        cones.push(Cone::Soc(3));
    }
    let m = rows.b.len();
    let a = Csc::from_triplets(m, Layout::n(), &rows.ri, &rows.ci, &rows.vals);
    let mut q = vec![0.0; Layout::n()];
    let mut pd = vec![0.0; Layout::n()];
    for k in 0..n {
        q[Layout::sigma(k)] = dt;
    }
    for k in 0..n - 1 {
        for i in 0..3 {
            pd[Layout::j(k) + i] = SMOOTH;
        }
    }
    (pd, q, a, rows.b, cones)
}

impl ApproachPlanner {
    pub fn new() -> ApproachPlanner {
        ApproachPlanner {
            solver: None,
            setups: 0,
        }
    }

    /// How many times a solver was set up (the pattern analysed, the
    /// memory allocated): once, however many plans.
    pub fn setups(&self) -> usize {
        self.setups
    }

    /// Plan with the problem's final time.
    pub fn plan(&mut self, p: &ApproachProblem) -> Result<ApproachPlan, GuidanceError> {
        let ok = p.t_f.is_finite()
            && p.t_f > 0.0
            && p.a_max > p.a_min
            && p.a_min >= 0.0
            && p.r0.iter().chain(&p.v0).all(|c| c.is_finite());
        if !ok {
            return Err(GuidanceError::InvalidProblem("bad approach problem"));
        }
        let (pd, q, a, b, cones) = assemble(p);
        let settings = Settings {
            max_iter: 60,
            tol_feas: 1e-7,
            tol_gap_abs: 1e-7,
            tol_gap_rel: 1e-7,
            ..Settings::default()
        };
        let reuse = self.solver.as_ref().is_some_and(|s| s.accepts(&a));
        if reuse {
            let s = self.solver.as_mut().expect("checked");
            s.update(&pd, &q, &a, &b)
                .map_err(|e| GuidanceError::SolverFailed(e.into()))?;
        } else {
            self.setups += 1;
            self.solver = Some(
                Solver::new(&pd, &q, &a, &b, &cones, settings)
                    .map_err(|e| GuidanceError::SolverFailed(e.into()))?,
            );
        }
        let solver = self.solver.as_mut().expect("set up");
        match solver.solve() {
            Status::Solved => {}
            Status::MaxIterations | Status::Diverged => return Err(GuidanceError::Infeasible),
            other => return Err(GuidanceError::SolverFailed(format!("{other:?}"))),
        }
        let x = solver.x();
        let n = NODES;
        let get = |at: usize| -> Vec3 { [x[at], x[at + 1], x[at + 2]] };
        let cost = q.iter().zip(x).map(|(c, v)| c * v).sum::<f64>()
            + 0.5 * pd.iter().zip(x).map(|(c, v)| c * v * v).sum::<f64>();
        Ok(ApproachPlan {
            t_f: p.t_f,
            r: (0..=n).map(|k| get(Layout::r(k))).collect(),
            v: (0..=n).map(|k| get(Layout::v(k))).collect(),
            a: (0..n).map(|k| get(Layout::a(k))).collect(),
            g: p.g,
            cost,
            iterations: solver.iterations(),
        })
    }

    /// Plan with each of `times` as the final time; keep the cheapest that
    /// solves.
    pub fn plan_free(
        &mut self,
        p: &ApproachProblem,
        times: &[f64],
    ) -> Result<ApproachPlan, GuidanceError> {
        let mut best: Option<ApproachPlan> = None;
        let mut last = GuidanceError::Infeasible;
        for &t in times {
            let mut q = p.clone();
            q.t_f = t;
            match self.plan(&q) {
                Ok(plan) if best.as_ref().is_none_or(|b| plan.cost < b.cost) => best = Some(plan),
                Ok(_) => {}
                Err(e) => last = e,
            }
        }
        best.ok_or(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: f64 = 9.81;

    /// The ship at its gate: 10 m out, 20 m above the caught position,
    /// moving in at 1 m/s and down at 2 m/s; about 120 t on one engine.
    fn ship_gate() -> ApproachProblem {
        let z_catch = 94.0;
        ApproachProblem {
            r0: [10.0, 0.0, z_catch + 20.0],
            v0: [-1.0, 0.0, -2.0],
            z_catch,
            t_f: 9.0,
            a_min: 0.4 * 2.45e6 / 120e3 * 1.05,
            a_max: 2.45e6 / 120e3 * 0.9,
            keep_x: -9.5 + 4.5 + 1.6,
            base_arm: 24.0,
            nose_arm: 28.0,
            g: G,
        }
    }

    fn check(p: &ApproachProblem, plan: &ApproachPlan) {
        let n = plan.a.len();
        assert_eq!(plan.r.len(), n + 1);
        let end = plan.r[n];
        assert!(
            end[0].hypot(end[1]) < 1e-3 && (end[2] - p.z_catch).abs() < 1e-3,
            "end {end:?}"
        );
        let ve = plan.v[n];
        assert!(
            ve[0].hypot(ve[1]) < 1e-3 && (ve[2] + SINK_AT_CONTACT).abs() < 1e-3,
            "end v {ve:?}"
        );
        for k in 0..n {
            let a = plan.a[k];
            let mag = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
            assert!(mag <= p.a_max * (1.0 + 1e-5), "node {k}: |a| {mag}");
            assert!(a[2] >= p.a_min * (1.0 - 1e-5), "node {k}: a_z {}", a[2]);
            assert!(
                a[0].hypot(a[1]) <= TILT.tan() * a[2] * (1.0 + 1e-5) + 1e-6,
                "node {k}: tilt"
            );
            let r = plan.r[k];
            let base = r[0] - p.base_arm * a[0] / p.g;
            let nose = r[0] + p.nose_arm * a[0] / p.g;
            assert!(
                base >= p.keep_x - 1e-5 && nose >= p.keep_x - 1e-5,
                "node {k}: base {base} nose {nose}"
            );
        }
        for k in 1..=n {
            let (r, v) = (plan.r[k], plan.v[k]);
            let h = r[2] - p.z_catch;
            assert!(
                r[0].hypot(r[1]) <= FUNNEL.0 + FUNNEL.1 * h + 1e-5,
                "node {k}: funnel"
            );
            assert!(
                v[2] >= -(SINK_CONE.0 + SINK_CONE.1 * h) - 1e-5,
                "node {k}: sink {}",
                v[2]
            );
            assert!(
                v[0].hypot(v[1]) <= SIDE_CONE.0 + SIDE_CONE.1 * h + 1e-5,
                "node {k}: side"
            );
        }
    }

    #[test]
    fn reaches_the_arms_from_the_gate() {
        let p = ship_gate();
        let mut planner = ApproachPlanner::new();
        let plan = planner.plan(&p).expect("plan");
        let end = plan.r[plan.a.len()];
        eprintln!("ends at {end:?}, {} s", plan.t_f);
        assert!(end[0].hypot(end[1]) < 1e-3 && (end[2] - p.z_catch).abs() < 1e-3);
        assert_eq!(plan.t_f, p.t_f);
    }

    #[test]
    fn respects_every_constraint() {
        let p = ship_gate();
        let plan = ApproachPlanner::new().plan(&p).expect("plan");
        check(&p, &plan);
    }

    #[test]
    fn plans_from_off_to_the_side() {
        let mut p = ship_gate();
        p.r0 = [9.0, 8.0, p.z_catch + 20.0];
        p.v0 = [-1.0, -0.5, -2.0];
        let plan = ApproachPlanner::new().plan(&p).expect("plan");
        check(&p, &plan);
    }

    /// Already at the mast's side and sliding toward it fast, low down:
    /// no plan keeps the base clear, and the planner says so.
    #[test]
    fn refuses_a_mast_collision_course() {
        let mut p = ship_gate();
        p.r0 = [-2.0, 0.0, p.z_catch + 4.0];
        p.v0 = [-6.0, 0.0, -2.0];
        p.t_f = 3.0;
        assert!(ApproachPlanner::new().plan(&p).is_err());
    }

    #[test]
    fn reuses_its_set_up_solver() {
        let mut planner = ApproachPlanner::new();
        let mut p = ship_gate();
        planner.plan(&p).expect("plan");
        p.r0[0] = 9.0;
        p.t_f = 8.5;
        planner.plan(&p).expect("second plan");
        assert_eq!(planner.setups(), 1);
    }

    #[test]
    fn free_time_keeps_the_cheapest() {
        let p = ship_gate();
        let mut planner = ApproachPlanner::new();
        let best = planner.plan_free(&p, &FINAL_TIMES).expect("plan");
        for &t in &FINAL_TIMES {
            let mut q = p.clone();
            q.t_f = t;
            if let Ok(other) = planner.plan(&q) {
                assert!(
                    best.cost <= other.cost + 1e-6,
                    "{} s ({}) beats {} s ({})",
                    t,
                    other.cost,
                    best.t_f,
                    best.cost
                );
            }
        }
        check(&p, &best);
    }

    #[test]
    fn sample_interpolates_the_plan() {
        let p = ship_gate();
        let plan = ApproachPlanner::new().plan(&p).expect("plan");
        let (r, v, _) = plan.sample(0.0);
        assert!((r[0] - p.r0[0]).abs() < 1e-6 && (v[2] - p.v0[2]).abs() < 1e-6);
        let (r, _, _) = plan.sample(1e9);
        assert!((r[2] - p.z_catch).abs() < 1e-3);
    }
}
