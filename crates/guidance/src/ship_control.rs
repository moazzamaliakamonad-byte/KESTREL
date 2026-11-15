//! Flight control for the ship, between guidance solves.
//!
//! - **Belly flop**: engines off. Hold belly-down with the nose along +x;
//!   pitch the nose up or down (≤ 12°) to slide toward the flip point.
//!   Flaps make pitch and roll torque; cold-gas thrusters do yaw.
//! - **Powered** (flip and landing burn): fly the SCvx plan. Position and
//!   velocity errors add to the planned force; the attitude follows the
//!   plan, nudged so the thrust points along the wanted force; the engine
//!   gimbal makes pitch/yaw torque, the thrusters roll.
//! - **Terminal**: upright over the gate axis, sinking at 1 m/s.
//!
//! Inputs are sensor estimates, never the truth. Simulation only.

use model::aero;
use model::dynamics::{self, State, M};
use model::quat::{self, Quat, Vec3};
use model::Scenario;

/// What the controller asks of the actuators (mirrors `sim::ship::Command`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Actuation {
    pub thrust_body: Vec3,
    pub flaps: [f64; 4],
    pub rcs: Vec3,
}

/// Neutral flap exposure.
pub const FLAP_TRIM: f64 = 0.5;
/// Largest belly-flop pitch offset, rad (12°).
pub const MAX_PITCH: f64 = 0.209_439_510_239_319_55;
/// Translation gains while powered.
pub const KP: f64 = 0.4;
pub const KD: f64 = 1.2;
/// Attitude loop: natural frequency (rad/s) and damping.
pub const WN: f64 = 1.5;
pub const ZETA: f64 = 0.8;
/// Belly flop attitude loop (slower: flaps are weak).
pub const WN_FLAP: f64 = 0.8;
/// Largest correction of the planned thrust direction, rad (20°).
const MAX_CORRECTION: f64 = 0.349_065_850_398_865_9;
/// Translation gains in the terminal descent.
pub const KP_TERMINAL: f64 = 0.12;
pub const KD_TERMINAL: f64 = 0.7;
/// Terminal sink rate, m/s.
pub const SINK: f64 = 1.0;

/// Belly down, nose along +x, nose pitched up by `pitch` (rad).
pub fn belly_attitude(pitch: f64) -> Quat {
    quat::from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2 - pitch)
}

/// Rotation vector (body axes) from `q` to `q_des`.
fn attitude_error(q: Quat, q_des: Quat) -> Vec3 {
    let mut e = quat::mul(quat::conj(q), q_des);
    if e[0] < 0.0 {
        e = e.map(|c| -c);
    }
    let s = (e[1] * e[1] + e[2] * e[2] + e[3] * e[3]).sqrt();
    if s < 1e-12 {
        return [0.0; 3];
    }
    let angle = 2.0 * s.atan2(e[0]);
    [e[1] / s * angle, e[2] / s * angle, e[3] / s * angle]
}

/// Body torque that drives `x` toward attitude `q_des` and rate `w_des`.
fn attitude_torque(scn: &Scenario, x: &State, q_des: Quat, w_des: Vec3, wn: f64) -> Vec3 {
    let q = dynamics::attitude(x);
    let w = dynamics::rates(x);
    let e = attitude_error(q, q_des);
    let j = scn.ship.inertia(x[M]);
    let jw = [j[0] * w[0], j[1] * w[1], j[2] * w[2]];
    let gyro = quat::cross(w, jw);
    std::array::from_fn(|i| j[i] * (wn * wn * e[i] + 2.0 * ZETA * wn * (w_des[i] - w[i])) + gyro[i])
}

fn rcs_request(scn: &Scenario, torque: Vec3) -> Vec3 {
    let s = &scn.ship;
    let arm = [
        s.rcs_thrust * (s.rcs_fore - s.rcs_aft),
        s.rcs_thrust * (s.rcs_fore - s.rcs_aft),
        2.0 * s.rcs_thrust * s.radius,
    ];
    std::array::from_fn(|i| (torque[i] / arm[i]).clamp(-1.0, 1.0))
}

