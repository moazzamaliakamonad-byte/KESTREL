//! The landing problem (inputs) and the planned trajectory (output).

use crate::params::{ParamError, Vec3, Vehicle};

/// One landing to plan: a vehicle, its current state, and how finely to
/// slice time. All SI units, pad at the origin, `z` up.
#[derive(Debug, Clone, PartialEq)]
pub struct LandingProblem {
    pub vehicle: Vehicle,
    /// Current position, m.
    pub position: Vec3,
    /// Current velocity, m/s.
    pub velocity: Vec3,
    /// Number of time intervals N. Thrust is constant within each.
    pub steps: usize,
}

/// Why guidance could not produce a trajectory.
#[derive(Debug, Clone, PartialEq)]
pub enum GuidanceError {
    /// The vehicle parameters are not physically meaningful.
    InvalidParams(ParamError),
    /// The problem is malformed before any solving (bad state, too few steps…).
    InvalidProblem(&'static str),
    /// No plan satisfies every constraint: the solver proved it (or nearly
    /// so), or the fuel-optimal plan over all landing times needs more
    /// propellant than the tanks hold.
    Infeasible,
    /// The convex relaxation was not tight: on `interval`, the thrust
    /// delivered (`ratio` × the thrust paid for in fuel) is below the
    /// engine's minimum. The plan is not physically flyable, so it is not
    /// returned. Seen on purely vertical descents.
    NotLossless { interval: usize, ratio: f64 },
    /// The solver stopped without an answer or a proof; holds its status.
    SolverFailed(String),
}

/// A planned landing, in SI units.
///
/// `t`, `position`, `velocity` and `mass` have one entry per node
/// (N + 1). `thrust` has one entry per interval (N): thrust `k` acts
/// from `t[k]` to `t[k + 1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    pub t: Vec<f64>,
    pub position: Vec<Vec3>,
    pub velocity: Vec<Vec3>,
    pub mass: Vec<f64>,
    pub thrust: Vec<Vec3>,
    pub final_time: f64,
}

impl LandingProblem {
    /// The example vehicle 2 km out and 1.5 km up, descending toward the pad.
    pub fn example() -> LandingProblem {
        LandingProblem {
            vehicle: Vehicle::example_mars(),
            position: [2000.0, 0.0, 1500.0],
            velocity: [-50.0, 0.0, -75.0],
            steps: 50,
        }
    }

    /// A problem starting mid-flight with `mass` kg on board. The solver
    /// treats the start mass as the "wet" mass, so this relabels it; the
    /// dry mass (empty tanks) is unchanged.
    pub fn from_state(
        vehicle: Vehicle,
        position: Vec3,
        velocity: Vec3,
        mass: f64,
        steps: usize,
    ) -> LandingProblem {
        LandingProblem {
            vehicle: Vehicle {
                wet_mass: mass,
                ..vehicle
            },
            position,
            velocity,
            steps,
        }
    }

    /// Reject problems that are malformed before trying to solve them.
    pub fn validate(&self) -> Result<(), GuidanceError> {
        self.vehicle
            .validate()
            .map_err(GuidanceError::InvalidParams)?;
        if self.steps < 2 {
            return Err(GuidanceError::InvalidProblem("steps < 2"));
        }
        let state_is_finite = self
            .position
            .iter()
            .chain(self.velocity.iter())
            .all(|x| x.is_finite());
        if !state_is_finite {
            return Err(GuidanceError::InvalidProblem("state not finite"));
        }
        let [x, y, z] = self.position;
        if z < self.vehicle.glide_slope.tan() * x.hypot(y) {
            return Err(GuidanceError::InvalidProblem("outside glide slope"));
        }
        Ok(())
    }
}

impl Trajectory {
    /// Propellant burned over the whole plan, kg.
    pub fn fuel_used(&self) -> f64 {
        self.mass[0] - self.mass[self.mass.len() - 1]
    }

