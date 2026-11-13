//! Tracking controller: flies the latest plan between re-plans.
//!
//! Guidance re-plans a few times a second; between solves the vehicle is
//! pushed off the plan by gusts, lag and noise. This controller adds a PD
//! correction to the plan's own thrust (feed-forward):
//!
//! `a_cmd = u_plan(τ) + Kp·(r_plan(τ) − r̂) + Kd·(v_plan(τ) − v̂)`, `T = m̂·a_cmd`,
//!
//! then keeps the command inside the tilt cone and the available thrust band.
//! `r̂, v̂, m̂` are the *measured* state — the controller never sees the truth.

use crate::params::Vec3;
use crate::problem::Trajectory;

/// PD gains. Defaults give ω ≈ 0.5 rad/s, ζ ≈ 0.9.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gains {
    /// s⁻²
    pub kp: f64,
    /// s⁻¹
    pub kd: f64,
}

impl Default for Gains {
    fn default() -> Self {
        Gains { kp: 0.25, kd: 0.9 }
    }
}

/// What the sensors say about the vehicle now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass: f64,
}

/// Limits the command must respect: the engines that still work, the tilt
/// cone, and gravity (for the hover term once the plan has ended).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    pub gravity: f64,
    /// rad
    pub max_tilt: f64,
    /// N
    pub min_thrust: f64,
    /// N
    pub max_thrust: f64,
}

/// The plan's position, velocity and thrust acceleration (`u = T/m`) at
/// `tau` seconds after its start; `None` once the plan has ended. Exact
/// between nodes (constant acceleration per interval).
pub fn reference(plan: &Trajectory, tau: f64, gravity: f64) -> Option<(Vec3, Vec3, Vec3)> {
    let n = plan.thrust.len();
    // A NaN tau gets no reference.
    if n == 0 || tau.is_nan() || tau > plan.final_time {
        return None;
    }
    let tau = tau.max(0.0);
    // Last interval whose start is at or before tau.
    let k = plan.t[..n].partition_point(|&t| t <= tau).saturating_sub(1);
    let s = tau - plan.t[k];
    let u = plan.thrust[k].map(|c| c / plan.mass[k]);
    let a = [u[0], u[1], u[2] - gravity];
    let (r0, v0) = (plan.position[k], plan.velocity[k]);
    let r = [0, 1, 2].map(|i| r0[i] + v0[i] * s + 0.5 * a[i] * s * s);
    let v = [0, 1, 2].map(|i| v0[i] + a[i] * s);
    Some((r, v, u))
}

/// Thrust command, N.
pub fn command(
    plan: Option<&Trajectory>,
    tau: f64,
    est: &Estimate,
    env: &Envelope,
    gains: Gains,
) -> Vec3 {
    let accel = match plan.and_then(|p| reference(p, tau, env.gravity)) {
        Some((r, v, u)) => [0, 1, 2].map(|i| {
            u[i] + gains.kp * (r[i] - est.position[i]) + gains.kd * (v[i] - est.velocity[i])
        }),
        None => {
            // No plan, or it has ended: sink straight down, slowing near
            // the ground, and kill any sideways drift.
            let sink = -(0.3 * est.position[2]).max(0.5);
            let want = [0.0, 0.0, sink];
            let mut a = [0, 1, 2].map(|i| gains.kd * (want[i] - est.velocity[i]));
            a[2] += env.gravity;
            a
        }
    };
    let thrust = accel.map(|c| c * est.mass);
    clamp_magnitude(clamp_tilt(thrust, env.max_tilt), env)
}

/// Full thrust against the velocity (within the tilt cone): the best a
/// vehicle with no landing plan can do.
pub fn brake(velocity: Vec3, env: &Envelope) -> Vec3 {
    let against = velocity.map(|c| -c);
    let dir = if norm(against) > 1e-9 {
        clamp_tilt(against, env.max_tilt)
    } else {
        [0.0, 0.0, 1.0]
    };
    let dir = if norm(dir) > 1e-12 {
        dir
    } else {
        [0.0, 0.0, 1.0]
    };
    dir.map(|c| c / norm(dir) * env.max_thrust)
}

/// Nearest vector inside the cone of half-angle `max_tilt` around +z.
/// Vectors pointing away from the cone project to zero.
fn clamp_tilt(t: Vec3, max_tilt: f64) -> Vec3 {
    let h = t[0].hypot(t[1]);
    if h.atan2(t[2]) <= max_tilt {
        return t;
    }
    // Unit vector on the cone's edge, on the same side as t.
    let (sin, cos) = max_tilt.sin_cos();
    let edge = if h > 0.0 {
        [sin * t[0] / h, sin * t[1] / h, cos]
    } else {
        [0.0, 0.0, 1.0]
    };
    let along = t[0] * edge[0] + t[1] * edge[1] + t[2] * edge[2];
    if along <= 0.0 {
        return [0.0; 3];
    }
    edge.map(|c| c * along)
}

