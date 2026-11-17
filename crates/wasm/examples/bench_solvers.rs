//! Benchmark of the planner's solve times with each interior-point solver.
//!
//! Method: fly the reference flights natively (the page's loop as in
//! `tests/ship_loop.rs`: low noise, seed 7, 0.3 s throttle lag, solve
//! latency 0.5 s of sim time), record every planning request they make,
//! then solve each recorded request with each solver, `REPEATS` times,
//! timing the whole request (every SCvx iteration) with a wall clock and
//! keeping the median per request.
//!
//! `cargo run --release -p kestrel-wasm --example bench_solvers`
//!
//! Simulation only.

use std::time::Instant;

use guidance::scvx::ShipProblem;
use kestrel_wasm::ship_flight::{
    Kind, Leg, Phase, REPLAN_PERIOD, ShipFlight, ShipFlightConfig, plan,
};
use sim::Noise;

const REPEATS: usize = 5;

/// A request in flight: (arrives at, measured at, legs, answer).
type Pending = (
    f64,
    f64,
    Vec<Leg>,
    Result<(usize, guidance::scvx::ShipPlan), guidance::GuidanceError>,
);
const FRAME: f64 = 1.0 / 60.0;

/// Fly one reference flight; return every request it made.
fn record(kind: Kind) -> Vec<Vec<(Leg, ShipProblem)>> {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed: 7,
        lag: 0.3,
        noise: Noise::LOW,
        ..ShipFlightConfig::new(kind)
    })
    .expect("flight");
    let mut requests = Vec::new();
    let mut pending: Option<Pending> = None;
    let mut last = f64::NEG_INFINITY;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        let t = f.sim().t;
        if pending.as_ref().is_some_and(|(at, ..)| t >= *at) {
            let (_, t0, legs, r) = pending.take().expect("pending");
            match r {
                Ok((i, p)) => f.set_plan(p, t0, legs[i]),
                Err(e) => f.plan_failed(&e),
            }
        }
        if pending.is_none() && f.wants_replan() && t - last >= REPLAN_PERIOD {
            last = t;
            let (t0, options) = f.problem();
            let r = plan(&options);
            let legs = options.iter().map(|(l, _)| *l).collect();
            requests.push(options);
            pending = Some((t + 0.5, t0, legs, r));
        }
    }
    eprintln!(
        "{kind:?}: {:?} at {:.1} s, {} requests",
        f.sim().outcome,
        f.sim().t,
        requests.len()
    );
    requests
}

fn set_backend(name: &str) {
    guidance::scvx::set_backend(if name == "kestrel" {
        guidance::scvx::Backend::Kestrel
    } else {
        guidance::scvx::Backend::Clarabel
    });
}

fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((q * (s.len() - 1) as f64).round() as usize).min(s.len() - 1)]
}

fn main() {
    for kind in [Kind::Booster, Kind::Earth] {
        set_backend("clarabel");
        let requests = record(kind);
        for backend in ["clarabel", "kestrel"] {
            set_backend(backend);
            // Warm-up (first solves page in code and data).
            let _ = plan(&requests[0]);
            let mut times = Vec::new();
            let mut iterations = Vec::new();
            let mut failed = 0;
            for options in &requests {
                let mut runs = Vec::new();
                for _ in 0..REPEATS {
                    let t = Instant::now();
                    let r = plan(options);
                    runs.push(t.elapsed().as_secs_f64() * 1e3);
                    match r {
                        Ok((_, p)) => iterations.push(p.iterations as f64),
                        Err(_) => failed += 1,
                    }
                }
                times.push(quantile(&runs, 0.5));
            }
            failed /= REPEATS;
            println!(
                "{kind:?} {backend:9} {} requests ({} failed): median {:.1} ms, p90 {:.1} ms, max {:.1} ms; SCvx iterations median {:.0}",
                requests.len(),
                failed,
                quantile(&times, 0.5),
                quantile(&times, 0.9),
                quantile(&times, 1.0),
                quantile(&iterations, 0.5)
            );
        }
    }
}
