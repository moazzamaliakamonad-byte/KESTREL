use super::*;
use model::dynamics::{R, V, W};

/// Belly down, nose toward the tower, as the flight flies it.
fn belly_down() -> Quat {
    crate::ship_control::belly_attitude(0.0)
}

fn earth_flip_problem(z: f64, vz: f64) -> ShipProblem {
    let scn = Scenario::earth();
    let m = scn.start_mass;
    let x0 = dynamics::state(m, [200.0, 0.0, z], [4.0, 0.0, vz], belly_down(), [0.0; 3]);
    let lim = scn.ship.thrust_limits(3);
    let mut p = ShipProblem::new(scn, x0, lim);
    p.late_limits = Some(p.scn.ship.thrust_limits(1));
    p.switch_node = (p.scn.switch_fraction * (p.nodes - 1) as f64).round() as usize;
    p
}

fn check_limits(p: &ShipProblem, plan: &ShipPlan) {
    let tan = p.scn.ship.gimbal_max.tan();
    for (k, (x, u)) in plan.x.iter().zip(&plan.u).enumerate() {
        let (tmin, tmax) = p.bounds(k);
        let t = quat::norm(*u);
        assert!(t <= tmax * 1.01 && t >= tmin * 0.99, "node {k}: thrust {t}");
        assert!(u[0].hypot(u[1]) <= tan * u[2] * 1.01 + 1.0, "node {k}: gimbal");
        if k > 1 {
            let w = quat::norm([x[W], x[W + 1], x[W + 2]]);
            assert!(w <= OMEGA_MAX * 1.01, "node {k}: rate {w}");
        }
        assert!(x[R + 2] >= p.scn.floor_z - 0.5, "node {k}: below floor");
        if let Some(c) = p.scn.clearance_x {
            assert!(x[R] >= c - 0.5, "node {k}: tower side");
        }
        assert!(x[M] >= p.scn.dry_mass - 1.0);
    }
}

fn ends_upright(plan: &ShipPlan) -> f64 {
    quat::tilt(dynamics::attitude(plan.x.last().unwrap())).to_degrees()
}

#[test]
fn earth_flip_converges() {
    let p = earth_flip_problem(650.0, -90.0);
    let plan = plan_ship(&p, None, COLD_ITERATIONS).expect("plan");
    eprintln!(
        "earth: {} it, tf {:.1} s, defect {:.2e}, gate err {:.2} m, fuel {:.0} kg",
        plan.iterations,
        plan.final_time,
        plan.defect,
        plan.gate_error,
        p.x0[M] - plan.x.last().unwrap()[M]
    );
    assert!(plan.converged, "not converged in {} iterations", plan.iterations);
    assert!(plan.defect < 1e-3);
    assert!(plan.gate_error < 1.0);
    assert!(ends_upright(&plan) < 3.0);
    check_limits(&p, &plan);
}

#[test]
fn warm_replan_is_fast() {
    let p = earth_flip_problem(650.0, -90.0);
    let plan = plan_ship(&p, None, COLD_ITERATIONS).expect("plan");
    // One second later, from the plan's own state (a little perturbed).
    let (mut x, _) = plan.sample(1.0);
    x[R] += 2.0;
    x[V + 2] -= 0.5;
    let p2 = ShipProblem { x0: x, ..p.clone() };
    let again = plan_ship(&p2, Some((&plan, 1.0)), WARM_ITERATIONS).expect("warm plan");
    assert!(again.iterations <= WARM_ITERATIONS);
    assert!(again.gate_error < 10.0);
    check_limits(&p2, &again);
}

#[test]
fn unreachable_gate_is_reported() {
    // 140 m up, 30 m above the gate, falling at 90 m/s, belly down.
    let p = earth_flip_problem(140.0, -90.0);
    assert!(plan_ship(&p, None, COLD_ITERATIONS).is_err());
}

#[test]
fn upright_like_keeps_heading() {
    let q = upright_like(belly_down());
    assert!(quat::tilt(q) < 1e-9);
    assert!(q[1].abs() < 1e-9 && q[2].abs() < 1e-9);
}

#[test]
#[ignore]
fn trace_one_engine_from_midflight() {
    let scn = Scenario::earth();
    let y: f64 = std::env::var("TY").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let q = quat::normalize([1.0, 0.002 * y, 0.01, 0.0]);
    let x0 = dynamics::state(128e3, [285.0, 0.8 * y, 560.0], [-22.7, 2.1 * y, -28.2], q, [0.01 * y, 0.02, 0.0]);
    let one = scn.ship.thrust_limits(1);
    let p = ShipProblem::new(scn, x0, (one.0, one.1 * 0.75));
    match plan_ship(&p, None, 30) {
        Ok(plan) => eprintln!("tf {:.1} it {} conv {} gate {:.2}", plan.final_time, plan.iterations, plan.converged, plan.gate_error),
        Err(e) => eprintln!("err {e:?}"),
    }
}

fn booster_burn_problem(x: f64, vx: f64, lean: f64) -> ShipProblem {
    let mut scn = Scenario::booster();
    let m = 381e3;
    let v = [vx, 0.0, -412.0];
    let q = quat::mul(crate::ship_control::descent_attitude(v), quat::from_axis_angle([0.0, 1.0, 0.0], lean));
    let x0 = dynamics::state(m, [x, 0.0, 2578.0], v, q, [0.0; 3]);
    scn.gate = [20.0, 0.0, 540.0];
    scn.gate_velocity = [0.0, 0.0, -50.0];
    let (lo, hi) = scn.ship.thrust_limits(13);
    ShipProblem::new(scn, x0, (lo, 0.9 * hi))
}

#[test]
fn booster_thirteen_engine_burn_plans_from_either_side() {
    // The 13-engine leg from the ignition point to the intermediate gate,
    // out from the tower and (after an overshoot) on its other side.
    for p in [booster_burn_problem(180.0, -25.0, 0.03), booster_burn_problem(60.0, 25.0, -0.03)] {
        let plan = plan_ship(&p, None, COLD_ITERATIONS).expect("plan");
        assert!(plan.converged && plan.gate_error < 1.0, "gate {}", plan.gate_error);
        assert!((6.0..14.0).contains(&plan.final_time), "tf {}", plan.final_time);
        check_limits(&p, &plan);
        // Never broadside: within the tilt limit (plus the soft slack).
        for x in &plan.x[2..] {
            let tilt = quat::tilt(dynamics::attitude(x));
            assert!(tilt < BOOSTER_TILT + 0.05, "tilt {}", tilt.to_degrees());
        }
    }
}

#[test]
fn booster_divert_beyond_reach_is_refused() {
    // 336 m sideways in a nine-second burn at ≤ 15° tilt cannot be done.
    let p = booster_burn_problem(356.0, -35.0, 0.03);
    assert!(plan_ship(&p, None, COLD_ITERATIONS).is_err());
}
