//! The vehicles (ship and booster), the planets and the scenarios.
//!
//! Both are generic "Starship-class" vehicles built from public figures
//! (sources in `docs/superpowers/specs/2026-11-15-m3-ship-design.md` and
//! `2026-11-15-m3-booster-and-console-design.md`).
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

/// Which vehicle a [`Ship`] describes: they differ in control surfaces
/// (flaps / grid fins) and in how they fall (belly first / engines first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleKind {
    /// The upper stage: belly flop on four flaps, flip, 3 → 1 engines.
    Ship,
    /// The first stage: engines first on grid fins, 13 → 3 engines.
    Booster,
}

/// The booster's grid fins (all *illustrative* except their number and
/// 90/90/180° arrangement): at body +y, −y and +x.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridFins {
    /// Distance along `b_z` from the centre of mass, m.
    pub station: f64,
    /// Centre of pressure's distance from the axis, m.
    pub radius: f64,
    /// Area of one fin, m².
    pub area: f64,
    /// Force slope per radian of deflection.
    pub c_delta: f64,
    /// Largest deflection, rad.
    pub max_deflection: f64,
}

/// Number of grid fins.
pub const FINS: usize = 3;

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
    pub kind: VehicleKind,
    /// m (ship, Block 2: 52.1 m; booster, Block 3: 72.3 m).
    pub length: f64,
    /// m (9 m diameter).
    pub radius: f64,
    /// Centre of mass above the base, m (*illustrative*).
    pub cm_height: f64,
    /// Engines used to land (ship: 3 sea-level; booster: the inner 13).
    pub engine_count: u8,
    /// Engines kept to the end (ship: 1; booster: the centre 3).
    pub final_engines: u8,
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
    /// Linear lift slope per radian, referenced to the base area (slender-
    /// body theory gives 2; 0 = crossflow only, as for the ship).
    pub c_n_alpha: f64,
    /// Body normal force acts this far ahead of the CoM, m (*illustrative*).
    pub cp_offset: f64,
    /// Drag coefficient of an exposed flap.
    pub c_flap: f64,
    pub fore: FlapPair,
    pub aft: FlapPair,
    /// Lateral position of the flaps' centres of pressure, m.
    pub flap_y: f64,
    /// Grid fins (booster only).
    pub fins: Option<GridFins>,
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
            kind: VehicleKind::Ship,
            length: 52.0,
            radius: 4.5,
            cm_height: 24.0,
            engine_count: 3,
            final_engines: 1,
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
            c_n_alpha: 0.0,
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
            fins: None,
            inertia_per_kg: [230.0, 230.0, 15.0],
            pin_height: 40.0,
            leg_drop: 3.0,
            leg_radius: 6.0,
        }
    }

    /// The first stage (Block 3 figures where published: 72.3 m × 9 m,
    /// 33 engines of 2.45 MN of which the inner 13 gimbal, Isp 330 s, three
    /// grid fins). Everything aerodynamic, the centre of mass, the inertia
    /// and the gimbal range are *illustrative*.
    pub fn super_heavy_class() -> Ship {
        Ship {
            kind: VehicleKind::Booster,
            length: 72.3,
            radius: 4.5,
            cm_height: 26.0,
            engine_count: 13,
            final_engines: 3,
            engine_max: 2.45e6,
            throttle_min: 0.4,
            isp: 330.0,
            gimbal_max: 15f64.to_radians(),
            gimbal_rate: 30f64.to_radians(),
            ignition_time: 1.0,
            rcs_thrust: 20.0e3,
            rcs_fore: 40.0,
            rcs_aft: -20.0,
            c_a: 1.0,
            c_n: 0.6,
            c_n_alpha: 2.0,
            cp_offset: 10.0,
            c_flap: 0.0,
            fore: FlapPair { station: 0.0, area: 0.0 },
            aft: FlapPair { station: 0.0, area: 0.0 },
            flap_y: 0.0,
            fins: Some(GridFins {
                station: 34.0,
                radius: 6.0,
                area: 20.0,
                c_delta: 2.0,
                max_deflection: 20f64.to_radians(),
            }),
            inertia_per_kg: [440.0, 440.0, 18.0],
            pin_height: 62.0,
            leg_drop: 0.0,
            leg_radius: 0.0,
        }
    }

    /// Control-surface command that makes no control force: flaps half
    /// out, fins straight.
    pub fn surface_trim(&self) -> f64 {
        match self.kind {
            VehicleKind::Ship => 0.5,
            VehicleKind::Booster => 0.0,
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

/// The Earth approach gate's distance from the catch axis, m (away from
/// the mast, on +x).
pub const APPROACH_OFFSET: f64 = 20.0;

/// The booster's approach gate's distance from the catch axis, m.
pub const BOOSTER_OFFSET: f64 = 8.0;

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
    /// Share of the burn flown on every lit engine before dropping to one.
    pub switch_fraction: f64,
    /// The final descent starts once the axis is this close to its target, m
    /// (the arms need ~1 m; legs can stand anywhere near the pad).
    pub centring: f64,
    /// …and it sinks only while sliding slower than this, m/s.
    pub settle_side_speed: f64,
    /// Speed (down) at the intermediate gate where the burn drops to the
    /// final engines, m/s.
    pub mid_gate_speed: f64,
    /// Where the unpowered descent aims to be at ignition (x, y), m.
    pub aim: [f64; 2],
    /// Multiplies the air density (a physics option; 1 = standard).
    pub density_scale: f64,
}

/// Physics options the page can change. Applied to a scenario with
/// [`Scenario::tuned`]; the simulator and the planner must use the same.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub density_scale: f64,
    /// Largest gimbal angle, rad.
    pub gimbal_max: f64,
    pub throttle_min: f64,
    pub isp: f64,
    pub ignition_time: f64,
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
            // 40 t of landing propellant (*illustrative*: header-tank
            // capacity is not published).
            start_mass: 140e3,
            dry_mass: 100e3,
            // Approach gate: out from the tower and above the arms. The
            // final translation over the catch axis and the descent onto
            // the arms are flown by a simple, slow terminal controller.
            gate: [APPROACH_OFFSET, 0.0, caught_cm + 30.0],
            gate_velocity: [0.0, 0.0, -1.0],
            floor_z: caught_cm,
            clearance_x: Some(-1.0),
            catch: Some(catch),
            // The flip needs room: ignite at least 1 km up (Everyday
            // Astronaut: a flip from 300 m to 2.5 km is possible).
            ignition_floor: 1000.0,
            switch_fraction: 0.45,
            centring: 1.0,
            settle_side_speed: 0.5,
            mid_gate_speed: 25.0,
            // The flip pushes the ship toward the tower: start it beyond.
            aim: [450.0, 0.0],
            density_scale: 1.0,
            ship,
        }
    }

    /// Earth: the booster falls back engines first and is caught.
    /// 120 t of landing propellant (*illustrative*).
    pub fn booster() -> Scenario {
        let ship = Ship::super_heavy_class();
        let catch = Catch {
            // The arms ride the tower: lower for the taller booster.
            arm_z: 95.0,
            mast_x: -14.0,
            mast_half: 4.5,
            tower_top: 145.0,
            capture_radius: 1.5,
        };
        let caught_cm = catch.arm_z - ship.pin_height + ship.cm_height;
        Scenario {
            planet: Planet::Earth,
            start_mass: 395e3,
            dry_mass: 275e3,
            // A short stand-off from the catch axis (the booster arrives
            // upright and tracks its plan closely, so less than the ship's;
            // none at all and sensor noise put it into the mast).
            gate: [BOOSTER_OFFSET, 0.0, caught_cm + 25.0],
            gate_velocity: [0.0, 0.0, -2.0],
            floor_z: caught_cm,
            clearance_x: Some(-1.0),
            catch: Some(catch),
            ignition_floor: 0.0,
            switch_fraction: 0.45,
            centring: 1.0,
            settle_side_speed: 0.5,
            mid_gate_speed: 50.0,
            // Offshore until the engines are running, then divert in.
            aim: [120.0, 0.0],
            density_scale: 1.0,
            ship,
        }
    }

    /// Mars: after entry, a long supersonic burn to a landing on legs.
    /// 100 t of landing propellant (*illustrative*).
    pub fn mars() -> Scenario {
        let ship = Ship::starship_class();
        let touchdown_cm = ship.cm_height + ship.leg_drop;
        Scenario {
            planet: Planet::Mars,
            start_mass: 360e3,
            dry_mass: 260e3,
            // Hand over to the terminal descent well above the ground:
            // short-horizon plans near the end converge poorly.
            gate: [0.0, 0.0, touchdown_cm + 60.0],
            gate_velocity: [0.0, 0.0, -3.0],
            floor_z: touchdown_cm,
            clearance_x: None,
            catch: None,
            ignition_floor: 0.0,
            switch_fraction: 0.8,
            centring: 5.0,
            settle_side_speed: 0.8,
            mid_gate_speed: 25.0,
            aim: [0.0, 0.0],
            density_scale: 1.0,
            ship,
        }
    }

    /// Air density at height `z`, kg/m³ (with the density option).
    pub fn density(&self, z: f64) -> f64 {
        self.density_scale * self.planet.density(z)
    }

    /// The physics options this scenario has now.
    pub fn tuning(&self) -> Tuning {
        Tuning {
            density_scale: self.density_scale,
            gimbal_max: self.ship.gimbal_max,
            throttle_min: self.ship.throttle_min,
            isp: self.ship.isp,
            ignition_time: self.ship.ignition_time,
        }
    }

    /// This scenario with the options applied (each kept in a sane range).
    pub fn tuned(mut self, t: &Tuning) -> Scenario {
        let ok = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.density_scale = ok(t.density_scale, 0.0, 3.0, 1.0);
        self.ship.gimbal_max = ok(t.gimbal_max, 2f64.to_radians(), 20f64.to_radians(), self.ship.gimbal_max);
        self.ship.throttle_min = ok(t.throttle_min, 0.2, 0.8, self.ship.throttle_min);
        self.ship.isp = ok(t.isp, 200.0, 400.0, self.ship.isp);
        self.ship.ignition_time = ok(t.ignition_time, 0.2, 3.0, self.ship.ignition_time);
        self
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
        // (The flight drops one more engine when too light to hover.)
        for s in [Scenario::earth(), Scenario::mars(), Scenario::booster()] {
            let (min, _) = s.ship.thrust_limits((s.ship.final_engines - 1).max(1));
            let weight = s.dry_mass * s.planet.gravity();
            assert!(min < 1.05 * weight, "{:?}: {min} vs {weight}", s.planet);
        }
    }

    #[test]
    fn tuning_round_trips_and_clamps() {
        let s = Scenario::booster();
        assert_eq!(s.clone().tuned(&s.tuning()), s);
        let mut t = s.tuning();
        t.density_scale = f64::NAN;
        t.gimbal_max = 9.0;
        let u = s.tuned(&t);
        assert_eq!(u.density_scale, 1.0);
        assert!((u.ship.gimbal_max - 20f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn earth_gates_are_above_the_caught_position() {
        for s in [Scenario::earth(), Scenario::booster()] {
            let c = s.catch.unwrap();
            // Pins at arm height when the CoM is at the floor.
            let pins = s.floor_z - s.ship.cm_height + s.ship.pin_height;
            assert!((pins - c.arm_z).abs() < 1e-9);
            assert!(s.gate[2] > s.floor_z);
        }
    }
}