    /// One row per node. Thrust columns hold the thrust of the interval that
    /// starts at that node, so they are empty on the last row.
    pub fn to_csv(&self) -> String {
        use std::fmt::Write;
        let mut out = String::from("t,x,y,z,vx,vy,vz,mass,tx,ty,tz,thrust_mag\n");
        for k in 0..self.t.len() {
            let [x, y, z] = self.position[k];
            let [vx, vy, vz] = self.velocity[k];
            // Writing into a String cannot fail.
            let _ = write!(
                out,
                "{},{x},{y},{z},{vx},{vy},{vz},{}",
                self.t[k], self.mass[k]
            );
            match self.thrust.get(k) {
                Some(&[tx, ty, tz]) => {
                    let mag = (tx * tx + ty * ty + tz * tz).sqrt();
                    let _ = writeln!(out, ",{tx},{ty},{tz},{mag}");
                }
                None => out.push_str(",,,,\n"),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_is_valid() {
        assert_eq!(LandingProblem::example().validate(), Ok(()));
    }

    #[test]
    fn from_state_relabels_wet_mass() {
        let v = Vehicle::example_mars();
        let p =
            LandingProblem::from_state(v.clone(), [10.0, 0.0, 500.0], [1.0, 0.0, -9.0], 1700.0, 40);
        assert_eq!(p.vehicle.wet_mass, 1700.0);
        assert_eq!(p.vehicle.dry_mass, v.dry_mass);
        assert_eq!(
            (p.position, p.velocity, p.steps),
            ([10.0, 0.0, 500.0], [1.0, 0.0, -9.0], 40)
        );
    }

    #[test]
    fn rejects_bad_vehicle() {
        let mut p = LandingProblem::example();
        p.vehicle.wet_mass = -5.0;
        assert!(matches!(p.validate(), Err(GuidanceError::InvalidParams(_))));
    }

    #[test]
    fn rejects_nan_state() {
        let mut p = LandingProblem::example();
        p.velocity[1] = f64::NAN;
        assert_eq!(
            p.validate(),
            Err(GuidanceError::InvalidProblem("state not finite"))
        );
    }

    #[test]
    fn rejects_below_ground() {
        let mut p = LandingProblem::example();
        p.position = [0.0, 0.0, -1.0];
        assert_eq!(
            p.validate(),
            Err(GuidanceError::InvalidProblem("outside glide slope"))
        );
    }

    #[test]
    fn rejects_outside_glide_slope() {
        // 10 km out but only 100 m up: elevation ≈ 0.6°, below the 4° cone.
        let mut p = LandingProblem::example();
        p.position = [10_000.0, 0.0, 100.0];
        assert_eq!(
            p.validate(),
            Err(GuidanceError::InvalidProblem("outside glide slope"))
        );
    }

    #[test]
    fn rejects_too_few_steps() {
        let mut p = LandingProblem::example();
        p.steps = 1;
        assert_eq!(
            p.validate(),
            Err(GuidanceError::InvalidProblem("steps < 2"))
        );
    }

    #[test]
    fn csv_has_header_one_row_per_node_and_no_thrust_after_landing() {
        let traj = Trajectory {
            t: vec![0.0, 1.0, 2.0],
            position: vec![[1.0, 2.0, 3.0]; 3],
            velocity: vec![[4.0, 5.0, 6.0]; 3],
            mass: vec![1905.0, 1900.0, 1895.0],
            thrust: vec![[0.0, 3000.0, 4000.0]; 2],
            final_time: 2.0,
        };
        let csv = traj.to_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "t,x,y,z,vx,vy,vz,mass,tx,ty,tz,thrust_mag");
        assert_eq!(lines.len(), 1 + 3);
        let first: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(first.len(), 12);
        assert_eq!(first[11].parse::<f64>().unwrap(), 5000.0);
        let last: Vec<&str> = lines[3].split(',').collect();
        assert_eq!(last.len(), 12);
        assert!(last[8..].iter().all(|c| c.is_empty()), "{:?}", lines[3]);
    }

    #[test]
    fn fuel_used_is_mass_difference() {
        let traj = Trajectory {
            t: vec![0.0, 1.0, 2.0],
            position: vec![[0.0; 3]; 3],
            velocity: vec![[0.0; 3]; 3],
            mass: vec![1905.0, 1899.0, 1890.5],
            thrust: vec![[0.0; 3]; 2],
            final_time: 2.0,
        };
        assert!((traj.fuel_used() - 14.5).abs() < 1e-12);
    }
}
