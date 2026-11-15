//! Flight control for the ship and the booster, between guidance solves.
//!
//! - **Descent** (booster): engines off, engines first. Lean the top away
//!   from where the booster should go (lift), by at most 8°, to null the
//!   predicted miss at the ignition altitude. Grid fins make the torque;
//!   cold-gas thrusters what the fins cannot.
//! - **Belly flop** (ship): engines off. Hold belly-down with the nose toward the
//!   tower (−x), so the flip carries the ship in rather than out; pitch the
//!   nose up or down (≤ 12°) to slide toward the flip point.
//!   Flaps make pitch and roll torque; cold-gas thrusters do yaw.
//! - **Powered** (flip and landing burn): fly the SCvx plan. Position and
//!   velocity errors add to the planned force; the attitude follows the
//!   plan, nudged so the thrust points along the wanted force; the engine
//!   gimbal makes pitch/yaw torque, the thrusters roll.
//! - **Flip**: every lit engine at minimum throttle; the gimbal swings the
//!   ship from belly-down to upright (scripted, rate-limited).
//! - **Terminal**: upright; slide over the catch axis / pad (x = y = 0) at
//!   no more than 2 m/s, then sink at 1 m/s once centred.
//!
//! Inputs are sensor estimates, never the truth. Simulation only.

use model::aero;
use model::dynamics::{self, State, M};
use model::quat::{self, Quat, Vec3};
use model::{Scenario, VehicleKind};

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
/// Largest body rate the attitude loop asks for from an angle error, rad/s
/// (35°/s): a quicker flip spends less time pushing the ship sideways.
pub const RATE_LIMIT: f64 = 0.610_865_238_198_015_2;
/// The attitude loop's bandwidth is capped so that this angle error (rad,
/// ~6°) does not saturate the gimbal.
const AUTHORITY_ANGLE: f64 = 0.1;
/// Belly flop attitude loop (slower: flaps are weak).
pub const WN_FLAP: f64 = 0.8;
/// Largest correction of the planned thrust direction, rad (8°). More, and
/// on one engine the gimbal cannot stop the rotation it starts.
const MAX_CORRECTION: f64 = 0.139_626_340_159_546_36;
/// Feedback acceleration caps, as fractions of g: sideways and vertical.
const LATERAL_CAP: f64 = 0.2;
const VERTICAL_CAP: f64 = 0.5;
/// Translation gains in the terminal descent.
pub const KP_TERMINAL: f64 = 0.12;
pub const KD_TERMINAL: f64 = 0.7;
/// Largest sideways acceleration in the terminal phase, m/s² (absolute: a
/// share of g was far too weak in Mars gravity).
pub const LATERAL_ACCEL: f64 = 0.4;
/// …and the booster's when slowing its slide, m/s².
pub const BRAKE_ACCEL: f64 = 1.2;
/// Largest sideways speed in the terminal phase, m/s.
pub const SLIDE: f64 = 4.0;
/// Above this height over the rest position the terminal phase sinks fast
/// (up to FAST_SINK, m/s); below it only once centred, at SINK, m.
pub const FINAL_HEIGHT: f64 = 8.0;
/// Below this height the fast descent continues only near the axis, m.
pub const APPROACH_HEIGHT: f64 = 20.0;
pub const FAST_SINK: f64 = 5.0;
/// Terminal sink rate, m/s.
pub const SINK: f64 = 1.0;

/// Largest lean of the booster's axis from the airflow in the descent,
/// rad (8°): the single yaw fin can still hold it there.
pub const MAX_LEAN: f64 = 0.139_626_340_159_546_36;
/// Descent attitude loop of the booster, rad/s.
pub const WN_FIN: f64 = 1.0;

/// The nose points along `HEADING · x` in the belly flop.
pub const HEADING: f64 = -1.0;

