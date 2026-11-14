//! Closed-loop scenarios, run natively: the same loop the page runs, with
//! the worker's solve latency modelled as one re-plan period. Seeded, so
//! every number is reproducible.
//!
//! Simulation only.

use guidance::LandingProblem;
use kestrel_wasm::flight::{Flight, FlightConfig, Outcome, plan};
use sim::Noise;

/// Re-plan period and modelled solve latency, s (sim time).
const PERIOD: f64 = 0.25;
/// Frame step of the page's render loop, s.
const FRAME: f64 = 1.0 / 60.0;

#[derive(Clone, Copy)]
enum Event {
    GustRight,
    GustLeft,
    EngineOut,
}

struct Scenario {
    config: FlightConfig,
    events: Vec<(f64, Event)>,
}

fn scenario() -> Scenario {
    let p = LandingProblem::example();
    Scenario {
        config: FlightConfig {
            position: p.position,
            velocity: p.velocity,
            vehicle: p.vehicle,
            seed: 1,
            lag: 0.0,
            noise: Noise::OFF,
        },
        events: Vec::new(),
    }
}

struct Run {
    outcome: Outcome,
    replans: usize,
    closest: usize,
}

fn fly(sc: Scenario) -> Run {
    let mut f = Flight::new(sc.config).expect("valid config");
    let mut events = sc.events;
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut events = events.into_iter().peekable();

    // The page starts with the plan it previewed before Land.
    let m = f.measure();
    let first = plan(&f.problem(&m), None).expect("start is landable");
    f.set_plan(first, m.t);

    let (mut replans, mut closest, mut solves, mut slow) = (0, 0, 0, 0);
    let mut pending: Option<(f64, Result<guidance::Replan, guidance::GuidanceError>, f64)> = None;
    let mut last_request = 0.0;
    while f.outcome().is_none() {
        let now = f.time();
        assert!(now < 300.0, "no touchdown after 300 s");
        while events.peek().is_some_and(|(t, _)| *t <= now) {
            match events.next().unwrap().1 {
                Event::GustRight => f.gust(1.0),
                Event::GustLeft => f.gust(-1.0),
                Event::EngineOut => {
                    f.engine_out();
                }
            }
        }
        f.step(FRAME);
        let now = f.time();
        if pending.as_ref().is_some_and(|(at, _, _)| now >= *at) {
            let (_, result, t_measured) = pending.take().unwrap();
            replans += 1;
            match result {
                Ok(r) => {
                    solves += r.solves;
                    slow += usize::from(r.solves > 3);
                    if r.mode == guidance::Mode::Closest {
                        closest += 1;
                    }
                    f.set_plan(r, t_measured);
                }
                Err(e) => f.plan_failed(&e),
            }
        }
        if pending.is_none() && now - last_request >= PERIOD && f.wants_replan() {
            last_request = now;
            let m = f.measure();
            let result = plan(&f.problem(&m), f.hint(m.t));
            pending = Some((now + PERIOD, result, m.t));
        }
    }
    let outcome = f.outcome().unwrap();
    println!(
        "landed={} speed={:.2} vz={:.2} vh={:.2} miss={:.1} fuel={:.1} replans={replans} closest={closest} solves={solves} slow={slow}",
        outcome.landed,
        outcome.speed,
        outcome.vertical,
        outcome.horizontal,
        outcome.miss,
        outcome.fuel_used
    );
    Run {
        outcome,
        replans,
        closest,
    }
}

#[test]
fn nominal_lands_on_pad() {
    let r = fly(scenario());
    assert!(r.outcome.landed);
    assert!(r.outcome.speed < 1.0, "speed {}", r.outcome.speed);
    assert!(r.outcome.miss < 2.0, "miss {}", r.outcome.miss);
    assert!(r.replans > 50);
    assert_eq!(r.closest, 0);
}

#[test]
fn noise_and_lag_still_land() {
    let mut sc = scenario();
    sc.config.noise = Noise::HIGH;
    sc.config.lag = 1.0;
    let r = fly(sc);
    assert!(r.outcome.landed, "speed {}", r.outcome.speed);
    assert!(r.outcome.miss < 30.0, "miss {}", r.outcome.miss);
}

#[test]
fn two_gusts_still_land() {
    let mut sc = scenario();
    sc.events = vec![(5.0, Event::GustRight), (15.0, Event::GustLeft)];
    let r = fly(sc);
    assert!(r.outcome.landed);
    assert!(r.outcome.miss < 5.0, "miss {}", r.outcome.miss);
}

#[test]
fn engine_out_midflight_lands() {
    let mut sc = scenario();
    sc.events = vec![(10.0, Event::EngineOut)];
    let r = fly(sc);
    assert!(r.outcome.landed);
    assert!(r.outcome.miss < 5.0, "miss {}", r.outcome.miss);
}

/// Two engines out leaves 8.8 kN against ~6.8 kN of weight: about
/// 1 m/s² of braking, where ~2.8 km would be needed from this start. No
/// landing exists; guidance must say so (brake) and nothing may break.
#[test]
#[ignore = "slow: every failing re-plan runs the full and closest searches"]
fn two_engines_out_crashes_cleanly() {
    let mut sc = scenario();
    sc.events = vec![(8.0, Event::EngineOut), (12.0, Event::EngineOut)];
    let r = fly(sc);
    assert!(!r.outcome.landed);
    assert!(r.outcome.speed.is_finite());
}

#[test]
fn fuel_starved_start_lands_closest_reports_miss() {
    let mut sc = scenario();
    // Measured: lands ~247 m short of the pad.
    sc.config.vehicle.dry_mass = 1670.0;
    let r = fly(sc);
    assert!(r.closest > 0);
    assert!(r.outcome.landed, "speed {}", r.outcome.speed);
    assert!(r.outcome.miss > 10.0, "miss {}", r.outcome.miss);
}

#[test]
#[ignore = "slow: every failing re-plan runs the full and closest searches"]
fn three_engines_out_early_crashes_without_panic() {
    let mut sc = scenario();
    sc.events = vec![
        (1.0, Event::EngineOut),
        (1.5, Event::EngineOut),
        (2.0, Event::EngineOut),
    ];
    let r = fly(sc);
    assert!(r.outcome.speed.is_finite());
    assert!(!r.outcome.landed, "3/6 engines can't hold 1.9 t on Mars");
}

#[test]
#[ignore = "slow: 50 seeded flights with high noise (run with --ignored)"]
fn monte_carlo_high_noise() {
    let mut speeds = Vec::new();
    let mut misses = Vec::new();
    let mut landed = 0;
    for seed in 0..50 {
        let mut sc = scenario();
        sc.config.seed = seed;
        sc.config.noise = Noise::HIGH;
        let r = fly(sc);
        landed += usize::from(r.outcome.landed);
        speeds.push(r.outcome.speed);
        misses.push(r.outcome.miss);
    }
    speeds.sort_by(f64::total_cmp);
    misses.sort_by(f64::total_cmp);
    println!(
        "landed {landed}/50; speed median {:.2} max {:.2}; miss median {:.1} max {:.1}",
        speeds[25], speeds[49], misses[25], misses[49]
    );
}
