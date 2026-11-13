//! Re-planning in flight (model predictive control).
//!
//! A full landing-time search costs ~25 solves (~250 ms). In flight we
//! already know roughly when we will land: the previous plan's landing time
//! minus the time since. So try three landing times around that hint and
//! keep the cheapest; successive re-plans walk toward the optimum. Only if
//! none works, fall back to the full search, and if no pinpoint landing
//! exists at all, to the closest landing (see [`crate::solve_closest`]).

use crate::params::Vec3;
use crate::problem::{GuidanceError, LandingProblem, Trajectory};
use crate::solve::{landing_time_bounds, solve_closest, solve_counted, solve_fixed_time};

/// What kind of landing the plan is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// At rest on the pad, inside the glide cone.
    Pinpoint,
    /// No pinpoint landing exists (or the vehicle left the glide cone):
    /// at rest on the ground as close to the pad as possible.
    Closest,
}

/// A re-plan's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Replan {
    pub trajectory: Trajectory,
    pub mode: Mode,
    /// Horizontal distance from the pad at touchdown, m.
    pub miss: f64,
    /// Convex solves it took.
    pub solves: usize,
}

/// Grid points for the closest-landing search when there is no hint.
pub const CLOSEST_GRID: usize = 10;

/// Spacing of the three warm-start landing times around a hint, s.
pub fn warm_step(hint: f64) -> f64 {
    (0.05 * hint).clamp(0.25, 2.0)
}

/// What the previous plan says about this one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hint {
    /// Expected landing time from now, s.
    pub final_time: f64,
    /// The previous plan's mode.
    pub mode: Mode,
}

/// Re-plan from the current (measured) state, warm-started from the
/// previous plan if there is one.
///
/// After a closest landing the full pinpoint search (~25 solves) is skipped:
/// the pad was out of reach a moment ago. The three cheap pinpoint tries
/// around the hint still run, so guidance can recover if it comes back.
pub fn replan(problem: &LandingProblem, hint: Option<Hint>) -> Result<Replan, GuidanceError> {
    problem.validate_state()?;
    let after_closest = hint.is_some_and(|h| h.mode == Mode::Closest);
    let hint = hint
        .map(|h| h.final_time)
        .filter(|h| h.is_finite() && *h > 0.0);
    let mut solves = 0;
    let mut error = GuidanceError::Infeasible;

    // The glide cone is centred on the pad: outside it, no pinpoint plan.
    if problem.validate().is_ok() {
        if let Some(h) = hint {
            let mut best = Best::default();
            for t in around(h) {
                solves += 1;
                best.offer(solve_fixed_time(problem, t), false);
            }
            if let Some(t) = best.trajectory {
                return Ok(answer(t, Mode::Pinpoint, solves));
            }
        }
        if !after_closest {
            let (result, n) = solve_counted(problem);
            solves += n;
            match result {
                Ok(t) => return Ok(answer(t, Mode::Pinpoint, solves)),
                Err(e) => error = e,
            }
        }
    }

    // Closest landing: warm first, then a grid with a little refinement.
    let mut best = Best::default();
    if let Some(h) = hint {
        for t in around(h) {
            solves += 1;
            best.offer(solve_closest(problem, t), true);
        }
    }
    if best.trajectory.is_none() {
        let (lo, hi) = closest_time_range(problem)?;
        let step = (hi - lo) / (CLOSEST_GRID - 1) as f64;
        for i in 0..CLOSEST_GRID {
            solves += 1;
            best.offer(solve_closest(problem, lo + i as f64 * step), true);
        }
        if let Some(centre) = best.trajectory.as_ref().map(|t| t.final_time) {
            for d in [-0.5, -0.25, 0.25, 0.5] {
                let t = centre + d * step;
                if t > 0.0 {
                    solves += 1;
                    best.offer(solve_closest(problem, t), true);
                }
            }
        }
    }
    match best.trajectory {
        Some(t) => Ok(answer(t, Mode::Closest, solves)),
        None => Err(error),
    }
}

/// Three landing times around a hint, skipping non-positive ones.
fn around(hint: f64) -> impl Iterator<Item = f64> {
    let h = warm_step(hint);
    [hint - h, hint, hint + h].into_iter().filter(|t| *t > 0.0)
}

/// Landing times worth trying for the closest landing: from the momentum
/// bound to the longest burn the propellant allows at minimum thrust.
fn closest_time_range(problem: &LandingProblem) -> Result<(f64, f64), GuidanceError> {
    let (lo, hi) = landing_time_bounds(problem)?;
    let v = &problem.vehicle;
    let longest_burn = (v.wet_mass - v.dry_mass) / (v.alpha() * v.min_thrust);
    let hi = hi.min(longest_burn);
    if hi > lo {
        Ok((lo, hi))
    } else {
        Err(GuidanceError::Infeasible)
    }
}

