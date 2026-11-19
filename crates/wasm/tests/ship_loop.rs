//! Closed-loop ship flights, run natively: the loop the page runs, with the
//! worker's solve latency modelled as one re-plan period. Seeded.
//! Simulation only.

use kestrel_wasm::ship_flight::{
    Guide, Kind, Leg, Phase, REPLAN_PERIOD, ShipFlight, ShipFlightConfig, plan,
};
use model::dynamics;
use sim::Noise;
use sim::ship::Outcome;

const FRAME: f64 = 1.0 / 60.0;
/// Solve time of the slow-computer test, s.
const SLOW: f64 = 1.0;

#[derive(Clone, Copy)]
enum Event {
    Gust(f64),
    EngineOut,
}

struct Report {
    outcome: Option<Outcome>,
    replans: usize,
    failures: usize,
    t: f64,
    fuel: f64,
    relights: u32,
    /// Engine-out envelope margin when the last engine-out event fired, m/s.
    margin_at_fail: Option<f64>,
}

type Pending = (
    f64,
    f64,
    Vec<Leg>,
    Result<(usize, guidance::scvx::ShipPlan), guidance::GuidanceError>,
);

/// Ask for a re-plan if wanted; solve now, deliver after `REPLAN_PERIOD`.
fn request(f: &mut ShipFlight, pending: &mut Option<Pending>, last_request: &mut f64) {
    request_after(f, pending, last_request, REPLAN_PERIOD);
}

/// As `request`, with the solver taking `latency` seconds of sim time.
fn request_after(
    f: &mut ShipFlight,
    pending: &mut Option<Pending>,
    last_request: &mut f64,
    latency: f64,
) {
    let t = f.sim().t;
    if pending.is_none() && f.wants_replan() && t - *last_request >= REPLAN_PERIOD {
        *last_request = t;
        let (t0, options) = f.problem();
        let r = plan(&options);
        let legs = options.iter().map(|(l, _)| *l).collect();
        *pending = Some((t + latency, t0, legs, r));
    }
}

/// Deliver a finished re-plan. Returns Some(ok) when one was delivered.
fn deliver(f: &mut ShipFlight, pending: &mut Option<Pending>) -> Option<bool> {
    let ready = pending.as_ref().is_some_and(|(at, ..)| f.sim().t >= *at);
    if !ready {
        return None;
    }
    let (_, t0, legs, r) = pending.take().unwrap();
    match r {
        Ok((i, p)) => {
            f.set_plan(p, t0, legs[i]);
            Some(true)
        }
        Err(e) => {
            f.plan_failed(&e);
            Some(false)
        }
    }
}

/// Fly to the end. Events fire at sim times; a solve requested at `t` is
/// applied at `t + REPLAN_PERIOD`, stamped `t`.
fn fly(
    kind: Kind,
    noise: Noise,
    lag: f64,
    events: &[(f64, Event)],
    ignition_events: &[Event],
) -> Report {
    fly_with(kind, noise, lag, events, ignition_events, true)
}

/// As `fly`, with wind awareness on or off.
fn fly_with(
    kind: Kind,
    noise: Noise,
    lag: f64,
    events: &[(f64, Event)],
    ignition_events: &[Event],
    wind_aware: bool,
) -> Report {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed: 7,
        lag,
        noise,
        wind_aware,
        ..ShipFlightConfig::new(kind)
    })
    .unwrap();
    let m0 = f.sim().x[dynamics::M];
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let mut fired = vec![false; events.len()];
    let mut ignition_fired = false;
    let (mut replans, mut failures) = (0, 0);
    let mut margin_at_fail = None;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        let t = f.sim().t;
        for (i, (at, e)) in events.iter().enumerate() {
            if !fired[i] && t >= *at {
                fired[i] = true;
                match e {
                    Event::Gust(d) => f.gust(*d),
                    Event::EngineOut => {
                        margin_at_fail = f.engine_out_margin();
                        f.engine_out();
                    }
                }
            }
        }
        if !ignition_fired && f.phase() == Phase::Flip {
            ignition_fired = true;
            for e in ignition_events {
                if let Event::EngineOut = e {
                    f.engine_out();
                }
            }
        }
        match deliver(&mut f, &mut pending) {
            Some(true) => replans += 1,
            Some(false) => failures += 1,
            None => {}
        }
        request(&mut f, &mut pending, &mut last_request);
    }
    let s = f.sim();
    let r = Report {
        outcome: s.outcome,
        replans,
        failures,
        t: s.t,
        fuel: m0 - s.x[dynamics::M],
        relights: f.relights(),
        margin_at_fail,
    };
    let p = dynamics::position(&s.x);
    eprintln!(
        "{kind:?}: {:?} at t {:.1} s, {} re-plans, {} failures, fuel {:.0} kg, end at ({:.2}, {:.2}, {:.2}), v {:?}",
        r.outcome, r.t, r.replans, r.failures, r.fuel, p[0], p[1], p[2], s.end_velocity
    );
    r
}