/// Body aerodynamic torque at the estimate (no wind known).
fn aero_torque(scn: &Scenario, x: &State) -> Vec3 {
    let rho = scn.planet.density(x[dynamics::R + 2]);
    aero::body_force(&scn.ship, rho, dynamics::attitude(x), dynamics::velocity(x)).1
}

/// Engines off: belly-flop toward `target_x` at the ignition altitude.
pub fn belly_flop(scn: &Scenario, x: &State, target_x: f64, ignition_z: f64) -> Actuation {
    let g = scn.planet.gravity();
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    let t_go = ((r[2] - ignition_z) / (-v[2]).max(10.0)).max(2.0);
    let miss = target_x - (r[0] + v[0] * t_go);
    // Pitching the nose up by θ slides the ship toward its tail (−x) at
    // about g·sin θ when drag carries the weight.
    let a_want = (2.0 * miss / (t_go * t_go)).clamp(-g, g);
    let pitch = (-a_want / g).clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);

    let mut tau = attitude_torque(scn, x, belly_attitude(pitch), [0.0; 3], WN_FLAP);
    // What the body and the trimmed flaps do anyway.
    let q = dynamics::attitude(x);
    let rho = scn.planet.density(r[2]);
    let trim = aero::flap_force(&scn.ship, rho, q, v, [FLAP_TRIM; 4]).1;
    let body = aero_torque(scn, x);
    tau = std::array::from_fn(|i| tau[i] - trim[i] - body[i]);

    // Per-unit-exposure torque of the flaps here.
    let wn = {
        let wb = quat::rotate_inv(q, v);
        wb[0].hypot(wb[1])
    };
    let s = &scn.ship;
    let k = 0.5 * rho * s.c_flap * wn * wn;
    let pitch_gain = k * 2.0 * (s.fore.area * s.fore.station + s.aft.area * -s.aft.station);
    let roll_gain = k * 2.0 * s.flap_y * (s.fore.area + s.aft.area);
    let mut flaps = [FLAP_TRIM; 4];
    let mut rest = tau;
    if pitch_gain > 1.0 {
        // Aft out / fore in pitches the nose down (+τ_y).
        let d = (tau[1] / pitch_gain).clamp(-0.45, 0.45);
        let rl = (tau[2] / roll_gain).clamp(-0.3, 0.3);
        flaps = [
            FLAP_TRIM - d + rl,
            FLAP_TRIM - d - rl,
            FLAP_TRIM + d + rl,
            FLAP_TRIM + d - rl,
        ];
        rest[1] -= d * pitch_gain;
        rest[2] -= rl * roll_gain;
    }
    Actuation {
        thrust_body: [0.0; 3],
        flaps,
        rcs: rcs_request(scn, rest),
    }
}

/// Thrust and torque that make force `force` (inertial, N) while turning
/// toward `q_des` at rate `w_des`.
fn fly_force(scn: &Scenario, x: &State, force: Vec3, q_des: Quat, w_des: Vec3, limits: (f64, f64)) -> Actuation {
    let q = dynamics::attitude(x);
    let body = aero_torque(scn, x);
    let tau = attitude_torque(scn, x, q_des, w_des, WN);
    let need: Vec3 = std::array::from_fn(|i| tau[i] - body[i]);
    let fb = quat::rotate_inv(q, force);
    let l = scn.ship.cm_height;
    let tz = fb[2].clamp(limits.0, limits.1);
    // r_T × T = (l T_y, −l T_x, 0).
    let mut lat = [-need[1] / l, need[0] / l];
    let cap = scn.ship.gimbal_max.tan() * tz;
    let n = lat[0].hypot(lat[1]);
    if n > cap {
        lat = [lat[0] * cap / n, lat[1] * cap / n];
    }
    let used: Vec3 = [l * lat[1], -l * lat[0], 0.0];
    let rest = std::array::from_fn(|i| need[i] - used[i]);
    Actuation {
        thrust_body: [lat[0], lat[1], tz],
        flaps: [FLAP_TRIM; 4],
        rcs: rcs_request(scn, rest),
    }
}

