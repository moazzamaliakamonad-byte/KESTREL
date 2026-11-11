//! Physical parameters of the vehicle and the landing problem, in SI units.
//!
//! Frame: the landing pad is at the origin, `z` points up, `x`/`y` are
//! horizontal. Gravity is constant and points down.

use std::f64::consts::{FRAC_PI_2, PI};

/// A 3-vector `[x, y, z]`.
pub type Vec3 = [f64; 3];

/// Standard gravity, m/s². Only used to turn specific impulse into exhaust
/// velocity (`v_e = isp · G0`), on any planet.
pub const G0: f64 = 9.80665;

/// Vehicle and environment constants for one landing problem.
#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    /// Mass with full propellant at the start of the burn, kg.
    pub wet_mass: f64,
    /// Mass with empty tanks, kg.
    pub dry_mass: f64,
    /// Specific impulse, s.
    pub isp: f64,
    /// Lowest thrust magnitude while the engine is lit, N (ρ₁).
    pub min_thrust: f64,
    /// Highest thrust magnitude, N (ρ₂).
    pub max_thrust: f64,
    /// Gravity acceleration magnitude, m/s², acting along −z.
    pub gravity: f64,
    /// Minimum elevation angle of the vehicle seen from the pad, rad.
    /// Enforced as `z ≥ tan(γ)·‖(x, y)‖`.
    pub glide_slope: f64,
    /// Largest allowed angle between thrust and +z, rad.
    pub max_tilt: f64,
}

/// Why a [`Vehicle`] was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamError {
    /// A value that must be finite and positive was not.
    NotPositive(&'static str),
    /// `dry_mass` must be strictly below `wet_mass`.
    DryMassNotBelowWetMass,
    /// `min_thrust` must be strictly below `max_thrust`.
    MinThrustNotBelowMax,
    /// An angle was outside its allowed range.
    AngleOutOfRange(&'static str),
}

impl Vehicle {
    /// Illustrative Mars-lander numbers in the range of the powered-descent
    /// literature. Not data from any real vehicle.
    pub fn example_mars() -> Vehicle {
        // Six 3.1 kN engines canted 27°, throttleable between 30% and 80%.
        let t_max = 6.0 * 3100.0 * 27.0_f64.to_radians().cos();
        Vehicle {
            wet_mass: 1905.0,
            dry_mass: 1505.0,
            isp: 225.0,
            min_thrust: 0.3 * t_max,
            max_thrust: 0.8 * t_max,
            gravity: 3.7114,
            glide_slope: 4.0_f64.to_radians(),
            max_tilt: 45.0_f64.to_radians(),
        }
    }

    /// Propellant burned per newton per second, kg/(N·s): `ṁ = −α‖T‖`.
    pub fn alpha(&self) -> f64 {
        1.0 / (self.isp * G0)
    }

    /// Check that every value is physically meaningful.
    pub fn validate(&self) -> Result<(), ParamError> {
        let must_be_positive = [
            ("wet_mass", self.wet_mass),
            ("dry_mass", self.dry_mass),
            ("isp", self.isp),
            ("min_thrust", self.min_thrust),
            ("max_thrust", self.max_thrust),
            ("gravity", self.gravity),
        ];
        for (name, value) in must_be_positive {
            // Written as "not (ok)" so that NaN, which fails every
            // comparison, is rejected too.
            if !(value.is_finite() && value > 0.0) {
                return Err(ParamError::NotPositive(name));
            }
        }
        if self.dry_mass >= self.wet_mass {
            return Err(ParamError::DryMassNotBelowWetMass);
        }
        if self.min_thrust >= self.max_thrust {
            return Err(ParamError::MinThrustNotBelowMax);
        }
        if !(self.glide_slope >= 0.0 && self.glide_slope < FRAC_PI_2) {
            return Err(ParamError::AngleOutOfRange("glide_slope"));
        }
        if !(self.max_tilt > 0.0 && self.max_tilt <= PI) {
            return Err(ParamError::AngleOutOfRange("max_tilt"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_is_valid() {
        assert_eq!(Vehicle::example_mars().validate(), Ok(()));
    }

    #[test]
    fn example_can_hover_when_full() {
        // If max thrust cannot beat full weight, no landing is ever possible.
        let v = Vehicle::example_mars();
        assert!(v.max_thrust > v.wet_mass * v.gravity);
    }

    #[test]
    fn rejects_negative_mass() {
        let v = Vehicle {
            wet_mass: -1.0,
            ..Vehicle::example_mars()
        };
        assert_eq!(v.validate(), Err(ParamError::NotPositive("wet_mass")));
    }

    #[test]
    fn rejects_nan_isp() {
        let v = Vehicle {
            isp: f64::NAN,
            ..Vehicle::example_mars()
        };
        assert_eq!(v.validate(), Err(ParamError::NotPositive("isp")));
    }

    #[test]
    fn rejects_dry_heavier_than_wet() {
        let v = Vehicle {
            dry_mass: 2000.0,
            ..Vehicle::example_mars()
        };
        assert_eq!(v.validate(), Err(ParamError::DryMassNotBelowWetMass));
    }

    #[test]
    fn rejects_inverted_thrust_bounds() {
        let v = Vehicle {
            min_thrust: 20_000.0,
            ..Vehicle::example_mars()
        };
        assert_eq!(v.validate(), Err(ParamError::MinThrustNotBelowMax));
    }

    #[test]
    fn rejects_vertical_glide_slope() {
        let v = Vehicle {
            glide_slope: FRAC_PI_2,
            ..Vehicle::example_mars()
        };
        assert_eq!(
            v.validate(),
            Err(ParamError::AngleOutOfRange("glide_slope"))
        );
    }

    #[test]
    fn alpha_matches_definition() {
        let v = Vehicle::example_mars();
        assert!((v.alpha() * v.isp * G0 - 1.0).abs() <= 1e-15);
    }
}