/// Belly down, nose along `HEADING · x`, pitched up by `pitch` (rad).
pub fn belly_attitude(pitch: f64) -> Quat {
    let along_plus_x = quat::from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2 - pitch);
    if HEADING > 0.0 {
        along_plus_x
    } else {
        quat::mul(quat::from_axis_angle([0.0, 0.0, 1.0], std::f64::consts::PI), along_plus_x)
    }
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
    // Rate-limited: the angle error asks for at most RATE_LIMIT of turning.
    let k = wn / (2.0 * ZETA);
    let mut want: Vec3 = std::array::from_fn(|i| k * e[i]);
    let n = quat::norm(want);
    if n > RATE_LIMIT {
        want = quat::scale(want, RATE_LIMIT / n);
    }
    let e: Vec3 = std::array::from_fn(|i| want[i] / k);
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
    let rho = scn.density(x[dynamics::R + 2]);
    aero::body_force(&scn.ship, rho, dynamics::attitude(x), dynamics::velocity(x)).1
}

/// Engines-first attitude for a velocity: the axis straight into the wind
/// of the fall (no lift), no twist about it.
pub fn descent_attitude(v: Vec3) -> Quat {
    let n = quat::norm(v);
    if n < 1e-6 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    turn([0.0, 0.0, 1.0], quat::scale(v, -1.0 / n), std::f64::consts::PI)
}

/// Booster, engines off: fall engines first toward `aim` (x, y) at the
/// ignition altitude, steering with body lift and the grid fins.
pub fn descent(scn: &Scenario, x: &State, aim: [f64; 2], ignition_z: f64) -> Actuation {
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    let m = x[M];
    let rho = scn.density(r[2]);
    let speed = quat::norm(v).max(1.0);
    let back = quat::scale(v, -1.0 / speed);
    // Never less than 6 s: closer in, the steering would flick from side
    // to side chasing the last metres (the landing burn takes those).
    let t_go = fall_time(scn, x, ignition_z).max(6.0);
    // Arrive over the aim point with no sideways speed left (the
    // acceleration that is linear in time and does both):
    // a = 6 (aim − r)/t² − 4 v/t. Nulling the position alone arrived
    // drifting at 20 m/s from a 45 km start, past the tower.
    let a_want: [f64; 2] = std::array::from_fn(|i| 6.0 * (aim[i] - r[i]) / (t_go * t_go) - 4.0 * v[i] / t_go);

    // Sideways acceleration per radian of lean, from the shared model
    // (negative: the lift points away from the lean).
    let eps = 0.05;
    let probe = normalize(quat::add(back, [eps, 0.0, 0.0]));
    let f0 = aero::body_force(&scn.ship, rho, descent_attitude(v), v).0;
    let f1 = aero::body_force(&scn.ship, rho, turn([0.0, 0.0, 1.0], probe, std::f64::consts::PI), v).0;
    let k = (f1[0] - f0[0]) / (m * eps);
    let mut lean = if k.abs() > 0.05 { [a_want[0] / k, a_want[1] / k] } else { [0.0, 0.0] };
    let n = lean[0].hypot(lean[1]);
    if n > MAX_LEAN {
        lean = [lean[0] * MAX_LEAN / n, lean[1] * MAX_LEAN / n];
    }
    let axis = normalize(quat::add(back, [lean[0], lean[1], 0.0]));
    let q_des = turn([0.0, 0.0, 1.0], axis, std::f64::consts::PI);

    let tau = attitude_torque(scn, x, q_des, [0.0; 3], WN_FIN);
    let body = aero_torque(scn, x);
    let need: Vec3 = std::array::from_fn(|i| tau[i] - body[i]);
    let gain = aero::fin_gain(&scn.ship, rho, dynamics::attitude(x), v);
    let (fins, made) = aero::fin_allocate(&scn.ship, gain, need);
    let rest = std::array::from_fn(|i| need[i] - made[i]);
    Actuation {
        thrust_body: [0.0; 3],
        flaps: fins,
        rcs: rcs_request(scn, rest),
    }
}