#[test]
fn earth_nominal_is_caught() {
    let r = fly(Kind::Earth, Noise::LOW, 0.3, &[], &[]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
    assert!(r.replans > 5 && r.failures == 0);
}

#[test]
fn earth_engine_out_at_ignition_is_caught() {
    let r = fly(Kind::Earth, Noise::LOW, 0.3, &[], &[Event::EngineOut]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

/// The one engine flying the landing burn fails: the flight lights a
/// spare and is still caught (it used to fall).
#[test]
fn earth_running_engine_fails_mid_burn_a_spare_relights() {
    for at in [55.0, 61.0] {
        let r = fly(Kind::Earth, Noise::LOW, 0.3, &[(at, Event::EngineOut)], &[]);
        eprintln!(
            "engine out at {at} s: {:?}, {} relight(s)",
            r.outcome, r.relights
        );
        assert_eq!(r.relights, 1, "at {at} s");
        assert_eq!(r.outcome, Some(Outcome::Caught), "at {at} s");
    }
}

#[test]
fn earth_gust_in_belly_flop_is_caught() {
    let r = fly(
        Kind::Earth,
        Noise::LOW,
        0.3,
        &[(20.0, Event::Gust(1.0)), (35.0, Event::Gust(-1.0))],
        &[],
    );
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

#[test]
fn booster_nominal_is_caught() {
    let r = fly(Kind::Booster, Noise::LOW, 0.3, &[], &[]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
    assert!(r.replans > 3);
}

#[test]
fn booster_centre_engine_out_at_ignition_is_caught() {
    let r = fly(Kind::Booster, Noise::LOW, 0.3, &[], &[Event::EngineOut]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

#[test]
fn booster_gusts_in_descent_is_caught() {
    let r = fly(
        Kind::Booster,
        Noise::LOW,
        0.3,
        &[(8.0, Event::Gust(1.0)), (20.0, Event::Gust(-1.0))],
        &[],
    );
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

/// A steep entry from 80 km at 2.5 km/s: the belly flop must hold through
/// the dense air and the engines must wait for it to slow the ship. Slow
/// (a 200 s flight): run with `--ignored`.
#[test]
#[ignore]
fn earth_fast_entry_is_caught() {
    let mut f = ShipFlight::new(ShipFlightConfig {
        position: [3000.0, 0.0, 80_000.0],
        velocity: [-100.0, 0.0, -2500.0],
        seed: 7,
        lag: 0.3,
        noise: Noise::LOW,
        ..ShipFlightConfig::new(Kind::Earth)
    })
    .unwrap();
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        deliver(&mut f, &mut pending);
        request(&mut f, &mut pending, &mut last_request);
    }
    assert_eq!(f.sim().outcome, Some(Outcome::Caught));
}

/// The browser's conditions, swept: solve latency, the solve time the page
/// reports (prediction lead) and the frame step (sim speed 2 at 30 fps is
/// 1/15 s). Debug aid: `--ignored --nocapture`.
#[test]
#[ignore]
fn booster_browser_sweep() {
    for (latency, lead, frame) in [
        (0.5, 0.0, FRAME),
        (0.5, 0.5, FRAME),
        (0.3, 0.3, FRAME),
        (0.3, 0.3, 1.0 / 15.0),
        (0.5, 0.55, 1.0 / 15.0),
        (0.25, 0.3, 0.1),
        (0.1, 0.15, FRAME),
        (1.0, 1.0, FRAME),
        (0.5, 0.55, 0.2),
        (0.5, 0.55, 0.133),
        (0.25, 0.3, 0.2),
    ] {
        for seed in [7u64, 11, 23] {
            let mut f = ShipFlight::new(ShipFlightConfig {
                seed,
                lag: 0.3,
                noise: Noise::LOW,
                ..ShipFlightConfig::new(Kind::Booster)
            })
            .unwrap();
            f.set_solve_time(lead);
            let mut pending: Option<Pending> = None;
            let mut last_request = f64::NEG_INFINITY;
            let (mut ok, mut bad) = (0, 0);
            while f.phase() != Phase::Done && f.sim().t < 400.0 {
                f.step(frame);
                match deliver(&mut f, &mut pending) {
                    Some(true) => ok += 1,
                    Some(false) => bad += 1,
                    None => {}
                }
                request_after(&mut f, &mut pending, &mut last_request, latency);
            }
            eprintln!(
                "latency {latency} lead {lead} frame {frame:.3} seed {seed}: {:?} t {:.0} plans {ok} failed {bad} prop {:.0} t",
                f.sim().outcome,
                f.sim().t,
                f.sim().propellant() / 1e3
            );
        }
    }
}

/// A slow computer: every solve takes a full second of sim time (the
/// booster's whole 13-engine leg is about 9 s). It must still be caught.
/// (At 1.5 s it is not: see LIMITATIONS.md.)
#[test]
fn booster_slow_solver_is_caught() {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed: 7,
        lag: 0.3,
        noise: Noise::LOW,
        ..ShipFlightConfig::new(Kind::Booster)
    })
    .unwrap();
    f.set_solve_time(SLOW);
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let mut next = 0.0;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        if let Some(ok) = deliver(&mut f, &mut pending) {
            eprintln!(
                "  plan {} at {:.2} leg {:?}",
                if ok { "ok" } else { "FAILED" },
                f.sim().t,
                f.leg().map(|l| (l.engines, l.gate[2] as i32))
            );
        }
        request_after(&mut f, &mut pending, &mut last_request, SLOW);
        if f.sim().t >= next && f.phase() != Phase::BellyFlop {
            next = f.sim().t + 1.0;
            let x = f.sim().x;
            let (r, v) = (dynamics::position(&x), dynamics::velocity(&x));
            eprintln!(
                "t {:5.1} {:12} r ({:6.1},{:6.1},{:7.1}) v ({:5.1},{:5.1},{:6.1}) tilt {:4.1} lit {}",
                f.sim().t,
                f.phase().label(Kind::Booster),
                r[0],
                r[1],
                r[2],
                v[0],
                v[1],
                v[2],
                model::quat::tilt(dynamics::attitude(&x)).to_degrees(),
                f.sim().cluster.lit()
            );
        }
    }
    eprintln!(
        "slow solver: {:?} at t {:.1}, {:.1} t left",
        f.sim().outcome,
        f.sim().t,
        f.sim().propellant() / 1e3
    );
    assert_eq!(f.sim().outcome, Some(Outcome::Caught));
}

#[test]
#[ignore]
fn trace_booster() {
    trace(Kind::Booster);
}

#[test]
#[ignore]
fn earth_high_noise_and_lag() {
    let r = fly(Kind::Earth, Noise::HIGH, 1.0, &[], &[]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

/// Debug aid: print the flight every half second (`--ignored --nocapture`).
#[test]
#[ignore]
fn trace_earth() {
    trace(Kind::Earth);
}

fn trace(kind: Kind) {
    use model::quat;
    let noise = match std::env::var("TRACE_NOISE").as_deref() {
        Ok("low") => Noise::LOW,
        Ok("high") => Noise::HIGH,
        _ => Noise::OFF,
    };
    let seed = std::env::var("TRACE_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed,
        lag: 0.3,
        noise,
        ..ShipFlightConfig::new(kind)
    })
    .unwrap();
    let mut next_print = 0.0;
    let mut last_request = f64::NEG_INFINITY;
    let mut pending: Option<Pending> = None;
    // Debug aid: FAIL_AT=63 fails the running engine at that sim time.
    let mut fail_at: Option<f64> = std::env::var("FAIL_AT").ok().and_then(|s| s.parse().ok());
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        let t = f.sim().t;
        if fail_at.is_some_and(|a| t >= a) {
            fail_at = None;
            f.engine_out();
            eprintln!("   ENGINE OUT at {t:.2}");
        }
        if let Some(ok) = deliver(&mut f, &mut pending) {
            match (ok, f.guide(), f.leg()) {
                (true, Guide::Plan { plan, t0 }, Some(leg)) => eprintln!(
                    "   plan @{t0:.2}: {} eng to z {:.0} tf {:.1} it {} conv {} gate_err {:.1}",
                    leg.engines,
                    leg.gate[2],
                    plan.final_time,
                    plan.iterations,
                    plan.converged,
                    plan.gate_error
                ),
                _ => eprintln!("   plan FAILED at {t:.2}"),
            }
        }
        request(&mut f, &mut pending, &mut last_request);
        if t >= next_print || f.phase() == Phase::Done {
            next_print += if f.phase() == Phase::BellyFlop {
                5.0
            } else {
                0.5
            };
            let s = f.sim();
            let x = s.x;
            let r = dynamics::position(&x);
            let v = dynamics::velocity(&x);
            let q = dynamics::attitude(&x);
            let nose = quat::rotate(q, [0.0, 0.0, 1.0]);
            let pr = match f.guide() {
                Guide::Plan { plan, t0 } => dynamics::position(&plan.sample(t - t0).0),
                _ => [f64::NAN; 3],
            };
            eprintln!(
                "t {t:6.2} {:12} r ({:7.1},{:6.1},{:7.1}) v ({:6.1},{:6.1},{:6.1}) nose ({:5.2},{:5.2}) w {:?} T {:5.2} MN n {} prop {:5.1} t | ref ({:7.1},{:7.1})",
                f.phase().label(kind),
                r[0],
                r[1],
                r[2],
                v[0],
                v[1],
                v[2],
                nose[0],
                nose[2],
                dynamics::rates(&x).map(|c| (c.to_degrees() * 10.0).round() / 10.0),
                quat::norm(s.cluster.thrust) / 1e6,
                s.cluster.lit(),
                s.propellant() / 1e3,
                pr[0],
                pr[2]
            );
        }
    }
    eprintln!("outcome {:?}", f.sim().outcome);
}

/// Debug aid: fly any start. `KIND=booster START=x,z,vx,vz cargo test … start_probe -- --ignored --nocapture`.
#[test]
#[ignore]
fn start_probe() {
    let kind =
        Kind::parse(&std::env::var("KIND").unwrap_or_else(|_| "booster".into())).expect("KIND");
    let s: Vec<f64> = std::env::var("START")
        .expect("START")
        .split(',')
        .map(|v| v.parse().expect("number"))
        .collect();
    let mut f = ShipFlight::new(ShipFlightConfig {
        position: [s[0], 0.0, s[1]],
        velocity: [s[2], 0.0, s[3]],
        seed: 7,
        lag: 0.3,
        noise: Noise::LOW,
        ..ShipFlightConfig::new(kind)
    })
    .unwrap();
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let (mut vmax, mut next) = (0.0f64, 0.0);
    while f.phase() != Phase::Done && f.sim().t < 900.0 {
        f.step(FRAME);
        deliver(&mut f, &mut pending);
        request(&mut f, &mut pending, &mut last_request);
        let x = f.sim().x;
        let v = dynamics::velocity(&x);
        vmax = vmax.max(model::quat::norm(v));
        if f.sim().t >= next {
            next += 10.0;
            let r = dynamics::position(&x);
            eprintln!(
                "t {:5.0} {:12} r ({:8.0},{:7.0}) v ({:6.0},{:6.0}) tilt {:4.0}",
                f.sim().t,
                f.phase().label(kind),
                r[0],
                r[2],
                v[0],
                v[2],
                model::quat::tilt(dynamics::attitude(&x)).to_degrees()
            );
        }
    }
    eprintln!(
        "outcome {:?} at t {:.0}, top speed {:.0} m/s, propellant left {:.1} t",
        f.sim().outcome,
        f.sim().t,
        vmax,
        f.sim().propellant() / 1e3
    );
}

/// When can the burn's engine fail and the ship still be caught? Sweeps
/// the failure time (`--ignored --nocapture`).
#[test]
#[ignore]
fn engine_out_window() {
    for at in [49.0, 51.0, 53.0, 55.0, 57.0, 59.0, 61.0, 63.0, 65.0, 67.0] {
        for noise in [Noise::OFF, Noise::LOW] {
            let r = fly(Kind::Earth, noise, 0.3, &[(at, Event::EngineOut)], &[]);
            eprintln!(
                "WINDOW fail at {at} s, noise {:.2}: {:?}, {} relight(s), envelope margin {}",
                noise.pos_sigma,
                r.outcome,
                r.relights,
                r.margin_at_fail
                    .map_or("n/a".to_string(), |m| format!("{m:+.1} m/s"))
            );
        }
    }
}

/// The booster loses engines mid-burn (as clicked on the page): which
/// losses, when, are caught? Solves take 0.5 s of sim time; frames of
/// 1/60 s, and 1/15 s (the page at sim speed 2 and 30 fps). Its 13-engine
/// leg runs about 25-33 s. 320 flights, threaded: `--ignored
/// --nocapture`. Debug one: `BOARD_SET="two outer" BOARD_ONLY=31,23,15
/// BOARD_TRACE=1`.
#[test]
#[ignore]
fn booster_engine_out_board() {
    const SETS: [(&str, &[usize]); 4] = [
        ("one outer", &[5]),
        ("two outer", &[5, 9]),
        ("three outer", &[4, 8, 12]),
        ("one centre", &[0]),
    ];
    let only_set = std::env::var("BOARD_SET").ok();
    let only = std::env::var("BOARD_ONLY").ok();
    let mut jobs = Vec::new();
    for (name, set) in SETS {
        for (fps, frame) in [(60, FRAME), (15, 1.0 / 15.0)] {
            for at in [26.0, 27.0, 28.0, 29.0, 30.0, 31.0, 32.0, 33.0] {
                for seed in [7u64, 11, 23, 31, 42] {
                    let pick = format!("{at},{seed},{fps}");
                    if only_set.as_deref().is_some_and(|o| o != name)
                        || only.as_deref().is_some_and(|o| o != pick)
                    {
                        continue;
                    }
                    jobs.push((name, set, fps, frame, at, seed));
                }
            }
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = jobs.len().div_ceil(threads).max(1);
    let mut lines: Vec<(String, bool)> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|&(name, set, fps, frame, at, seed)| {
                            let r = booster_fail_run(set, at, seed, REPLAN_PERIOD, frame);
                            (
                                format!(
                                    "BOARD {name:11} at {at} s {fps} fps seed {seed:2}: {:?} t {:.1} plans {} failed {} prop {:.0} t, at hand-over {}",
                                    r.outcome,
                                    r.t,
                                    r.plans,
                                    r.failed,
                                    r.prop,
                                    r.handover_tilt.zip(r.handover_rate).map_or(
                                        "—".to_string(),
                                        |(a, w)| format!(
                                            "tilt {:.1}° turning {:.1}°/s",
                                            a.to_degrees(),
                                            w.to_degrees()
                                        )
                                    )
                                ),
                                r.outcome == Some(Outcome::Caught),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    lines.sort();
    for (l, _) in &lines {
        eprintln!("{l}");
    }
    for (name, _) in SETS {
        for fps in [60, 15] {
            let cell: Vec<_> = lines
                .iter()
                .filter(|(l, _)| {
                    l.contains(&format!("{name:11} ")) && l.contains(&format!(" {fps} fps "))
                })
                .collect();
            let c = cell.iter().filter(|(_, ok)| *ok).count();
            eprintln!("BOARD {name:11} {fps} fps: caught {c} of {}", cell.len());
        }
    }
    let c = lines.iter().filter(|(_, ok)| *ok).count();
    eprintln!("BOARD caught {c} of {}", lines.len());
}

/// What one engine-out flight did.
struct FailRun {
    outcome: Option<Outcome>,
    t: f64,
    plans: usize,
    failed: usize,
    /// Propellant left, t.
    prop: f64,
    /// Tilt when it dropped to its last engines after the failure, rad.
    handover_tilt: Option<f64>,
    /// Turn rate then, rad/s.
    handover_rate: Option<f64>,
}

/// One booster flight losing engines `set` at `at` s; the solver takes
/// `latency` s; frames of `frame` s.
fn booster_fail_run(set: &[usize], at: f64, seed: u64, latency: f64, frame: f64) -> FailRun {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed,
        lag: 0.3,
        noise: Noise::LOW,
        ..ShipFlightConfig::new(Kind::Booster)
    })
    .unwrap();
    f.set_solve_time(latency);
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let (mut ok, mut bad) = (0, 0);
    let mut fired = false;
    let verbose = std::env::var_os("BOARD_TRACE").is_some();
    let mut handover_tilt = None;
    let mut handover_rate = None;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        let lit_before = f.sim().cluster.lit();
        f.step(frame);
        let final_engines = f.sim().scn.ship.final_engines;
        if fired
            && handover_tilt.is_none()
            && lit_before > final_engines
            && f.sim().cluster.lit() <= final_engines
        {
            handover_tilt = Some(model::quat::tilt(dynamics::attitude(&f.sim().x)));
            handover_rate = Some(model::quat::norm(dynamics::rates(&f.sim().x)));
        }
        if !fired && f.sim().t >= at {
            fired = true;
            let mut any = false;
            for &i in set {
                any |= f.fail_engine(i);
            }
            if any {
                f.request_replan();
            }
            if verbose {
                eprintln!(
                    "   FAIL {set:?} at {:.2}, lit {}",
                    f.sim().t,
                    f.sim().cluster.lit()
                );
            }
        }
        match deliver(&mut f, &mut pending) {
            Some(true) => {
                ok += 1;
                if verbose && let Guide::Plan { plan, t0 } = f.guide() {
                    let (x0, _) = plan.sample(0.0);
                    let (x1, _) = plan.sample(f.sim().t - t0);
                    eprintln!(
                        "   plan ok at {:.2} (from {t0:.2}) leg {:?} tf {:.1} it {} conv {} start x {:.1} vx {:.1} now-ref x {:.1} vx {:.1}",
                        f.sim().t,
                        f.leg().map(|l| (l.engines, l.gate[2] as i32)),
                        plan.final_time,
                        plan.iterations,
                        plan.converged,
                        x0[1],
                        x0[4],
                        x1[1],
                        x1[4],
                    );
                    let path: Vec<String> = (0..8)
                        .map(|i| {
                            let (x, _) = plan.sample(plan.final_time * f64::from(i) / 7.0);
                            let nose = model::quat::rotate(dynamics::attitude(&x), [0.0, 0.0, 1.0]);
                            let tilt = nose[0].asin().to_degrees();
                            format!("{:.0}/{:.1}/{:.0}°", x[1], x[4], tilt)
                        })
                        .collect();
                    eprintln!("      path x/vx/thrust-tilt: {}", path.join("  "));
                    eprintln!("      x0 {:?}", plan.x[0]);
                }
            }
            Some(false) => {
                bad += 1;
                if verbose {
                    eprintln!("   plan FAILED at {:.2}", f.sim().t);
                }
            }
            None => {}
        }
        request_after(&mut f, &mut pending, &mut last_request, latency);
        if verbose && fired {
            let x = f.sim().x;
            let (r, v) = (dynamics::position(&x), dynamics::velocity(&x));
            if (f.sim().t * 4.0).fract() < frame * 4.0 {
                // Lean toward +x, degrees: the vehicle's, and the plan's now.
                let lean = |x: &model::dynamics::State| {
                    model::quat::rotate(dynamics::attitude(x), [0.0, 0.0, 1.0])[0]
                        .asin()
                        .to_degrees()
                };
                let (ref_lean, ref_x, ref_vx) = match f.guide() {
                    Guide::Plan { plan, t0 } => {
                        let (xr, _) = plan.sample(f.sim().t - t0);
                        (lean(&xr), xr[1], xr[4])
                    }
                    _ => (f64::NAN, f64::NAN, f64::NAN),
                };
                let th = f.sim().cluster.thrust;
                eprintln!(
                    "t {:5.2} {:12} r ({:6.1},{:6.1},{:7.1}) v ({:5.1},{:5.1},{:6.1}) lit {} T {:.1} MN | lean {:5.1} plan {:5.1} | ref x {:6.1} vx {:5.1} | gimbal {:5.1}",
                    f.sim().t,
                    f.phase().label(Kind::Booster),
                    r[0],
                    r[1],
                    r[2],
                    v[0],
                    v[1],
                    v[2],
                    f.sim().cluster.lit(),
                    model::quat::norm(th) / 1e6,
                    lean(&x),
                    ref_lean,
                    ref_x,
                    ref_vx,
                    th[0].atan2(th[2]).to_degrees()
                );
            }
        }
    }
    let s = f.sim();
    FailRun {
        outcome: s.outcome,
        t: s.t,
        plans: ok,
        failed: bad,
        prop: s.propellant() / 1e3,
        handover_tilt,
        handover_rate,
    }
}

/// Something happens (a gust, a failure, a changed setting): guidance asks
/// for a new plan at once, not only when the ship has drifted.
#[test]
fn an_event_asks_for_a_plan_at_once() {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed: 7,
        lag: 0.3,
        noise: Noise::OFF,
        ..ShipFlightConfig::new(Kind::Earth)
    })
    .unwrap();
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    // Fly until the ship is on a plan with nothing to re-plan for.
    while !(matches!(f.guide(), Guide::Plan { .. }) && !f.wants_replan()) && f.sim().t < 200.0 {
        f.step(FRAME);
        deliver(&mut f, &mut pending);
        request(&mut f, &mut pending, &mut last_request);
    }
    assert!(!f.wants_replan(), "never settled on a plan");
    f.request_replan();
    assert!(f.wants_replan());
    // Asking for the plan clears it.
    let _ = f.problem();
    assert!(!f.wants_replan());
}

/// From orbit: the guided entry, the hand-over, the belly flop, the
/// landing burn and the catch (low noise, seed 7).
#[test]
fn earth_from_orbit_is_caught() {
    use kestrel_wasm::ship_flight::EntryConfig;
    let t0 = std::time::Instant::now();
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed: 7,
        lag: 0.3,
        noise: Noise::LOW,
        entry: Some(EntryConfig::default()),
        ..ShipFlightConfig::new(Kind::Earth)
    })
    .unwrap();
    assert_eq!(f.phase(), Phase::Entry);
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let mut handed = None;
    while f.phase() != Phase::Done && f.sim().t < 2_000.0 {
        f.step(if f.phase() == Phase::Entry {
            0.5
        } else {
            FRAME
        });
        if handed.is_none() && f.phase() != Phase::Entry {
            handed = Some((
                f.sim().t,
                dynamics::position(&f.sim().x),
                dynamics::velocity(&f.sim().x),
            ));
        }
        deliver(&mut f, &mut pending);
        request(&mut f, &mut pending, &mut last_request);
    }
    let s = f.entry_summary().expect("entry summary");
    eprintln!(
        "orbit: hand-over {handed:?}; entry {:.0} s, miss {:.0} m, peak tile {:.0} K, peak {:.2} MW/m2, {:.1} g, {} reversals; end {:?} at {:.0} s ({:.1} s wall)",
        s.duration,
        s.miss,
        s.peak_tile,
        s.peak_heat_flux / 1e6,
        s.peak_g,
        s.reversals,
        f.sim().outcome,
        f.sim().t,
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(f.sim().outcome, Some(Outcome::Caught));
    assert!(s.peak_tile < model::entry::TILE_LIMIT);
}

/// From orbit over entry angles and distances (`--ignored --nocapture`).
#[test]
#[ignore]
fn orbit_sweep() {
    use kestrel_wasm::ship_flight::EntryConfig;
    for range_km in [3_000.0, 3_150.0, 3_300.0, 3_450.0, 3_600.0] {
        for gamma_deg in [-0.8f64, -1.0, -1.2, -1.5] {
            let mut f = ShipFlight::new(ShipFlightConfig {
                seed: 7,
                lag: 0.3,
                noise: Noise::LOW,
                entry: Some(EntryConfig {
                    range: range_km * 1e3,
                    gamma: gamma_deg.to_radians(),
                    ..EntryConfig::default()
                }),
                ..ShipFlightConfig::new(Kind::Earth)
            })
            .unwrap();
            let mut pending: Option<Pending> = None;
            let mut last_request = f64::NEG_INFINITY;
            let mut handed = None;
            while f.phase() != Phase::Done && f.sim().t < 2_000.0 {
                f.step(if f.phase() == Phase::Entry {
                    0.5
                } else {
                    FRAME
                });
                if handed.is_none() && f.phase() != Phase::Entry {
                    handed = Some(dynamics::position(&f.sim().x)[0]);
                }
                deliver(&mut f, &mut pending);
                request(&mut f, &mut pending, &mut last_request);
            }
            let s = f.entry_summary();
            eprintln!(
                "ORBIT {range_km} km {gamma_deg} deg: hand-over x {:?} m, peak tile {:?} K, {:?}",
                handed.map(|x| x.round()),
                s.map(|s| s.peak_tile.round()),
                f.sim().outcome
            );
        }
    }
}

/// One flight of the wind sweep.
struct WindRun {
    outcome: Option<Outcome>,
    t: f64,
    /// Propellant left, t.
    left: f64,
    /// Sideways distance from the approach gate and sideways speed when
    /// the terminal phase began, m and m/s (None: it never began).
    gate: Option<(f64, f64)>,
    /// Error of the wind estimate when the engines lit, m/s.
    est_err: Option<f64>,
}

fn wind_run(kind: Kind, wind: [f64; 2], noise: Noise, seed: u64) -> WindRun {
    wind_run_with(kind, wind, noise, seed, true)
}

fn wind_run_with(kind: Kind, wind: [f64; 2], noise: Noise, seed: u64, aware: bool) -> WindRun {
    wind_run_cfg(kind, wind, noise, seed, aware, false, true, true)
}

#[allow(clippy::too_many_arguments)]
fn wind_run_cfg(
    kind: Kind,
    wind: [f64; 2],
    noise: Noise,
    seed: u64,
    aware: bool,
    planned_approach: bool,
    glide_terminal: bool,
    chosen_bearing: bool,
) -> WindRun {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed,
        lag: 0.3,
        noise,
        wind,
        wind_aware: aware,
        planned_approach,
        glide_terminal,
        chosen_bearing,
        ..ShipFlightConfig::new(kind)
    })
    .unwrap();
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let mut gate = None;
    let mut est_err = None;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        if est_err.is_none() && f.phase() != Phase::BellyFlop {
            let e = f.wind_estimate().w;
            est_err = Some((e[0] - wind[0]).hypot(e[1] - wind[1]));
        }
        if gate.is_none() && f.phase() == Phase::Terminal {
            let x = f.sim().x;
            let (r, v) = (dynamics::position(&x), dynamics::velocity(&x));
            let g = f.sim().scn.gate;
            gate = Some(((r[0] - g[0]).hypot(r[1] - g[1]), v[0].hypot(v[1])));
        }
        deliver(&mut f, &mut pending);
        request(&mut f, &mut pending, &mut last_request);
    }
    WindRun {
        outcome: f.sim().outcome,
        t: f.sim().t,
        left: f.sim().propellant() / 1e3,
        gate,
        est_err,
    }
}