/// Fly the plan: `reference` = (planned state, planned body thrust) now.
pub fn powered(scn: &Scenario, x: &State, reference: (&State, Vec3), limits: (f64, f64)) -> Actuation {
    let (xr, ur) = reference;
    let m = x[M];
    let g = scn.planet.gravity();
    let qr = dynamics::attitude(xr);
    let f_plan = quat::rotate(qr, ur);
    let dr = quat::sub(dynamics::position(xr), dynamics::position(x));
    let dv = quat::sub(dynamics::velocity(xr), dynamics::velocity(x));
    let mut a: Vec3 = std::array::from_fn(|i| KP * dr[i] + KD * dv[i]);
    let an = quat::norm(a);
    if an > 0.5 * g {
        a = quat::scale(a, 0.5 * g / an);
    }
    let force = quat::add(f_plan, quat::scale(a, m));
    // Turn the planned attitude so its thrust line meets the wanted force.
    let planned_dir = quat::rotate(qr, normalize(ur));
    let q_des = quat::normalize(quat::mul(turn(planned_dir, normalize(force), MAX_CORRECTION), qr));
    fly_force(scn, x, force, q_des, dynamics::rates(xr), limits)
}

/// Upright over the gate axis, sinking at [`SINK`].
pub fn terminal(scn: &Scenario, x: &State, limits: (f64, f64)) -> Actuation {
    let m = x[M];
    let g = scn.planet.gravity();
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    // Gentler than while powered: one engine near hover has little gimbal
    // torque, and a tight loop through the attitude limit-cycles.
    let a = [
        KP_TERMINAL * (scn.gate[0] - r[0]) - KD_TERMINAL * v[0],
        KP_TERMINAL * (scn.gate[1] - r[1]) - KD_TERMINAL * v[1],
        2.0 * (-SINK - v[2]),
    ];
    let lat_cap = 0.06 * g;
    let h = a[0].hypot(a[1]);
    let a = if h > lat_cap {
        [a[0] * lat_cap / h, a[1] * lat_cap / h, a[2]]
    } else {
        a
    };
    let force = [m * a[0], m * a[1], m * (a[2] + g)];
    let q = dynamics::attitude(x);
    let up = crate::scvx::upright_like(q);
    let q_des = quat::normalize(quat::mul(turn([0.0, 0.0, 1.0], normalize(force), MAX_CORRECTION), up));
    fly_force(scn, x, force, q_des, [0.0; 3], limits)
}

/// Height to light the engines: enough to stop at 70 % of full thrust,
/// with margin, never below the scenario's floor.
pub fn ignition_altitude(scn: &Scenario, x: &State, max_thrust: f64) -> f64 {
    let g = scn.planet.gravity();
    let vz = dynamics::velocity(x)[2].min(0.0);
    let decel = (0.7 * max_thrust / x[M] - g).max(0.5);
    (scn.gate[2] + 1.4 * vz * vz / (2.0 * decel)).max(scn.ignition_floor)
}

fn normalize(v: Vec3) -> Vec3 {
    let n = quat::norm(v);
    if n > 1e-12 { quat::scale(v, 1.0 / n) } else { [0.0, 0.0, 1.0] }
}

