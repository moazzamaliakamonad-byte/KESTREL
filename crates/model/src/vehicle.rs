//! The ship, the planets and the two scenarios.
//!
//! The ship is a generic "Starship-class" vehicle built from public figures
//! (sources in `docs/superpowers/specs/2026-11-15-m3-ship-design.md`).
//! Numbers marked *illustrative* are not published and were chosen to be
//! plausible. Simulation only: this is not any real vehicle's data.

use crate::quat::Vec3;

/// Standard gravity, used for specific impulse, m/s².
pub const G0: f64 = 9.80665;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planet {
    Earth,
    Mars,
}

impl Planet {
    /// Surface gravity, m/s² (constant with height).
    pub fn gravity(self) -> f64 {
        match self {
            Planet::Earth => G0,
            Planet::Mars => 3.721,
        }
    }

    /// Air density at height `z` (m) from an exponential atmosphere, kg/m³.
    pub fn density(self, z: f64) -> f64 {
        let (rho0, h) = match self {
            Planet::Earth => (1.225, 8500.0),
            Planet::Mars => (0.020, 11_100.0),
        };
        rho0 * (-z.max(-500.0) / h).exp()
    }
}

/// One pair of flaps (left and right) at a station along the body axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlapPair {
    /// Distance along `b_z` from the centre of mass, m (+ toward the nose).
    pub station: f64,
    /// Area of one flap when fully exposed, m².
    pub area: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ship {
    /// m (Block 2: 52.1 m).
    pub length: f64,
    /// m (9 m diameter).
    pub radius: f64,
    /// Centre of mass above the base, m (*illustrative*).
    pub cm_height: f64,
    /// Sea-level engines used to land.
    pub engine_count: u8,
    /// Maximum thrust of one engine, N (Raptor 3 sea level, 250 tf).
    pub engine_max: f64,
    /// Lowest throttle as a fraction of maximum (40 %).
    pub throttle_min: f64,
    /// Specific impulse, s (sea level, 327 s).
    pub isp: f64,
    /// Largest gimbal angle, rad (*illustrative*: 15°).
    pub gimbal_max: f64,
    /// Gimbal slew rate, rad/s (*illustrative*).
    pub gimbal_rate: f64,
    /// Time from the ignition command to thrust, s (*illustrative*).
    pub ignition_time: f64,
    /// Thrust of one cold-gas thruster, N (*illustrative*).
    pub rcs_thrust: f64,
    /// Stations of the forward and aft thruster groups from the CoM, m.
    pub rcs_fore: f64,
    pub rcs_aft: f64,
    /// Axial and crossflow drag coefficients of the body.
    pub c_a: f64,
    pub c_n: f64,
    /// Body normal force acts this far ahead of the CoM, m (*illustrative*).
    pub cp_offset: f64,
    /// Drag coefficient of an exposed flap.
    pub c_flap: f64,
    pub fore: FlapPair,
    pub aft: FlapPair,
    /// Lateral position of the flaps' centres of pressure, m.
    pub flap_y: f64,
    /// Moments of inertia per kilogram, m² (a 52 × 9 m cylinder).
    pub inertia_per_kg: Vec3,
    /// Catch pins above the base, m (near the forward flaps).
    pub pin_height: f64,
    /// Legs (Mars): feet this far below the base, m, on this radius, m.
    pub leg_drop: f64,
    pub leg_radius: f64,
}

impl Ship {
    pub fn starship_class() -> Ship {
        Ship {
            length: 52.0,
            radius: 4.5,
            cm_height: 24.0,
            engine_count: 3,
            engine_max: 2.45e6,
            throttle_min: 0.4,
            isp: 327.0,
            gimbal_max: 15f64.to_radians(),
            gimbal_rate: 30f64.to_radians(),
            ignition_time: 1.0,
            rcs_thrust: 4.0e3,
            rcs_fore: 20.0,
            rcs_aft: -18.0,
            c_a: 0.8,
            c_n: 0.6,
            cp_offset: 1.0,
            c_flap: 1.2,
            fore: FlapPair {
                station: 22.0,
                area: 30.0,
            },
            aft: FlapPair {
                station: -20.0,
                area: 40.0,
            },
            flap_y: 6.5,
            inertia_per_kg: [230.0, 230.0, 15.0],
            pin_height: 40.0,
            leg_drop: 3.0,
            leg_radius: 6.0,
        }
    }

    /// Propellant per newton-second, kg/(N·s).
    pub fn alpha(&self) -> f64 {
        1.0 / (self.isp * G0)
    }

