//! Aerodynamics of a long cylinder (crossflow model) plus four flaps.
//!
//! With the air-relative velocity split into an axial part `w_a = w·b_z`
//! and a normal part `w_n = w − w_a b_z` (body axes):
//!
//! `F = −½ρ (C_A S_A |w_a| w_a b_z + C_N S_N |w_n| w_n)`
//!
//! with `S_A = πR²` and `S_N = L·2R`. The normal force acts `cp_offset`
//! ahead of the centre of mass. No Mach effects.

use crate::quat::{self, Quat, Vec3};
use crate::vehicle::Ship;

/// Flap order: fore-left, fore-right, aft-left, aft-right (left = +b_y).
pub const FLAPS: usize = 4;

fn split(q: Quat, w_air: Vec3) -> (f64, Vec3) {
    let wb = quat::rotate_inv(q, w_air);
    (wb[2], [wb[0], wb[1], 0.0])
}

/// Force on the bare body (inertial frame, N) and its torque about the
/// centre of mass (body frame, N·m). `w_air` = velocity relative to the
/// air, inertial frame.
pub fn body_force(ship: &Ship, rho: f64, q: Quat, w_air: Vec3) -> (Vec3, Vec3) {
    let (wa, wn) = split(q, w_air);
    let s_a = std::f64::consts::PI * ship.radius * ship.radius;
    let s_n = ship.length * 2.0 * ship.radius;
    let axial = -0.5 * rho * ship.c_a * s_a * wa.abs() * wa;
    let k = -0.5 * rho * ship.c_n * s_n * quat::norm(wn);
    let normal = quat::scale(wn, k);
    let f_body = [normal[0], normal[1], axial];
    let torque = quat::cross([0.0, 0.0, ship.cp_offset], normal);
    (quat::rotate(q, f_body), torque)
}

/// Force (inertial, N) and torque (body, N·m) of the four flaps, each
/// exposed by `e ∈ [0, 1]`.
pub fn flap_force(ship: &Ship, rho: f64, q: Quat, w_air: Vec3, exposure: [f64; FLAPS]) -> (Vec3, Vec3) {
    let (_, wn) = split(q, w_air);
    let unit = -0.5 * rho * ship.c_flap * quat::norm(wn);
    let mut force = [0.0; 3];
    let mut torque = [0.0; 3];
    for (i, p) in flap_points(ship).iter().enumerate() {
        let area = if i < 2 { ship.fore.area } else { ship.aft.area };
        let f = quat::scale(wn, unit * area * exposure[i].clamp(0.0, 1.0));
        force = quat::add(force, f);
        torque = quat::add(torque, quat::cross(*p, f));
    }
    (quat::rotate(q, force), torque)
}

/// Flap positions in body axes relative to the CoM, m.
pub fn flap_points(ship: &Ship) -> [Vec3; FLAPS] {
    [
        [0.0, ship.flap_y, ship.fore.station],
        [0.0, -ship.flap_y, ship.fore.station],
        [0.0, ship.flap_y, ship.aft.station],
        [0.0, -ship.flap_y, ship.aft.station],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle::{Planet, Scenario};
    use std::f64::consts::FRAC_PI_2;

    /// Belly down, nose toward +x: b_z = +x, b_x = −z, b_y = +y.
    pub(crate) fn belly_down() -> Quat {
        quat::from_axis_angle([0.0, 1.0, 0.0], FRAC_PI_2)
    }

    #[test]
    fn belly_down_frame_is_as_documented() {
        let q = belly_down();
        let bz = quat::rotate(q, [0.0, 0.0, 1.0]);
        let bx = quat::rotate(q, [1.0, 0.0, 0.0]);
        assert!((bz[0] - 1.0).abs() < 1e-12 && (bx[2] + 1.0).abs() < 1e-12);
    }

    #[test]
    fn broadside_terminal_speed_near_90() {
        let s = Scenario::earth();
        let rho = Planet::Earth.density(1000.0);
        // Find the speed where drag balances weight (falling straight down).
        let weight = s.start_mass * Planet::Earth.gravity();
        let drag = |v: f64| body_force(&s.ship, rho, belly_down(), [0.0, 0.0, -v]).0[2];
        let (mut lo, mut hi) = (1.0, 500.0);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if drag(mid) < weight { lo = mid } else { hi = mid }
        }
        assert!((80.0..100.0).contains(&lo), "terminal speed {lo}");
    }

    #[test]
    fn axial_flow_has_no_torque_and_opposes_motion() {
        let s = Scenario::earth();
        // Upright, falling straight down.
        let (f, t) = body_force(&s.ship, 1.0, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -50.0]);
        assert!(f[2] > 0.0 && f[0].abs() < 1e-9 && quat::norm(t) < 1e-9);
    }

    #[test]
    fn broadside_flow_pitches_nose_up() {
        // Centre of pressure ahead of the CoM: upward drag lifts the nose,
        // a negative torque about b_y (positive b_y rotation is nose down).
        let s = Scenario::earth();
        let (_, t) = body_force(&s.ship, 1.0, belly_down(), [0.0, 0.0, -50.0]);
        assert!(t[1] < 0.0, "{t:?}");
    }

    #[test]
    fn flaps_fore_pitch_up_aft_pitch_down_left_rolls_positive() {
        let s = Scenario::earth();
        let w = [0.0, 0.0, -50.0];
        let fore = flap_force(&s.ship, 1.0, belly_down(), w, [1.0, 1.0, 0.0, 0.0]).1;
        let aft = flap_force(&s.ship, 1.0, belly_down(), w, [0.0, 0.0, 1.0, 1.0]).1;
        let left = flap_force(&s.ship, 1.0, belly_down(), w, [1.0, 0.0, 1.0, 0.0]).1;
        assert!(fore[1] < 0.0 && aft[1] > 0.0 && left[2] > 0.0);
        let (f, _) = flap_force(&s.ship, 1.0, belly_down(), w, [1.0; 4]);
        assert!(f[2] > 0.0, "flaps add drag");
    }
}
