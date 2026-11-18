//! Cone operations on one block: the nonnegative orthant (elementwise) and
//! the second-order cone `{(t, v) : ‖v‖ ≤ t}`. Nesterov–Todd scaling as in
//! Vandenberghe's CVXOPT notes ("The CVXOPT linear and quadratic cone
//! program solvers", 2010, section 4).

/// `t² − ‖v‖²` of a second-order-cone point.
pub fn soc_det(x: &[f64]) -> f64 {
    x[0] * x[0] - x[1..].iter().map(|c| c * c).sum::<f64>()
}

/// Distance of the point into its cone: `t − ‖v‖` (SOC) or the entry.
pub fn soc_margin(x: &[f64]) -> f64 {
    x[0] - x[1..].iter().map(|c| c * c).sum::<f64>().sqrt()
}

/// Largest α ≥ 0 with `x + α·d` in the second-order cone (`x` strictly
/// inside); `f64::INFINITY` if every α is.
pub fn soc_max_step(x: &[f64], d: &[f64]) -> f64 {
    // det(x + αd) = a α² + 2 b α + c, with x0 + α d0 > 0.
    let a = soc_det(d);
    let b = x[0] * d[0] - x[1..].iter().zip(&d[1..]).map(|(p, q)| p * q).sum::<f64>();
    let c = soc_det(x).max(0.0);
    let mut alpha = f64::INFINITY;
    if d[0] < 0.0 {
        alpha = -x[0] / d[0];
    }
    // Smallest positive root of a α² + 2 b α + c = 0.
    let root = if a.abs() < 1e-14 * (1.0 + b.abs()) {
        if b < 0.0 {
            -c / (2.0 * b)
        } else {
            f64::INFINITY
        }
    } else {
        let disc = b * b - a * c;
        if disc < 0.0 {
            // No real root: det stays positive (a > 0) or the cone is
            // never left along d (cannot happen for a < 0 with c > 0).
            f64::INFINITY
        } else {
            let sq = disc.sqrt();
            // Numerically stable roots.
            let q = -(b + b.signum() * sq);
            let (r1, r2) = if q != 0.0 {
                (q / a, c / q)
            } else {
                (f64::INFINITY, f64::INFINITY)
            };
            let mut best = f64::INFINITY;
            for r in [r1, r2] {
                if r > 0.0 && r < best {
                    best = r;
                }
            }
            best
        }
    };
    alpha.min(root)
}

/// Jordan product `u ∘ v` of one SOC block into `out`.
pub fn soc_prod(u: &[f64], v: &[f64], out: &mut [f64]) {
    let d = u.len();
    out[0] = (0..d).map(|i| u[i] * v[i]).sum();
    for i in 1..d {
        out[i] = u[0] * v[i] + v[0] * u[i];
    }
}

/// `x` with `λ ∘ x = d` (one SOC block, λ strictly inside).
pub fn soc_div(lambda: &[f64], d: &[f64], out: &mut [f64]) {
    let n = lambda.len();
    let det = soc_det(lambda);
    let dot: f64 = (1..n).map(|i| lambda[i] * d[i]).sum();
    let x0 = (lambda[0] * d[0] - dot) / det;
    out[0] = x0;
    for i in 1..n {
        out[i] = (d[i] - x0 * lambda[i]) / lambda[0];
    }
}

