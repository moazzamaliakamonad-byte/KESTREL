//! Non-dimensional ship dynamics and their first-order-hold discretization.
//!
//! Over interval k the control is linear between nodes,
//! `u(τ) = λ⁻ u_k + λ⁺ u_{k+1}`, and the state obeys `dx/dτ = σ f(x, u)`.
//! Integrating the nonlinear state together with its sensitivities from
//! the reference node `x̄_k` gives the linear model
//!
//! `x_{k+1} ≈ A_k x_k + B⁻_k u_k + B⁺_k u_{k+1} + S_k σ + z_k`
//!
//! (Szmuk & Açıkmeşe 2018, arXiv:1802.03827, §III). Jacobians of `f` are
//! taken by central finite differences of the shared model, so the planner
//! linearizes exactly the equations the simulator integrates.

#![allow(clippy::needless_range_loop)]

use model::dynamics::{self, Extra, State, STATE};
use model::{Scenario, Vec3};

pub const NX: usize = STATE;
pub const NU: usize = 3;

/// Reference units: length 100 m, time √(100 m / g), mass = start mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Units {
    pub l: f64,
    pub t: f64,
    pub m: f64,
}

impl Units {
    pub fn new(scn: &Scenario, m0: f64) -> Units {
        let l = 100.0;
        Units {
            l,
            t: (l / scn.planet.gravity()).sqrt(),
            m: m0,
        }
    }
    pub fn force(&self) -> f64 {
        self.m * self.l / (self.t * self.t)
    }
    /// SI value of one unit of each state component.
    pub fn state_scale(&self) -> State {
        let v = self.l / self.t;
        let w = 1.0 / self.t;
        [
            self.m, self.l, self.l, self.l, v, v, v, 1.0, 1.0, 1.0, 1.0, w, w, w,
        ]
    }
    pub fn x_nd(&self, x: &State) -> State {
        let s = self.state_scale();
        std::array::from_fn(|i| x[i] / s[i])
    }
    pub fn x_si(&self, x: &State) -> State {
        let s = self.state_scale();
        std::array::from_fn(|i| x[i] * s[i])
    }
    pub fn u_nd(&self, u: Vec3) -> Vec3 {
        u.map(|c| c / self.force())
    }
    pub fn u_si(&self, u: Vec3) -> Vec3 {
        u.map(|c| c * self.force())
    }
}

/// `f` in non-dimensional units (per unit of non-dimensional time).
pub fn f_nd(scn: &Scenario, units: &Units, x: &State, u: Vec3) -> State {
    let d = dynamics::f(scn, &units.x_si(x), units.u_si(u), &Extra::default());
    let s = units.state_scale();
    std::array::from_fn(|i| d[i] * units.t / s[i])
}

/// Jacobians `∂f/∂x` (14×14) and `∂f/∂u` (14×3) by central differences.
fn jacobians(scn: &Scenario, units: &Units, x: &State, u: Vec3) -> ([[f64; NX]; NX], [[f64; NU]; NX]) {
    const H: f64 = 1e-6;
    let mut a = [[0.0; NX]; NX];
    let mut b = [[0.0; NU]; NX];
    for j in 0..NX {
        let (mut xp, mut xm) = (*x, *x);
        xp[j] += H;
        xm[j] -= H;
        let (fp, fm) = (f_nd(scn, units, &xp, u), f_nd(scn, units, &xm, u));
        for i in 0..NX {
            a[i][j] = (fp[i] - fm[i]) / (2.0 * H);
        }
    }
    for j in 0..NU {
        let (mut up, mut um) = (u, u);
        up[j] += H;
        um[j] -= H;
        let (fp, fm) = (f_nd(scn, units, x, up), f_nd(scn, units, x, um));
        for i in 0..NX {
            b[i][j] = (fp[i] - fm[i]) / (2.0 * H);
        }
    }
    (a, b)
}

/// The linear model of one interval.
#[derive(Debug, Clone)]
pub struct Interval {
    pub a: [[f64; NX]; NX],
    pub bm: [[f64; NU]; NX],
    pub bp: [[f64; NU]; NX],
    pub s: [f64; NX],
    pub z: [f64; NX],
    /// Nonlinear end state from the reference node (defect = this − x̄_{k+1}).
    pub end: State,
}

