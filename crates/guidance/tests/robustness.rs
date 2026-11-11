//! Cases beyond the single example, from the M0 final review: vertical
//! descents, tilt limits, tight propellant, absurd speeds. The rule they
//! enforce: guidance either returns a plan that obeys every physical
//! constraint, or an error. Never an `Ok` with broken physics.

mod common;

use common::*;
use guidance::{GuidanceError, LandingProblem, solve, solve_fixed_time};

fn vertical(height: f64, vz: f64, tilt_deg: f64) -> LandingProblem {
    let mut p = LandingProblem::example();
    p.position = [0.0, 0.0, height];
    p.velocity = [0.0, 0.0, vz];
    p.vehicle.max_tilt = tilt_deg.to_radians();
    p
}

fn cases() -> Vec<(&'static str, LandingProblem)> {
    let mut upward = LandingProblem::example();
    upward.velocity = [0.0, 0.0, 60.0];
    vec![
        ("example", LandingProblem::example()),
        ("vertical from rest, 500 m", vertical(500.0, 0.0, 45.0)),
        ("vertical, 10 m, 2 m/s down", vertical(10.0, -2.0, 45.0)),
        ("example moving up 60 m/s", upward),
        ("vertical, tilt 10°", vertical(500.0, 0.0, 10.0)),
        ("vertical, tilt 90°", vertical(500.0, 0.0, 90.0)),
    ]
}

#[test]
fn every_ok_plan_obeys_every_constraint() {
    for (name, p) in cases() {
        match solve(&p) {
            Ok(t) => {
                // Run the checks, naming the case if one panics.
                let r = std::panic::catch_unwind(|| assert_constraints(&p, &t));
                assert!(r.is_ok(), "case '{name}' returned Ok with broken physics");
            }
            Err(GuidanceError::Infeasible | GuidanceError::NotLossless { .. }) => {}
            Err(other) => panic!("case '{name}': unexpected error {other:?}"),
        }
    }
}

#[test]
fn slack_relaxation_is_reported_not_hidden() {
    // Vertical drop with no effective tilt limit: the relaxed problem can
    // "pay" for minimum thrust while pushing less. That must be an error.
    let p = vertical(500.0, 0.0, 90.0);
    let r = solve_fixed_time(&p, 25.0);
    assert!(
        matches!(r, Err(GuidanceError::NotLossless { .. })),
        "expected NotLossless, got {:?}",
        r.map(|t| t.fuel_used())
    );
}

#[test]
fn tight_propellant_landing_is_found() {
    // 195 kg of propellant: feasible landing times exist only in a ~2 s
    // window (measured by fixed-time scan: about 35.75 s to 37.75 s).
    let mut p = LandingProblem::example();
    p.vehicle.dry_mass = p.vehicle.wet_mass - 195.0;
    let t = solve(&p).expect("a tight but feasible landing must be found");
    assert_constraints(&p, &t);
    assert!(t.fuel_used() <= 195.0 + 1e-6);
}

#[test]
fn free_time_beats_a_fine_scan_near_the_optimum() {
    let p = LandingProblem::example();
    let best = solve(&p).unwrap();
    for i in 0..=6 {
        let tf = 35.0 + 0.5 * i as f64;
        if let Ok(fixed) = solve_fixed_time(&p, tf) {
            assert!(
                best.fuel_used() <= fixed.fuel_used() + 0.05,
                "search found {:.3} kg at {:.2} s, but {tf} s needs {:.3} kg",
                best.fuel_used(),
                best.final_time,
                fixed.fuel_used()
            );
        }
    }
}

#[test]
fn absurd_speeds_are_infeasible_not_invalid() {
    for vz in [-3000.0, -1e5] {
        let mut p = LandingProblem::example();
        p.velocity = [0.0, 0.0, vz];
        assert_eq!(
            solve(&p).err(),
            Some(GuidanceError::Infeasible),
            "vz = {vz}"
        );
    }
}
