//! Solve the landing SOCP with Clarabel and convert the answer back to SI.

use clarabel::solver::{DefaultSettings, DefaultSolver, IPSolver, SolverStatus};

use crate::params::Vec3;
use crate::problem::{GuidanceError, LandingProblem, Trajectory};
use crate::socp::{self, Socp};

/// Grid points for the coarse scan over landing times.
const GRID_POINTS: usize = 12;
/// Stop refining once the bracket on the landing time is this narrow, s.
const TIME_TOLERANCE: f64 = 0.05;
const MAX_REFINEMENTS: usize = 30;

/// Plan the fuel-optimal landing, choosing the landing time as well.
///
/// The convex problem needs a fixed final time, so we search over it: a
/// coarse grid between the Açıkmeşe & Ploen bounds, then a golden-section
/// search around the best grid point. Fuel versus landing time is
/// unimodal in practice for this problem (not proven here).
pub fn solve(problem: &LandingProblem) -> Result<Trajectory, GuidanceError> {
    problem.validate()?;
    let veh = &problem.vehicle;
    let [vx, vy, vz] = problem.velocity;
    let speed = (vx * vx + vy * vy + vz * vz).sqrt();

    // Bounds: can't stop sooner than the momentum allows at full thrust, and
    // can't fly longer than the propellant lasts at minimum thrust. Also stay
    // below full-thrust burn-out, where the log-mass reference is undefined.
    let burn_out = veh.wet_mass / (veh.alpha() * veh.max_thrust);
    let t_max =
        ((veh.wet_mass - veh.dry_mass) / (veh.alpha() * veh.min_thrust)).min(0.999 * burn_out);
    // A start at rest gives a textbook bound of 0, which is no landing time.
    let t_min = (veh.dry_mass * speed / veh.max_thrust).max(1e-3 * t_max);

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
    search.finish()
}

/// Remembers the best landing seen so far while the search tries times.
struct Search<'a> {
    problem: &'a LandingProblem,
    best: Option<Trajectory>,
    failure: Option<GuidanceError>,
}

impl<'a> Search<'a> {
    fn new(problem: &'a LandingProblem) -> Self {
        Search {
            problem,
            best: None,
            failure: None,
        }
    }

    /// Fuel used for a landing at `final_time`, or +∞ if there is none.
    fn fuel_at(&mut self, final_time: f64) -> f64 {
        match solve_fixed_time(self.problem, final_time) {
            Ok(traj) => {
                let fuel = traj.fuel_used();
                let better = self.best.as_ref().is_none_or(|b| fuel < b.fuel_used());
                if better {
                    self.best = Some(traj);
                }
                fuel
            }
            Err(GuidanceError::Infeasible) => f64::INFINITY,
            Err(other) => {
                self.failure.get_or_insert(other);
                f64::INFINITY
            }
        }
    }

    /// The best landing found; otherwise why there was none. A solver
    /// failure is reported in preference to "infeasible", so bugs don't
    /// hide behind "impossible landing".
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
    let s = socp::build(problem, final_time)?;
    let settings = DefaultSettings {
        verbose: false,
        ..Default::default()
    };
    let mut solver = DefaultSolver::new(&s.p, &s.q, &s.a, &s.b, &s.cones, settings)
        .map_err(|e| GuidanceError::SolverFailed(format!("setup: {e:?}")))?;
    solver.solve();
    match solver.solution.status {
        SolverStatus::Solved => Ok(to_trajectory(problem, &s, &solver.solution.x)),
        SolverStatus::PrimalInfeasible | SolverStatus::AlmostPrimalInfeasible => {
            Err(GuidanceError::Infeasible)
        }
        other => Err(GuidanceError::SolverFailed(format!("{other:?}"))),
    }
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
