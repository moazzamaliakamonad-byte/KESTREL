//! Entry guidance: steer a lifting entry from orbit to a point above the
//! landing site by rolling the lift (the bank angle).
//!
//! A numerical predictor-corrector, the family of FNPEG (Lu 2014,
//! "Entry guidance: a unified method", J. Guid. Control Dyn. 37(3)):
//! every cycle, fly the rest of the entry ahead in the model with a
//! constant bank magnitude, and adjust that magnitude (by bisection: more
//! bank, less lift up, a shorter flight) until the predicted hand-over
//! point is the aim point. The sign of the bank steers sideways: it is
//! reversed whenever the heading error leaves a deadband that narrows as
//! the ship slows (as the Space Shuttle did). Near the tiles' limit a
//! heating override lifts the ship (less bank) to stop it diving deeper.
//! Simulation only.

use model::entry::{self, EntryAero, EntryState, unit};
use model::quat::{self, Vec3};

/// Seconds between guidance commands.
pub const CYCLE: f64 = 1.0;
/// Integration step of the predictor, s.
pub const PREDICT_STEP: f64 = 2.0;
/// Largest bank magnitude commanded, rad (80°).
pub const BANK_MAX: f64 = 1.396_263_401_595_463_8;
/// Bisection steps on the bank magnitude (resolution 80°/2^12).
const BISECTION: usize = 12;
/// The heating override acts above this share of the tile limit.
pub const HEAT_GUARD: f64 = 0.97;
/// Longest flight the predictor will fly ahead, s.
const HORIZON: f64 = 4_000.0;

/// Where the entry must end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryTarget {
    /// Unit vector of the point on the surface below the hand-over.
    pub aim: Vec3,
    /// Unit vector of the landing site: the heading is steered at it, so
    /// the ship arrives moving straight toward it.
    pub site: Vec3,
    /// Hand-over altitude, m.
    pub handover_altitude: f64,
}

/// What one prediction found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediction {
    /// How far past the aim point the flight ends, m (negative: short).
    pub overshoot: f64,
    /// Hottest tile temperature on the way, K.
    pub peak_tile: f64,
    /// Seconds to the hand-over.
    pub time: f64,
}

/// Fly the rest of the entry ahead with bank magnitude `bank` (lift's
/// vertical part only).
pub fn predict(
    aero: &EntryAero,
    x: &EntryState,
    m: f64,
    bank: f64,
    target: &EntryTarget,
) -> Prediction {
    let c = bank.cos();
    let r0 = entry::position(x);
    let mut y = *x;
    let mut t = 0.0;
    let mut peak: f64 = 0.0;
    while entry::altitude(&y) > target.handover_altitude && t < HORIZON {
        // Finer steps low down, where the air changes fast.
        let dt = if entry::altitude(&y) < 40_000.0 {
            0.5
        } else {
            PREDICT_STEP
        };
        y = entry::rk4_vertical(aero, &y, m, c, dt);
        t += dt;
        let q = entry::heat_flux(
            entry::density(entry::altitude(&y)),
            quat::norm(entry::velocity(&y)),
        );
        peak = peak.max(entry::tile_temperature(q));
        if entry::altitude(&y) > entry::ENTRY_INTERFACE + 60_000.0 {
            // Skipped out of the atmosphere: as long a flight as there is.
            break;
        }
    }
    let flown = entry::surface_distance(r0, entry::position(&y));
    let wanted = entry::surface_distance(r0, target.aim);
    Prediction {
        overshoot: flown - wanted,
        peak_tile: peak,
        time: t,
    }
}

/// A start at the entry interface on the great circle through the
/// landing site, `range` metres (along the surface) before it, moving
/// toward it (eastward) at `speed` m/s with flight-path angle `gamma`
/// (rad, negative: descending). Returns the state and the guidance
/// target, whose aim point is `aim_offset` metres before the site.
pub fn entry_start(
    site: Vec3,
    range: f64,
    speed: f64,
    gamma: f64,
    aim_offset: f64,
    handover_altitude: f64,
) -> (EntryState, EntryTarget) {
    let site = unit(site);
    // East at the site (any horizontal direction if it is a pole).
    let mut east = quat::cross([0.0, 0.0, 1.0], site);
    if quat::norm(east) < 1e-9 {
        east = [0.0, 1.0, 0.0];
    }
    let east = unit(east);
    let point = |phi: f64| quat::add(quat::scale(site, phi.cos()), quat::scale(east, phi.sin()));
    let phi0 = -range / entry::RADIUS;
    let p = point(phi0);
    let ahead = quat::add(
        quat::scale(site, -phi0.sin()),
        quat::scale(east, phi0.cos()),
    );
    let r = quat::scale(p, entry::RADIUS + entry::ENTRY_INTERFACE);
    let v = quat::scale(
        quat::add(quat::scale(ahead, gamma.cos()), quat::scale(p, gamma.sin())),
        speed,
    );
    (
        [r[0], r[1], r[2], v[0], v[1], v[2]],
        EntryTarget {
            aim: point(-aim_offset / entry::RADIUS),
            site,
            handover_altitude,
        },
    )
}