/// Nesterov–Todd scaling of one SOC block from `s`, `z` (both strictly
/// inside): fills `w` (d×d, row-major) with W and `w2` with W·W, and
/// `winv` with W⁻¹. W is symmetric and `W z = W⁻¹ s`.
pub fn soc_scaling(
    s: &[f64],
    z: &[f64],
    w: &mut [f64],
    w2: &mut [f64],
    winv: &mut [f64],
    wbar: &mut [f64],
) {
    let d = s.len();
    let sdet = soc_det(s).max(1e-300).sqrt();
    let zdet = soc_det(z).max(1e-300).sqrt();
    // Normalized points, then w̄ = (s̄ + J z̄) / (2γ).
    let mut dot = 0.0;
    for i in 0..d {
        dot += (s[i] / sdet) * (z[i] / zdet);
    }
    let gamma = ((1.0 + dot) / 2.0).max(1e-300).sqrt();
    for i in 0..d {
        let jz = if i == 0 { z[0] / zdet } else { -z[i] / zdet };
        wbar[i] = (s[i] / sdet + jz) / (2.0 * gamma);
    }
    let eta = (sdet / zdet).sqrt();
    let w0 = wbar[0];
    // W = η [[w̄0, w̄1ᵀ], [w̄1, I + w̄1 w̄1ᵀ / (1 + w̄0)]]; W⁻¹ the same with
    // −w̄1 and 1/η.
    for r in 0..d {
        for c in 0..d {
            let v = if r == 0 && c == 0 {
                w0
            } else if r == 0 {
                wbar[c]
            } else if c == 0 {
                wbar[r]
            } else {
                (if r == c { 1.0 } else { 0.0 }) + wbar[r] * wbar[c] / (1.0 + w0)
            };
            w[r * d + c] = eta * v;
            let sgn = if (r == 0) != (c == 0) { -1.0 } else { 1.0 };
            winv[r * d + c] = sgn * v / eta;
        }
    }
    for r in 0..d {
        for c in 0..d {
            w2[r * d + c] = (0..d).map(|k| w[r * d + k] * w[k * d + c]).sum();
        }
    }
}

/// out = M x for a dense row-major d×d block.
pub fn dense_mul(m: &[f64], x: &[f64], out: &mut [f64]) {
    let d = x.len();
    for r in 0..d {
        out[r] = (0..d).map(|c| m[r * d + c] * x[c]).sum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nt_scaling_maps_z_and_s_to_the_same_point() {
        let s = [3.0, 1.0, -0.5, 0.2];
        let z = [2.0, -0.3, 0.8, 0.1];
        let mut w = [0.0; 16];
        let mut w2 = [0.0; 16];
        let mut wi = [0.0; 16];
        let mut wb = [0.0; 4];
        soc_scaling(&s, &z, &mut w, &mut w2, &mut wi, &mut wb);
        let mut wz = [0.0; 4];
        let mut wis = [0.0; 4];
        dense_mul(&w, &z, &mut wz);
        dense_mul(&wi, &s, &mut wis);
        for i in 0..4 {
            assert!((wz[i] - wis[i]).abs() < 1e-12, "{wz:?} {wis:?}");
        }
        // W W⁻¹ = I
        for r in 0..4 {
            for c in 0..4 {
                let v: f64 = (0..4).map(|k| w[r * 4 + k] * wi[k * 4 + c]).sum();
                assert!((v - if r == c { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn division_inverts_the_jordan_product() {
        let l = [2.0, 0.5, -0.4];
        let x = [0.3, 1.0, 2.0];
        let mut d = [0.0; 3];
        soc_prod(&l, &x, &mut d);
        let mut back = [0.0; 3];
        soc_div(&l, &d, &mut back);
        for i in 0..3 {
            assert!((back[i] - x[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn step_to_the_cone_boundary() {
        let x = [1.0, 0.0];
        assert!((soc_max_step(&x, &[0.0, 1.0]) - 1.0).abs() < 1e-12);
        assert!((soc_max_step(&x, &[-1.0, 0.0]) - 1.0).abs() < 1e-12);
        assert!(soc_max_step(&x, &[1.0, 0.5]).is_infinite());
        let x = [2.0, 1.0, 0.0];
        let a = soc_max_step(&x, &[-1.0, 1.0, 1.0]);
        let p: Vec<f64> = (0..3).map(|i| x[i] + a * [-1.0, 1.0, 1.0][i]).collect();
        assert!(soc_margin(&p).abs() < 1e-9 && a > 0.0);
    }
}