    /// Diagonal inertia (body x, y, z) at mass `m`, kg·m².
    pub fn inertia(&self, m: f64) -> Vec3 {
        self.inertia_per_kg.map(|j| j * m)
    }

    /// Thrust point (gimbal) in body axes relative to the CoM, m.
    pub fn thrust_point(&self) -> Vec3 {
        [0.0, 0.0, -self.cm_height]
    }

    /// `(min, max)` thrust with `n` engines running (each at 40–100 %), N.
    pub fn thrust_limits(&self, n: u8) -> (f64, f64) {
        let n = f64::from(n);
        (self.engine_max * self.throttle_min * n, self.engine_max * n)
    }
}

/// The tower on Earth (all *illustrative* except the ~145 m height).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Catch {
    /// Height of the arms, m.
    pub arm_z: f64,
    /// Centre of the square mast, m (it stands on the −x side).
    pub mast_x: f64,
    /// Half-width of the mast, m.
    pub mast_half: f64,
    pub tower_top: f64,
    /// Largest distance of the ship's axis from the catch axis, m.
    pub capture_radius: f64,
}

/// Everything that differs between the Earth and Mars flights.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    pub planet: Planet,
    pub ship: Ship,
    /// Mass at the start, kg.
    pub start_mass: f64,
    /// Mass with empty tanks (structure, plus cargo on Mars), kg.
    pub dry_mass: f64,
    /// Guidance ends here (centre of mass), m, at this velocity, m/s.
    pub gate: Vec3,
    pub gate_velocity: Vec3,
    /// The planned centre of mass never goes below this height, m.
    pub floor_z: f64,
    /// Planned positions keep `x ≥ clearance_x` (tower side), m.
    pub clearance_x: Option<f64>,
    pub catch: Option<Catch>,
    /// Lowest ignition altitude, m (the flip needs room).
    pub ignition_floor: f64,
}

impl Scenario {
    /// Earth: belly flop back to the tower, caught by its arms.
    pub fn earth() -> Scenario {
        let ship = Ship::starship_class();
        let catch = Catch {
            arm_z: 110.0,
            mast_x: -14.0,
            mast_half: 4.5,
            tower_top: 145.0,
            capture_radius: 1.5,
        };
        let caught_cm = catch.arm_z - ship.pin_height + ship.cm_height;
        Scenario {
            planet: Planet::Earth,
            start_mass: 125e3,
            dry_mass: 100e3,
            gate: [0.0, 0.0, caught_cm + 6.0],
            gate_velocity: [0.0, 0.0, -1.0],
            floor_z: caught_cm,
            clearance_x: Some(-3.0),
            catch: Some(catch),
            ignition_floor: 600.0,
            ship,
        }
    }

    /// Mars: after entry, a long supersonic burn to a landing on legs.
    pub fn mars() -> Scenario {
        let ship = Ship::starship_class();
        let touchdown_cm = ship.cm_height + ship.leg_drop;
        Scenario {
            planet: Planet::Mars,
            start_mass: 330e3,
            dry_mass: 260e3,
            gate: [0.0, 0.0, touchdown_cm + 15.0],
            gate_velocity: [0.0, 0.0, -1.0],
            floor_z: touchdown_cm,
            clearance_x: None,
            catch: None,
            ignition_floor: 0.0,
            ship,
        }
    }

    /// Height of the centre of mass when caught / standing on its legs, m.
    pub fn rest_cm_z(&self) -> f64 {
        self.floor_z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_values() {
        assert!((Planet::Earth.density(0.0) - 1.225).abs() < 1e-12);
        let mars = Planet::Mars.density(11_100.0);
        assert!((mars - 0.020 / std::f64::consts::E).abs() < 1e-12);
    }

    #[test]
    fn one_engine_can_hover_at_the_end() {
        // At minimum throttle one engine must not out-push the landed weight
        // by much, or the terminal descent cannot hold 1 m/s.
        for s in [Scenario::earth(), Scenario::mars()] {
            let (min, _) = s.ship.thrust_limits(1);
            let weight = s.dry_mass * s.planet.gravity();
            assert!(min < 1.05 * weight, "{:?}: {min} vs {weight}", s.planet);
        }
    }

    #[test]
    fn earth_gate_is_above_the_caught_position() {
        let s = Scenario::earth();
        let c = s.catch.unwrap();
        // Pins at arm height when the CoM is at the floor.
        let pins = s.floor_z - s.ship.cm_height + s.ship.pin_height;
        assert!((pins - c.arm_z).abs() < 1e-9);
        assert!(s.gate[2] > s.floor_z);
    }
}