/// One guidance command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryCommand {
    /// Commanded bank, rad (sign: positive rolls the lift right).
    pub bank: f64,
    /// The corrector's magnitude before any heating override, rad.
    pub magnitude: f64,
    pub prediction: Prediction,
    /// The heating override lowered the bank.
    pub heat_limited: bool,
    /// The sign flipped this cycle.
    pub reversed: bool,
    /// Heading error to the landing site, rad (positive: to the left).
    pub heading_error: f64,
}

/// Bank-angle guidance state (the bank's sign and how often it flipped).
#[derive(Debug, Clone)]
pub struct EntryGuidance {
    pub target: EntryTarget,
    sign: f64,
    pub reversals: u32,
}

impl EntryGuidance {
    pub fn new(target: EntryTarget) -> EntryGuidance {
        EntryGuidance {
            target,
            sign: 1.0,
            reversals: 0,
        }
    }

    /// Heading error deadband at speed `v`, rad: about 10° at orbital
    /// speed, narrowing to a quarter of a degree slow (the belly flop
    /// after the hand-over cannot steer sideways, so the ship must arrive
    /// lined up on the site).
    pub fn deadband(v: f64) -> f64 {
        (0.25 + 9.75 * (v / 7_500.0).clamp(0.0, 1.0).powi(2)).to_radians()
    }

    /// The command for state `x` (truth or estimate) and mass `m`.
    pub fn command(&mut self, aero: &EntryAero, x: &EntryState, m: f64) -> EntryCommand {
        // Corrector: bisection on the magnitude (overshoot falls as bank grows).
        let (mut lo, mut hi) = (0.0, BANK_MAX);
        let p_lo = predict(aero, x, m, lo, &self.target);
        let p_hi = predict(aero, x, m, hi, &self.target);
        let (magnitude, prediction) = if p_lo.overshoot <= 0.0 {
            (lo, p_lo) // even all lift up falls short: fly as far as possible
        } else if p_hi.overshoot >= 0.0 {
            (hi, p_hi) // even the steepest overshoots: shed as much as possible
        } else {
            let mut p = p_lo;
            for _ in 0..BISECTION {
                let mid = 0.5 * (lo + hi);
                p = predict(aero, x, m, mid, &self.target);
                if p.overshoot > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (0.5 * (lo + hi), p)
        };
        // Heating override: near the tiles' limit and still descending,
        // keep enough lift up to stop the descent (equilibrium glide).
        let mut bank = magnitude;
        let mut heat_limited = false;
        let tile = entry::tile_temperature(entry::heat_flux(
            entry::density(entry::altitude(x)),
            quat::norm(entry::velocity(x)),
        ));
        if tile > HEAT_GUARD * entry::TILE_LIMIT && entry::flight_path_angle(x) < 0.0 {
            let r = quat::norm(entry::position(x));
            let v = quat::norm(entry::velocity(x));
            let g = entry::MU / (r * r);
            let l = entry::loads(aero, x, m).lift.max(1e-6);
            let need = ((g - v * v / r) / l * 1.2).clamp(0.0, 1.0);
            let cap = need.acos();
            if bank > cap {
                bank = cap;
                heat_limited = true;
            }
        }
        // Lateral: bank toward the aim when the heading error leaves the deadband.
        let heading_error = self.heading_error(x);
        let v = quat::norm(entry::velocity(x));
        let mut reversed = false;
        if heading_error.abs() > Self::deadband(v) {
            // Aim to the left (positive error) needs the lift rolled left.
            let want = if heading_error > 0.0 { -1.0 } else { 1.0 };
            if want != self.sign {
                self.sign = want;
                self.reversals += 1;
                reversed = true;
            }
        }
        EntryCommand {
            bank: self.sign * bank,
            magnitude,
            prediction,
            heat_limited,
            reversed,
            heading_error,
        }
    }

    /// Heading error to the landing site, rad (positive: it is to the
    /// left of the ground track).
    pub fn heading_error(&self, x: &EntryState) -> f64 {
        let r = unit(entry::position(x));
        let v = entry::velocity(x);
        // Horizontal direction of motion, and toward the aim.
        let along = unit(quat::sub(v, quat::scale(r, quat::dot(v, r))));
        let to_aim = quat::sub(
            self.target.site,
            quat::scale(r, quat::dot(self.target.site, r)),
        );
        let to_aim = unit(to_aim);
        let left = quat::cross(r, along);
        quat::dot(to_aim, left).atan2(quat::dot(to_aim, along))
    }
}

#[cfg(test)]
mod tests;
