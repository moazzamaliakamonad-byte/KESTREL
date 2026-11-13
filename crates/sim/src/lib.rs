//! Kestrel simulator: the **true** state of the vehicle and everything that
//! happens to it — engine (lag, failures, limits), gusts, and the sensors
//! that report a noisy version of the truth to guidance.
//!
//! Deliberately independent of the guidance crate: guidance never reads the
//! truth, only [`sensor::Measurement`]s.
//!
//! Simulation only. Not flight software.

pub mod engine;
pub mod rng;
pub mod sensor;
pub mod sim;

/// A 3-vector `[x, y, z]`, SI units, pad at the origin, `z` up.
pub type Vec3 = [f64; 3];

pub(crate) fn norm(v: Vec3) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}

pub use engine::Engine;
pub use rng::Rng;
pub use sensor::{Measurement, Noise, Sensor};
pub use sim::{Gust, Params, Sim, Truth};
