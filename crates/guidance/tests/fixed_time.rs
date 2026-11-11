//! Physical checks on a solved fixed-final-time landing. Every check is on
//! SI outputs, so a scaling or sign mistake anywhere shows up here.

mod common;

use common::*;
use guidance::{GuidanceError, LandingProblem, Trajectory, solve_fixed_time};

const TF: f64 = 50.0;

fn solved() -> (LandingProblem, Trajectory) {
    let p = LandingProblem::example();
    let traj = solve_fixed_time(&p, TF).expect("example should be solvable at 50 s");
    (p, traj)
}

#[test]
fn lands_on_pad_at_rest() {
    let (_, t) = solved();
    check_lands_at_rest(&t);
    assert!((t.final_time - TF).abs() < 1e-9);
}

#[test]
fn starts_at_initial_state() {
    let (p, t) = solved();
    check_starts_at_initial_state(&p, &t);
}

#[test]
fn thrust_magnitude_within_bounds() {
    let (p, t) = solved();
    check_thrust_within_bounds(&p, &t);
}

#[test]
fn tilt_within_limit() {
    let (p, t) = solved();
    check_tilt_within_limit(&p, &t);
}

#[test]
fn glide_slope_respected() {
    let (p, t) = solved();
    check_glide_slope(&p, &t);
}

#[test]
fn never_below_dry_mass() {
    let (p, t) = solved();
    check_never_below_dry_mass(&p, &t);
}

#[test]
fn mass_decreases_monotonically() {
    let (_, t) = solved();
    check_mass_decreases(&t);
}

#[test]
fn too_short_time_is_infeasible() {
    // From 1.5 km up and 75 m/s down, 5 s is far too short to stop.
    let p = LandingProblem::example();
    assert_eq!(
        solve_fixed_time(&p, 5.0).err(),
        Some(GuidanceError::Infeasible)
    );
}
