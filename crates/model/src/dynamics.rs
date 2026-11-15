//! Rigid-body equations of the ship, shared by the simulator (truth) and
//! the planner (which linearizes them).
//!
//! State `x = [m, r(3), v(3), q(4), ω(3)]`: mass, inertial position and
//! velocity (z up), body→inertial quaternion, body rates. Control: thrust
//! vector in body axes, acting at the gimbal point below the CoM.
//!
//! `ṁ = −α‖T‖`, `ṙ = v`, `v̇ = (q T q* + F_aero)/m − g ẑ`,
//! `q̇ = ½ q ⊗ ω`, `J ω̇ = r_T × T + τ_aero + τ_ext − ω × Jω`.

use crate::aero;
use crate::quat::{self, Quat, Vec3};
use crate::vehicle::Scenario;

pub const STATE: usize = 14;
pub type State = [f64; STATE];

/// Index helpers into [`State`].
pub const M: usize = 0;
pub const R: usize = 1;
pub const V: usize = 4;
pub const Q: usize = 7;
pub const W: usize = 11;

pub fn position(x: &State) -> Vec3 {
    [x[R], x[R + 1], x[R + 2]]
}
pub fn velocity(x: &State) -> Vec3 {
    [x[V], x[V + 1], x[V + 2]]
}
pub fn attitude(x: &State) -> Quat {
    [x[Q], x[Q + 1], x[Q + 2], x[Q + 3]]
}
pub fn rates(x: &State) -> Vec3 {
    [x[W], x[W + 1], x[W + 2]]
}

/// Build a state from its parts.
pub fn state(m: f64, r: Vec3, v: Vec3, q: Quat, w: Vec3) -> State {
    let mut x = [0.0; STATE];
    x[M] = m;
    x[R..R + 3].copy_from_slice(&r);
    x[V..V + 3].copy_from_slice(&v);
    x[Q..Q + 4].copy_from_slice(&q);
    x[W..W + 3].copy_from_slice(&w);
    x
}

/// Extra forces the simulator adds (flaps, wind, RCS); zero for the planner.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Extra {
    /// Inertial force, N.
    pub force: Vec3,
    /// Body torque, N·m.
    pub torque: Vec3,
    /// Wind velocity, inertial, m/s (changes the body's aerodynamics).
    pub wind: Vec3,
}

/// `ẋ = f(x, T)` (thrust in body axes, N).
pub fn f(scn: &Scenario, x: &State, thrust: Vec3, extra: &Extra) -> State {
    let ship = &scn.ship;
    let m = x[M].max(1.0);
    let (v, q, w) = (velocity(x), attitude(x), rates(x));
    let rho = scn.density(x[R + 2]);
    let (fa, ta) = aero::body_force(ship, rho, q, quat::sub(v, extra.wind));

    let force = quat::add(quat::add(quat::rotate(q, thrust), fa), extra.force);
    let g = scn.planet.gravity();
    let j = ship.inertia(m);
    let jw = [j[0] * w[0], j[1] * w[1], j[2] * w[2]];
    let torque = quat::add(
        quat::add(quat::cross(ship.thrust_point(), thrust), ta),
        quat::sub(extra.torque, quat::cross(w, jw)),
    );
    // Smooth near zero so the planner's finite differences stay finite.
    let t_mag = (quat::dot(thrust, thrust) + 1e-6).sqrt();
    let dq = quat::derivative(q, w);

    let mut d = [0.0; STATE];
    d[M] = -ship.alpha() * t_mag;
    d[R..R + 3].copy_from_slice(&v);
    for i in 0..3 {
        d[V + i] = force[i] / m - if i == 2 { g } else { 0.0 };
        d[W + i] = torque[i] / j[i];
    }
    d[Q..Q + 4].copy_from_slice(&dq);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upright(m: f64, z: f64) -> State {
        state(m, [0.0, 0.0, z], [0.0; 3], [1.0, 0.0, 0.0, 0.0], [0.0; 3])
    }

    #[test]
    fn hover_is_zero_acceleration() {
        let s = Scenario::earth();
        let m = 120e3;
        let x = upright(m, 500.0);
        let d = f(&s, &x, [0.0, 0.0, m * s.planet.gravity()], &Extra::default());
        assert!(d[V..V + 3].iter().all(|a| a.abs() < 1e-9));
        assert!(d[W..W + 3].iter().all(|a| a.abs() < 1e-12));
        assert!((d[M] + s.ship.alpha() * m * s.planet.gravity()).abs() < 1e-9);
    }

    #[test]
    fn thrust_through_cm_has_no_torque() {
        let s = Scenario::earth();
        let d = f(&s, &upright(120e3, 500.0), [0.0, 0.0, 2e6], &Extra::default());
        assert!(d[W..W + 3].iter().all(|a| a.abs() < 1e-12));
    }

    #[test]
    fn gimbal_toward_plus_x_pitches_nose_toward_minus_x() {
        // Thrust pushed toward +x at the base, below the CoM: the base goes
        // +x, so the nose tips toward −x. That is a negative rotation about y.
        let s = Scenario::earth();
        let d = f(&s, &upright(120e3, 500.0), [1e5, 0.0, 2e6], &Extra::default());
        assert!(d[W + 1] < 0.0, "{:?}", &d[W..]);
    }

    #[test]
    fn free_fall_in_vacuum_of_mars_high_up() {
        let s = Scenario::mars();
        let d = f(&s, &upright(300e3, 200_000.0), [0.0; 3], &Extra::default());
        assert!((d[V + 2] + 3.721).abs() < 1e-3);
    }
}
