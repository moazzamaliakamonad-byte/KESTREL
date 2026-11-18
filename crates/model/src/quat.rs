//! Small vector and quaternion helpers.
//!
//! Quaternions are scalar-first `[w, x, y, z]` and rotate body-frame
//! vectors into the inertial frame: `v_inertial = q ⊗ v_body ⊗ q*`.

pub type Vec3 = [f64; 3];
pub type Quat = [f64; 4];

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// Hamilton product `a ⊗ b`.
pub fn mul(a: Quat, b: Quat) -> Quat {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

pub fn conj(q: Quat) -> Quat {
    [q[0], -q[1], -q[2], -q[3]]
}

/// Unit quaternion (the identity if `q` is zero or not finite).
pub fn normalize(q: Quat) -> Quat {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n > 0.0 && n.is_finite() {
        [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

/// Body vector → inertial vector.
pub fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let p = mul(mul(q, [0.0, v[0], v[1], v[2]]), conj(q));
    [p[1], p[2], p[3]]
}

/// Inertial vector → body vector.
pub fn rotate_inv(q: Quat, v: Vec3) -> Vec3 {
    rotate(conj(q), v)
}

/// Rotation by `angle` (rad) about `axis` (need not be unit length).
pub fn from_axis_angle(axis: Vec3, angle: f64) -> Quat {
    let n = norm(axis);
    if n == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let (s, c) = (angle / 2.0).sin_cos();
    [c, s * axis[0] / n, s * axis[1] / n, s * axis[2] / n]
}

/// `q̇ = ½ q ⊗ [0, ω]` with `ω` in body axes.
pub fn derivative(q: Quat, w: Vec3) -> Quat {
    let d = mul(q, [0.0, w[0], w[1], w[2]]);
    [d[0] / 2.0, d[1] / 2.0, d[2] / 2.0, d[3] / 2.0]
}

/// The rotation whose body axes are `x`, `y`, `z` in inertial coordinates
/// (orthonormal, right-handed).
pub fn from_axes(x: Vec3, y: Vec3, z: Vec3) -> Quat {
    // Rotation matrix columns are the body axes; Shepperd's method.
    let (m00, m01, m02) = (x[0], y[0], z[0]);
    let (m10, m11, m12) = (x[1], y[1], z[1]);
    let (m20, m21, m22) = (x[2], y[2], z[2]);
    let tr = m00 + m11 + m22;
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [0.25 * s, (m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [(m21 - m12) / s, 0.25 * s, (m01 + m10) / s, (m02 + m20) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m02 - m20) / s, (m01 + m10) / s, 0.25 * s, (m12 + m21) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m10 - m01) / s, (m02 + m20) / s, (m12 + m21) / s, 0.25 * s]
    };
    normalize(q)
}

/// Spherical interpolation from `a` (s = 0) to `b` (s = 1), the short way.
pub fn slerp(a: Quat, b: Quat, s: f64) -> Quat {
    let mut d: f64 = (0..4).map(|i| a[i] * b[i]).sum();
    let b = if d < 0.0 {
        d = -d;
        b.map(|c| -c)
    } else {
        b
    };
    if d > 0.9995 {
        return normalize(std::array::from_fn(|i| a[i] + s * (b[i] - a[i])));
    }
    let th = d.clamp(-1.0, 1.0).acos();
    let (wa, wb) = (((1.0 - s) * th).sin() / th.sin(), (s * th).sin() / th.sin());
    normalize(std::array::from_fn(|i| wa * a[i] + wb * b[i]))
}

/// Angle between the body axis `b_z` and inertial up, rad.
pub fn tilt(q: Quat) -> f64 {
    rotate(q, [0.0, 0.0, 1.0])[2].clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_axes_recovers_a_rotation() {
        let q = normalize([0.3, -0.5, 0.7, 0.2]);
        let (x, y, z) = (
            rotate(q, [1.0, 0.0, 0.0]),
            rotate(q, [0.0, 1.0, 0.0]),
            rotate(q, [0.0, 0.0, 1.0]),
        );
        let p = from_axes(x, y, z);
        let same = (0..4).all(|i| (p[i] - q[i]).abs() < 1e-12)
            || (0..4).all(|i| (p[i] + q[i]).abs() < 1e-12);
        assert!(same, "{q:?} {p:?}");
        let h = slerp(q, p, 0.5);
        assert!(
            (0..4).all(|i| (h[i] - q[i]).abs() < 1e-9)
                || (0..4).all(|i| (h[i] + q[i]).abs() < 1e-9)
        );
    }

    fn close(a: Vec3, b: Vec3, tol: f64) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < tol)
    }

    #[test]
    fn rotate_matches_axis_angle() {
        // +90° about z takes x to y.
        let q = from_axis_angle([0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2);
        assert!(close(rotate(q, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0], 1e-12));
        // +90° about y takes z to x.
        let q = from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2);
        assert!(close(rotate(q, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0], 1e-12));
        assert!(close(
            rotate_inv(q, [1.0, 0.0, 0.0]),
            [0.0, 0.0, 1.0],
            1e-12
        ));
    }

    #[test]
    fn derivative_matches_finite_rotation() {
        let q = normalize([0.9, 0.1, -0.3, 0.2]);
        let w = [0.3, -0.2, 0.5];
        let h = 1e-6;
        // Body rate: q(t+h) = q ⊗ exp(ω h).
        let next = mul(q, from_axis_angle(w, norm(w) * h));
        let d = derivative(q, w);
        for i in 0..4 {
            assert!(((next[i] - q[i]) / h - d[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn tilt_of_upright_and_horizontal() {
        assert!(tilt([1.0, 0.0, 0.0, 0.0]).abs() < 1e-12);
        let q = from_axis_angle([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2);
        assert!((tilt(q) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }
}
