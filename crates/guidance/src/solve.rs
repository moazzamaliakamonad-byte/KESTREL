//! Solve the landing SOCP with Clarabel and convert the answer back to SI.

use clarabel::solver::{DefaultSettings, DefaultSolver, IPSolver, SolverStatus};

use crate::params::Vec3;
use crate::problem::{GuidanceError, LandingProblem, Trajectory};
use crate::socp::{self, Socp, Target};

/// Grid points for the coarse scan over landing times.
const GRID_POINTS: usize = 12;
/// Stop refining once the bracket on the landing time is this narrow, s.
const TIME_TOLERANCE: f64 = 0.05;
const MAX_REFINEMENTS: usize = 30;

/// Plan the fuel-optimal landing, choosing the landing time as well.
///
/// The convex problem needs a fixed final time, so we search over it: a
/// coarse grid, then a golden-section search around the best grid point.
/// Fuel versus landing time is unimodal in practice for this problem
/// (cited by Malyuta et al.; not proven here).
///
/// The search ignores the propellant limit, which makes the set of workable
/// landing times wide (only kinematics limit it), so a coarse grid cannot
/// step over it. The limit is applied once, at the best time found: if even
/// the fuel-optimal landing over all times needs more propellant than the
/// tanks hold, no landing time works.
pub fn solve(problem: &LandingProblem) -> Result<Trajectory, GuidanceError> {
    solve_counted(problem).0
}

/// The range of landing times worth trying, s: no sooner than the momentum
/// allows at full thrust, and before full-thrust burn-out (where the
/// log-mass reference is undefined).
pub(crate) fn landing_time_bounds(problem: &LandingProblem) -> Result<(f64, f64), GuidanceError> {
    let veh = &problem.vehicle;
    let [vx, vy, vz] = problem.velocity;
    let speed = vx.hypot(vy).hypot(vz);
    let burn_out = veh.wet_mass / (veh.alpha() * veh.max_thrust);
    let t_max = 0.999 * burn_out;
    let t_min_textbook = veh.dry_mass * speed / veh.max_thrust;
    // Explicit NaN test: speed can overflow to ∞ (fine, ≥ t_max) or be NaN.
    if t_min_textbook.is_nan() || t_min_textbook >= t_max {
        return Err(GuidanceError::Infeasible);
    }
    // A start at rest gives a textbook bound of 0, which is no landing time.
    Ok((t_min_textbook.max(1e-3 * t_max), t_max))
}

/// As [`solve`], also returning how many convex solves it ran.
pub(crate) fn solve_counted(
    problem: &LandingProblem,
) -> (Result<Trajectory, GuidanceError>, usize) {
    if let Err(e) = problem.validate() {
        return (Err(e), 0);
    }
    let (t_min, t_max) = match landing_time_bounds(problem) {
        Ok(b) => b,
        Err(e) => return (Err(e), 0),
    };

    let mut search = Search::new(problem);
    let step = (t_max - t_min) / (GRID_POINTS - 1) as f64;
    let grid: Vec<f64> = (0..GRID_POINTS).map(|i| t_min + i as f64 * step).collect();
    let costs: Vec<f64> = grid.iter().map(|&tf| search.fuel_at(tf)).collect();

    let (best_i, best_cost) = costs
        .iter()
        .copied()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("grid is not empty");
    if best_cost.is_finite() {
        // Golden-section search inside the best grid point's neighbours.
        let mut lo = grid[best_i.saturating_sub(1)];
        let mut hi = grid[(best_i + 1).min(GRID_POINTS - 1)];
        let ratio = (5.0_f64.sqrt() - 1.0) / 2.0; // ≈ 0.618
        let mut c = hi - ratio * (hi - lo);
        let mut d = lo + ratio * (hi - lo);
        let mut fc = search.fuel_at(c);
        let mut fd = search.fuel_at(d);
        for _ in 0..MAX_REFINEMENTS {
            if hi - lo <= TIME_TOLERANCE {
                break;
            }
            if fc < fd {
                hi = d;
                (d, fd) = (c, fc);
                c = hi - ratio * (hi - lo);
                fc = search.fuel_at(c);
            } else {
                lo = c;
                (c, fc) = (d, fd);
                d = lo + ratio * (hi - lo);
                fd = search.fuel_at(d);
            }
        }
    }
    // Apply the propellant limit at the best time found.
    let solves = search.solves + 1;
    let result = search
        .finish()
        .and_then(|best| solve_fixed_time(problem, best.final_time));
    (result, solves)
}

/// Remembers the best landing seen so far while the search tries times.
/// Searches with the propellant limit switched off (see [`solve`]).
struct Search<'a> {
    problem: &'a LandingProblem,
    best: Option<Trajectory>,
    failure: Option<GuidanceError>,
    solves: usize,
}

impl<'a> Search<'a> {
    fn new(problem: &'a LandingProblem) -> Self {
        Search {
            problem,
            best: None,
            failure: None,
            solves: 0,
        }
    }