/// Augmented state: x, Φ, P⁻, P⁺, Pσ.
const NAUG: usize = NX + NX * NX + 2 * NX * NU + NX;

fn aug_deriv(scn: &Scenario, units: &Units, y: &[f64; NAUG], u: Vec3, lm: f64, lp: f64, sigma: f64) -> [f64; NAUG] {
    let x: State = std::array::from_fn(|i| y[i]);
    let fx = f_nd(scn, units, &x, u);
    let (a, b) = jacobians(scn, units, &x, u);
    let mut d = [0.0; NAUG];
    for i in 0..NX {
        d[i] = sigma * fx[i];
    }
    let phi = NX;
    let pm = phi + NX * NX;
    let pp = pm + NX * NU;
    let ps = pp + NX * NU;
    for i in 0..NX {
        // Φ' = σA Φ
        for j in 0..NX {
            let mut acc = 0.0;
            for k in 0..NX {
                acc += a[i][k] * y[phi + k * NX + j];
            }
            d[phi + i * NX + j] = sigma * acc;
        }
        // P±' = σ(A P± + B λ±)
        for j in 0..NU {
            let (mut am, mut ap) = (0.0, 0.0);
            for k in 0..NX {
                am += a[i][k] * y[pm + k * NU + j];
                ap += a[i][k] * y[pp + k * NU + j];
            }
            d[pm + i * NU + j] = sigma * (am + b[i][j] * lm);
            d[pp + i * NU + j] = sigma * (ap + b[i][j] * lp);
        }
        // Pσ' = σ A Pσ + f
        let mut acc = 0.0;
        for k in 0..NX {
            acc += a[i][k] * y[ps + k];
        }
        d[ps + i] = sigma * acc + fx[i];
    }
    d
}

fn lerp(a: Vec3, b: Vec3, lm: f64, lp: f64) -> Vec3 {
    std::array::from_fn(|i| lm * a[i] + lp * b[i])
}

