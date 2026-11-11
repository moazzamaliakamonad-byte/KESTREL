//! Physical checks on a solved fixed-final-time landing. Every check is on
//! SI outputs, so a scaling or sign mistake anywhere shows up here.

use guidance::{GuidanceError, LandingProblem, Trajectory, Vec3, solve_fixed_time};

const TF: f64 = 50.0;

fn norm(v: Vec3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn solved() -> (LandingProblem, Trajectory) {
    let p = LandingProblem::example();
    let traj = solve_fixed_time(&p, TF).expect("example should be solvable at 50 s");
    (p, traj)
}

#[test]
fn lands_on_pad_at_rest() {
    let (_, t) = solved();
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
    assert!((t.t[n] - TF).abs() < 1e-9);
}

#[test]
fn starts_at_initial_state() {
    let (p, t) = solved();
    for i in 0..3 {
        assert!((t.position[0][i] - p.position[i]).abs() < 1e-6);
        assert!((t.velocity[0][i] - p.velocity[i]).abs() < 1e-6);
    }
    assert!((t.mass[0] - p.vehicle.wet_mass).abs() < 1e-6);
}

#[test]
fn thrust_magnitude_within_bounds() {
    // The lossless-convexification check: the relaxation allows ‖u‖ < σ,
    // which would let true thrust drop below ρ₁. It must not happen.
    let (p, t) = solved();
    let (lo, hi) = (p.vehicle.min_thrust, p.vehicle.max_thrust);
    for (k, thrust) in t.thrust.iter().enumerate() {
        let mag = norm(*thrust);
        assert!(
            mag >= lo * (1.0 - 1e-4) && mag <= hi * (1.0 + 1e-4),
            "interval {k}: ‖T‖ = {mag:.1} N outside [{lo:.1}, {hi:.1}]"
        );
    }
}

#[test]
fn tilt_within_limit() {
    let (p, t) = solved();
    let cos_max = p.vehicle.max_tilt.cos();
    for (k, thrust) in t.thrust.iter().enumerate() {
        let mag = norm(*thrust);
        assert!(
            thrust[2] >= mag * cos_max - 1e-6 * p.vehicle.max_thrust,
            "interval {k}: tilt too large, T = {thrust:?}"
        );
    }
}

#[test]
fn glide_slope_respected() {
    let (p, t) = solved();
    let tan_gs = p.vehicle.glide_slope.tan();
    for (k, r) in t.position.iter().enumerate() {
        let floor = tan_gs * r[0].hypot(r[1]);
        assert!(r[2] >= floor - 1e-3, "node {k}: z = {} below {floor}", r[2]);
    }
}

#[test]
fn never_below_dry_mass() {
    let (p, t) = solved();
    for (k, m) in t.mass.iter().enumerate() {
        assert!(*m >= p.vehicle.dry_mass - 1e-6, "node {k}: mass {m}");
    }
}

#[test]
fn mass_decreases_monotonically() {
    let (_, t) = solved();
    for (k, pair) in t.mass.windows(2).enumerate() {
        assert!(pair[1] <= pair[0] + 1e-9, "mass rose on interval {k}");
    }
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
