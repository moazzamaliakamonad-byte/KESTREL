use super::*;
use model::dynamics::{R, V, W};
use std::f64::consts::FRAC_PI_2;

fn belly_down() -> Quat {
    quat::from_axis_angle([0.0, 1.0, 0.0], FRAC_PI_2)
}

fn earth_flip_problem(z: f64, vz: f64) -> ShipProblem {
    let scn = Scenario::earth();
    let m = scn.start_mass;
    let x0 = dynamics::state(m, [150.0, 0.0, z], [5.0, 0.0, vz], belly_down(), [0.0; 3]);
    let lim = (scn.ship.thrust_limits(1).0, scn.ship.thrust_limits(3).1);
    ShipProblem::new(scn, x0, lim)
}

fn mars_problem() -> ShipProblem {
    let scn = Scenario::mars();
    let m = scn.start_mass;
    let x0 = dynamics::state(m, [400.0, 0.0, 9000.0], [-20.0, 0.0, -480.0], belly_down(), [0.0; 3]);
    let lim = (scn.ship.thrust_limits(1).0, scn.ship.thrust_limits(3).1);
    ShipProblem::new(scn, x0, lim)
}

fn check_limits(p: &ShipProblem, plan: &ShipPlan) {
    let (tmin, tmax) = p.thrust_limits;
    let tan = p.scn.ship.gimbal_max.tan();
    for (k, (x, u)) in plan.x.iter().zip(&plan.u).enumerate() {
        let t = quat::norm(*u);
        assert!(t <= tmax * 1.01 && t >= tmin * 0.99, "node {k}: thrust {t}");
        assert!(u[0].hypot(u[1]) <= tan * u[2] * 1.01 + 1.0, "node {k}: gimbal");
        if k > 0 {
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
    assert!(plan.gate_error < 10.0);
    assert!(ends_upright(&plan) < 3.0);
    check_limits(&p, &plan);
}

#[test]
fn mars_burn_converges() {
    let p = mars_problem();
    let plan = plan_ship(&p, None, COLD_ITERATIONS).expect("plan");
    eprintln!(
        "mars: {} it, tf {:.1} s, defect {:.2e}, gate err {:.2} m, fuel {:.0} kg",
        plan.iterations,
        plan.final_time,
        plan.defect,
        plan.gate_error,
        p.x0[M] - plan.x.last().unwrap()[M]
    );
    assert!(plan.converged);
    assert!(plan.gate_error < 10.0);
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