fn answer(trajectory: Trajectory, mode: Mode, solves: usize) -> Replan {
    let miss = miss_distance(&trajectory);
    Replan {
        trajectory,
        mode,
        miss,
        solves,
    }
}

/// Keeps the best plan offered: least fuel, or, for closest landings,
/// least miss first (within 0.1 m) and then least fuel.
#[derive(Default)]
struct Best {
    trajectory: Option<Trajectory>,
}

impl Best {
    fn offer(&mut self, result: Result<Trajectory, GuidanceError>, by_miss: bool) {
        let Ok(t) = result else { return };
        let better = match &self.trajectory {
            None => true,
            Some(b) => {
                let (dm, df) = (
                    miss_distance(&t) - miss_distance(b),
                    t.fuel_used() - b.fuel_used(),
                );
                if by_miss && dm.abs() > 0.1 {
                    dm < 0.0
                } else {
                    df < 0.0
                }
            }
        };
        if better {
            self.trajectory = Some(t);
        }
    }
}

/// Horizontal distance of the trajectory's end point from the pad, m.
pub fn miss_distance(t: &Trajectory) -> f64 {
    let end: Vec3 = t.position[t.position.len() - 1];
    end[0].hypot(end[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vehicle, solve};

    fn pin(final_time: f64) -> Option<Hint> {
        Some(Hint {
            final_time,
            mode: Mode::Pinpoint,
        })
    }

    fn example() -> LandingProblem {
        LandingProblem::example()
    }

    #[test]
    fn warm_matches_full_search() {
        let full = solve(&example()).unwrap();
        let r = replan(&example(), pin(full.final_time + 1.5)).unwrap();
        assert_eq!(r.mode, Mode::Pinpoint);
        assert_eq!(r.solves, 3);
        let rel = (r.trajectory.fuel_used() - full.fuel_used()) / full.fuel_used();
        assert!(rel.abs() < 0.01, "fuel off by {rel}");
        assert!(r.miss < 0.01);
    }

    #[test]
    fn no_hint_uses_full_search() {
        let full = solve(&example()).unwrap();
        let r = replan(&example(), None).unwrap();
        assert_eq!(r.mode, Mode::Pinpoint);
        assert!(r.solves > 3, "{}", r.solves);
        assert!((r.trajectory.fuel_used() - full.fuel_used()).abs() < 1e-9);
    }

    #[test]
    fn bad_hint_falls_back() {
        let full = solve(&example()).unwrap();
        for hint in [1.0, f64::NAN, -5.0, 1e9] {
            let r = replan(&example(), pin(hint)).unwrap();
            assert_eq!(r.mode, Mode::Pinpoint, "hint {hint}");
            assert!((r.trajectory.fuel_used() - full.fuel_used()).abs() < 1e-6);
        }
    }

    #[test]
    fn outside_cone_goes_closest() {
        let mut p = example();
        p.position = [3000.0, 0.0, 100.0];
        p.velocity = [-20.0, 0.0, -5.0];
        let r = replan(&p, None).unwrap();
        assert_eq!(r.mode, Mode::Closest);
        let n = r.trajectory.position.len() - 1;
        assert!(r.trajectory.position[n][2].abs() < 0.01);
    }

    #[test]
    fn short_of_propellant_goes_closest() {
        let mut p = example();
        p.vehicle.dry_mass = 1720.0;
        let r = replan(&p, None).unwrap();
        assert_eq!(r.mode, Mode::Closest);
        assert!(r.miss > 10.0, "miss {}", r.miss);
        // A warm closest re-plan from the answer's own landing time.
        let again = replan(
            &p,
            Some(Hint {
                final_time: r.trajectory.final_time,
                mode: r.mode,
            }),
        )
        .unwrap();
        assert_eq!(again.mode, Mode::Closest);
        assert!(again.solves <= 3 + CLOSEST_GRID + 4);
    }

    #[test]
    fn nearly_empty_tank_fails_cleanly() {
        let v = Vehicle::example_mars();
        let dry = v.dry_mass;
        for mass in [dry + 0.01, dry, dry - 1.0] {
            let p = LandingProblem::from_state(
                v.clone(),
                [2000.0, 0.0, 1500.0],
                [-50.0, 0.0, -75.0],
                mass,
                40,
            );
            assert!(replan(&p, pin(30.0)).is_err(), "mass {mass}");
        }
    }

    #[test]
    fn hopeless_is_error() {
        let mut p = example();
        p.velocity = [0.0, 0.0, -3000.0];
        assert!(replan(&p, pin(30.0)).is_err());
    }
}
