//! Solve the landing SOCP with Clarabel and convert the answer back to SI.

use clarabel::solver::{DefaultSettings, DefaultSolver, IPSolver, SolverStatus};

use crate::params::Vec3;
use crate::problem::{GuidanceError, LandingProblem, Trajectory};
use crate::socp::{self, Socp};

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
