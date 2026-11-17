//! Entry from orbit: a round, non-rotating Earth, the US Standard
//! Atmosphere 1976, the ship's aerodynamics at high angle of attack and the
//! heat its tiles must survive. Point-mass (3-DoF) dynamics in an
//! Earth-centred frame, steered by the bank angle: how the ship's lift is
//! rolled around its velocity. Shared by the simulator (truth) and entry
//! guidance (its predictor). Simulation only.
//!
//! Published: Earth's gravitational parameter and mean radius, the 1976
//! standard atmosphere, the Sutton–Graves constant. *Illustrative*: every
//! vehicle number below (lift and drag coefficients, nose radius, tile
//! emissivity and temperature limit); SpaceX has not published them.

use crate::quat::{self, Vec3};

/// Earth's gravitational parameter, m³/s² (WGS 84).
pub const MU: f64 = 3.986_004_418e14;
/// Earth's mean radius, m.
pub const RADIUS: f64 = 6_371_000.0;
/// Top of the modelled atmosphere (entry interface), m.
pub const ENTRY_INTERFACE: f64 = 120_000.0;

/// Sutton–Graves stagnation-point heating constant for air, SI.
pub const SUTTON_GRAVES: f64 = 1.7415e-4;
/// Effective nose radius of the belly-first ship, m (*illustrative*).
pub const NOSE_RADIUS: f64 = 4.5;
/// Stefan–Boltzmann constant, W/(m²·K⁴).
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
/// Tile emissivity (*illustrative*; ceramic tiles are about 0.8-0.9).
pub const TILE_EMISSIVITY: f64 = 0.85;
/// Hottest the tiles may get, K (*illustrative*: about 1 430 °C; silica
/// tiles are rated around 1 500-1 700 K for reuse).
pub const TILE_LIMIT: f64 = 1_700.0;
/// Seconds above the limit the heat shield survives (*illustrative*).
pub const OVERHEAT_TIME: f64 = 8.0;

/// Altitudes (km) and densities (kg/m³) of the US Standard Atmosphere
/// 1976; between them density is interpolated in its logarithm.
const DENSITY: [(f64, f64); 23] = [
    (0.0, 1.225),
    (5.0, 0.7364),
    (10.0, 0.4135),
    (15.0, 0.1948),
    (20.0, 0.088_91),
    (25.0, 0.040_08),
    (30.0, 0.018_41),
    (35.0, 0.008_463),
    (40.0, 0.003_996),
    (45.0, 0.001_966),
    (50.0, 0.001_027),
    (55.0, 5.681e-4),
    (60.0, 3.097e-4),
    (65.0, 1.632e-4),
    (70.0, 8.283e-5),
    (75.0, 3.992e-5),
    (80.0, 1.846e-5),
    (85.0, 7.220e-6),
    (90.0, 3.416e-6),
    (95.0, 1.393e-6),
    (100.0, 5.604e-7),
    (110.0, 9.708e-8),
    (120.0, 2.222e-8),
];

/// Temperature of the 1976 atmosphere (K) at its break points (km), for
/// the speed of sound; linear between.
const TEMPERATURE: [(f64, f64); 12] = [
    (0.0, 288.15),
    (11.0, 216.65),
    (20.0, 216.65),
    (32.0, 228.65),
    (47.0, 270.65),
    (51.0, 270.65),
    (71.0, 214.65),
    (84.85, 186.87),
    (90.0, 186.87),
    (100.0, 195.08),
    (110.0, 240.0),
    (120.0, 360.0),
];

/// Air density at altitude `h` (m), kg/m³. Above the table it keeps
/// falling with the last segment's scale height.
pub fn density(h: f64) -> f64 {
    let km = h / 1000.0;
    if km <= 0.0 {
        return DENSITY[0].1;
    }
    let i = DENSITY
        .windows(2)
        .position(|w| km <= w[1].0)
        .unwrap_or(DENSITY.len() - 2);
    let ((h0, r0), (h1, r1)) = (DENSITY[i], DENSITY[i + 1]);
    let s = (km - h0) / (h1 - h0);
    (r0.ln() + s * (r1.ln() - r0.ln())).exp()
}

/// Speed of sound at altitude `h` (m), m/s.
pub fn speed_of_sound(h: f64) -> f64 {
    let km = (h / 1000.0).clamp(0.0, 120.0);
    let i = TEMPERATURE
        .windows(2)
        .position(|w| km <= w[1].0)
        .unwrap_or(TEMPERATURE.len() - 2);
    let ((h0, t0), (h1, t1)) = (TEMPERATURE[i], TEMPERATURE[i + 1]);
    let t = t0 + (km - h0) / (h1 - h0) * (t1 - t0);
    (1.4 * 287.053 * t).sqrt()
}

/// The ship's entry aerodynamics (*illustrative*). Hypersonic: belly
/// forward at about 60° angle of attack, lift-to-drag about 0.45. Below
/// Mach 1.5 it is the belly flop: broadside, drag only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryAero {
    /// Reference area (length × diameter), m².
    pub area: f64,
    pub cl_hypersonic: f64,
    pub cd_hypersonic: f64,
    pub cd_belly: f64,
}

