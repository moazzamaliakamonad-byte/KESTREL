//! The fallback problem: when no pinpoint landing exists, land at rest as
//! close to the pad as possible (Blackmore, Açıkmeşe & Scharf 2010 pose it
//! as a two-step problem; we use one weighted objective, see socp.rs).

mod common;

use common::*;
use guidance::{GuidanceError, LandingProblem, Trajectory, solve, solve_closest};

fn miss(t: &Trajectory) -> f64 {
    let end = t.position[t.position.len() - 1];
    end[0].hypot(end[1])
}

/// Lands at rest on the ground, obeying every limit except the glide cone.
fn check_lands_somewhere(p: &LandingProblem, t: &Trajectory) {
    let n = t.position.len() - 1;
    assert!(
        t.position[n][2].abs() < 0.01,
        "ends at z = {}",
        t.position[n][2]
    );
    assert!(
        norm(t.velocity[n]) < 0.01,
        "ends at v = {:?}",
        t.velocity[n]
    );
    check_starts_at_initial_state(p, t);
    check_thrust_within_bounds(p, t);
    check_tilt_within_limit(p, t);
    check_never_below_dry_mass(p, t);
    check_mass_decreases(t);
    for (k, r) in t.position.iter().enumerate() {
        assert!(r[2] >= -1e-6, "node {k} below ground");
    }
}

#[test]
fn pinpoint_reachable_gives_zero_miss() {
    let p = LandingProblem::example();
    let t = solve_closest(&p, 40.0).expect("feasible");
    check_lands_somewhere(&p, &t);
    assert!(miss(&t) < 1.0, "miss {}", miss(&t));
}

#[test]
fn too_little_propellant_lands_off_pad_at_rest() {
    let mut p = LandingProblem::example();
    p.vehicle.dry_mass = DRY_MASS_SHORT;
    assert_eq!(solve(&p).err(), Some(GuidanceError::Infeasible));
    let t = solve_closest(&p, 35.0).expect("can still stop");
    check_lands_somewhere(&p, &t);
    assert!(miss(&t) > 10.0, "miss {}", miss(&t));
}

#[test]
fn outside_glide_cone_still_lands() {
    let mut p = LandingProblem::example();
    p.position = [3000.0, 0.0, 100.0];
    p.velocity = [-20.0, 0.0, -5.0];
    assert!(p.validate().is_err(), "below the 4° cone");
    let t = solve_closest(&p, 60.0).expect("feasible without the cone");
    check_lands_somewhere(&p, &t);
}

/// Dry mass that leaves enough propellant to stop, but not to reach the pad
/// (measured: pinpoint infeasible; closest at 35 s lands ~268 m short).
const DRY_MASS_SHORT: f64 = 1720.0;
