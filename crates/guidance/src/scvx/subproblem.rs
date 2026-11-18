//! One convex subproblem of successive convexification, as a Clarabel SOCP.
//!
//! Variables (non-dimensional), for K nodes:
//! `x_k` (14 each), `u_k` (3 each), `σ`, virtual control `ν_k` (14 per
//! interval) and its absolute-value slack `s_k`.
//!
//! Minimize `−m_K + w_ν Σ s + w_tr (‖x − x̄‖² + ‖u − ū‖² + (σ − σ̄)²)`.

#![allow(clippy::needless_range_loop)]

use clarabel::algebra::CscMatrix;
use clarabel::solver::{DefaultSettings, DefaultSolver, IPSolver, SolverStatus, SupportedConeT};
use model::dynamics::{M, Q, R, V, W};
use model::{State, Vec3};

use super::discretize::{Interval, NU, NX};
use crate::GuidanceError;

/// Everything the subproblem needs, non-dimensional.
pub struct Sub<'a> {
    pub xs: &'a [State],
    pub us: &'a [Vec3],
    pub sigma: f64,
    pub ivs: &'a [Interval],
    pub x0: State,
    pub gate_r: Vec3,
    pub gate_v: Vec3,
    pub m_dry: f64,
    /// Thrust magnitude bounds per node.
    pub t_bounds: &'a [(f64, f64)],
    pub tan_gimbal: f64,
    pub w_max: f64,
    pub floor_z: f64,
    pub clearance_x: Option<f64>,
    /// Approach cone around the gate axis: horizontal distance
    /// `≤ r0 + tan_gamma · (z − gate_z)` at every node after the first.
    pub approach: (f64, f64),
    /// From this node on, tilt ≤ θ, as `‖(q_x, q_y)‖ ≤ sin(θ/2)`.
    pub upright_from: usize,
    pub upright_sin_half: f64,
    /// sin(half the largest tilt) allowed from node `tilt_from` on (soft), if any.
    pub tilt_sin_half: Option<f64>,
    pub tilt_from: usize,
    /// Sideways speed `‖v_xy‖ ≤ c0 + c1 · (z − gate_z)` from node 2 on.
    pub side_speed: (f64, f64),
    pub sigma_bounds: (f64, f64),
    pub w_virtual: f64,
    pub w_trust: f64,
    pub w_trust_sigma: f64,
    /// Interior-point tolerance (gap and feasibility).
    pub tolerance: f64,
}

/// Price of loosening a comfort constraint by one non-dimensional unit.
const W_SOFT: f64 = 100.0;

pub struct SubSolution {
    pub xs: Vec<State>,
    pub us: Vec<Vec3>,
    pub sigma: f64,
    /// Σ|ν| (non-dimensional).
    pub virtual_l1: f64,
}

struct Layout {
    k: usize,
}

impl Layout {
    fn x(&self, n: usize, i: usize) -> usize {
        n * NX + i
    }
    fn u(&self, n: usize, i: usize) -> usize {
        self.k * NX + n * NU + i
    }
    fn sigma(&self) -> usize {
        self.k * (NX + NU)
    }
    fn nu(&self, n: usize, i: usize) -> usize {
        self.sigma() + 1 + n * NX + i
    }
    fn slack(&self, n: usize, i: usize) -> usize {
        self.nu(self.k - 1, 0) + n * NX + i
    }
    /// Softening slack of the comfort constraints at node `n`.
    fn soft(&self, n: usize) -> usize {
        self.slack(self.k - 1, 0) + n
    }
    fn len(&self) -> usize {
        self.soft(self.k)
    }
}

/// Rows of one cone block. Each row is `expr = Σ c·x + const` and the
/// solver sees `s = b − A x` with `A = −c`, `b = const`.
#[derive(Default)]
struct Block {
    rows: Vec<(Vec<(usize, f64)>, f64)>,
}

impl Block {
    fn push(&mut self, terms: Vec<(usize, f64)>, constant: f64) {
        self.rows.push((terms, constant));
    }
}

/// Which interior-point solver answers the subproblems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `kestrel-socp`: allocation-free iterations, bounded iteration count.
    Kestrel,
    /// Clarabel, the reference.
    Clarabel,
}

