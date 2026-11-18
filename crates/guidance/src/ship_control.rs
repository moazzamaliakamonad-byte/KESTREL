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
use model::dynamics::{self, M, State};
use model::quat::{self, Quat, Vec3};
use model::{Scenario, Ship, VehicleKind};

/// What the controller asks of the actuators (mirrors `sim::ship::Command`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Actuation {
    pub thrust_body: Vec3,
    pub flaps: [f64; 4],
    pub rcs: Vec3,
}

/// The air as guidance models it: the estimated wind (inertial, m/s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Air {
    pub wind: Vec3,
}

impl Air {
    pub const STILL: Air = Air { wind: [0.0; 3] };
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
/// Velocity-loop gain in the terminal descent, 1/s.
pub const KD_TERMINAL: f64 = 0.7;
/// Largest sideways acceleration in the terminal phase, m/s².
pub const LATERAL_ACCEL: f64 = 0.4;
/// …and the booster's when slowing its slide, m/s².
pub const BRAKE_ACCEL: f64 = 1.2;
/// Largest sideways speed in the terminal phase, m/s.
pub const SLIDE: f64 = 3.0;
/// …and per metre from the axis close in, 1/s.
pub const SLIDE_GAIN: f64 = 0.2;
/// Far from the axis the slide is as fast as this braking (m/s²) can
/// still stop at the edge of the centred zone.
pub const FAR_BRAKE: f64 = 0.3;
/// Above this height over the rest position the terminal phase sinks fast
/// (up to FAST_SINK, m/s); below it only once centred, at SINK, m.
pub const FINAL_HEIGHT: f64 = 5.0;
/// Above this height over the rest position the descent is as fast as can
/// be stopped (up to HIGH_SINK, m/s), m.
pub const HIGH: f64 = 100.0;
pub const HIGH_SINK: f64 = 60.0;
pub const FAST_SINK: f64 = 5.0;
/// Sink rate per metre above FINAL_HEIGHT while sliding in, and per metre
/// above the rest position once centred, 1/s.
pub const APPROACH_GAIN: f64 = 0.5;
pub const SETTLE_GAIN: f64 = 0.3;
/// Terminal sink rate, m/s.
pub const SINK: f64 = 1.3;
/// The glide's funnel: the height above the rest position it may come down
/// to while still off the axis is GLIDE_SLOPE · (offset − GLIDE_FREE), m
/// (at most FINAL_HEIGHT).
pub const GLIDE_SLOPE: f64 = 1.5;
pub const GLIDE_FREE: f64 = 1.0;
/// Gliding, the final settle slides toward the axis at most this fast,
/// m/s (it can begin while still moving in; one high-noise flight touched
/// down at 1.1 m/s sideways. Settling only within 1.5 m of the axis was
/// tried: a booster in a 20 m/s crosswind could not hold that and hovered
/// until its tanks ran dry).
pub const GLIDE_SETTLE_SLIDE: f64 = 0.4;
/// Gliding, the contact sink rate, m/s: it arrives at the settle already
/// coming down, and at high noise touched down at 2.02 m/s with SINK.
pub const GLIDE_SINK: f64 = 1.0;
/// Gliding, below its funnel floor while still sliding sideways, it climbs
/// back at up to this, m/s.
pub const GLIDE_RISE: f64 = 0.3;

/// Largest lean of the booster's axis from the airflow in the descent,
/// rad (8°): the single yaw fin can still hold it there.
pub const MAX_LEAN: f64 = 0.139_626_340_159_546_36;
/// Descent attitude loop of the booster, rad/s.
pub const WN_FIN: f64 = 1.0;

/// The nose points along `HEADING · x` in the belly flop.
pub const HEADING: f64 = -1.0;

/// Belly down, nose toward the tower from bearing `bearing` (rad; 0 is the
/// +x side, nose along −x), pitched up by `pitch` (rad). M4.3.
pub fn belly_attitude_at(pitch: f64, bearing: f64) -> Quat {
    let q = belly_attitude(pitch);
    if bearing == 0.0 {
        return q;
    }
    quat::mul(quat::from_axis_angle([0.0, 0.0, 1.0], bearing), q)
}

/// Belly down, nose along `HEADING · x`, pitched up by `pitch` (rad).
pub fn belly_attitude(pitch: f64) -> Quat {
    let along_plus_x = quat::from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2 - pitch);
    if HEADING > 0.0 {
        along_plus_x
    } else {
        quat::mul(
            quat::from_axis_angle([0.0, 0.0, 1.0], std::f64::consts::PI),
            along_plus_x,
        )
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

/// Body aerodynamic torque at the estimate, in the estimated wind.
fn aero_torque(scn: &Scenario, x: &State, air: &Air) -> Vec3 {
    let rho = scn.density(x[dynamics::R + 2]);
    let v_air = quat::sub(dynamics::velocity(x), air.wind);
    aero::body_force(&scn.ship, rho, dynamics::attitude(x), v_air).1
}

/// Force of the air on the body and on the surfaces at `surfaces`
/// (inertial, N) for airspeed `v_air` at attitude `q`.
fn air_force(ship: &Ship, rho: f64, q: Quat, v_air: Vec3, surfaces: [f64; 4]) -> Vec3 {
    let body = aero::body_force(ship, rho, q, v_air).0;
    let surf = match ship.kind {
        VehicleKind::Ship => aero::flap_force(ship, rho, q, v_air, surfaces).0,
        VehicleKind::Booster => aero::fin_force(ship, rho, q, v_air, surfaces).0,
    };
    quat::add(body, surf)
}

/// What the estimated wind adds to the air's force (inertial, N) at
/// attitude `q`: the model in the wind minus the model in still air. Zero,
/// exactly, in still air, so a wind-blind flight is unchanged (D-046).
fn wind_force(ship: &Ship, rho: f64, q: Quat, v: Vec3, air: &Air, surfaces: [f64; 4]) -> Vec3 {
    if air.wind == [0.0; 3] {
        return [0.0; 3];
    }
    quat::sub(
        air_force(ship, rho, q, quat::sub(v, air.wind), surfaces),
        air_force(ship, rho, q, v, surfaces),
    )
}

/// Engines-first attitude for a velocity: the axis straight into the wind
/// of the fall (no lift), no twist about it.
pub fn descent_attitude(v: Vec3) -> Quat {
    let n = quat::norm(v);
    if n < 1e-6 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    turn(
        [0.0, 0.0, 1.0],
        quat::scale(v, -1.0 / n),
        std::f64::consts::PI,
    )
}

/// Booster, engines off: fall engines first toward `aim` (x, y) at the
/// ignition altitude, steering with body lift and the grid fins.
pub fn descent(scn: &Scenario, x: &State, aim: [f64; 2], ignition_z: f64, air: &Air) -> Actuation {
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    let m = x[M];
    let rho = scn.density(r[2]);
    let speed = quat::norm(v).max(1.0);
    let back = quat::scale(v, -1.0 / speed);
    // Never less than 6 s: closer in, the steering would flick from side
    // to side chasing the last metres (the landing burn takes those).
    let t_go = fall_time(scn, x, ignition_z).max(6.0);
    // Arrive over the aim point with as little sideways speed as the fins
    // allow (the acceleration that is linear in time and does both):
    // a = 6 (aim − r)/t² − 4 v/t. Nulling the position alone arrived
    // drifting at 20 m/s from a 45 km start, past the tower. (Steering the
    // point where the burn would bring it to rest was tried: it lit the
    // engines 200 m out, and a sideways divert at 400 m/s is fought by the
    // body's own lift.)
    // What the wind pushes, held engines first along the fall: the lean
    // must make the rest (so it leans into a crosswind before it drifts).
    let push = wind_force(&scn.ship, rho, descent_attitude(v), v, air, [0.0; 4]);
    let a_want: [f64; 2] = std::array::from_fn(|i| {
        6.0 * (aim[i] - r[i]) / (t_go * t_go) - 4.0 * v[i] / t_go - push[i] / m
    });
    // Sideways acceleration per radian of lean, from the shared model
    // (negative: the lift points away from the lean).
    let eps = 0.05;
    let probe = normalize(quat::add(back, [eps, 0.0, 0.0]));
    let f0 = aero::body_force(&scn.ship, rho, descent_attitude(v), v).0;
    let f1 = aero::body_force(
        &scn.ship,
        rho,
        turn([0.0, 0.0, 1.0], probe, std::f64::consts::PI),
        v,
    )
    .0;
    let k = (f1[0] - f0[0]) / (m * eps);
    let mut lean = if k.abs() > 0.05 {
        [a_want[0] / k, a_want[1] / k]
    } else {
        [0.0, 0.0]
    };
    let n = lean[0].hypot(lean[1]);
    if n > MAX_LEAN {
        lean = [lean[0] * MAX_LEAN / n, lean[1] * MAX_LEAN / n];
    }
    let axis = normalize(quat::add(back, [lean[0], lean[1], 0.0]));
    let q_des = turn([0.0, 0.0, 1.0], axis, std::f64::consts::PI);

    let tau = attitude_torque(scn, x, q_des, [0.0; 3], WN_FIN);
    let body = aero_torque(scn, x, air);
    let need: Vec3 = std::array::from_fn(|i| tau[i] - body[i]);
    let gain = aero::fin_gain(
        &scn.ship,
        rho,
        dynamics::attitude(x),
        quat::sub(v, air.wind),
    );
    let (fins, made) = aero::fin_allocate(&scn.ship, gain, need);
    let rest = std::array::from_fn(|i| need[i] - made[i]);
    Actuation {
        thrust_body: [0.0; 3],
        flaps: fins,
        rcs: rcs_request(scn, rest),
    }
}

/// The unpowered phase of either vehicle toward the scenario's aim point.
pub fn unpowered(scn: &Scenario, x: &State, ignition_z: f64, air: &Air) -> Actuation {
    match scn.ship.kind {
        VehicleKind::Ship => belly_flop(scn, x, scn.aim, ignition_z, air),
        VehicleKind::Booster => descent(scn, x, scn.aim, ignition_z, air),
    }
}

/// Engines off: belly-flop toward `target_x` at the ignition altitude.
pub fn belly_flop(
    scn: &Scenario,
    x: &State,
    target: [f64; 2],
    ignition_z: f64,
    air: &Air,
) -> Actuation {
    let g = scn.planet.gravity();
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    let t_go = ((r[2] - ignition_z) / (-v[2]).max(10.0)).max(2.0);
    // The miss along the approach's bearing (the pitch slides the ship
    // along its nose; across it, the chosen bearing absorbs the drift).
    let b = scn.approach_dir();
    let miss =
        (target[0] - (r[0] + v[0] * t_go)) * b[0] + (target[1] - (r[1] + v[1] * t_go)) * b[1];
    // Pitching the nose up by θ slides the ship toward its tail (−HEADING·x)
    // at about g·sin θ when drag carries the weight.
    let a_want = (2.0 * miss / (t_go * t_go)).clamp(-g, g);
    let pitch = (-HEADING * a_want / g)
        .clamp(-1.0, 1.0)
        .asin()
        .clamp(-MAX_PITCH, MAX_PITCH);

    let mut tau = attitude_torque(
        scn,
        x,
        belly_attitude_at(pitch, scn.bearing),
        [0.0; 3],
        WN_FLAP,
    );
    // What the body and the trimmed flaps do anyway.
    let q = dynamics::attitude(x);
    let rho = scn.density(r[2]);
    let trim = aero::flap_force(&scn.ship, rho, q, quat::sub(v, air.wind), [FLAP_TRIM; 4]).1;
    let body = aero_torque(scn, x, air);
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
fn fly_force(
    scn: &Scenario,
    x: &State,
    force: Vec3,
    q_des: Quat,
    w_des: Vec3,
    limits: (f64, f64),
    air: &Air,
) -> Actuation {
    let q = dynamics::attitude(x);
    let body = aero_torque(scn, x, air);
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
pub fn powered(
    scn: &Scenario,
    x: &State,
    reference: (&State, Vec3),
    limits: (f64, f64),
    air: &Air,
) -> Actuation {
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
    // The plan already flew in the estimated wind (D-046); add only what
    // its model left out: the booster plans without crossflow drag.
    let unplanned = if scn.ship.kind == VehicleKind::Booster && air.wind != [0.0; 3] {
        let rho = scn.density(x[dynamics::R + 2]);
        let q = dynamics::attitude(x);
        let v = dynamics::velocity(x);
        let mut planned = scn.ship.clone();
        planned.c_n = 0.0;
        let trim = [scn.ship.surface_trim(); 4];
        quat::sub(
            wind_force(&scn.ship, rho, q, v, air, trim),
            wind_force(&planned, rho, q, v, air, trim),
        )
    } else {
        [0.0; 3]
    };
    let force = quat::sub(quat::add(f_plan, quat::scale(a, m)), unplanned);
    // Turn the planned attitude so its thrust line meets the wanted force.
    let planned_dir = quat::rotate(qr, normalize(ur));
    let q_des = quat::normalize(quat::mul(
        turn(planned_dir, normalize(force), MAX_CORRECTION),
        qr,
    ));
    fly_force(scn, x, force, q_des, dynamics::rates(xr), limits, air)
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
/// rad/s: a heavy vehicle on few engines saturates a fixed-gain loop and
/// wobbles.
fn bandwidth(scn: &Scenario, x: &State, tz: f64) -> f64 {
    let j = scn.ship.inertia(x[M]);
    let alpha = scn.ship.cm_height * scn.ship.gimbal_max.tan() * tz / j[0].max(j[1]);
    WN.min((alpha / AUTHORITY_ANGLE).sqrt()).max(0.3)
}

/// The flip: every lit engine at minimum throttle along the body axis, the
/// gimbal swinging the ship upright at no more than [`RATE_LIMIT`].
pub fn flip(scn: &Scenario, x: &State, limits: (f64, f64), air: &Air) -> Actuation {
    let q = dynamics::attitude(x);
    let force = quat::rotate(q, [0.0, 0.0, limits.0]);
    fly_force(
        scn,
        x,
        force,
        crate::scvx::upright_like(q),
        [0.0; 3],
        limits,
        air,
    )
}

/// Burn steadily at `thrust` along the body axis while coming upright:
/// what the booster does between lighting its engines and receiving its
/// first plan. Close to what the plan will ask for, so the plan still
/// fits when it arrives (braking flat out meanwhile did not: a 1.5 s solve
/// in the browser left the booster stopped a kilometre up).
pub fn steady_burn(
    scn: &Scenario,
    x: &State,
    thrust: f64,
    limits: (f64, f64),
    air: &Air,
) -> Actuation {
    let q = dynamics::attitude(x);
    let t = thrust.clamp(limits.0, limits.1);
    let force = quat::rotate(q, [0.0, 0.0, t]);
    fly_force(
        scn,
        x,
        force,
        crate::scvx::upright_like(q),
        [0.0; 3],
        limits,
        air,
    )
}

/// Upright; over the catch axis / pad (x = y = 0), then sinking at [`SINK`].
pub fn terminal(scn: &Scenario, x: &State, limits: (f64, f64), air: &Air) -> Actuation {
    terminal_shaped(scn, x, limits, air, false)
}

/// As [`terminal`]; with `glide`, it does not stop at [`FINAL_HEIGHT`] to
/// wait until centred: the height it may come down to falls with its
/// distance from the axis ([`GLIDE_SLOPE`] metres per metre beyond
/// [`GLIDE_FREE`]), so it slides in and comes down in one movement, over
/// the axis by the time it is low (M4.2: the owner found the stop-and-wait
/// robotic).
pub fn terminal_shaped(
    scn: &Scenario,
    x: &State,
    limits: (f64, f64),
    air: &Air,
    glide: bool,
) -> Actuation {
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
    let settling = off < scn.centring && side <= scn.settle_side_speed;
    let sink = if h > HIGH && scn.ship.kind == VehicleKind::Booster {
        // The booster far above the rest position (guidance had no plan
        // and braked high up; the ship's flights were tuned without this
        // and are left alone): come down as fast as can still be stopped with a third
        // of the spare thrust, not at a crawl that empties the tanks.
        let spare = (limits.1 / m - g).max(0.5);
        (2.0 * 0.33 * spare * (h - HIGH))
            .sqrt()
            .clamp(FAST_SINK, HIGH_SINK)
    } else if settling {
        // Centred and steady: straight down onto the arms, slowing to
        // SINK at contact (the catch allows 2 m/s).
        let sink = if glide { GLIDE_SINK } else { SINK };
        (SETTLE_GAIN * h + sink).clamp(sink, FAST_SINK)
    } else if glide {
        // Still sliding in: down a funnel over the axis, never waiting.
        // Never faster than the final settle allows at this height (gliding
        // low and quick touched down too fast at high noise: seen).
        let floor = FINAL_HEIGHT.min(GLIDE_SLOPE * (off - GLIDE_FREE).max(0.0));
        // (Below the rest height h < 0 and this could go negative: clamp
        // would panic; a glide that low only holds.)
        let settle = (SETTLE_GAIN * h + SINK).clamp(0.0, FAST_SINK);
        // Held a few metres off the axis but steady (a strong crosswind):
        // creep down as the stop-and-wait does, or it hovers over its floor
        // until the tanks run dry (25 m/s crosswinds: seen).
        let creep = if off < 2.0 * scn.centring && side <= scn.settle_side_speed {
            0.3 * SINK
        } else {
            0.0
        };
        // Below the floor and still sliding: climb back to it, gently. A
        // vehicle held at the floor's height only by "don't sink" settled
        // into the arms mid-correction, sliding at 1 m/s (seen).
        let lowest = if side > scn.settle_side_speed {
            -GLIDE_RISE
        } else {
            creep
        };
        (APPROACH_GAIN * (h - floor)).clamp(lowest, settle.max(creep))
    } else {
        // Still sliding in: come down to FINAL_HEIGHT meanwhile and wait
        // there (hovering high until centred took half a minute). Nearly
        // centred, keep creeping down rather than hover.
        let creep = if off < 2.0 * scn.centring && side <= scn.settle_side_speed {
            0.3 * SINK
        } else {
            0.0
        };
        (APPROACH_GAIN * (h - FINAL_HEIGHT)).clamp(creep, FAST_SINK)
    };
    // The sideways loop runs through the attitude loop: slow it down with
    // it, or it circles the axis.
    let k = (bandwidth(scn, x, m * g) / WN).min(translation_gain(scn, x, m * g));
    let kd = KD_TERMINAL * k;
    // Velocity command toward the axis, at most SLIDE m/s. The gain is
    // low on purpose: steering sideways goes through the attitude loop,
    // and a stiffer law (a square-root braking curve was tried) weaves
    // across the axis.
    let gain = SLIDE_GAIN * k;
    let far = (2.0 * FAR_BRAKE * (off - scn.centring).max(0.0)).sqrt();
    let speed = (gain * off).max(far);
    let mut want = if off > 1e-6 {
        [-speed * r[0] / off, -speed * r[1] / off]
    } else {
        [0.0, 0.0]
    };
    // Close enough: only damp the slide, don't keep chasing the exact axis.
    if off < 0.5 * scn.centring {
        want = [0.0, 0.0];
    }
    // Gliding, the final settle slides in no faster than GLIDE_SETTLE_SLIDE:
    // it starts while still moving in, and kept sliding at up to 1 m/s into
    // contact at high noise (seen).
    let cap = if glide && settling {
        GLIDE_SETTLE_SLIDE
    } else {
        SLIDE
    };
    let n = want[0].hypot(want[1]);
    if n > cap {
        want = [want[0] * cap / n, want[1] * cap / n];
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
    let lat_cap = if braking && scn.ship.kind == VehicleKind::Booster {
        BRAKE_ACCEL
    } else {
        LATERAL_ACCEL
    };
    let h = a[0].hypot(a[1]);
    let a = if h > lat_cap {
        [a[0] * lat_cap / h, a[1] * lat_cap / h, a[2]]
    } else {
        a
    };
    let q = dynamics::attitude(x);
    // Lean into the estimated wind at once, before it has pushed (D-046).
    let rho = scn.density(r[2]);
    let push = wind_force(&scn.ship, rho, q, v, air, [scn.ship.surface_trim(); 4]);
    let force = [
        m * a[0] - push[0],
        m * a[1] - push[1],
        m * (a[2] + g) - push[2],
    ];
    let up = crate::scvx::upright_like(q);
    let q_des = quat::normalize(quat::mul(
        turn([0.0, 0.0, 1.0], normalize(force), MAX_CORRECTION),
        up,
    ));
    fly_force(scn, x, force, q_des, [0.0; 3], limits, air)
}

/// Fly a planned approach (`guidance::approach`): `reference` is the plan's
/// position, velocity, commanded acceleration (thrust / mass, inertial) and
/// its rate of change now.
///
/// Flown the way the terminal controller flies, by commanded velocity: the
/// plan's velocity plus a gentle pull toward its position, tracked with the
/// terminal controller's low gain and capped sideways acceleration. A
/// vehicle on its last engine near hover tilts slowly, and feeding the
/// plan's acceleration and tilt rate forward with a position loop made it
/// swing past the plan (traced: 18° of tilt). The plan gives the shape (one
/// coordinated descent and slide onto the arms, no stop-and-wait); this
/// loop gives the robustness.
pub fn approach(
    scn: &Scenario,
    x: &State,
    reference: (Vec3, Vec3, Vec3, Vec3),
    limits: (f64, f64),
    air: &Air,
) -> Actuation {
    let (rr, vr, _, _) = reference;
    let m = x[M];
    let g = scn.planet.gravity();
    let r = dynamics::position(x);
    let v = dynamics::velocity(x);
    let k = (bandwidth(scn, x, m * g) / WN).min(translation_gain(scn, x, m * g));
    let kd = KD_TERMINAL * k;
    // The velocity to fly: the plan's, pulled toward its position.
    let mut want = [
        vr[0] + APPROACH_PULL * (rr[0] - r[0]),
        vr[1] + APPROACH_PULL * (rr[1] - r[1]),
        vr[2] + APPROACH_PULL_Z * (rr[2] - r[2]),
    ];
    let n = want[0].hypot(want[1]);
    if n > SLIDE {
        want[0] *= SLIDE / n;
        want[1] *= SLIDE / n;
    }
    // (The plan's acceleration fed forward too was tried: closer in a
    // flight without sensor error, but in the closed loop, with the
    // throttle's lag and the gimbal's slew, it swung 5 m across the axis.)
    let mut a = [
        kd * (want[0] - v[0]),
        kd * (want[1] - v[1]),
        2.0 * (want[2] - v[2]),
    ];
    let braking = a[0] * v[0] + a[1] * v[1] < 0.0;
    let lat_cap = if braking && scn.ship.kind == VehicleKind::Booster {
        BRAKE_ACCEL
    } else {
        LATERAL_ACCEL
    };
    let h = a[0].hypot(a[1]);
    if h > lat_cap {
        a[0] *= lat_cap / h;
        a[1] *= lat_cap / h;
    }
    let q = dynamics::attitude(x);
    let rho = scn.density(r[2]);
    let push = wind_force(&scn.ship, rho, q, v, air, [scn.ship.surface_trim(); 4]);
    let force = [
        m * a[0] - push[0],
        m * a[1] - push[1],
        m * (a[2] + g) - push[2],
    ];
    let up = crate::scvx::upright_like(q);
    let q_des = quat::normalize(quat::mul(
        turn([0.0, 0.0, 1.0], normalize(force), MAX_CORRECTION),
        up,
    ));
    fly_force(scn, x, force, q_des, [0.0; 3], limits, air)
}

/// Pull of the plan's position on the commanded velocity, 1/s: sideways
/// and vertical.
pub const APPROACH_PULL: f64 = 0.2;
pub const APPROACH_PULL_Z: f64 = 0.5;

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
    let drag =
        quat::norm(aero::body_force(&scn.ship, 1.0, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -1.0]).0);
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
    let decel = (0.7 * max_thrust / x[M] - scn.planet.gravity()).max(0.5);
    ignition_for(scn, x, decel)
}

/// As [`ignition_altitude`], for a burn that decelerates at `decel` m/s²
/// net of gravity (the ship plans its burn on two engines, whose thrust
/// range is narrow: it must light where that thrust stops it, not higher).
pub fn ignition_for(scn: &Scenario, x: &State, decel: f64) -> f64 {
    let g = scn.planet.gravity();
    let m = x[M];
    let decel = decel.max(0.5);
    let z0 = x[dynamics::R + 2];
    let vz = dynamics::velocity(x)[2].min(0.0);
    if scn.ship.kind == VehicleKind::Ship {
        let need = |v2: f64| scn.gate[2] + IGNITION_MARGIN * v2 / (2.0 * decel);
        let now = need(vz * vz);
        // A steep, fast entry is faster than it could stop from: the air
        // will slow it first. So also judge by the speed the fall predicts
        // (belly-first drag), and take the later of the two. Where the
        // air is thin and the fall speeds up, the simple rule wins.
        let drag =
            quat::norm(aero::body_force(&scn.ship, 1.0, belly_attitude(0.0), [0.0, 0.0, -1.0]).0);
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
    let drag =
        quat::norm(aero::body_force(&scn.ship, 1.0, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -1.0]).0);
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
    if n > 1e-12 {
        quat::scale(v, 1.0 / n)
    } else {
        [0.0, 0.0, 1.0]
    }
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
    fn fly(scn: &Scenario, x: State, seconds: f64, ctl: impl Fn(&State) -> Actuation) -> State {
        fly_in(scn, x, seconds, [0.0; 3], ctl)
    }

    /// As `fly`, in a steady `wind`.
    fn fly_in(
        scn: &Scenario,
        mut x: State,
        seconds: f64,
        wind: Vec3,
        ctl: impl Fn(&State) -> Actuation,
    ) -> State {
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
            let air = quat::sub(dynamics::velocity(&x), wind);
            let (ff, ft) = match s.kind {
                VehicleKind::Ship => aero::flap_force(s, rho, q, air, c.flaps),
                VehicleKind::Booster => aero::fin_force(s, rho, q, air, c.flaps),
            };
            let rcs: Vec3 = std::array::from_fn(|i| {
                if c.rcs[i].abs() > 0.2 {
                    c.rcs[i].signum() * arm[i]
                } else {
                    0.0
                }
            });
            let extra = Extra {
                force: ff,
                torque: quat::add(ft, rcs),
                wind,
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

    /// Turned to bearing ψ the belly-down attitude points its nose at the
    /// tower from that side (along −b); at ψ = 0 it is the old one.
    #[test]
    fn belly_attitude_turns_with_the_bearing() {
        assert_eq!(belly_attitude_at(0.1, 0.0), belly_attitude(0.1));
        let psi = 0.7f64;
        let nose = quat::rotate(belly_attitude_at(0.0, psi), [0.0, 0.0, 1.0]);
        assert!(
            (nose[0] + psi.cos()).abs() < 1e-9 && (nose[1] + psi.sin()).abs() < 1e-9,
            "{nose:?}"
        );
    }

    /// At a turned bearing the belly flop steers its along-track miss: from
    /// 400 m short along b it ends near the target.
    #[test]
    fn belly_flop_steers_along_the_bearing() {
        let psi = 0.6f64;
        let scn = Scenario::earth().turned(psi);
        let b = [psi.cos(), psi.sin()];
        let x0 = dynamics::state(
            scn.start_mass,
            [-400.0 * b[0], -400.0 * b[1], 6000.0],
            [0.0, 0.0, -80.0],
            belly_attitude_at(0.0, psi),
            [0.0; 3],
        );
        let target = [0.0, 0.0];
        let x = fly(&scn, x0, 60.0, |x| {
            belly_flop(&scn, x, target, 600.0, &Air::STILL)
        });
        let along = x[R] * b[0] + x[R + 1] * b[1];
        assert!(along.abs() < 150.0, "along-track {along} m");
    }

    #[test]
    fn belly_flop_holds_attitude() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(
            scn.start_mass,
            [0.0, 0.0, 6000.0],
            [0.0, 0.0, -80.0],
            belly_attitude(0.05),
            [0.0, 0.02, 0.0],
        );
        let x = fly(&scn, x0, 30.0, |x| {
            belly_flop(&scn, x, [0.0, 0.0], 600.0, &Air::STILL)
        });
        let tilt = quat::tilt(dynamics::attitude(&x)).to_degrees();
        assert!((90.0 - tilt).abs() < 13.0, "tilt {tilt}");
        assert!(quat::norm(dynamics::rates(&x)) < 0.05);
    }

    #[test]
    fn belly_flop_steers_toward_target() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(
            scn.start_mass,
            [-400.0, 0.0, 6000.0],
            [0.0, 0.0, -80.0],
            belly_attitude(0.0),
            [0.0; 3],
        );
        let x = fly(&scn, x0, 60.0, |x| {
            belly_flop(&scn, x, [0.0, 0.0], 600.0, &Air::STILL)
        });
        let r = dynamics::position(&x);
        assert!(r[0].abs() < 150.0, "x {} at z {}", r[0], r[2]);
    }

    #[test]
    fn descent_holds_engines_first_and_steers_to_the_aim() {
        let scn = Scenario::booster();
        let v0 = [-20.0, 10.0, -380.0];
        // Start tipped 5° off and turning.
        let q0 = quat::mul(
            descent_attitude(v0),
            quat::from_axis_angle([0.0, 1.0, 0.0], 0.09),
        );
        let x0 = dynamics::state(
            scn.start_mass,
            [600.0, -200.0, 12_000.0],
            v0,
            q0,
            [0.0, 0.03, 0.0],
        );
        let ign = 1800.0;
        let mut x = x0;
        // Fly until the ignition altitude.
        while x[R + 2] > ign {
            x = fly(&scn, x, 0.5, |x| {
                descent(&scn, x, scn.aim, ign, &Air::STILL)
            });
        }
        let off = (x[R] - scn.aim[0]).hypot(x[R + 1] - scn.aim[1]);
        assert!(
            off < 120.0,
            "missed the aim by {off} m: {:?}",
            dynamics::position(&x)
        );
        let tilt = quat::tilt(dynamics::attitude(&x)).to_degrees();
        assert!(tilt < 15.0, "tilt {tilt}");
        assert!(quat::norm(dynamics::rates(&x)) < 0.1);
        assert!(
            (250.0..450.0).contains(&-x[V + 2]),
            "falling at {}",
            -x[V + 2]
        );
    }

    /// Falling engines first through a 15 m/s crosswind: knowing the wind,
    /// the booster leans into it at once and is blown off its line no more
    /// than when it only corrects after the push (that feedback alone
    /// already arrives within a few metres).
    #[test]
    fn descent_leans_into_a_crosswind() {
        let scn = Scenario::booster();
        let v0 = [0.0, 0.0, -380.0];
        let x0 = dynamics::state(
            scn.start_mass,
            [scn.aim[0], scn.aim[1], 12_000.0],
            v0,
            descent_attitude(v0),
            [0.0; 3],
        );
        let wind = [0.0, 15.0, 0.0];
        let ign = 1800.0;
        // (end miss, largest sideways error on the way down), m.
        let miss = |air: Air| {
            let mut x = x0;
            let mut worst: f64 = 0.0;
            while x[R + 2] > ign {
                x = fly_in(&scn, x, 0.5, wind, |x| descent(&scn, x, scn.aim, ign, &air));
                worst = worst.max((x[R] - scn.aim[0]).hypot(x[R + 1] - scn.aim[1]));
            }
            ((x[R] - scn.aim[0]).hypot(x[R + 1] - scn.aim[1]), worst)
        };
        let (blind, aware) = (miss(Air::STILL), miss(Air { wind }));
        eprintln!("descent in a crosswind (end, worst): blind {blind:.1?} m, aware {aware:.1?} m");
        assert!(aware.0 < 60.0, "aware missed by {} m", aware.0);
        assert!(aware.1 <= blind.1, "aware {aware:?} vs blind {blind:?}");
    }

    /// The belly flop in a 10 m/s wind along its track: its feedback on the
    /// predicted miss absorbs the wind; knowing the wind (in its torque
    /// model) it ends no farther from the target. (A drift term that aimed
    /// off for the wind was tried: 3 m worse.)
    #[test]
    fn belly_flop_no_worse_in_an_along_track_wind() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(
            scn.start_mass,
            [-400.0, 0.0, 6000.0],
            [0.0, 0.0, -80.0],
            belly_attitude(0.0),
            [0.0; 3],
        );
        let wind = [10.0, 0.0, 0.0];
        let end = |air: Air| {
            let x = fly_in(&scn, x0, 60.0, wind, |x| {
                belly_flop(&scn, x, [0.0, 0.0], 600.0, &air)
            });
            let tilt = quat::tilt(dynamics::attitude(&x)).to_degrees();
            assert!((90.0 - tilt).abs() < 15.0, "tilt {tilt}");
            x[R].abs()
        };
        let (blind, aware) = (end(Air::STILL), end(Air { wind }));
        eprintln!("belly flop in an along-track wind: blind {blind:.0} m, aware {aware:.0} m");
        assert!(
            aware < 150.0 && aware <= blind + 1.0,
            "aware {aware} m vs blind {blind} m"
        );
    }

    /// Settling onto the arms in a 10 m/s wind toward +x: leaning into it
    /// from the start, the ship is pushed off the axis less than when it
    /// only corrects after being pushed.
    #[test]
    fn terminal_leans_into_the_wind_at_once() {
        let scn = Scenario::earth();
        let x0 = dynamics::state(
            120e3,
            [0.0, 0.0, scn.rest_cm_z() + 30.0],
            [0.0; 3],
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
        );
        let lim = (0.98e6, 2.45e6);
        let wind = [10.0, 0.0, 0.0];
        // (largest offset, mean offset over the last 10 s), m.
        let pushed = |air: Air| {
            let mut x = x0;
            let (mut worst, mut late): (f64, f64) = (0.0, 0.0);
            for k in 0..200 {
                x = fly_in(&scn, x, 0.1, wind, |s| terminal(&scn, s, lim, &air));
                let off = x[R].hypot(x[R + 1]);
                worst = worst.max(off);
                if k >= 100 {
                    late += off / 100.0;
                }
            }
            (worst, late)
        };
        let (blind, aware) = (pushed(Air::STILL), pushed(Air { wind }));
        eprintln!(
            "terminal in a 10 m/s wind (worst, late mean): blind {blind:.2?} m, aware {aware:.2?} m"
        );
        assert!(
            aware.1 < 0.7 * blind.1,
            "aware {aware:?} m vs blind {blind:?} m"
        );
    }

    /// From the ship's gate, flying a planned approach in the shared model
    /// (no sensor error), re-planned every 0.25 s from where it is with the
    /// same arrival time, as the flight does: it ends on the caught
    /// position, inside every catch limit.
    #[test]
    fn tracks_an_approach_plan() {
        use crate::approach::{
            ApproachPlanner, ApproachProblem, FINAL_TIMES, REPLAN_GAP, REPLAN_OFF, REPLAN_SLIP,
        };
        let scn = Scenario::earth();
        let m = 120e3;
        let lim = scn.ship.thrust_limits(1);
        let z_c = scn.rest_cm_z();
        let g = scn.planet.gravity();
        let c = scn.catch.expect("tower");
        let mut x = dynamics::state(
            m,
            [10.0, 0.0, z_c + 20.0],
            [-1.0, 0.0, -2.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
        );
        // `a0`: the sideways acceleration to start from: the vehicle's own
        // tilt for the first plan, the last plan's command after.
        let problem = |x: &State, t_f: f64, a0: Option<[f64; 2]>| {
            let axis = quat::rotate(dynamics::attitude(x), [0.0, 0.0, 1.0]);
            ApproachProblem {
                r0: dynamics::position(x),
                v0: dynamics::velocity(x),
                a0: a0.unwrap_or([g * axis[0], g * axis[1]]),
                z_catch: z_c,
                t_f,
                a_min: lim.0 / x[M] * 1.05,
                a_max: lim.1 / x[M] * 0.9,
                keep_x: c.mast_x + c.mast_half + scn.ship.radius + 1.6,
                base_arm: scn.ship.cm_height,
                nose_arm: scn.ship.length - scn.ship.cm_height,
                g,
            }
        };
        let mut planner = ApproachPlanner::new();
        let mut plan = planner
            .plan_free(&problem(&x, 0.0, None), &FINAL_TIMES)
            .expect("plan");
        let mut arrival = plan.t_f;
        let (mut t, mut since) = (0.0, 0.0);
        let mut replans_free = 0;
        let mut replans = 0;
        let mut worst_tilt: f64 = 0.0;
        while t < arrival {
            x = fly(&scn, x, 0.05, |s| {
                let (r, v, a) = plan.sample(since);
                approach(&scn, s, (r, v, a, plan.jerk(since)), lim, &Air::STILL)
            });
            t += 0.05;
            since += 0.05;
            worst_tilt = worst_tilt.max(quat::tilt(dynamics::attitude(&x)).to_degrees());
            // Re-plan only when it has drifted off the plan (as the burn
            // does), keeping the arrival time; later if that cannot be kept.
            let (rr, vr, cmd) = plan.sample(since);
            let off = quat::norm(quat::sub(rr, dynamics::position(&x)));
            let slip = quat::norm(quat::sub(vr, dynamics::velocity(&x)));
            if (off > REPLAN_OFF || slip > REPLAN_SLIP) && arrival - t > 0.5 && since >= REPLAN_GAP
            {
                let a0 = Some([cmd[0], cmd[1]]);
                let left = arrival - t;
                let got = planner.plan(&problem(&x, left, a0)).or_else(|_| {
                    planner.plan_free(&problem(&x, 0.0, a0), &[left + 1.0, left + 2.0, left + 4.0])
                });
                if let Ok(p) = got {
                    if (p.t_f - left).abs() > 1e-9 {
                        replans_free += 1;
                        arrival = t + p.t_f;
                    }
                    plan = p;
                    since = 0.0;
                    replans += 1;
                }
            }
        }
        let (r, v) = (dynamics::position(&x), dynamics::velocity(&x));
        let tilt = quat::tilt(dynamics::attitude(&x)).to_degrees();
        eprintln!(
            "approach over {arrival:.1} s ({replans} re-plans, {replans_free} new arrival times) ends at {r:.2?} moving {v:.2?}, tilt {tilt:.1} (worst {worst_tilt:.1}), {} solver set-up(s)",
            planner.setups()
        );
        // Inside the catch's limits with margin (it allows 4 m off the axis,
        // 1 m/s sideways, 5 deg).
        assert!(
            r[0].hypot(r[1]) < 1.5,
            "off the axis by {:.2} m",
            r[0].hypot(r[1])
        );
        assert!((r[2] - z_c).abs() < 0.5, "height {:.2} m off", r[2] - z_c);
        assert!(-v[2] < 2.0 && v[0].hypot(v[1]) < 1.0, "too fast: {v:?}");
        assert!(tilt < 5.0, "tilt {tilt}");
        assert_eq!(planner.setups(), 1);
    }

    /// Gliding while well below the rest height (blown off and down past
    /// the arms in a strong wind) must still command something finite: the
    /// sink cap went negative there and `clamp` panicked (wind sweep).
    #[test]
    fn glide_below_the_rest_height_does_not_panic() {
        let scn = Scenario::booster();
        let x = dynamics::state(
            scn.dry_mass + 20e3,
            [8.0, 3.0, scn.rest_cm_z() - 6.0],
            [0.5, -0.5, -1.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
        );
        let a = terminal_shaped(&scn, &x, scn.ship.thrust_limits(3), &Air::STILL, true);
        assert!(a.thrust_body.iter().all(|c| c.is_finite()), "{a:?}");
    }

    #[test]
    fn glide_does_not_sink_into_the_arms_while_sliding() {
        // Low over the arms, below the funnel floor for 2.4 m off the axis
        // and sliding at 1 m/s (the catch allows 1): climb back to the
        // floor, don't let the catch happen mid-slide (a prediction
        // touched down at 1.02 m/s sideways: seen).
        let scn = Scenario::earth();
        let m = 120e3;
        let x = dynamics::state(
            m,
            [-2.4, 0.0, scn.rest_cm_z() + 0.3],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
        );
        let a = terminal_shaped(&scn, &x, (0.98e6, 7.35e6), &Air::STILL, true);
        let mg = m * scn.planet.gravity();
        assert!(
            a.thrust_body[2] > 1.03 * mg,
            "{} vs hover {mg}",
            a.thrust_body[2]
        );
    }

    #[test]
    fn powered_zero_error_returns_plan_thrust() {
        let scn = Scenario::earth();
        let x = dynamics::state(
            120e3,
            [0.0, 0.0, 300.0],
            [0.0, 0.0, -30.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
        );
        let u = [0.0, 0.0, 2.0e6];
        let a = powered(&scn, &x, (&x, u), (0.98e6, 7.35e6), &Air::STILL);
        assert!(
            (a.thrust_body[2] - 2.0e6).abs() / 2.0e6 < 1e-3,
            "{:?}",
            a.thrust_body
        );
        assert!(a.thrust_body[0].abs() < 2e4 && a.thrust_body[1].abs() < 2e4);
    }

    #[test]
    fn attitude_error_commands_gimbal_sign() {
        // Nose tipped toward +x (a + rotation about y): righting it needs
        // a −y torque, which needs the thrust pushed toward +x (T_x > 0).
        let scn = Scenario::earth();
        let q = quat::from_axis_angle([0.0, 1.0, 0.0], 0.1);
        let x = dynamics::state(120e3, [0.0, 0.0, 300.0], [0.0; 3], q, [0.0; 3]);
        let a = terminal(&scn, &x, (0.98e6, 7.35e6), &Air::STILL);
        assert!(a.thrust_body[0] > 0.0, "{:?}", a.thrust_body);
    }

    #[test]
    fn terminal_slides_in_and_settles() {
        let scn = Scenario::earth();
        let rest = scn.rest_cm_z();
        // 14 m above the caught position, 8 m out (about where the plan ends).
        let x0 = dynamics::state(
            110e3,
            [8.0, 0.0, rest + 14.0],
            [-2.0, 0.0, -2.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
        );
        // Within 12 s it is over the arms' rails, upright, and going down
        // at a speed the catch allows (it used to hover for half a minute).
        let mut x = x0;
        let mut t = 0.0;
        while x[R + 2] > rest && t < 12.0 {
            x = fly(&scn, x, 0.1, |x| {
                terminal(&scn, x, (0.98e6, 2.45e6), &Air::STILL)
            });
            t += 0.1;
        }
        let off = x[R].hypot(x[R + 1]);
        assert!(t < 12.0, "still {} m up after {t} s", x[R + 2] - rest);
        assert!(off < scn.centring, "off axis by {off}");
        assert!(
            -x[V + 2] < 2.0 && x[V].hypot(x[V + 1]) < 1.0,
            "arriving at {:?}",
            dynamics::velocity(&x)
        );
        assert!(quat::tilt(dynamics::attitude(&x)).to_degrees() < 3.0);
        let _ = (W, SINK);
    }

    #[test]
    fn ignition_altitude_earth_floor() {
        let e = Scenario::earth();
        let x = dynamics::state(
            e.start_mass,
            [0.0, 0.0, 3000.0],
            [0.0, 0.0, -90.0],
            belly_attitude(0.0),
            [0.0; 3],
        );
        assert_eq!(ignition_altitude(&e, &x, 7.35e6), e.ignition_floor);
    }
}