/// Keep the magnitude in the available band. A zero vector (the command
/// wants less than nothing) becomes minimum thrust straight up.
fn clamp_magnitude(t: Vec3, env: &Envelope) -> Vec3 {
    let mag = norm(t);
    if mag < 1e-9 {
        return [0.0, 0.0, env.min_thrust];
    }
    let target = mag.clamp(env.min_thrust, env.max_thrust);
    t.map(|c| c / mag * target)
}

fn norm(v: Vec3) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LandingProblem, solve};

    fn tilt(t: Vec3) -> f64 {
        t[0].hypot(t[1]).atan2(t[2])
    }

    fn env() -> Envelope {
        let v = crate::Vehicle::example_mars();
        Envelope {
            gravity: v.gravity,
            max_tilt: v.max_tilt,
            min_thrust: v.min_thrust,
            max_thrust: v.max_thrust,
        }
    }

    fn plan() -> Trajectory {
        solve(&LandingProblem::example()).expect("example solves")
    }

    #[test]
    fn zero_error_returns_plan_thrust() {
        let p = plan();
        let tau = 3.3;
        let (r, v, u) = reference(&p, tau, env().gravity).unwrap();
        let est = Estimate {
            position: r,
            velocity: v,
            mass: 1850.0,
        };
        let t = command(Some(&p), tau, &est, &env(), Gains::default());
        for i in 0..3 {
            assert!(
                (t[i] - 1850.0 * u[i]).abs() <= 1e-6 * norm(t),
                "{t:?} vs {u:?}"
            );
        }
    }

    #[test]
    fn reference_hits_the_nodes() {
        let p = plan();
        let (r, v, _) = reference(&p, p.t[7], env().gravity).unwrap();
        for i in 0..3 {
            assert!((r[i] - p.position[7][i]).abs() < 1e-6);
            assert!((v[i] - p.velocity[7][i]).abs() < 1e-6);
        }
        assert!(reference(&p, p.final_time + 0.01, env().gravity).is_none());
    }

    #[test]
    fn position_error_pushes_back() {
        let p = plan();
        let (r, v, _) = reference(&p, 5.0, env().gravity).unwrap();
        let on = Estimate {
            position: r,
            velocity: v,
            mass: 1850.0,
        };
        let off = Estimate {
            position: [r[0] + 10.0, r[1], r[2]],
            ..on
        };
        let t_on = command(Some(&p), 5.0, &on, &env(), Gains::default());
        let t_off = command(Some(&p), 5.0, &off, &env(), Gains::default());
        assert!(t_off[0] < t_on[0] - 100.0, "{t_on:?} {t_off:?}");
    }

    #[test]
    fn tilt_is_clamped() {
        let est = Estimate {
            position: [0.0, 0.0, 100.0],
            velocity: [80.0, 0.0, 0.0],
            mass: 1800.0,
        };
        let t = command(None, 0.0, &est, &env(), Gains::default());
        assert!(
            tilt(t) <= env().max_tilt + 1e-9,
            "tilt {}",
            tilt(t).to_degrees()
        );
        assert!(t[0] < 0.0, "pushes against the drift");
    }

    #[test]
    fn magnitude_is_clamped_both_ways() {
        let e = env();
        let fast = Estimate {
            position: [0.0, 0.0, 100.0],
            velocity: [0.0, 0.0, -200.0],
            mass: 1800.0,
        };
        let t = command(None, 0.0, &fast, &e, Gains::default());
        assert!((norm(t) - e.max_thrust).abs() < 1e-6);
        let rising = Estimate {
            position: [0.0, 0.0, 100.0],
            velocity: [0.0, 0.0, 30.0],
            mass: 1800.0,
        };
        let t = command(None, 0.0, &rising, &e, Gains::default());
        assert!((norm(t) - e.min_thrust).abs() < 1e-6);
        assert!(t[2] > 0.0);
    }

    #[test]
    fn after_plan_descends_vertically() {
        let p = plan();
        let est = Estimate {
            position: [0.0, 0.0, 10.0],
            velocity: [0.0; 3],
            mass: 1600.0,
        };
        let t = command(Some(&p), p.final_time + 1.0, &est, &env(), Gains::default());
        assert!(t[2] < 1600.0 * env().gravity, "lets it sink: {t:?}");
        assert!(t[0].abs() < 1e-9 && t[1].abs() < 1e-9);
        // Sinking at the target rate (−3 m/s at 10 m): hold it.
        let est = Estimate {
            velocity: [0.0, 0.0, -3.0],
            ..est
        };
        let t = command(None, 0.0, &est, &env(), Gains::default());
        assert!((t[2] - 1600.0 * env().gravity).abs() < 1e-6);
    }

    #[test]
    fn brake_opposes_velocity() {
        let e = env();
        let t = brake([0.0, 0.0, -50.0], &e);
        assert!((t[2] - e.max_thrust).abs() < 1e-6);
        let t = brake([40.0, 0.0, -10.0], &e);
        assert!(t[0] < 0.0 && t[2] > 0.0);
        assert!(tilt(t) <= e.max_tilt + 1e-9);
        assert!((norm(t) - e.max_thrust).abs() < 1e-6);
    }
}