impl EntryAero {
    pub fn ship(length: f64, radius: f64) -> EntryAero {
        EntryAero {
            area: length * 2.0 * radius,
            cl_hypersonic: 0.55,
            cd_hypersonic: 1.2,
            cd_belly: 1.1,
        }
    }

    /// (lift, drag) coefficients at Mach `mach`: hypersonic above Mach 5,
    /// belly flop below Mach 1.5, linear between.
    pub fn coefficients(&self, mach: f64) -> (f64, f64) {
        let s = ((mach - 1.5) / (5.0 - 1.5)).clamp(0.0, 1.0);
        (
            s * self.cl_hypersonic,
            self.cd_belly + s * (self.cd_hypersonic - self.cd_belly),
        )
    }
}

/// Stagnation-point heat flux, W/m² (Sutton–Graves).
pub fn heat_flux(rho: f64, speed: f64) -> f64 {
    SUTTON_GRAVES * (rho / NOSE_RADIUS).sqrt() * speed.powi(3)
}

/// Tile surface temperature in radiative equilibrium with `heat_flux`
/// (the tile re-radiates what it receives), K.
pub fn tile_temperature(heat_flux: f64) -> f64 {
    (heat_flux.max(0.0) / (TILE_EMISSIVITY * STEFAN_BOLTZMANN)).powf(0.25)
}

/// Point-mass state in the Earth-centred frame: position (m) and velocity (m/s).
pub type EntryState = [f64; 6];

pub fn position(x: &EntryState) -> Vec3 {
    [x[0], x[1], x[2]]
}

pub fn velocity(x: &EntryState) -> Vec3 {
    [x[3], x[4], x[5]]
}

/// `v` scaled to length 1 (zero stays zero).
pub fn unit(v: Vec3) -> Vec3 {
    let n = quat::norm(v);
    if n < 1e-12 {
        [0.0; 3]
    } else {
        quat::scale(v, 1.0 / n)
    }
}

/// Altitude above the round Earth, m.
pub fn altitude(x: &EntryState) -> f64 {
    quat::norm(position(x)) - RADIUS
}

/// Flight-path angle (above the local horizontal), rad.
pub fn flight_path_angle(x: &EntryState) -> f64 {
    let (r, v) = (position(x), velocity(x));
    let s = quat::dot(r, v) / (quat::norm(r) * quat::norm(v)).max(1e-9);
    s.clamp(-1.0, 1.0).asin()
}

/// Forces on the ship at `x` with bank `bank` (rad; 0 = lift straight up,
/// positive rolls the lift to the right of the velocity), divided by mass:
/// (lift, drag) accelerations, m/s², and their directions.
pub struct Loads {
    pub lift: f64,
    pub drag: f64,
    pub q_dyn: f64,
    pub mach: f64,
    pub rho: f64,
}

/// Aerodynamic loads per unit mass at `x` for mass `m`.
pub fn loads(aero: &EntryAero, x: &EntryState, m: f64) -> Loads {
    let h = altitude(x);
    let v = quat::norm(velocity(x));
    let rho = density(h);
    let q_dyn = 0.5 * rho * v * v;
    let mach = v / speed_of_sound(h);
    let (cl, cd) = aero.coefficients(mach);
    Loads {
        lift: q_dyn * aero.area * cl / m,
        drag: q_dyn * aero.area * cd / m,
        q_dyn,
        mach,
        rho,
    }
}

/// Lift direction for bank `bank`: in the plane normal to the velocity,
/// "up" (away from Earth) at zero bank, rolled toward the right at
/// positive bank.
pub fn lift_direction(x: &EntryState, bank: f64) -> Vec3 {
    let (r, v) = (position(x), velocity(x));
    let vh = unit(v);
    let up = quat::sub(unit(r), quat::scale(vh, quat::dot(unit(r), vh)));
    let up = if quat::norm(up) < 1e-9 {
        [0.0, 0.0, 1.0]
    } else {
        unit(up)
    };
    // Right of the velocity, seen from above.
    let right = quat::cross(vh, up);
    quat::add(quat::scale(up, bank.cos()), quat::scale(right, bank.sin()))
}

/// x' for bank `bank` and mass `m`.
pub fn derivative(aero: &EntryAero, x: &EntryState, m: f64, bank: f64) -> EntryState {
    let r = position(x);
    let rn = quat::norm(r);
    let g = quat::scale(r, -MU / (rn * rn * rn));
    let l = loads(aero, x, m);
    let vh = unit(velocity(x));
    let lift = quat::scale(lift_direction(x, bank), l.lift);
    let drag = quat::scale(vh, -l.drag);
    let a = quat::add(g, quat::add(lift, drag));
    [x[3], x[4], x[5], a[0], a[1], a[2]]
}

