//! Physical checks shared by the integration tests. Each panics with a
//! message naming the node or interval that broke the rule.

#![allow(dead_code)] // each test file uses a different subset

use guidance::{LandingProblem, Trajectory, Vec3};

pub fn norm(v: Vec3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

pub fn check_lands_at_rest(t: &Trajectory) {
    let n = t.position.len() - 1;
    assert!(
        norm(t.position[n]) < 0.01,
        "final position {:?}",
        t.position[n]
    );
    assert!(
        norm(t.velocity[n]) < 0.01,
        "final velocity {:?}",
        t.velocity[n]
    );
    assert!((t.t[n] - t.final_time).abs() < 1e-9);
}

pub fn check_starts_at_initial_state(p: &LandingProblem, t: &Trajectory) {
    for i in 0..3 {
        assert!((t.position[0][i] - p.position[i]).abs() < 1e-6);
        assert!((t.velocity[0][i] - p.velocity[i]).abs() < 1e-6);
    }
    assert!((t.mass[0] - p.vehicle.wet_mass).abs() < 1e-6);
}

/// The lossless-convexification check: the relaxation allows ‖u‖ < σ, which
/// would let true thrust drop below ρ₁. It must not happen.
pub fn check_thrust_within_bounds(p: &LandingProblem, t: &Trajectory) {
    let (lo, hi) = (p.vehicle.min_thrust, p.vehicle.max_thrust);
    for (k, thrust) in t.thrust.iter().enumerate() {
        let mag = norm(*thrust);
        assert!(
            mag >= lo * (1.0 - 1e-4) && mag <= hi * (1.0 + 1e-4),
            "interval {k}: ‖T‖ = {mag:.1} N outside [{lo:.1}, {hi:.1}]"
        );
    }
}

pub fn check_tilt_within_limit(p: &LandingProblem, t: &Trajectory) {
    let cos_max = p.vehicle.max_tilt.cos();
    for (k, thrust) in t.thrust.iter().enumerate() {
        let mag = norm(*thrust);
        assert!(
            thrust[2] >= mag * cos_max - 1e-6 * p.vehicle.max_thrust,
            "interval {k}: tilt too large, T = {thrust:?}"
        );
    }
}

pub fn check_glide_slope(p: &LandingProblem, t: &Trajectory) {
    let tan_gs = p.vehicle.glide_slope.tan();
    for (k, r) in t.position.iter().enumerate() {
        let floor = tan_gs * r[0].hypot(r[1]);
        assert!(r[2] >= floor - 1e-3, "node {k}: z = {} below {floor}", r[2]);
    }
}

pub fn check_never_below_dry_mass(p: &LandingProblem, t: &Trajectory) {
    for (k, m) in t.mass.iter().enumerate() {
        assert!(*m >= p.vehicle.dry_mass - 1e-6, "node {k}: mass {m}");
    }
}

pub fn check_mass_decreases(t: &Trajectory) {
    for (k, pair) in t.mass.windows(2).enumerate() {
        assert!(pair[1] <= pair[0] + 1e-9, "mass rose on interval {k}");
    }
}

/// Every physical rule at once.
pub fn assert_constraints(p: &LandingProblem, t: &Trajectory) {
    check_lands_at_rest(t);
    check_starts_at_initial_state(p, t);
    check_thrust_within_bounds(p, t);
    check_tilt_within_limit(p, t);
    check_glide_slope(p, t);
    check_never_below_dry_mass(p, t);
    check_mass_decreases(t);
}
