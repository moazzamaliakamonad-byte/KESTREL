//! Engine-out survivability of a single-engine descent.
//!
//! If the only running engine fails, a spare gives thrust `gap` seconds
//! later (ignition plus throttle lag); meanwhile the vehicle falls. It can
//! still stop above a height `floor` only if
//!
//! `(s + g·τ)² ≤ 2·a·(h − s·τ − g·τ²/2)`
//!
//! where `s` is its sink speed (m/s, down positive), `h` its height above
//! the floor, `τ` the gap and `a` the braking the spare gives, net of
//! gravity. Solved for `s`:
//!
//! `s ≤ −(g + a)·τ + √(τ²·a·(g + a) + 2·a·h)`.
//!
//! Below a certain height no sink speed is survivable, not even hovering:
//! the fall during the gap alone takes more height than is left. This is a
//! point-mass estimate: it assumes full braking the moment the spare
//! lights and ignores attitude, the sideways approach and sensing errors,
//! so being inside it is no guarantee; and the arms accept up to 2 m/s, so
//! a ship a little outside it can still be caught. It was tried as a
//! constraint in the planner and made plans converge badly (D-039); it is
//! used to explain outcomes and is shown on the page. Simulation only.

/// The bound's inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    /// Seconds without thrust after a failure.
    pub gap: f64,
    /// Braking available once the spare runs, net of gravity, m/s².
    pub decel: f64,
    /// Gravity, m/s².
    pub g: f64,
}

impl Envelope {
    /// Largest survivable sink speed at `h` metres above the floor, m/s
    /// (negative: not even hovering is survivable there).
    pub fn max_sink(&self, h: f64) -> f64 {
        let (t, a, g) = (self.gap, self.decel, self.g);
        if a <= 0.0 {
            return f64::NEG_INFINITY;
        }
        -(g + a) * t + (t * t * a * (g + a) + 2.0 * a * h.max(0.0)).sqrt()
    }

    /// Height above the floor below which hovering is not survivable, m.
    pub fn lowest(&self) -> f64 {
        let (t, a, g) = (self.gap, self.decel, self.g);
        let s = (g + a) * t;
        ((s * s - t * t * a * (g + a)) / (2.0 * a)).max(0.0)
    }

    /// How much slower than the bound the vehicle is sinking, m/s
    /// (negative: an engine loss now cannot be recovered).
    pub fn margin(&self, h: f64, sink: f64) -> f64 {
        self.max_sink(h) - sink
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const E: Envelope = Envelope {
        gap: 1.3,
        decel: 6.0,
        g: 9.81,
    };

    /// Simulate the gap and the braking: the bound's edge stops exactly at the floor.
    #[test]
    fn edge_of_the_bound_stops_at_the_floor() {
        for h in [40.0, 80.0, 200.0] {
            let s = E.max_sink(h);
            let (mut z, mut v) = (h, -s);
            let dt = 1e-4;
            let mut t = 0.0;
            while v < 0.0 {
                let a = if t < E.gap { -E.g } else { E.decel };
                v += a * dt;
                z += v * dt;
                t += dt;
            }
            assert!(z.abs() < 0.05, "h {h}: stopped at {z}");
        }
    }

    #[test]
    fn hovering_low_is_not_survivable() {
        let low = E.lowest();
        assert!(E.max_sink(low - 1.0) < 0.0);
        assert!(E.max_sink(low + 1.0) > 0.0);
        assert!((E.max_sink(low)).abs() < 1e-9);
    }
}