    /// Fuel used for a landing at `final_time`, or +∞ if there is none.
    fn fuel_at(&mut self, final_time: f64) -> f64 {
        self.solves += 1;
        match solve_at(self.problem, final_time, DryMass::Ignore, Target::Pinpoint) {
            Ok(traj) => {
                let fuel = traj.fuel_used();
                let better = self.best.as_ref().is_none_or(|b| fuel < b.fuel_used());
                if better {
                    self.best = Some(traj);
                }
                fuel
            }
            // Times the search picks itself can be unworkable; that is not
            // the caller's fault, so it is never reported as InvalidProblem.
            Err(GuidanceError::Infeasible | GuidanceError::InvalidProblem(_)) => f64::INFINITY,
            Err(other) => {
                self.failure.get_or_insert(other);
                f64::INFINITY
            }
        }
    }

    /// The best landing found; otherwise why there was none. A solver
    /// failure or a loose relaxation is reported in preference to
    /// "infeasible", so problems don't hide behind "impossible landing".
    fn finish(self) -> Result<Trajectory, GuidanceError> {
        match (self.best, self.failure) {
            (Some(best), _) => Ok(best),
            (None, Some(failure)) => Err(failure),
            (None, None) => Err(GuidanceError::Infeasible),
        }
    }
}

/// Plan the fuel-optimal landing that touches down exactly `final_time`
/// seconds from now.
pub fn solve_fixed_time(
    problem: &LandingProblem,
    final_time: f64,
) -> Result<Trajectory, GuidanceError> {
    solve_at(problem, final_time, DryMass::Enforce, Target::Pinpoint)
}

/// The fallback when no pinpoint landing exists: land at rest, `final_time`
/// seconds from now, as close to the pad as possible (then with least fuel).
/// The glide cone is dropped: it is centred on the pad, and the vehicle may
/// already be outside it.
pub fn solve_closest(
    problem: &LandingProblem,
    final_time: f64,
) -> Result<Trajectory, GuidanceError> {
    solve_at(problem, final_time, DryMass::Enforce, Target::Closest)
}

/// Whether the final mass must stay above the dry mass.
#[derive(Clone, Copy)]
enum DryMass {
    Enforce,
    Ignore,
}

/// Relative gap between thrust paid for (σ) and delivered (‖u‖) above which
/// the relaxation counts as loose.
const LOSSLESS_TOLERANCE: f64 = 1e-4;

fn solve_at(
    problem: &LandingProblem,
    final_time: f64,
    dry: DryMass,
    target: Target,
) -> Result<Trajectory, GuidanceError> {
    let s = socp::build_with(problem, final_time, matches!(dry, DryMass::Enforce), target)?;
    let settings = DefaultSettings {
        verbose: false,
        ..Default::default()
    };
    let mut solver = DefaultSolver::new(&s.p, &s.q, &s.a, &s.b, &s.cones, settings)
        .map_err(|e| GuidanceError::SolverFailed(format!("setup: {e:?}")))?;
    solver.solve();
    match solver.solution.status {
        SolverStatus::Solved => {
            let x = &solver.solution.x;
            check_lossless(&s, x)?;
            Ok(to_trajectory(problem, &s, x))
        }
        SolverStatus::PrimalInfeasible | SolverStatus::AlmostPrimalInfeasible => {
            Err(GuidanceError::Infeasible)
        }
        other => Err(GuidanceError::SolverFailed(format!("{other:?}"))),
    }
}

/// The relaxation `‖u‖ ≤ σ` is only valid if it is tight at the optimum.
/// Lossless-convexification theory guarantees that under conditions a
/// purely vertical descent can break, so check every solution.
fn check_lossless(s: &Socp, x: &[f64]) -> Result<(), GuidanceError> {
    let lay = s.layout;
    for k in 0..lay.n {
        let sigma = x[lay.sigma(k)];
        let [ux, uy, uz] = [0, 1, 2].map(|i| x[lay.u(k, i)]);
        let u = ux.hypot(uy).hypot(uz);
        if sigma > 0.0 && u < sigma * (1.0 - LOSSLESS_TOLERANCE) {
            return Err(GuidanceError::NotLossless {
                interval: k,
                ratio: u / sigma,
            });
        }
    }
    Ok(())
}

/// Read the solver's flat vector back into an SI trajectory.
fn to_trajectory(problem: &LandingProblem, s: &Socp, x: &[f64]) -> Trajectory {
    let (lay, sc) = (s.layout, s.scaling);
    let wet = problem.vehicle.wet_mass;
    let n = lay.n;

    let t = (0..=n)
        .map(|k| sc.from_nd_time(k as f64 * s.dt_nd))
        .collect();
    let position = (0..=n)
        .map(|k| vec3(|i| sc.from_nd_length(x[lay.r(k, i)])))
        .collect();
    let velocity = (0..=n)
        .map(|k| vec3(|i| sc.from_nd_velocity(x[lay.v(k, i)])))
        .collect();
    let mass: Vec<f64> = (0..=n).map(|k| wet * x[lay.z(k)].exp()).collect();
    // Thrust on interval k uses the mass at its start node: T = m·u.
    let thrust = (0..n)
        .map(|k| {
            let m_nd = mass[k] / wet;
            vec3(|i| sc.from_nd_force(m_nd * x[lay.u(k, i)]))
        })
        .collect();

    Trajectory {
        t,
        position,
        velocity,
        mass,
        thrust,
        final_time: sc.from_nd_time(n as f64 * s.dt_nd),
    }
}

fn vec3(f: impl Fn(usize) -> f64) -> Vec3 {
    [f(0), f(1), f(2)]
}
