//! Kestrel guidance: fuel-optimal powered-descent guidance using lossless
//! convexification (Açıkmeşe & Ploen 2007), posed as a second-order cone
//! program.
//!
//! Simulation only. Not flight software.

pub mod params;
pub mod scaling;

pub use params::{ParamError, Vec3, Vehicle};
