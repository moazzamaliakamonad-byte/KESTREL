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
    // Crossflow drag (∝ sin²α) plus linear lift (∝ sin α cos α).
    let k = -0.5 * rho * (ship.c_n * s_n * quat::norm(wn) + ship.c_n_alpha * s_a * wa.abs());
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

/// Unit radial directions of the grid fins in body axes: +y, −y, +x
/// (a 90/90/180° arrangement; the ±y pair carries the catch hardpoints).
pub const FIN_RADIAL: [Vec3; crate::vehicle::FINS] = [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]];

/// The force of one fin per radian of deflection, N/rad: dynamic pressure
/// of the axial flow × fin area × slope.
pub fn fin_gain(ship: &Ship, rho: f64, q: Quat, w_air: Vec3) -> f64 {
    let Some(f) = ship.fins else { return 0.0 };
    let (wa, _) = split(q, w_air);
    0.5 * rho * wa * wa * f.area * f.c_delta
}

/// Control force (inertial, N) and torque (body, N·m) of the grid fins
/// deflected by `deflection` (rad). A fin turns about its radial axis and
/// pushes tangentially: `F_i = k δ_i (b_z × e_r,i)`. Their lift at an
/// angle of attack is part of the body model (`c_n_alpha`).
pub fn fin_force(ship: &Ship, rho: f64, q: Quat, w_air: Vec3, deflection: [f64; FLAPS]) -> (Vec3, Vec3) {
    let Some(f) = ship.fins else { return ([0.0; 3], [0.0; 3]) };
    let k = fin_gain(ship, rho, q, w_air);
    let mut force = [0.0; 3];
    let mut torque = [0.0; 3];
    for (i, e_r) in FIN_RADIAL.iter().enumerate() {
        let d = deflection[i].clamp(-f.max_deflection, f.max_deflection);
        let fi = quat::scale(quat::cross([0.0, 0.0, 1.0], *e_r), k * d);
        let at = quat::add(quat::scale(*e_r, f.radius), [0.0, 0.0, f.station]);
        force = quat::add(force, fi);
        torque = quat::add(torque, quat::cross(at, fi));
    }
    (quat::rotate(q, force), torque)
}

/// Fin deflections (rad) that make body torque `tau` (N·m) as well as the
/// fins can with gain `k` (N/rad), and the torque they actually make.
/// Pitch (about y) comes from the ±y pair, yaw (about x) from the +x fin,
/// roll from all three.
pub fn fin_allocate(ship: &Ship, k: f64, tau: Vec3) -> ([f64; FLAPS], Vec3) {
    let Some(f) = ship.fins else { return ([0.0; FLAPS], [0.0; 3]) };
    if k * f.station.abs() < 1.0 {
        return ([0.0; FLAPS], [0.0; 3]);
    }
    let lim = f.max_deflection;
    // τ_x = −k z δ2; τ_y = k z (δ1 − δ0); τ_z = k R (δ0 + δ1 + δ2).
    let d2 = (-tau[0] / (k * f.station)).clamp(-lim, lim);
    let diff = (tau[1] / (k * f.station)).clamp(-2.0 * lim, 2.0 * lim);
    let room = lim - diff.abs() / 2.0;
    let mean = ((tau[2] / (k * f.radius) - d2) / 2.0).clamp(-room, room);
    let d = [mean - diff / 2.0, mean + diff / 2.0, d2, 0.0];
    let made = [
        -k * f.station * d[2],
        k * f.station * (d[1] - d[0]),
        k * f.radius * (d[0] + d[1] + d[2]),
    ];
    (d, made)
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
    fn booster_terminal_speed_and_weathercock() {
        let s = Scenario::booster();
        let up = [1.0, 0.0, 0.0, 0.0];
        let weight = s.start_mass * Planet::Earth.gravity();
        let drag = |v: f64| body_force(&s.ship, 1.2, up, [0.0, 0.0, -v]).0[2];
        let (mut lo, mut hi) = (1.0, 2000.0);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if drag(mid) < weight { lo = mid } else { hi = mid }
        }
        assert!((280.0..360.0).contains(&lo), "terminal speed {lo}");
        // Top tipped toward +x (a + rotation about y) while falling: the
        // air pushes it back (−y torque) and the lift points to −x.
        let q = quat::from_axis_angle([0.0, 1.0, 0.0], 0.1);
        let (f, t) = body_force(&s.ship, 1.0, q, [0.0, 0.0, -300.0]);
        assert!(t[1] < 0.0, "{t:?}");
        assert!(f[0] < 0.0, "{f:?}");
    }

    #[test]
    fn fins_make_the_torque_asked_for() {
        let s = Scenario::booster();
        let up = [1.0, 0.0, 0.0, 0.0];
        let w = [0.0, 0.0, -300.0];
        let k = fin_gain(&s.ship, 1.0, up, w);
        for tau in [[2e6, 0.0, 0.0], [0.0, -3e6, 0.0], [0.0, 0.0, 4e5], [1e6, 2e6, -3e5]] {
            let (d, made) = fin_allocate(&s.ship, k, tau);
            let (_, t) = fin_force(&s.ship, 1.0, up, w, d);
            for i in 0..3 {
                assert!((t[i] - tau[i]).abs() < 1.0, "{tau:?} gave {t:?}");
                assert!((made[i] - t[i]).abs() < 1.0);
            }
        }
        // Too much: clamped to the deflection limit, same sign.
        let (d, made) = fin_allocate(&s.ship, k, [0.0, 1e12, 0.0]);
        let lim = s.ship.fins.unwrap().max_deflection;
        assert!(d.iter().all(|x| x.abs() <= lim + 1e-12) && made[1] > 0.0);
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
