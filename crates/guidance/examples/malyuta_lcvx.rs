//! The LCvx rocket-landing example of Malyuta et al., "Convex Optimization
//! for Trajectory Generation" (arXiv:2106.09125), Part III, eq. (108),
//! Figure 21. Paper result: optimal time of flight 75 s, minimum-thrust
//! segment from about 40 s to 70 s.
//!
//! Differences from the paper (our model is simpler):
//! - no planet rotation (paper: ω = (3.5 x̂ + 2 ẑ)·10⁻³ °/s, Coriolis and
//!   centrifugal terms in the dynamics);
//! - glide slope as an exact cone (paper: four half-spaces);
//! - no 500 km/h speed limit (the paper reports it is never active);
//! - N = 75 intervals, so Δt ≈ 1 s near the optimum (paper: Δt = 1 s).
//!
//! cargo run --release -p kestrel-guidance --example malyuta_lcvx > target/malyuta.csv

use guidance::{LandingProblem, Vehicle, solve};

fn main() {
    let kmh = 1000.0 / 3600.0;
    let problem = LandingProblem {
        vehicle: Vehicle {
            wet_mass: 1905.0,
            dry_mass: 1505.0,
            isp: 225.0,
            min_thrust: 4971.0,
            max_thrust: 13258.0,
            gravity: 3.71,
            // Paper: γ_gs = 86° measured from vertical → 4° elevation.
            glide_slope: (90.0_f64 - 86.0).to_radians(),
            // Paper: γ_p = 40° pointing limit.
            max_tilt: 40.0_f64.to_radians(),
        },
        position: [2000.0, 0.0, 1500.0],
        velocity: [288.0 * kmh, 108.0 * kmh, -270.0 * kmh],
        steps: 75,
    };

    match solve(&problem) {
        Ok(t) => {
            print!("{}", t.to_csv());
            let lo = problem.vehicle.min_thrust * 1.001;
            let at_min: Vec<f64> = t
                .thrust
                .iter()
                .zip(&t.t)
                .filter(|(f, _)| (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt() <= lo)
                .map(|(_, &time)| time)
                .collect();
            eprintln!(
                "time of flight {:.2} s (paper: 75 s), propellant {:.2} kg",
                t.final_time,
                t.fuel_used()
            );
            if let (Some(first), Some(last)) = (at_min.first(), at_min.last()) {
                eprintln!(
                    "minimum-thrust intervals start between {first:.1} s and {last:.1} s \
                     (paper: about 40 s to 70 s)"
                );
            }
        }
        Err(e) => {
            eprintln!("no landing: {e:?}");
            std::process::exit(1);
        }
    }
}
