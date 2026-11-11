//! Kestrel guidance: fuel-optimal powered-descent guidance using lossless
//! convexification (Açıkmeşe & Ploen 2007), posed as a second-order cone
//! program.
//!
//! Simulation only. Not flight software.

pub mod params;
pub mod problem;
pub mod scaling;
mod socp;

pub use params::{ParamError, Vec3, Vehicle};
pub use problem::{GuidanceError, LandingProblem, Trajectory};