/// x' with the lift's vertical part only (`cos_bank` of it): the
/// longitudinal motion a predictor flies, ignoring the sideways part that
/// bank reversals average out.
pub fn derivative_vertical(aero: &EntryAero, x: &EntryState, m: f64, cos_bank: f64) -> EntryState {
    let r = position(x);
    let rn = quat::norm(r);
    let g = quat::scale(r, -MU / (rn * rn * rn));
    let l = loads(aero, x, m);
    let vh = unit(velocity(x));
    let lift = quat::scale(lift_direction(x, 0.0), l.lift * cos_bank);
    let drag = quat::scale(vh, -l.drag);
    let a = quat::add(g, quat::add(lift, drag));
    [x[3], x[4], x[5], a[0], a[1], a[2]]
}

/// One RK4 step of the vertical-lift motion (see [`derivative_vertical`]).
pub fn rk4_vertical(
    aero: &EntryAero,
    x: &EntryState,
    m: f64,
    cos_bank: f64,
    dt: f64,
) -> EntryState {
    let add = |a: &EntryState, k: &EntryState, s: f64| -> EntryState {
        std::array::from_fn(|i| a[i] + s * k[i])
    };
    let k1 = derivative_vertical(aero, x, m, cos_bank);
    let k2 = derivative_vertical(aero, &add(x, &k1, dt / 2.0), m, cos_bank);
    let k3 = derivative_vertical(aero, &add(x, &k2, dt / 2.0), m, cos_bank);
    let k4 = derivative_vertical(aero, &add(x, &k3, dt), m, cos_bank);
    std::array::from_fn(|i| x[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]))
}

/// One RK4 step of `dt` seconds at constant bank.
pub fn rk4(aero: &EntryAero, x: &EntryState, m: f64, bank: f64, dt: f64) -> EntryState {
    let add = |a: &EntryState, k: &EntryState, s: f64| -> EntryState {
        std::array::from_fn(|i| a[i] + s * k[i])
    };
    let k1 = derivative(aero, x, m, bank);
    let k2 = derivative(aero, &add(x, &k1, dt / 2.0), m, bank);
    let k3 = derivative(aero, &add(x, &k2, dt / 2.0), m, bank);
    let k4 = derivative(aero, &add(x, &k3, dt), m, bank);
    std::array::from_fn(|i| x[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]))
}

/// Unit vector of the point at latitude `lat`, longitude `lon` (rad).
pub fn surface_point(lat: f64, lon: f64) -> Vec3 {
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// (latitude, longitude) of a position, rad.
pub fn lat_lon(r: Vec3) -> (f64, f64) {
    let n = quat::norm(r).max(1e-9);
    ((r[2] / n).clamp(-1.0, 1.0).asin(), r[1].atan2(r[0]))
}

/// Great-circle distance along the surface between the points under two
/// positions, m.
pub fn surface_distance(a: Vec3, b: Vec3) -> f64 {
    let c = quat::dot(unit(a), unit(b)).clamp(-1.0, 1.0);
    RADIUS * c.acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_atmosphere_values() {
        assert!((density(0.0) - 1.225).abs() < 1e-9);
        assert!((density(10_000.0) - 0.4135).abs() < 1e-6);
        // Between table points: log-linear, so between the neighbours.
        let d = density(62_500.0);
        assert!(d < 3.097e-4 && d > 1.632e-4);
        assert!(density(130_000.0) < 2.222e-8);
        assert!((speed_of_sound(0.0) - 340.3).abs() < 0.5);
        assert!((speed_of_sound(15_000.0) - 295.1).abs() < 0.5);
    }

    #[test]
    fn circular_orbit_above_the_air_stays_circular() {
        let aero = EntryAero::ship(50.0, 4.5);
        let r = RADIUS + 400_000.0;
        let v = (MU / r).sqrt();
        let mut x: EntryState = [r, 0.0, 0.0, 0.0, v, 0.0];
        let period = 2.0 * std::f64::consts::PI * r / v;
        let steps = 5000;
        for _ in 0..steps {
            x = rk4(&aero, &x, 140e3, 0.0, period / steps as f64);
        }
        // Back where it started, at the same height (no air at 400 km in the table's extrapolation).
        assert!((altitude(&x) - 400_000.0).abs() < 50.0, "{}", altitude(&x));
        assert!(quat::norm(quat::sub(position(&x), [r, 0.0, 0.0])) < 2_000.0);
    }

    #[test]
    fn lift_rolls_with_bank() {
        let x: EntryState = [RADIUS + 80e3, 0.0, 0.0, 0.0, 7000.0, 0.0];
        let up = lift_direction(&x, 0.0);
        assert!((up[0] - 1.0).abs() < 1e-12);
        // Moving +y with up +x: right of the velocity is −z... (v × up) = y × x = −z.
        let right = lift_direction(&x, std::f64::consts::FRAC_PI_2);
        assert!((right[2] + 1.0).abs() < 1e-12, "{right:?}");
    }

    #[test]
    fn tiles_reradiate_their_heating() {
        let t = tile_temperature(400e3);
        let back = TILE_EMISSIVITY * STEFAN_BOLTZMANN * t.powi(4);
        assert!((back - 400e3).abs() < 1e-6 * 400e3);
        assert!(t > 1500.0 && t < 1800.0, "{t}");
    }
}