/// The unpowered phase of either vehicle toward the scenario's aim point.
pub fn unpowered(scn: &Scenario, x: &State, ignition_z: f64) -> Actuation {
    match scn.ship.kind {
        VehicleKind::Ship => belly_flop(scn, x, scn.aim[0], ignition_z),
        VehicleKind::Booster => descent(scn, x, scn.aim, ignition_z),
    }
}

/// Engines off: belly-flop toward `target_x` at the ignition altitude.
pub fn belly_flop(scn: &Scenario, x: &State, target_x: f64, ignition_z: f64) -> Actuation {
    let g = scn.planet.gravity();
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    let t_go = ((r[2] - ignition_z) / (-v[2]).max(10.0)).max(2.0);
    let miss = target_x - (r[0] + v[0] * t_go);
    // Pitching the nose up by θ slides the ship toward its tail (−HEADING·x)
    // at about g·sin θ when drag carries the weight.
    let a_want = (2.0 * miss / (t_go * t_go)).clamp(-g, g);
    let pitch = (-HEADING * a_want / g).clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);

    let mut tau = attitude_torque(scn, x, belly_attitude(pitch), [0.0; 3], WN_FLAP);
    // What the body and the trimmed flaps do anyway.
    let q = dynamics::attitude(x);
    let rho = scn.density(r[2]);
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
    let fb = quat::rotate_inv(q, force);
    let l = scn.ship.cm_height;
    let tz = fb[2].clamp(limits.0, limits.1);
    let wn = bandwidth(scn, x, tz);
    let tau = attitude_torque(scn, x, q_des, w_des, wn);
    let need: Vec3 = std::array::from_fn(|i| tau[i] - body[i]);
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
        flaps: [scn.ship.surface_trim(); 4],
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
    // Steering sideways goes through the attitude, and gimballing to turn
    // first pushes the base the wrong way: a right-half-plane zero at
    // √(l·(T/m)/(J/m)) rad/s. The sideways loop must stay well below it,
    // or it weaves (seen on the booster's three-engine leg: zero at 0.9).
    let k = translation_gain(scn, x, quat::norm(ur));
    let mut a: Vec3 = [
        k * k * KP * dr[0] + k * KD * dv[0],
        k * k * KP * dr[1] + k * KD * dv[1],
        KP * dr[2] + KD * dv[2],
    ];
    let side = a[0].hypot(a[1]);
    if side > LATERAL_CAP * g {
        a[0] *= LATERAL_CAP * g / side;
        a[1] *= LATERAL_CAP * g / side;
    }
    a[2] = a[2].clamp(-VERTICAL_CAP * g, VERTICAL_CAP * g);
    let force = quat::add(f_plan, quat::scale(a, m));
    // Turn the planned attitude so its thrust line meets the wanted force.
    let planned_dir = quat::rotate(qr, normalize(ur));
    let q_des = quat::normalize(quat::mul(turn(planned_dir, normalize(force), MAX_CORRECTION), qr));
    fly_force(scn, x, force, q_des, dynamics::rates(xr), limits)
}

/// Scale (≤ 1) for the sideways position loop so that it stays below the
/// vehicle's non-minimum-phase zero. 1 for the ship (unchanged); below 1
/// only for the booster on few engines.
fn translation_gain(scn: &Scenario, x: &State, thrust: f64) -> f64 {
    if scn.ship.kind == VehicleKind::Ship {
        return 1.0;
    }
    let j = scn.ship.inertia_per_kg[0].max(scn.ship.inertia_per_kg[1]);
    let zero = (scn.ship.cm_height * thrust / x[M] / j).sqrt();
    (zero / NMP_REFERENCE).clamp(0.3, 1.0)
}

/// The sideways loop runs at full gain when the non-minimum-phase zero is
/// at least this fast, rad/s.
const NMP_REFERENCE: f64 = 1.6;

