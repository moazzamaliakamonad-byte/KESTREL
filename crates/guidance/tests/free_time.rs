//! The free-final-time search: guidance picks the landing time too.

mod common;

use common::*;
use guidance::{GuidanceError, LandingProblem, solve, solve_fixed_time};

#[test]
fn free_time_beats_grid() {
    let p = LandingProblem::example();
    let best = solve(&p).expect("example should be solvable");
    let mut compared = 0;
    for tf in [40.0, 50.0, 60.0, 70.0] {
        if let Ok(fixed) = solve_fixed_time(&p, tf) {
            compared += 1;
            assert!(
                best.fuel_used() <= fixed.fuel_used() + 0.1,
                "free time ({:.2} s) used {:.2} kg but t_f = {tf} uses {:.2} kg",
                best.final_time,
                best.fuel_used(),
                fixed.fuel_used()
            );
        }
    }
    assert!(compared > 0, "no fixed-time comparison was feasible");
}

#[test]
fn unreachable_start_is_infeasible() {
    // 400 m/s straight down from 1.5 km: no thrust profile can stop in time.
    let mut p = LandingProblem::example();
    p.velocity = [0.0, 0.0, -400.0];
    assert_eq!(solve(&p).err(), Some(GuidanceError::Infeasible));
}

#[test]
fn free_time_respects_bounds_too() {
    let p = LandingProblem::example();
    let t = solve(&p).unwrap();
    assert_constraints(&p, &t);
}

#[test]
fn starting_at_rest_does_not_break_the_search() {
    // ‖v₀‖ = 0 makes the textbook lower bound t_min = 0, which is not a
    // valid landing time. The search must still work.
    let mut p = LandingProblem::example();
    p.velocity = [0.0; 3];
    let t = solve(&p).expect("hovering start should be landable");
    assert_constraints(&p, &t);
}
