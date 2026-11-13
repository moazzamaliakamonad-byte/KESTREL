//! Assemble the lossless-convexified landing problem as a second-order cone
//! program for Clarabel, for one fixed final time.
//!
//! Formulation: Açıkmeşe & Ploen (2007); see also Malyuta et al. (2021),
//! arXiv:2106.09125. Everything here is non-dimensional (see `scaling`).
//!
//! Variables, for N intervals:
//! - per node k = 0..=N: position `r_k` (3), velocity `v_k` (3),
//!   log-mass `z_k = ln(m_k / wet_mass)` (1)
//! - per interval k = 0..N: thrust acceleration `u_k = T_k / m_k` (3) and
//!   its slack magnitude `σ_k ≥ ‖u_k‖` (1)

// Index loops mirror the maths (k indexes several arrays at once).
#![allow(clippy::needless_range_loop)]

use std::ops::Range;

use clarabel::algebra::CscMatrix;
use clarabel::solver::SupportedConeT;

use crate::problem::{GuidanceError, LandingProblem};
use crate::scaling::Scaling;

/// Where each variable lives in the solver's flat vector `x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Layout {
    /// Number of intervals N.
    pub n: usize,
}

impl Layout {
    fn nodes(&self) -> usize {
        self.n + 1
    }
    /// Position component `i` at node `k`.
    pub fn r(&self, k: usize, i: usize) -> usize {
        3 * k + i
    }
    /// Velocity component `i` at node `k`.
    pub fn v(&self, k: usize, i: usize) -> usize {
        3 * self.nodes() + 3 * k + i
    }
    /// Log-mass at node `k`.
    pub fn z(&self, k: usize) -> usize {
        6 * self.nodes() + k
    }
    /// Thrust-acceleration component `i` on interval `k`.
    pub fn u(&self, k: usize, i: usize) -> usize {
        7 * self.nodes() + 3 * k + i
    }
    /// Thrust-acceleration magnitude slack on interval `k`.
    pub fn sigma(&self, k: usize) -> usize {
        7 * self.nodes() + 3 * self.n + k
    }
    /// Total number of variables.
    pub fn len(&self) -> usize {
        7 * self.nodes() + 4 * self.n
    }
}

/// The assembled problem in Clarabel's form:
/// minimize `½xᵀPx + qᵀx` subject to `A·x + s = b`, `s ∈ cones`.
pub(crate) struct Socp {
    pub p: CscMatrix<f64>,
    pub q: Vec<f64>,
    pub a: CscMatrix<f64>,
    pub b: Vec<f64>,
    pub cones: Vec<SupportedConeT<f64>>,
    pub layout: Layout,
    pub scaling: Scaling,
    /// Non-dimensional interval length.
    pub dt_nd: f64,
    /// Rows of the dynamics equalities (read by tests only).
    #[cfg_attr(not(test), allow(dead_code))]
    pub dynamics_rows: Range<usize>,
}

/// An affine expression `Σ coeff·x[index] + constant`.
#[derive(Default)]
struct Aff {
    terms: Vec<(usize, f64)>,
    constant: f64,
}

impl Aff {
    fn new(terms: &[(usize, f64)], constant: f64) -> Aff {
        Aff {
            terms: terms.to_vec(),
            constant,
        }
    }
}

/// Collects constraint rows. Clarabel's convention is `s = b − A·x ∈ K`.
#[derive(Default)]
struct Rows {
    rows: Vec<usize>,
    cols: Vec<usize>,
    vals: Vec<f64>,
    b: Vec<f64>,
}

impl Rows {
    fn len(&self) -> usize {
        self.b.len()
    }

    /// Append a row with `s = expr` (so `A = −terms`, `b = constant`).
    fn push_slack(&mut self, expr: &Aff) {
        let row = self.len();
        for &(col, val) in &expr.terms {
            self.rows.push(row);
            self.cols.push(col);
            self.vals.push(-val);
        }
        self.b.push(expr.constant);
    }

    /// `expr = 0` (zero cone: `s = 0`, equivalently `s = expr`).
    fn eq(&mut self, expr: Aff) {
        self.push_slack(&expr);
    }

    /// `expr ≥ 0` (nonnegative cone).
    fn nonneg(&mut self, expr: Aff) {
        self.push_slack(&expr);
    }

    /// `parts[0] ≥ ‖parts[1..]‖` (second-order cone).
    fn soc(&mut self, parts: &[Aff]) {
        for part in parts {
            self.push_slack(part);
        }
    }
}