/// Attitude-loop bandwidth the gimbal can actually give at thrust `tz`,
/// rad/s: a heavy ship on one engine (Mars) saturates a fixed-gain loop and
/// wobbles.
fn bandwidth(scn: &Scenario, x: &State, tz: f64) -> f64 {
    let j = scn.ship.inertia(x[M]);
    let alpha = scn.ship.cm_height * scn.ship.gimbal_max.tan() * tz / j[0].max(j[1]);
    WN.min((alpha / AUTHORITY_ANGLE).sqrt()).max(0.3)
}

/// The flip: every lit engine at minimum throttle along the body axis, the
/// gimbal swinging the ship upright at no more than [`RATE_LIMIT`].
pub fn flip(scn: &Scenario, x: &State, limits: (f64, f64)) -> Actuation {
    let q = dynamics::attitude(x);
    let force = quat::rotate(q, [0.0, 0.0, limits.0]);
    fly_force(scn, x, force, crate::scvx::upright_like(q), [0.0; 3], limits)
}

/// Upright; over the catch axis / pad (x = y = 0), then sinking at [`SINK`].
pub fn terminal(scn: &Scenario, x: &State, limits: (f64, f64)) -> Actuation {
    let m = x[M];
    let g = scn.planet.gravity();
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    // Gentler than while powered: one engine near hover has little gimbal
    // torque, and a tight loop through the attitude limit-cycles.
    // Descend while sliding in (briskly high up, hovering costs propellant);
    // the last FINAL_HEIGHT only once centred and steady over the axis.
    let off = r[0].hypot(r[1]);
    let h = r[2] - scn.rest_cm_z();
    let side = v[0].hypot(v[1]);
    let sink = if h > APPROACH_HEIGHT || (h > FINAL_HEIGHT && off < 3.0 * scn.centring) {
        (0.1 * h).clamp(SINK, FAST_SINK)
    } else if h > FINAL_HEIGHT {
        0.0
    } else {
        // Full sink rate when centred, a slow one when nearly so, a hover
        // when far off (a hard centred-and-still test hovered for ages).
        // Never sink onto the arms or legs while still sliding.
        if side > scn.settle_side_speed {
            0.0
        } else if off < scn.centring {
            SINK
        } else if off < 2.0 * scn.centring {
            0.3 * SINK
        } else {
            0.0
        }
    };
    // The sideways loop runs through the attitude loop: slow it down with
    // it, or it circles the axis (seen on Mars).
    let k = (bandwidth(scn, x, m * g) / WN).min(translation_gain(scn, x, m * g));
    let kd = KD_TERMINAL * k;
    // Velocity command toward the axis, at most SLIDE m/s.
    let mut want = [-KP_TERMINAL / KD_TERMINAL * k * r[0], -KP_TERMINAL / KD_TERMINAL * k * r[1]];
    // Close enough: only damp the slide, don't keep chasing the exact
    // axis (that position loop circled the pad on Mars).
    if off < scn.centring {
        want = [0.0, 0.0];
    }
    let n = want[0].hypot(want[1]);
    if n > SLIDE {
        want = [want[0] * SLIDE / n, want[1] * SLIDE / n];
    }
    let a = [
        kd * (want[0] - v[0]),
        kd * (want[1] - v[1]),
        2.0 * (-sink - v[2]),
    ];
    // One cap throughout for the ship: switching between two acted like a
    // relay and it circled the pad. The booster may brake harder than it
    // speeds up: it slides in toward the mast, and at the gentle cap a
    // 3 m/s arrival needed 11 m to stop.
    let braking = a[0] * v[0] + a[1] * v[1] < 0.0;
    let lat_cap = if braking && scn.ship.kind == VehicleKind::Booster { BRAKE_ACCEL } else { LATERAL_ACCEL };
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

/// Seconds until the booster, falling engines first with drag, reaches
/// height `z_to` (1-D). At the present sink rate alone the estimate was
/// half the truth high up, where the air has yet to slow the fall.
pub fn fall_time(scn: &Scenario, x: &State, z_to: f64) -> f64 {
    let g = scn.planet.gravity();
    let m = x[M];
    let z0 = x[dynamics::R + 2];
    let mut speed = (-dynamics::velocity(x)[2]).max(10.0);
    if z0 <= z_to {
        return 0.0;
    }
    let drag = quat::norm(aero::body_force(&scn.ship, 1.0, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -1.0]).0);
    const STEPS: usize = 80;
    let dz = (z0 - z_to) / STEPS as f64;
    let mut t = 0.0;
    for i in 0..STEPS {
        let z = z0 - (i as f64 + 0.5) * dz;
        let a = g - scn.density(z) * drag * speed * speed / m;
        let next = (speed * speed + 2.0 * a * dz).max(100.0).sqrt();
        t += dz / (0.5 * (speed + next));
        speed = next;
    }
    t
}

