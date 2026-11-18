//! A small primal-dual interior-point solver for second-order cone programs
//! with a diagonal quadratic objective:
//!
//! ```text
//! minimize    ½ xᵀ P x + qᵀ x          (P diagonal, ≥ 0)
//! subject to  A x + s = b,  s ∈ K       (K: zero cone × nonnegative orthant × second-order cones)
//! ```
//!
//! the same form Clarabel takes, so the two can be swapped and compared.
//!
//! Built the way embedded ("flight-style") solvers are: everything the
//! iterations need (the KKT matrix, its factor, every work vector) is
//! allocated once in [`Solver::new`]; [`Solver::solve`] allocates nothing,
//! and runs at most `max_iter` iterations of a fixed amount of work each,
//! so its run time is bounded. The same pattern can be solved again with
//! new numbers ([`Solver::update`]) without allocating.
//!
//! Method: an infeasible-start path-following method with Nesterov–Todd
//! scaling and Mehrotra's predictor–corrector (as in CVXOPT's `coneqp` and
//! ECOS), the Newton systems solved by a sparse LDLᵀ factorization of the
//! regularized, quasi-definite KKT matrix (a port of QDLDL's algorithm)
//! with a minimum-degree ordering and iterative refinement.
//!
//! Simulation only: not flight software.

// Numerical loops index several arrays at once; `!(x > 0.0)` is meant to
// catch NaN too.
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]

mod cones;
mod csc;
mod ldl;
mod solver;

pub use csc::Csc;
pub use solver::{Cone, Settings, Solver, Status};