/// Where the landing must end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Target {
    /// At rest on the pad, inside the glide cone the whole way.
    Pinpoint,
    /// At rest on the ground as close to the pad as possible: the end point's
    /// horizontal position is free and the glide cone is dropped. Objective
    /// `W·e − z_N` with `e ≥ ‖r_N,xy‖`: distance first, then fuel.
    Closest,
}

/// Weight of the (non-dimensional) miss distance against final log-mass.
/// One unit of miss is the whole start distance, so any real miss
/// outweighs the fuel term.
const MISS_WEIGHT: f64 = 10.0;

/// Build the SOCP for one fixed final time `final_time` (seconds).
#[cfg(test)]
pub(crate) fn build(problem: &LandingProblem, final_time: f64) -> Result<Socp, GuidanceError> {
    build_with(problem, final_time, true, Target::Pinpoint)
}

/// As [`build`], optionally leaving out the final dry-mass constraint
/// (used by the landing-time search; see `solve::solve`), for either target.
pub(crate) fn build_with(
    problem: &LandingProblem,
    final_time: f64,
    enforce_dry_mass: bool,
    target: Target,
) -> Result<Socp, GuidanceError> {
    let closest = target == Target::Closest;
    if closest {
        problem.validate_state()?;
    } else {
        problem.validate()?;
    }
    if !(final_time.is_finite() && final_time > 0.0) {
        return Err(GuidanceError::InvalidProblem("final time not positive"));
    }

    let veh = &problem.vehicle;
    let [rx, ry, rz] = problem.position;
    let length = (rx * rx + ry * ry + rz * rz).sqrt().max(1.0);
    let sc = Scaling::new(length, veh.gravity, veh.wet_mass);

    let n = problem.steps;
    let lay = Layout { n };
    let dt = sc.to_nd_time(final_time) / n as f64;
    let alpha = sc.nd_alpha(veh.alpha());
    let rho1 = sc.to_nd_force(veh.min_thrust);
    let rho2 = sc.to_nd_force(veh.max_thrust);
    let g = [0.0, 0.0, -1.0];

    // Reference log-mass profiles: lightest possible (full thrust from t = 0)
    // and heaviest possible (minimum thrust from t = 0).
    let mut z_lo = Vec::with_capacity(n + 1);
    let mut z_hi = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let t = k as f64 * dt;
        let lightest = 1.0 - alpha * rho2 * t;
        if lightest <= 0.0 {
            return Err(GuidanceError::InvalidProblem(
                "final time exceeds propellant",
            ));
        }
        z_lo.push(lightest.ln());
        z_hi.push((1.0 - alpha * rho1 * t).ln());
    }

    let r0 = problem.position.map(|c| sc.to_nd_length(c));
    let v0 = problem.velocity.map(|c| sc.to_nd_velocity(c));

    // ---- Zero cone: equalities ----
    let mut eq = Rows::default();
    for i in 0..3 {
        eq.eq(Aff::new(&[(lay.r(0, i), 1.0)], -r0[i]));
        eq.eq(Aff::new(&[(lay.v(0, i), 1.0)], -v0[i]));
    }
    eq.eq(Aff::new(&[(lay.z(0), 1.0)], 0.0));

    let dyn_start = eq.len();
    for k in 0..n {
        for i in 0..3 {
            // r_{k+1} − r_k − dt·v_k − ½dt²·u_k − ½dt²·g = 0
            eq.eq(Aff::new(
                &[
                    (lay.r(k + 1, i), 1.0),
                    (lay.r(k, i), -1.0),
                    (lay.v(k, i), -dt),
                    (lay.u(k, i), -0.5 * dt * dt),
                ],
                -0.5 * dt * dt * g[i],
            ));
            // v_{k+1} − v_k − dt·u_k − dt·g = 0
            eq.eq(Aff::new(
                &[
                    (lay.v(k + 1, i), 1.0),
                    (lay.v(k, i), -1.0),
                    (lay.u(k, i), -dt),
                ],
                -dt * g[i],
            ));
        }
        // z_{k+1} − z_k + α'·dt·σ_k = 0
        eq.eq(Aff::new(
            &[
                (lay.z(k + 1), 1.0),
                (lay.z(k), -1.0),
                (lay.sigma(k), alpha * dt),
            ],
            0.0,
        ));
    }
    let dyn_end = eq.len();

    for i in 0..3 {
        // Closest landing: only the altitude of the end point is fixed.
        if !closest || i == 2 {
            eq.eq(Aff::new(&[(lay.r(n, i), 1.0)], 0.0));
        }
        eq.eq(Aff::new(&[(lay.v(n, i), 1.0)], 0.0));
    }
    // Miss-distance epigraph variable, after the layout's own variables.
    let miss = lay.len();

    // ---- Nonnegative cone: inequalities ----
    let mut ineq = Rows::default();
    if enforce_dry_mass {
        let z_dry = (veh.dry_mass / veh.wet_mass).ln();
        ineq.nonneg(Aff::new(&[(lay.z(n), 1.0)], -z_dry));
    }
    let cos_tilt = veh.max_tilt.cos();
    for k in 0..n {
        // z_lo ≤ z_k ≤ z_hi
        ineq.nonneg(Aff::new(&[(lay.z(k), 1.0)], -z_lo[k]));
        ineq.nonneg(Aff::new(&[(lay.z(k), -1.0)], z_hi[k]));
        // σ_k ≤ ρ₂·e^{−z_lo}·(1 − (z_k − z_lo))   [tangent line: conservative]
        let mu2 = rho2 * (-z_lo[k]).exp();
        ineq.nonneg(Aff::new(
            &[(lay.sigma(k), -1.0), (lay.z(k), -mu2)],
            mu2 * (1.0 + z_lo[k]),
        ));
        // u_k,z ≥ σ_k·cos θ
        ineq.nonneg(Aff::new(
            &[(lay.u(k, 2), 1.0), (lay.sigma(k), -cos_tilt)],
            0.0,
        ));
    }
    for k in 0..=n {
        ineq.nonneg(Aff::new(&[(lay.r(k, 2), 1.0)], 0.0));
    }

    // ---- Second-order cones ----
    let mut soc = Rows::default();
    let mut soc_dims = Vec::new();
    for k in 0..n {
        // ‖u_k‖ ≤ σ_k
        soc.soc(&[
            Aff::new(&[(lay.sigma(k), 1.0)], 0.0),
            Aff::new(&[(lay.u(k, 0), 1.0)], 0.0),
            Aff::new(&[(lay.u(k, 1), 1.0)], 0.0),
            Aff::new(&[(lay.u(k, 2), 1.0)], 0.0),
        ]);
        soc_dims.push(4);
    }
    let tan_gs = veh.glide_slope.tan();
    if closest {
        // e ≥ ‖(r_N,x, r_N,y)‖
        soc.soc(&[
            Aff::new(&[(miss, 1.0)], 0.0),
            Aff::new(&[(lay.r(n, 0), 1.0)], 0.0),
            Aff::new(&[(lay.r(n, 1), 1.0)], 0.0),
        ]);
        soc_dims.push(3);
    } else if tan_gs > 0.0 {
        for k in 0..=n {
            // tan γ·‖(r_x, r_y)‖ ≤ r_z
            soc.soc(&[
                Aff::new(&[(lay.r(k, 2), 1.0)], 0.0),
                Aff::new(&[(lay.r(k, 0), tan_gs)], 0.0),
                Aff::new(&[(lay.r(k, 1), tan_gs)], 0.0),
            ]);
            soc_dims.push(3);
        }
    }
    for k in 0..n {
        // σ_k ≥ μ(1 − δ + δ²/2), δ = z_k − z_lo   [2nd-order Taylor: conservative]
        // ⇔ (μ/2)δ² ≤ w,  w = σ_k + μ·z_k − μ(1 + z_lo)
        // ⇔ ‖(2·√(μ/2)·δ, w − 1)‖ ≤ w + 1
        let mu1 = rho1 * (-z_lo[k]).exp();
        let a = (mu1 / 2.0).sqrt();
        let w_const = -mu1 * (1.0 + z_lo[k]);
        let w_terms = [(lay.sigma(k), 1.0), (lay.z(k), mu1)];
        soc.soc(&[
            Aff::new(&w_terms, w_const + 1.0),
            Aff::new(&[(lay.z(k), 2.0 * a)], -2.0 * a * z_lo[k]),
            Aff::new(&w_terms, w_const - 1.0),
        ]);
        soc_dims.push(3);
    }

    // ---- Stack the three blocks into one A, b ----
    let mut rows = Vec::new();
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    let mut b = Vec::new();
    for block in [&eq, &ineq, &soc] {
        let offset = b.len();
        rows.extend(block.rows.iter().map(|r| r + offset));
        cols.extend_from_slice(&block.cols);
        vals.extend_from_slice(&block.vals);
        b.extend_from_slice(&block.b);
    }
    let nvars = if closest { lay.len() + 1 } else { lay.len() };
    let a = CscMatrix::new_from_triplets(b.len(), nvars, rows, cols, vals);

    let mut cones = vec![
        SupportedConeT::ZeroConeT(eq.len()),
        SupportedConeT::NonnegativeConeT(ineq.len()),
    ];
    cones.extend(soc_dims.into_iter().map(SupportedConeT::SecondOrderConeT));

    // Objective: maximize final log-mass ⇔ minimize −z_N.
    let mut q = vec![0.0; nvars];
    q[lay.z(n)] = -1.0;
    if closest {
        q[miss] = MISS_WEIGHT;
    }

    Ok(Socp {
        p: CscMatrix::zeros((nvars, nvars)),
        q,
        a,
        b,
        cones,
        layout: lay,
        scaling: sc,
        dt_nd: dt,
        dynamics_rows: dyn_start..dyn_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problem::LandingProblem;

    fn cone_dim(c: &SupportedConeT<f64>) -> usize {
        match c {
            SupportedConeT::ZeroConeT(d)
            | SupportedConeT::NonnegativeConeT(d)
            | SupportedConeT::SecondOrderConeT(d) => *d,
            other => panic!("unexpected cone {other:?}"),
        }
    }

    /// y = A·x for a CSC matrix.
    fn mat_vec(a: &CscMatrix<f64>, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; a.m];
        for col in 0..a.n {
            for idx in a.colptr[col]..a.colptr[col + 1] {
                y[a.rowval[idx]] += a.nzval[idx] * x[col];
            }
        }
        y
    }

    #[test]
    fn layout_indices_are_unique_and_dense() {
        let l = Layout { n: 3 };
        let mut all = Vec::new();
        for k in 0..=3 {
            for i in 0..3 {
                all.push(l.r(k, i));
                all.push(l.v(k, i));
            }
            all.push(l.z(k));
        }
        for k in 0..3 {
            for i in 0..3 {
                all.push(l.u(k, i));
            }
            all.push(l.sigma(k));
        }
        all.sort_unstable();
        assert_eq!(all, (0..l.len()).collect::<Vec<_>>());
    }

    #[test]
    fn dimensions_match_cones() {
        let p = LandingProblem::example();
        let s = build(&p, 50.0).expect("example should build");
        let total: usize = s.cones.iter().map(cone_dim).sum();
        assert_eq!(total, s.b.len());
        assert_eq!(s.a.m, s.b.len());
        assert_eq!(s.a.n, s.layout.len());
        assert_eq!(s.q.len(), s.layout.len());
    }

    #[test]
    fn rejects_final_time_beyond_burnout() {
        let p = LandingProblem::example();
        let v = &p.vehicle;
        let tf = 10.0 * (v.wet_mass - v.dry_mass) / (v.alpha() * v.min_thrust);
        assert_eq!(
            build(&p, tf).err(),
            Some(GuidanceError::InvalidProblem(
                "final time exceeds propellant"
            ))
        );
    }

    #[test]
    fn free_fall_satisfies_dynamics_rows() {
        // With zero thrust, propagate the same constant-acceleration formulas
        // by hand; every dynamics equality row must then hold exactly.
        let p = LandingProblem::example();
        let s = build(&p, 50.0).unwrap();
        let (l, sc, dt) = (s.layout, s.scaling, s.dt_nd);
        let g = [0.0, 0.0, -1.0];
        let mut x = vec![0.0; l.len()];
        let mut r = p.position.map(|c| sc.to_nd_length(c));
        let mut v = p.velocity.map(|c| sc.to_nd_velocity(c));
        for k in 0..=l.n {
            for i in 0..3 {
                x[l.r(k, i)] = r[i];
                x[l.v(k, i)] = v[i];
            }
            for i in 0..3 {
                r[i] += dt * v[i] + 0.5 * dt * dt * g[i];
                v[i] += dt * g[i];
            }
        }
        let ax = mat_vec(&s.a, &x);
        assert!(!s.dynamics_rows.is_empty());
        for row in s.dynamics_rows.clone() {
            assert!(
                (ax[row] - s.b[row]).abs() <= 1e-12,
                "row {row}: A·x = {} but b = {}",
                ax[row],
                s.b[row]
            );
        }
    }
}