/// Steady wind against both vehicles: 0-25 m/s from +x, -x and +y, noise
/// low and high, seeds 7-9. Prints one line per flight and the catches per
/// wind speed. Slow (216 flights, threaded): `--ignored --nocapture`.
#[test]
#[ignore]
fn wind_sweep() {
    const SPEEDS: [f64; 6] = [0.0, 5.0, 10.0, 15.0, 20.0, 25.0];
    // Where the wind blows from: +x (from the sea side the vehicles come
    // in from), -x (from behind the tower), +y (across).
    const FROM: [(&str, [f64; 2]); 3] =
        [("+x", [1.0, 0.0]), ("-x", [-1.0, 0.0]), ("+y", [0.0, 1.0])];
    let cells: Vec<(Kind, &str, Noise)> = [Kind::Booster, Kind::Earth]
        .into_iter()
        .flat_map(|k| [(k, "low", Noise::LOW), (k, "high", Noise::HIGH)])
        .collect();
    let mut lines: Vec<(String, Kind, f64, bool, Option<f64>)> = std::thread::scope(|s| {
        let handles: Vec<_> = cells
            .iter()
            .map(|&(kind, noise_name, noise)| {
                s.spawn(move || {
                    let mut out = Vec::new();
                    for speed in SPEEDS {
                        for (from, d) in FROM {
                            for seed in [7u64, 8, 9] {
                                let wind = [-speed * d[0], -speed * d[1]];
                                let r = wind_run(kind, wind, noise, seed);
                                let caught = r.outcome == Some(Outcome::Caught);
                                let gate = r.gate.map_or("     —      ".to_string(), |(e, v)| {
                                    format!("{e:5.1} m {v:4.1} m/s")
                                });
                                let est = r.est_err.map_or("—".to_string(), |e| format!("{e:.1}"));
                                out.push((
                                    format!(
                                        "{kind:?} {speed:4.0} m/s from {from} noise {noise_name:4} seed {seed}: {:?} t {:5.1} left {:5.1} t gate {gate} est err {est} m/s",
                                        r.outcome, r.t, r.left
                                    ),
                                    kind,
                                    speed,
                                    caught,
                                    r.gate.map(|g| g.0),
                                ));
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    for l in &lines {
        eprintln!("{}", l.0);
    }
    eprintln!("\nkind     wind  caught  mean gate error (m)");
    for kind in [Kind::Booster, Kind::Earth] {
        for speed in SPEEDS {
            let cell: Vec<_> = lines
                .iter()
                .filter(|l| l.1 == kind && l.2 == speed)
                .collect();
            let caught = cell.iter().filter(|l| l.3).count();
            let errs: Vec<f64> = cell.iter().filter_map(|l| l.4).collect();
            let mean = errs.iter().sum::<f64>() / errs.len().max(1) as f64;
            eprintln!(
                "{kind:?}  {speed:4.0}   {caught:2}/{}   {mean:5.1}",
                cell.len()
            );
        }
    }
}

/// With every M4 switch off (wind awareness, the planned approach, the
/// glide, the chosen bearing), guidance flies exactly the flight it flew
/// before M4 (numbers read from that code, 2026-11-18). That code flew on
/// Clarabel, so this test does too (the reference solver, tests only).
#[test]
fn unaware_flight_matches_today() {
    use guidance::scvx::{Backend, set_backend};
    set_backend(Backend::Clarabel);
    for (kind, wind, t, left) in [
        (
            Kind::Earth,
            [0.0, 0.0],
            82.64333333333248,
            19.43316637947771,
        ),
        (
            Kind::Booster,
            [0.0, -15.0],
            73.74999999999964,
            25.712377419400028,
        ),
    ] {
        let r = wind_run_cfg(kind, wind, Noise::LOW, 7, false, false, false, false);
        assert_eq!(r.outcome, Some(Outcome::Caught), "{kind:?}");
        assert_eq!(r.t, t, "{kind:?}");
        assert!((r.left - left).abs() < 1e-9, "{kind:?}: {} t left", r.left);
    }
}

/// Falling in a 15 m/s crosswind with low noise, the estimate is within
/// 2 m/s of the wind by the time the engines light.
#[test]
fn estimate_settles_in_flight() {
    for kind in [Kind::Booster, Kind::Earth] {
        let mut f = ShipFlight::new(ShipFlightConfig {
            seed: 7,
            noise: Noise::LOW,
            wind: [0.0, -15.0],
            ..ShipFlightConfig::new(kind)
        })
        .unwrap();
        while f.phase() == Phase::BellyFlop && f.sim().t < 300.0 {
            f.step(FRAME);
        }
        let e = f.wind_estimate();
        let err = e.w[0].hypot(e.w[1] + 15.0);
        eprintln!(
            "{kind:?} at ignition (t {:.1}): {:?} ± {:?}",
            f.sim().t,
            e.w,
            e.sigma
        );
        assert!(err < 2.0, "{kind:?}: error {err}");
    }
}

/// Dragging the vehicle on the page resets navigation, not the wind.
#[test]
fn estimate_survives_grab() {
    let mut f = ShipFlight::new(ShipFlightConfig {
        seed: 7,
        noise: Noise::LOW,
        wind: [10.0, 0.0],
        ..ShipFlightConfig::new(Kind::Booster)
    })
    .unwrap();
    for _ in 0..600 {
        f.step(FRAME);
    }
    let before = f.wind_estimate();
    f.grab([500.0, 0.0, 9000.0]);
    f.release();
    assert_eq!(f.wind_estimate(), before);
}

/// A crosswind the booster was lost in before guidance knew the wind (wind
/// sweep baseline, 2026-11-18, low noise, seed 7: 20 m/s from +y, out of
/// propellant). The ship's crosswind losses are not here: its belly flop
/// can only slide along its nose, so a crosswind needs a heading choice
/// (M4 sub-project 3).
#[test]
fn booster_crosswind_is_caught() {
    let r = wind_run(Kind::Booster, [0.0, -20.0], Noise::LOW, 7);
    eprintln!(
        "booster, 20 m/s from +y: {:?} t {:.1} left {:.1} t",
        r.outcome, r.t, r.left
    );
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

/// The robustness boards flown twice on the same build, wind awareness off
/// (exactly the guidance before M4.1) and on, in still air: nominal ship
/// flights (seeds 7-10, noise off/low/high), ship and booster seeds 11-30
/// (noise low/high), and the ship's engine-out window (running engine
/// failed at 49-67 s, noise off/low). Slow (224 flights, threaded):
/// `--ignored --nocapture`.
#[test]
#[ignore]
fn wind_aware_board() {
    #[derive(Clone, Copy)]
    enum Job {
        Seed(Kind, Noise, &'static str, u64),
        EngineOut(Noise, &'static str, f64),
    }
    let mut jobs = Vec::new();
    for aware in [false, true] {
        for seed in 7..=10 {
            for (name, noise) in [
                ("off", Noise::OFF),
                ("low", Noise::LOW),
                ("high", Noise::HIGH),
            ] {
                jobs.push((
                    "ship nominal 7-10",
                    aware,
                    Job::Seed(Kind::Earth, noise, name, seed),
                ));
            }
        }
        for seed in 11..=30 {
            for (name, noise) in [("low", Noise::LOW), ("high", Noise::HIGH)] {
                jobs.push((
                    "ship seeds 11-30",
                    aware,
                    Job::Seed(Kind::Earth, noise, name, seed),
                ));
                jobs.push((
                    "booster seeds 11-30",
                    aware,
                    Job::Seed(Kind::Booster, noise, name, seed),
                ));
            }
        }
        for at in [49.0, 51.0, 53.0, 55.0, 57.0, 59.0, 61.0, 63.0, 65.0, 67.0] {
            for (name, noise) in [("off", Noise::OFF), ("low", Noise::LOW)] {
                jobs.push((
                    "ship engine-out window",
                    aware,
                    Job::EngineOut(noise, name, at),
                ));
            }
        }
    }
    let queue = std::sync::Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..6 {
            sc.spawn(|| {
                loop {
                    let Some((i, (group, aware, job))) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let (caught, line) = match job {
                        Job::Seed(kind, noise, name, seed) => {
                            let r = wind_run_with(kind, [0.0, 0.0], noise, seed, aware);
                            (
                                r.outcome == Some(Outcome::Caught),
                                format!(
                                    "{kind:?} seed {seed} noise {name}: {:?} left {:.1} t",
                                    r.outcome, r.left
                                ),
                            )
                        }
                        Job::EngineOut(noise, name, at) => {
                            let r = fly_with(
                                Kind::Earth,
                                noise,
                                0.3,
                                &[(at, Event::EngineOut)],
                                &[],
                                aware,
                            );
                            (
                                r.outcome == Some(Outcome::Caught),
                                format!("engine out at {at} s noise {name}: {:?}", r.outcome),
                            )
                        }
                    };
                    results
                        .lock()
                        .unwrap()
                        .push((i, group, aware, caught, line));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    for (_, group, aware, caught, line) in &results {
        if !caught {
            eprintln!("LOST  aware {aware}: {group}: {line}");
        }
    }
    eprintln!(
        "
board                     aware off   aware on"
    );
    for group in [
        "ship nominal 7-10",
        "ship seeds 11-30",
        "booster seeds 11-30",
        "ship engine-out window",
    ] {
        let count = |aware: bool| {
            let cell: Vec<_> = results
                .iter()
                .filter(|r| r.1 == group && r.2 == aware)
                .collect();
            format!("{:2}/{:2}", cell.iter().filter(|r| r.3).count(), cell.len())
        };
        eprintln!("{group:24}  {}       {}", count(false), count(true));
    }
}

/// How far the navigation estimate's position is from the truth in the
/// final approach (RMS over the terminal phase), per vehicle and noise
/// level, seeds 7-9: the approach plan's keep-out margin is built on it.
/// `--ignored --nocapture`.
#[test]
#[ignore]
fn nav_error_by_noise() {
    for kind in [Kind::Earth, Kind::Booster] {
        for (name, noise) in [("low", Noise::LOW), ("high", Noise::HIGH)] {
            let (mut sum, mut n, mut worst) = (0.0, 0usize, 0.0f64);
            for seed in [7u64, 8, 9] {
                let mut f = ShipFlight::new(ShipFlightConfig {
                    seed,
                    lag: 0.3,
                    noise,
                    ..ShipFlightConfig::new(kind)
                })
                .unwrap();
                let mut pending: Option<Pending> = None;
                let mut last_request = f64::NEG_INFINITY;
                while f.phase() != Phase::Done && f.sim().t < 400.0 {
                    f.step(FRAME);
                    deliver(&mut f, &mut pending);
                    request(&mut f, &mut pending, &mut last_request);
                    if f.phase() == Phase::Terminal {
                        let (e, x) = (
                            dynamics::position(f.estimate()),
                            dynamics::position(&f.sim().x),
                        );
                        let d = (e[0] - x[0]).hypot(e[1] - x[1]);
                        sum += d * d;
                        n += 1;
                        worst = worst.max(d);
                    }
                }
            }
            eprintln!(
                "NAV {kind:?} noise {name}: horizontal error RMS {:.2} m, worst {worst:.2} m ({n} steps)",
                (sum / n.max(1) as f64).sqrt()
            );
        }
    }
}

/// The terminal controller's glide (M4.2) against its stop-and-wait, same
/// build, wind awareness on: the boards of `wind_aware_board` plus, per
/// flight, the time from the gate to the catch and the speeds at contact.
/// (It measured the planned approach the same way: see D-047.)
/// `--ignored --nocapture`.
#[test]
#[ignore]
fn approach_board() {
    #[derive(Clone, Copy)]
    enum Job {
        Seed(Kind, Noise, &'static str, u64),
        EngineOut(Noise, &'static str, f64),
    }
    struct Out {
        caught: bool,
        approach_s: Option<f64>,
        contact: Option<(f64, f64)>,
        line: String,
    }
    let run = |job: Job, planned: bool| -> Out {
        let (kind, noise, name, seed, fail) = match job {
            Job::Seed(k, n, name, s) => (k, n, name, s, None),
            Job::EngineOut(n, name, at) => (Kind::Earth, n, name, 7, Some(at)),
        };
        let mut f = ShipFlight::new(ShipFlightConfig {
            seed,
            lag: 0.3,
            noise,
            glide_terminal: planned,
            ..ShipFlightConfig::new(kind)
        })
        .unwrap();
        let mut pending: Option<Pending> = None;
        let mut last_request = f64::NEG_INFINITY;
        let mut gate_t = None;
        let mut failed = false;
        while f.phase() != Phase::Done && f.sim().t < 400.0 {
            f.step(FRAME);
            if let Some(at) = fail
                && !failed
                && f.sim().t >= at
            {
                failed = true;
                f.engine_out();
            }
            if gate_t.is_none() && f.phase() == Phase::Terminal {
                gate_t = Some(f.sim().t);
            }
            deliver(&mut f, &mut pending);
            request(&mut f, &mut pending, &mut last_request);
        }
        let s = f.sim();
        let caught = s.outcome == Some(Outcome::Caught);
        let contact = s.end_velocity.map(|v| (-v[2], v[0].hypot(v[1])));
        Out {
            caught,
            approach_s: gate_t.filter(|_| caught).map(|g| s.t - g),
            contact: contact.filter(|_| caught),
            line: format!(
                "{kind:?} seed {seed} noise {name} fail {fail:?}: {:?} at {:.1} s, approach {}",
                s.outcome,
                s.t,
                f.approach_mode()
            ),
        }
    };
    let mut jobs = Vec::new();
    for planned in [false, true] {
        for seed in 7..=10 {
            for (name, noise) in [
                ("off", Noise::OFF),
                ("low", Noise::LOW),
                ("high", Noise::HIGH),
            ] {
                jobs.push((
                    "ship nominal 7-10",
                    planned,
                    Job::Seed(Kind::Earth, noise, name, seed),
                ));
            }
        }
        for seed in 11..=30 {
            for (name, noise) in [("low", Noise::LOW), ("high", Noise::HIGH)] {
                jobs.push((
                    "ship seeds 11-30",
                    planned,
                    Job::Seed(Kind::Earth, noise, name, seed),
                ));
                jobs.push((
                    "booster seeds 11-30",
                    planned,
                    Job::Seed(Kind::Booster, noise, name, seed),
                ));
            }
        }
        for at in [49.0, 51.0, 53.0, 55.0, 57.0, 59.0, 61.0, 63.0, 65.0, 67.0] {
            for (name, noise) in [("off", Noise::OFF), ("low", Noise::LOW)] {
                jobs.push((
                    "ship engine-out window",
                    planned,
                    Job::EngineOut(noise, name, at),
                ));
            }
        }
    }
    let queue = std::sync::Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..6 {
            sc.spawn(|| {
                loop {
                    let Some((i, (group, planned, job))) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let o = run(job, planned);
                    results.lock().unwrap().push((i, group, planned, o));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    for (_, group, planned, o) in &results {
        if !o.caught {
            eprintln!("LOST planned {planned}: {group}: {}", o.line);
        }
    }
    eprintln!("\nboard                     stop-and-wait                    glide");
    for group in [
        "ship nominal 7-10",
        "ship seeds 11-30",
        "booster seeds 11-30",
        "ship engine-out window",
    ] {
        let stats = |planned: bool| {
            let cell: Vec<_> = results
                .iter()
                .filter(|r| r.1 == group && r.2 == planned)
                .collect();
            let caught = cell.iter().filter(|r| r.3.caught).count();
            let ts: Vec<f64> = cell.iter().filter_map(|r| r.3.approach_s).collect();
            let cs: Vec<(f64, f64)> = cell.iter().filter_map(|r| r.3.contact).collect();
            let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
            format!(
                "{caught:2}/{:2} gate->catch {:4.1} s down {:.2} side {:.2} m/s",
                cell.len(),
                mean(&ts),
                mean(&cs.iter().map(|c| c.0).collect::<Vec<_>>()),
                mean(&cs.iter().map(|c| c.1).collect::<Vec<_>>())
            )
        };
        eprintln!("{group:24}  {}   {}", stats(false), stats(true));
    }
}

/// Still air, the default start: the approach stays on the +x side, exactly
/// (the old geometry). In a 15 m/s crosswind from +y it turns toward where
/// the wind carries the fall (M4.3).
#[test]
fn the_approach_bearing_follows_the_fall() {
    let bearing = |wind: [f64; 2]| {
        let mut f = ShipFlight::new(ShipFlightConfig {
            seed: 7,
            noise: Noise::LOW,
            wind,
            ..ShipFlightConfig::new(Kind::Earth)
        })
        .unwrap();
        while f.phase() == Phase::BellyFlop && f.sim().t < 300.0 && !f.bearing_chosen() {
            f.step(FRAME);
        }
        f.bearing()
    };
    assert_eq!(bearing([0.0, 0.0]), 0.0);
    let turned = bearing([0.0, -15.0]).to_degrees();
    eprintln!("bearing in a 15 m/s crosswind from +y: {turned:.1} deg");
    assert!(turned < -10.0, "{turned}");
}

/// The ship in a 15 m/s crosswind from +y: lost in every version before
/// M4.3 (it drifted ~500 m across its approach in the belly flop).
#[test]
fn earth_crosswind_is_caught() {
    let r = wind_run(Kind::Earth, [0.0, -15.0], Noise::LOW, 7);
    eprintln!(
        "ship, 15 m/s from +y: {:?} t {:.1} left {:.1} t",
        r.outcome, r.t, r.left
    );
    assert_eq!(r.outcome, Some(Outcome::Caught));
}
