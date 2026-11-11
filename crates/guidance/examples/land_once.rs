//! Solve the example landing once, choosing the landing time too.
//!
//! cargo run --release -p kestrel-guidance --example land_once > target/landing.csv
//!
//! CSV goes to stdout; a summary goes to stderr.

use std::time::Instant;

use guidance::{LandingProblem, solve, solve_fixed_time};

fn main() {
    let problem = LandingProblem::example();

    let start = Instant::now();
    let traj = match solve(&problem) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("no landing: {e:?}");
            std::process::exit(1);
        }
    };
    let search_time = start.elapsed();

    // Time one fixed-time solve on its own, as the per-solve cost.
    let start = Instant::now();
    let _ = solve_fixed_time(&problem, traj.final_time);
    let one_solve = start.elapsed();

    print!("{}", traj.to_csv());
    eprintln!(
        "landing time {:.2} s, propellant used {:.2} kg, N = {}",
        traj.final_time,
        traj.fuel_used(),
        problem.steps
    );
    eprintln!(
        "free-time search took {:.1} ms; one fixed-time solve took {:.1} ms",
        search_time.as_secs_f64() * 1e3,
        one_solve.as_secs_f64() * 1e3
    );
}
