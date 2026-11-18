//! Kestrel guidance: fuel-optimal powered-descent guidance using lossless
//! convexification (Açıkmeşe & Ploen 2007), posed as a second-order cone
//! program.
//!
//! Simulation only. Not flight software.

pub mod entry;
pub mod envelope;
pub mod params;
pub mod problem;
pub mod replan;
pub mod scaling;
pub mod scvx;
pub mod ship_control;
mod socp;
mod solve;
pub mod track;
pub mod wind;

pub use params::{ParamError, Vec3, Vehicle};
pub use problem::{GuidanceError, LandingProblem, Trajectory};
pub use replan::{Hint, Mode, Replan, replan};
pub use solve::{solve, solve_closest, solve_fixed_time};
