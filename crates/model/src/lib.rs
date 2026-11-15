//! Kestrel shared vehicle model: what the simulator (truth) and guidance
//! must agree on. A generic Starship-class ship, Earth and Mars, body
//! aerodynamics and the rigid-body equations. Simulation only.

pub mod aero;
pub mod dynamics;
pub mod quat;
pub mod vehicle;

pub use dynamics::{Extra, State};
pub use quat::{Quat, Vec3};
pub use vehicle::{Catch, Planet, Scenario, Ship};