/// Rotation taking unit `from` toward unit `to`, by at most `max` rad.
fn turn(from: Vec3, to: Vec3, max: f64) -> Quat {
    let axis = quat::cross(from, to);
    let angle = quat::dot(from, to).clamp(-1.0, 1.0).acos().min(max);
    if quat::norm(axis) < 1e-12 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    quat::from_axis_angle(axis, angle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::dynamics::{Extra, R, V, W};

    /// Integrate the shared model with flaps and RCS as commanded (instant
    /// actuators): a stand-in for the simulator, which guidance can't use.
    fn fly(scn: &Scenario, mut x: State, seconds: f64, ctl: impl Fn(&State) -> Actuation) -> State {
        let h = 0.01;
        let s = &scn.ship;
        let arm = [
            s.rcs_thrust * (s.rcs_fore - s.rcs_aft),
            s.rcs_thrust * (s.rcs_fore - s.rcs_aft),
            2.0 * s.rcs_thrust * s.radius,
        ];
        for _ in 0..(seconds / h) as usize {
            let c = ctl(&x);
            let q = dynamics::attitude(&x);
            let rho = scn.planet.density(x[R + 2]);
            let (ff, ft) = aero::flap_force(s, rho, q, dynamics::velocity(&x), c.flaps);
            let rcs: Vec3 = std::array::from_fn(|i| if c.rcs[i].abs() > 0.2 { c.rcs[i].signum() * arm[i] } else { 0.0 });
            let extra = Extra {
                force: ff,
                torque: quat::add(ft, rcs),
                wind: [0.0; 3],
            };
            let d = dynamics::f(scn, &x, c.thrust_body, &extra);
            for i in 0..x.len() {
                x[i] += h * d[i];
            }
            let q = quat::normalize(dynamics::attitude(&x));
            x[dynamics::Q..dynamics::Q + 4].copy_from_slice(&q);
        }
        x
    }

    #[test]
    fn belly_flop_holds_attitude() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(scn.start_mass, [0.0, 0.0, 6000.0], [0.0, 0.0, -80.0], belly_attitude(0.05), [0.0, 0.02, 0.0]);
        let x = fly(&scn, x0, 30.0, |x| belly_flop(&scn, x, 0.0, 600.0));
        let tilt = quat::tilt(dynamics::attitude(&x)).to_degrees();
        assert!((90.0 - tilt).abs() < 13.0, "tilt {tilt}");
        assert!(quat::norm(dynamics::rates(&x)) < 0.05);
    }

    #[test]
    fn belly_flop_steers_toward_target() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(scn.start_mass, [-400.0, 0.0, 6000.0], [0.0, 0.0, -80.0], belly_attitude(0.0), [0.0; 3]);
        let x = fly(&scn, x0, 60.0, |x| belly_flop(&scn, x, 0.0, 600.0));
        let r = dynamics::position(&x);
        assert!(r[0].abs() < 150.0, "x {} at z {}", r[0], r[2]);
    }

    #[test]
    fn powered_zero_error_returns_plan_thrust() {
        let scn = Scenario::earth();
        let x = dynamics::state(120e3, [0.0, 0.0, 300.0], [0.0, 0.0, -30.0], [1.0, 0.0, 0.0, 0.0], [0.0; 3]);
        let u = [0.0, 0.0, 2.0e6];
        let a = powered(&scn, &x, (&x, u), (0.98e6, 7.35e6));
        assert!((a.thrust_body[2] - 2.0e6).abs() / 2.0e6 < 1e-3, "{:?}", a.thrust_body);
        assert!(a.thrust_body[0].abs() < 2e4 && a.thrust_body[1].abs() < 2e4);
    }

    #[test]
    fn attitude_error_commands_gimbal_sign() {
        // Nose tipped toward +x (a + rotation about y): righting it needs
        // a −y torque, which needs the thrust pushed toward +x (T_x > 0).
        let scn = Scenario::earth();
        let q = quat::from_axis_angle([0.0, 1.0, 0.0], 0.1);
        let x = dynamics::state(120e3, [0.0, 0.0, 300.0], [0.0; 3], q, [0.0; 3]);
        let a = terminal(&scn, &x, (0.98e6, 7.35e6));
        assert!(a.thrust_body[0] > 0.0, "{:?}", a.thrust_body);
    }

    #[test]
    fn terminal_sinks_at_one_metre_per_second() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(110e3, [0.5, 0.0, 200.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0, 0.0], [0.0; 3]);
        let x = fly(&scn, x0, 10.0, |x| terminal(&scn, x, (0.98e6, 2.45e6)));
        assert!((x[R + 2] - 190.0).abs() < 1.5, "z {}", x[R + 2]);
        assert!((x[V + 2] + 1.0).abs() < 0.2);
        assert!(quat::norm([x[W], x[W + 1], x[W + 2]]) < 0.02, "rates {:?}", &x[W..]);
    }

    #[test]
    fn ignition_altitude_earth_floor_and_mars_physics() {
        let e = Scenario::earth();
        let x = dynamics::state(e.start_mass, [0.0, 0.0, 3000.0], [0.0, 0.0, -90.0], belly_attitude(0.0), [0.0; 3]);
        assert_eq!(ignition_altitude(&e, &x, 7.35e6), 600.0);
        let m = Scenario::mars();
        let x = dynamics::state(m.start_mass, [0.0, 0.0, 12000.0], [0.0, 0.0, -450.0], belly_attitude(0.0), [0.0; 3]);
        let z = ignition_altitude(&m, &x, 7.35e6);
        assert!((5000.0..12000.0).contains(&z), "{z}");
    }
}