/// Height to light the engines: enough to stop at 70 % of full thrust,
/// with margin, never below the scenario's floor.
///
/// Ship: from the present speed, thrust alone (it falls at a steady speed
/// and flips before braking).
///
/// Booster: it may be falling at three times the speed it will have when
/// the thick air has slowed it, and the air goes on braking it during the
/// burn. So both are integrated in 1-D: the fall from here with drag
/// (speed at each height), and, upward from the gate, the fastest speed
/// from which thrust plus drag still stops in time. The engines light
/// where the two meet. (Judging by the present speed and thrust alone lit
/// them 12 km up from a 45 km start, and the tanks ran dry.)
pub fn ignition_altitude(scn: &Scenario, x: &State, max_thrust: f64) -> f64 {
    let g = scn.planet.gravity();
    let m = x[M];
    let decel = (0.7 * max_thrust / m - g).max(0.5);
    let z0 = x[dynamics::R + 2];
    let vz = dynamics::velocity(x)[2].min(0.0);
    if scn.ship.kind == VehicleKind::Ship {
        let need = |v2: f64| scn.gate[2] + IGNITION_MARGIN * v2 / (2.0 * decel);
        let now = need(vz * vz);
        // A steep, fast entry is faster than it could stop from: the air
        // will slow it first. So also judge by the speed the fall predicts
        // (belly-first drag), and take the later of the two. Where the
        // air is thin and the fall speeds up (Mars), the simple rule wins.
        let drag = quat::norm(aero::body_force(&scn.ship, 1.0, belly_attitude(0.0), [0.0, 0.0, -1.0]).0);
        let step = ((z0 - scn.gate[2]) / 200.0).max(5.0);
        let (mut z, mut v2) = (z0, vz * vz);
        // The lowest height where "can still stop" turns into "cannot".
        let mut ignite = now;
        let mut could = false;
        while z > scn.gate[2] {
            let a = g - scn.density(z) * drag * v2 / m;
            v2 = (v2 + 2.0 * a * step).max(0.0);
            z -= step;
            let can = z > need(v2);
            if could && !can {
                ignite = need(v2);
            }
            could = can;
        }
        return ignite.min(now).max(scn.ignition_floor);
    }
    // Axial drag per unit of ρ·speed² (engines first, falling and braking).
    let drag = quat::norm(aero::body_force(&scn.ship, 1.0, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -1.0]).0);
    let gate = scn.gate[2];
    if z0 <= gate {
        return z0.max(scn.ignition_floor);
    }
    const STEPS: usize = 160;
    let dz = (z0 - gate) / STEPS as f64;
    // Fastest speed² at each height from which the burn still stops.
    let mut stop = [0.0f64; STEPS + 1];
    for i in 1..=STEPS {
        let z = gate + (i as f64 - 0.5) * dz;
        let a = decel + scn.density(z) * drag * stop[i - 1] / m;
        stop[i] = stop[i - 1] + 2.0 * a * dz;
    }
    // The fall from here, top down.
    let mut speed2 = vz * vz;
    for i in (1..=STEPS).rev() {
        let z = gate + i as f64 * dz;
        if IGNITION_MARGIN * speed2 >= stop[i] {
            return z.max(scn.ignition_floor);
        }
        let a = g - scn.density(z - 0.5 * dz) * drag * speed2 / m;
        speed2 = (speed2 + 2.0 * a * dz).max(0.0);
    }
    gate.max(scn.ignition_floor)
}