/// 0: not chosen yet (read `KESTREL_SOLVER` once), 1: Clarabel, 2: Kestrel.
static BACKEND: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The solver answering subproblems: Clarabel by default (faster on the
/// booster's plans, D-041); `set_backend` or `KESTREL_SOLVER=kestrel`
/// (native) chooses Kestrel's own.
pub fn backend() -> Backend {
    use std::sync::atomic::Ordering;
    match BACKEND.load(Ordering::Relaxed) {
        1 => Backend::Clarabel,
        2 => Backend::Kestrel,
        _ => {
            let b = match std::env::var("KESTREL_SOLVER").as_deref() {
                Ok("kestrel") => Backend::Kestrel,
                _ => Backend::Clarabel,
            };
            set_backend(b);
            b
        }
    }
}

/// Choose the solver for every later subproblem (this process / worker).
pub fn set_backend(b: Backend) {
    let v = match b {
        Backend::Clarabel => 1,
        Backend::Kestrel => 2,
    };
    BACKEND.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Most interior-point iterations per subproblem with `kestrel-socp`; with
/// its fixed work per iteration this bounds the subproblem's run time.
pub const IPM_ITERATIONS: usize = 40;
/// Iterative refinement steps per KKT solve.
pub const IPM_REFINE: usize = 1;
/// Set-up solvers kept per thread, one per problem shape (ship with and
/// without an engine switch, booster legs).
const CACHED_SOLVERS: usize = 6;

thread_local! {
    /// Solvers already set up (ordering, symbolic factorization, every
    /// buffer), reused for any subproblem with the same pattern and
    /// settings: after the first plan of each shape nothing is allocated
    /// inside the solver.
    static SOLVERS: std::cell::RefCell<Vec<(socp::Settings, socp::Solver)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn solve_kestrel(
    pdiag: &[f64],
    q: &[f64],
    a: &socp::Csc,
    b: &[f64],
    cones: &[socp::Cone],
    settings: socp::Settings,
) -> Result<Vec<f64>, GuidanceError> {
    SOLVERS.with(|cell| {
        let mut list = cell.borrow_mut();
        let found = list
            .iter()
            .position(|(st, s)| *st == settings && s.accepts(a));
        let trace = std::env::var_os("SOCP_TRACE").is_some();
        // (No clock in WebAssembly: time only when tracing natively.)
        let clock = trace.then(std::time::Instant::now);
        let i = match found {
            Some(i) => {
                list[i]
                    .1
                    .update(pdiag, q, a, b)
                    .map_err(|e| GuidanceError::SolverFailed(e.into()))?;
                i
            }
            None => {
                let s = socp::Solver::new(pdiag, q, a, b, cones, settings)
                    .map_err(|e| GuidanceError::SolverFailed(format!("setup: {e}")))?;
                if list.len() >= CACHED_SOLVERS {
                    list.remove(0);
                }
                list.push((settings, s));
                list.len() - 1
            }
        };
        let setup = clock.map(|c| c.elapsed().as_secs_f64() * 1e3);
        let clock = trace.then(std::time::Instant::now);
        let s = &mut list[i].1;
        let st = s.solve();
        if let (Some(setup), Some(c)) = (setup, clock) {
            let (kn, knz, lnz) = s.sizes();
            eprintln!(
                "socp {st:?} it {} reused {} kkt {kn} nnz {knz} L {lnz} setup {setup:.2} ms solve {:.2} ms",
                s.iterations(),
                found.is_some(),
                c.elapsed().as_secs_f64() * 1e3
            );
        }
        match st {
            socp::Status::Solved => Ok(s.x().to_vec()),
            socp::Status::Diverged => Err(GuidanceError::Infeasible),
            other => Err(GuidanceError::SolverFailed(format!("{other:?}"))),
        }
    })
}

pub fn solve(p: &Sub) -> Result<SubSolution, GuidanceError> {
    let k = p.xs.len();
    let l = Layout { k };
    let n = l.len();

    let mut zero = Block::default();
    let mut nonneg = Block::default();
    let mut socs: Vec<Block> = Vec::new();

    // Dynamics: x_{k+1} − A x_k − B⁻u_k − B⁺u_{k+1} − Sσ − ν_k − z_k = 0.
    for (m, iv) in p.ivs.iter().enumerate() {
        for i in 0..NX {
            let mut t = vec![
                (l.x(m + 1, i), 1.0),
                (l.nu(m, i), -1.0),
                (l.sigma(), -iv.s[i]),
            ];
            for j in 0..NX {
                if iv.a[i][j] != 0.0 {
                    t.push((l.x(m, j), -iv.a[i][j]));
                }
            }
            for j in 0..NU {
                t.push((l.u(m, j), -iv.bm[i][j]));
                t.push((l.u(m + 1, j), -iv.bp[i][j]));
            }
            zero.push(t, -iv.z[i]);
        }
    }
    // Initial state.
    for i in 0..NX {
        zero.push(vec![(l.x(0, i), 1.0)], -p.x0[i]);
    }
    // Final gate: position, velocity, upright (q_x = q_y = 0), not turning
    // in pitch or yaw. Roll rate is left free: the gimbal makes no roll
    // torque, so a measured roll rate would make the gate unreachable.
    let last = k - 1;
    for i in 0..3 {
        zero.push(vec![(l.x(last, R + i), 1.0)], -p.gate_r[i]);
        zero.push(vec![(l.x(last, V + i), 1.0)], -p.gate_v[i]);
        if i < 2 {
            zero.push(vec![(l.x(last, W + i), 1.0)], 0.0);
        }
    }
    zero.push(vec![(l.x(last, Q + 1), 1.0)], 0.0);
    zero.push(vec![(l.x(last, Q + 2), 1.0)], 0.0);

    // |ν| ≤ s.
    for m in 0..k - 1 {
        for i in 0..NX {
            nonneg.push(vec![(l.slack(m, i), 1.0), (l.nu(m, i), -1.0)], 0.0);
            nonneg.push(vec![(l.slack(m, i), 1.0), (l.nu(m, i), 1.0)], 0.0);
        }
    }
    for m in 0..k {
        nonneg.push(vec![(l.x(m, M), 1.0)], -p.m_dry);
        if m > 0 {
            nonneg.push(vec![(l.x(m, R + 2), 1.0)], -p.floor_z);
        }
        if let Some(cx) = p.clearance_x {
            nonneg.push(vec![(l.x(m, R), 1.0)], -cx);
        }
        // Lower thrust bound, linearized around the reference direction.
        let ub = p.us[m];
        let nb = (ub[0] * ub[0] + ub[1] * ub[1] + ub[2] * ub[2]).sqrt();
        let dir = if nb > 1e-9 {
            ub.map(|c| c / nb)
        } else {
            [0.0, 0.0, 1.0]
        };
        nonneg.push(
            (0..3).map(|j| (l.u(m, j), dir[j])).collect(),
            -p.t_bounds[m].0,
        );
    }
    for m in 0..k {
        nonneg.push(vec![(l.soft(m), 1.0)], 0.0);
    }
    nonneg.push(vec![(l.sigma(), 1.0)], -p.sigma_bounds.0);
    nonneg.push(vec![(l.sigma(), -1.0)], p.sigma_bounds.1);

    for m in 0..k {
        // ‖u‖ ≤ T_max
        let mut b = Block::default();
        b.push(vec![], p.t_bounds[m].1);
        for j in 0..3 {
            b.push(vec![(l.u(m, j), 1.0)], 0.0);
        }
        socs.push(b);
        // ‖(u_x, u_y)‖ ≤ tan(δ) u_z
        let mut b = Block::default();
        b.push(vec![(l.u(m, 2), p.tan_gimbal)], 0.0);
        b.push(vec![(l.u(m, 0), 1.0)], 0.0);
        b.push(vec![(l.u(m, 1), 1.0)], 0.0);
        socs.push(b);
        // ‖ω‖ ≤ ω_max, not at the measured first node, nor the next (the
        // ship may be turning faster than the limit when measured).
        if m == 0 {
            continue;
        }
        // Comfort constraints (approach cone, sideways speed, upright) are
        // soft: each may be loosened by the node's slack, at a high price,
        // so a ship pushed off its plan still gets a plan back.
        let (r0, tg) = p.approach;
        let mut b = Block::default();
        b.push(
            vec![(l.x(m, R + 2), tg), (l.soft(m), 1.0)],
            r0 - tg * p.gate_r[2],
        );
        b.push(vec![(l.x(m, R), 1.0)], -p.gate_r[0]);
        b.push(vec![(l.x(m, R + 1), 1.0)], -p.gate_r[1]);
        socs.push(b);
        if m >= 2 {
            let (c0, c1) = p.side_speed;
            let mut b = Block::default();
            b.push(
                vec![(l.x(m, R + 2), c1), (l.soft(m), 1.0)],
                c0 - c1 * p.gate_r[2],
            );
            b.push(vec![(l.x(m, V), 1.0)], 0.0);
            b.push(vec![(l.x(m, V + 1), 1.0)], 0.0);
            socs.push(b);
        }
        if let Some(s) = p
            .tilt_sin_half
            .filter(|_| m >= p.tilt_from && m < p.upright_from)
        {
            let mut b = Block::default();
            b.push(vec![(l.soft(m), 0.1)], s);
            b.push(vec![(l.x(m, Q + 1), 1.0)], 0.0);
            b.push(vec![(l.x(m, Q + 2), 1.0)], 0.0);
            socs.push(b);
        }
        if m >= p.upright_from {
            let mut b = Block::default();
            b.push(vec![(l.soft(m), 0.1)], p.upright_sin_half);
            b.push(vec![(l.x(m, Q + 1), 1.0)], 0.0);
            b.push(vec![(l.x(m, Q + 2), 1.0)], 0.0);
            socs.push(b);
        }
        if m == 1 {
            continue;
        }
        let mut b = Block::default();
        b.push(vec![], p.w_max);
        for j in 0..3 {
            b.push(vec![(l.x(m, W + j), 1.0)], 0.0);
        }
        socs.push(b);
    }

    // Objective.
    let mut q = vec![0.0; n];
    q[l.x(last, M)] = -1.0;
    for m in 0..k - 1 {
        for i in 0..NX {
            q[l.slack(m, i)] = p.w_virtual;
        }
    }
    for m in 0..k {
        q[l.soft(m)] = W_SOFT;
    }
    let mut pdiag = vec![0.0; n];
    let w = p.w_trust;
    for m in 0..k {
        for i in 0..NX {
            pdiag[l.x(m, i)] = 2.0 * w;
            q[l.x(m, i)] += -2.0 * w * p.xs[m][i];
        }
        for j in 0..NU {
            pdiag[l.u(m, j)] = 2.0 * w;
            q[l.u(m, j)] += -2.0 * w * p.us[m][j];
        }
    }
    pdiag[l.sigma()] = 2.0 * p.w_trust_sigma;
    q[l.sigma()] += -2.0 * p.w_trust_sigma * p.sigma;

    // Assemble A, b, cones in block order.
    let (mut ri, mut ci, mut vv, mut bv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut cones = Vec::new();
    let emit = |blk: &Block,
                ri: &mut Vec<usize>,
                ci: &mut Vec<usize>,
                vv: &mut Vec<f64>,
                bv: &mut Vec<f64>| {
        for (terms, c) in &blk.rows {
            let row = bv.len();
            for &(col, val) in terms {
                ri.push(row);
                ci.push(col);
                vv.push(-val);
            }
            bv.push(*c);
        }
    };
    emit(&zero, &mut ri, &mut ci, &mut vv, &mut bv);
    cones.push(SupportedConeT::ZeroConeT(zero.rows.len()));
    emit(&nonneg, &mut ri, &mut ci, &mut vv, &mut bv);
    cones.push(SupportedConeT::NonnegativeConeT(nonneg.rows.len()));
    for b in &socs {
        emit(b, &mut ri, &mut ci, &mut vv, &mut bv);
        cones.push(SupportedConeT::SecondOrderConeT(b.rows.len()));
    }
    let x = match backend() {
        Backend::Kestrel => {
            let a = socp::Csc::from_triplets(bv.len(), n, &ri, &ci, &vv);
            let clarabel_cones = &cones;
            let cones: Vec<socp::Cone> = cones
                .iter()
                .map(|c| match c {
                    SupportedConeT::ZeroConeT(k) => socp::Cone::Zero(*k),
                    SupportedConeT::NonnegativeConeT(k) => socp::Cone::Nonneg(*k),
                    SupportedConeT::SecondOrderConeT(k) => socp::Cone::Soc(*k),
                    _ => unreachable!("only these cones are built"),
                })
                .collect();
            let settings = socp::Settings {
                max_iter: IPM_ITERATIONS,
                tol_feas: p.tolerance,
                tol_gap_abs: p.tolerance,
                tol_gap_rel: p.tolerance,
                refine: IPM_REFINE,
                ..Default::default()
            };
            let mine = solve_kestrel(&pdiag, &q, &a, &bv, &cones, settings)?;
            if std::env::var_os("SOCP_COMPARE").is_some() {
                let obj = |x: &[f64]| -> f64 {
                    (0..x.len())
                        .map(|j| 0.5 * pdiag[j] * x[j] * x[j] + q[j] * x[j])
                        .sum()
                };
                match solve_clarabel(
                    n,
                    pdiag.clone(),
                    &q,
                    ri.clone(),
                    ci.clone(),
                    vv.clone(),
                    &bv,
                    clarabel_cones,
                    p.tolerance,
                ) {
                    Ok(xc) => {
                        let dx = mine
                            .iter()
                            .zip(&xc)
                            .map(|(u, v)| (u - v).abs())
                            .fold(0.0, f64::max);
                        eprintln!(
                            "compare obj {:.6e} vs {:.6e} (rel {:.1e}) max dx {dx:.1e}",
                            obj(&mine),
                            obj(&xc),
                            (obj(&mine) - obj(&xc)).abs() / (1.0 + obj(&xc).abs())
                        );
                    }
                    Err(e) => eprintln!("compare clarabel {e:?}"),
                }
            }
            mine
        }
        Backend::Clarabel => solve_clarabel(n, pdiag, &q, ri, ci, vv, &bv, &cones, p.tolerance)?,
    };
    let x = &x;
    let xs = (0..k)
        .map(|m| std::array::from_fn(|i| x[l.x(m, i)]))
        .collect();
    let us = (0..k)
        .map(|m| std::array::from_fn(|j| x[l.u(m, j)]))
        .collect();
    let virtual_l1 = (0..k - 1)
        .flat_map(|m| (0..NX).map(move |i| (m, i)))
        .map(|(m, i)| x[l.nu(m, i)].abs())
        .sum();
    Ok(SubSolution {
        xs,
        us,
        sigma: x[l.sigma()],
        virtual_l1,
    })
}

#[allow(clippy::too_many_arguments)]
fn solve_clarabel(
    n: usize,
    pdiag: Vec<f64>,
    q: &[f64],
    ri: Vec<usize>,
    ci: Vec<usize>,
    vv: Vec<f64>,
    bv: &[f64],
    cones: &[SupportedConeT<f64>],
    tolerance: f64,
) -> Result<Vec<f64>, GuidanceError> {
    let a = CscMatrix::new_from_triplets(bv.len(), n, ri, ci, vv);
    let idx: Vec<usize> = (0..n).collect();
    let pm = CscMatrix::new_from_triplets(n, n, idx.clone(), idx, pdiag);
    let settings = DefaultSettings {
        verbose: false,
        max_iter: 100,
        tol_gap_abs: tolerance,
        tol_gap_rel: tolerance,
        tol_feas: tolerance,
        tol_ktratio: tolerance.max(1e-6),
        ..Default::default()
    };
    let mut solver = DefaultSolver::new(&pm, q, &a, bv, cones, settings)
        .map_err(|e| GuidanceError::SolverFailed(format!("setup: {e:?}")))?;
    solver.solve();
    match solver.solution.status {
        SolverStatus::Solved | SolverStatus::AlmostSolved => Ok(solver.solution.x.clone()),
        SolverStatus::PrimalInfeasible | SolverStatus::AlmostPrimalInfeasible => {
            Err(GuidanceError::Infeasible)
        }
        other => Err(GuidanceError::SolverFailed(format!("{other:?}"))),
    }
}