/// Discretize around the reference `(xs, us, sigma)` (non-dimensional).
pub fn discretize(scn: &Scenario, units: &Units, xs: &[State], us: &[Vec3], sigma: f64, substeps: usize) -> Vec<Interval> {
    let k = xs.len();
    let dtau = 1.0 / (k - 1) as f64;
    let h = dtau / substeps as f64;
    (0..k - 1)
        .map(|n| {
            let mut y = [0.0; NAUG];
            y[..NX].copy_from_slice(&xs[n]);
            for i in 0..NX {
                y[NX + i * NX + i] = 1.0;
            }
            let (u0, u1) = (us[n], us[n + 1]);
            for s in 0..substeps {
                let t0 = s as f64 * h;
                let stage = |t: f64, y: &[f64; NAUG]| {
                    let lp = t / dtau;
                    let lm = 1.0 - lp;
                    aug_deriv(scn, units, y, lerp(u0, u1, lm, lp), lm, lp, sigma)
                };
                let k1 = stage(t0, &y);
                let k2 = stage(t0 + h / 2.0, &add(&y, &k1, h / 2.0));
                let k3 = stage(t0 + h / 2.0, &add(&y, &k2, h / 2.0));
                let k4 = stage(t0 + h, &add(&y, &k3, h));
                for i in 0..NAUG {
                    y[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
                }
            }
            let mut iv = Interval {
                a: [[0.0; NX]; NX],
                bm: [[0.0; NU]; NX],
                bp: [[0.0; NU]; NX],
                s: [0.0; NX],
                z: [0.0; NX],
                end: std::array::from_fn(|i| y[i]),
            };
            let phi = NX;
            let pm = phi + NX * NX;
            let pp = pm + NX * NU;
            let ps = pp + NX * NU;
            for i in 0..NX {
                for j in 0..NX {
                    iv.a[i][j] = y[phi + i * NX + j];
                }
                for j in 0..NU {
                    iv.bm[i][j] = y[pm + i * NU + j];
                    iv.bp[i][j] = y[pp + i * NU + j];
                }
                iv.s[i] = y[ps + i];
            }
            for i in 0..NX {
                let mut lin = iv.s[i] * sigma;
                for j in 0..NX {
                    lin += iv.a[i][j] * xs[n][j];
                }
                for j in 0..NU {
                    lin += iv.bm[i][j] * u0[j] + iv.bp[i][j] * u1[j];
                }
                iv.z[i] = iv.end[i] - lin;
            }
            iv
        })
        .collect()
}

fn add<const N: usize>(y: &[f64; N], d: &[f64; N], s: f64) -> [f64; N] {
    std::array::from_fn(|i| y[i] + s * d[i])
}

/// Integrate one interval of length `dtau` from `x0`, control linear from
/// `u0` to `u1`.
pub fn propagate_interval(scn: &Scenario, units: &Units, x0: &State, u0: Vec3, u1: Vec3, sigma: f64, dtau: f64, substeps: usize) -> State {
    let h = dtau / substeps as f64;
    let mut x = *x0;
    for s in 0..substeps {
        let t0 = s as f64 * h;
        let g = |t: f64, x: &State| {
            let lp = t / dtau;
            let f = f_nd(scn, units, x, lerp(u0, u1, 1.0 - lp, lp));
            f.map(|c| c * sigma)
        };
        let k1 = g(t0, &x);
        let k2 = g(t0 + h / 2.0, &add(&x, &k1, h / 2.0));
        let k3 = g(t0 + h / 2.0, &add(&x, &k2, h / 2.0));
        let k4 = g(t0 + h, &add(&x, &k3, h));
        for i in 0..NX {
            x[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
    }
    x
}

/// Integrate the nonlinear dynamics through all intervals from `x0` with
/// first-order-hold controls (single shooting). Returns every node.
pub fn propagate(scn: &Scenario, units: &Units, x0: &State, us: &[Vec3], sigma: f64, substeps: usize) -> Vec<State> {
    let dtau = 1.0 / (us.len() - 1) as f64;
    let mut out = vec![*x0];
    for n in 0..us.len() - 1 {
        let next = propagate_interval(scn, units, out.last().expect("node"), us[n], us[n + 1], sigma, dtau, substeps);
        out.push(next);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::quat;

    /// A short powered reference: belly down, starting to flip.
    fn reference(scn: &Scenario, units: &Units) -> (Vec<State>, Vec<Vec3>, f64) {
        let q0 = quat::from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2);
        let x0 = dynamics::state(scn.start_mass, [0.0, 0.0, 600.0], [10.0, 0.0, -85.0], q0, [0.0, -0.2, 0.0]);
        let us = vec![units.u_nd([2e5, 0.0, 5e6]); 5];
        let sigma = 4.0 / units.t;
        let xs = propagate(scn, units, &units.x_nd(&x0), &us, sigma, 10);
        (xs, us, sigma)
    }

    #[test]
    fn discretization_matches_propagation() {
        let scn = Scenario::earth();
        let units = Units::new(&scn, scn.start_mass);
        let (xs, us, sigma) = reference(&scn, &units);
        let ivs = discretize(&scn, &units, &xs, &us, sigma, 10);
        // Perturb node, controls and time; the linear model must predict the
        // nonlinear result to second order.
        let eps = 1e-4;
        for (n, iv) in ivs.iter().enumerate() {
            let mut x = xs[n];
            x[5] += eps;
            x[12] += eps;
            let du = [eps, -eps, eps];
            let u0: Vec3 = std::array::from_fn(|i| us[n][i] + du[i]);
            let u1: Vec3 = std::array::from_fn(|i| us[n + 1][i] - du[i]);
            let s = sigma + eps;
            let truth = propagate_interval(&scn, &units, &x, u0, u1, s, 1.0 / (xs.len() - 1) as f64, 40);
            for i in 0..NX {
                let mut lin = iv.z[i] + iv.s[i] * s;
                for j in 0..NX {
                    lin += iv.a[i][j] * x[j];
                }
                for j in 0..NU {
                    lin += iv.bm[i][j] * u0[j] + iv.bp[i][j] * u1[j];
                }
                assert!((lin - truth[i]).abs() < 1e-6, "interval {n} comp {i}: {lin} vs {}", truth[i]);
            }
        }
    }

    #[test]
    fn units_round_trip() {
        let scn = Scenario::earth();
        let u = Units::new(&scn, scn.start_mass);
        let x = dynamics::state(3e5, [1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [1.0, 0.0, 0.0, 0.0], [0.1, 0.2, 0.3]);
        let back = u.x_si(&u.x_nd(&x));
        assert!((0..NX).all(|i| (back[i] - x[i]).abs() < 1e-9));
    }
}