/// The burn is planned to stop in 1/1.4 of the height it has.
const IGNITION_MARGIN: f64 = 1.4;

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
            let rho = scn.density(x[R + 2]);
            let (ff, ft) = match s.kind {
                VehicleKind::Ship => aero::flap_force(s, rho, q, dynamics::velocity(&x), c.flaps),
                VehicleKind::Booster => aero::fin_force(s, rho, q, dynamics::velocity(&x), c.flaps),
            };
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
    fn descent_holds_engines_first_and_steers_to_the_aim() {
        let scn = Scenario::booster();
        let v0 = [-20.0, 10.0, -380.0];
        // Start tipped 5° off and turning.
        let q0 = quat::mul(descent_attitude(v0), quat::from_axis_angle([0.0, 1.0, 0.0], 0.09));
        let x0 = dynamics::state(scn.start_mass, [600.0, -200.0, 12_000.0], v0, q0, [0.0, 0.03, 0.0]);
        let ign = 1800.0;
        let mut x = x0;
        // Fly until the ignition altitude.
        while x[R + 2] > ign {
            x = fly(&scn, x, 0.5, |x| descent(&scn, x, scn.aim, ign));
        }
        let off = (x[R] - scn.aim[0]).hypot(x[R + 1] - scn.aim[1]);
        assert!(off < 120.0, "missed the aim by {off} m: {:?}", dynamics::position(&x));
        let tilt = quat::tilt(dynamics::attitude(&x)).to_degrees();
        assert!(tilt < 15.0, "tilt {tilt}");
        assert!(quat::norm(dynamics::rates(&x)) < 0.1);
        assert!((250.0..450.0).contains(&-x[V + 2]), "falling at {}", -x[V + 2]);
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
    fn terminal_slides_in_then_settles_slowly() {
        let scn = Scenario::earth();
        let rest = scn.rest_cm_z();
        // 30 m above the arms' catch position, 6 m out.
        let x0 = dynamics::state(110e3, [6.0, 0.0, rest + 30.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0, 0.0], [0.0; 3]);
        let x = fly(&scn, x0, 20.0, |x| terminal(&scn, x, (0.98e6, 2.45e6)));
        let off = x[R].hypot(x[R + 1]);
        assert!(off < 1.0, "off axis by {off}");
        assert!(x[R + 2] < rest + 8.0 && x[R + 2] > rest, "z {}", x[R + 2]);
        assert!(quat::tilt(dynamics::attitude(&x)).to_degrees() < 2.0);
        assert!((x[V + 2] + SINK).abs() < 0.3, "sinking at {}", x[V + 2]);
        let _ = W;
    }

    #[test]
    fn ignition_altitude_earth_floor_and_mars_physics() {
        let e = Scenario::earth();
        let x = dynamics::state(e.start_mass, [0.0, 0.0, 3000.0], [0.0, 0.0, -90.0], belly_attitude(0.0), [0.0; 3]);
        assert_eq!(ignition_altitude(&e, &x, 7.35e6), e.ignition_floor);
        let m = Scenario::mars();
        let x = dynamics::state(m.start_mass, [0.0, 0.0, 12000.0], [0.0, 0.0, -450.0], belly_attitude(0.0), [0.0; 3]);
        let z = ignition_altitude(&m, &x, 7.35e6);
        assert!((5000.0..14000.0).contains(&z), "{z}");
    }
}
