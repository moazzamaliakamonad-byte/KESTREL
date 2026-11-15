//! Closed-loop ship flights, run natively: the loop the page runs, with the
//! worker's solve latency modelled as one re-plan period. Seeded.
//! Simulation only.

use kestrel_wasm::ship_flight::{plan, Guide, Kind, Leg, Phase, ShipFlight, ShipFlightConfig, REPLAN_PERIOD};
use model::dynamics;
use sim::ship::Outcome;
use sim::Noise;

const FRAME: f64 = 1.0 / 60.0;

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
}

type Pending = (f64, f64, Vec<Leg>, Result<(usize, guidance::scvx::ShipPlan), guidance::GuidanceError>);

/// Ask for a re-plan if wanted; solve now, deliver after `REPLAN_PERIOD`.
fn request(f: &mut ShipFlight, pending: &mut Option<Pending>, last_request: &mut f64) {
    let t = f.sim().t;
    if pending.is_none() && f.wants_replan() && t - *last_request >= REPLAN_PERIOD {
        *last_request = t;
        let (t0, options) = f.problem();
        let r = plan(&options);
        let legs = options.iter().map(|(l, _)| *l).collect();
        *pending = Some((t0 + REPLAN_PERIOD, t0, legs, r));
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
fn fly(kind: Kind, noise: Noise, lag: f64, events: &[(f64, Event)], ignition_events: &[Event]) -> Report {
    let mut f = ShipFlight::new(ShipFlightConfig { seed: 7, lag, noise, ..ShipFlightConfig::new(kind) }).unwrap();
    let m0 = f.sim().x[dynamics::M];
    let mut pending: Option<Pending> = None;
    let mut last_request = f64::NEG_INFINITY;
    let mut fired = vec![false; events.len()];
    let mut ignition_fired = false;
    let (mut replans, mut failures) = (0, 0);
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        let t = f.sim().t;
        for (i, (at, e)) in events.iter().enumerate() {
            if !fired[i] && t >= *at {
                fired[i] = true;
                match e {
                    Event::Gust(d) => f.gust(*d),
                    Event::EngineOut => {
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
fn mars_nominal_lands() {
    let r = fly(Kind::Mars, Noise::LOW, 0.3, &[], &[]);
    assert_eq!(r.outcome, Some(Outcome::Landed));
}

#[test]
fn earth_engine_out_at_ignition_is_caught() {
    let r = fly(Kind::Earth, Noise::LOW, 0.3, &[], &[Event::EngineOut]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
}

#[test]
fn earth_gust_in_belly_flop_is_caught() {
    let r = fly(Kind::Earth, Noise::LOW, 0.3, &[(20.0, Event::Gust(1.0)), (35.0, Event::Gust(-1.0))], &[]);
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
    let r = fly(Kind::Booster, Noise::LOW, 0.3, &[(8.0, Event::Gust(1.0)), (20.0, Event::Gust(-1.0))], &[]);
    assert_eq!(r.outcome, Some(Outcome::Caught));
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

#[test]
#[ignore]
fn trace_mars() {
    trace(Kind::Mars);
}

fn trace(kind: Kind) {
    use model::quat;
    let noise = match std::env::var("TRACE_NOISE").as_deref() {
        Ok("low") => Noise::LOW,
        Ok("high") => Noise::HIGH,
        _ => Noise::OFF,
    };
    let mut f = ShipFlight::new(ShipFlightConfig { seed: 7, lag: 0.3, noise, ..ShipFlightConfig::new(kind) }).unwrap();
    let mut next_print = 0.0;
    let mut last_request = f64::NEG_INFINITY;
    let mut pending: Option<Pending> = None;
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(FRAME);
        let t = f.sim().t;
        if let Some(ok) = deliver(&mut f, &mut pending) {
            match (ok, f.guide(), f.leg()) {
                (true, Guide::Plan { plan, t0 }, Some(leg)) => eprintln!(
                    "   plan @{t0:.2}: {} eng to z {:.0} tf {:.1} it {} conv {} gate_err {:.1}",
                    leg.engines, leg.gate[2], plan.final_time, plan.iterations, plan.converged, plan.gate_error
                ),
                _ => eprintln!("   plan FAILED at {t:.2}"),
            }
        }
        request(&mut f, &mut pending, &mut last_request);
        if t >= next_print || f.phase() == Phase::Done {
            next_print += if f.phase() == Phase::BellyFlop { 5.0 } else { 0.5 };
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
                f.phase().label(kind), r[0], r[1], r[2], v[0], v[1], v[2], nose[0], nose[2],
                dynamics::rates(&x).map(|c| (c.to_degrees() * 10.0).round() / 10.0),
                quat::norm(s.cluster.thrust) / 1e6, s.cluster.lit(), s.propellant() / 1e3, pr[0], pr[2]
            );
        }
    }
    eprintln!("outcome {:?}", f.sim().outcome);
}
