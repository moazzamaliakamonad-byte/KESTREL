//! The closed loop as JavaScript sees it: a `Flight` class on the main
//! thread and a `replan` function for the worker. JSON in and out; nothing
//! here panics or throws (D-009).
//!
//! Per re-plan: `flight.measure_request()` → worker `replan(json)` →
//! `flight.apply_replan(json, t)`. The request already carries everything
//! guidance needs (terminal gate, propellant reserve, failed engines), so
//! the worker holds no flight state.

use guidance::{GuidanceError, Hint, LandingProblem, Mode, Replan};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sim::Noise;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::dto::{TrajectoryDto, VehicleDto};
use crate::flight::{Flight, FlightConfig, Guide, plan};
use crate::{MAX_STEPS, error_kind, failure};

#[derive(Debug, Deserialize)]
struct ConfigDto {
    position: [f64; 3],
    velocity: [f64; 3],
    vehicle: VehicleDto,
    #[serde(default)]
    seed: u64,
    #[serde(default)]
    lag: f64,
    #[serde(default)]
    noise: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ModeDto {
    Pinpoint,
    Closest,
}

#[derive(Debug, Serialize, Deserialize)]
struct HintDto {
    final_time: f64,
    mode: ModeDto,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProblemDto {
    position: [f64; 3],
    velocity: [f64; 3],
    /// `wet_mass` is the measured mass.
    vehicle: VehicleDto,
    steps: usize,
}

/// What the page posts to the worker.
#[derive(Debug, Serialize, Deserialize)]
struct ReplanRequestDto {
    problem: ProblemDto,
    hint: Option<HintDto>,
}

fn noise_level(name: &str) -> Option<Noise> {
    match name {
        "off" => Some(Noise::OFF),
        "low" => Some(Noise::LOW),
        "high" => Some(Noise::HIGH),
        _ => None,
    }
}

fn mode_dto(m: Mode) -> ModeDto {
    match m {
        Mode::Pinpoint => ModeDto::Pinpoint,
        Mode::Closest => ModeDto::Closest,
    }
}

fn mode_from(m: ModeDto) -> Mode {
    match m {
        ModeDto::Pinpoint => Mode::Pinpoint,
        ModeDto::Closest => Mode::Closest,
    }
}

/// Solve a re-plan request from [`FlightJs::measure_request`].
#[wasm_bindgen]
pub fn replan(request_json: &str) -> String {
    let req = match serde_json::from_str::<ReplanRequestDto>(request_json) {
        Ok(r) => r,
        Err(e) => {
            return failure("BadRequest", format!("could not read request: {e}")).to_string();
        }
    };
    if req.problem.steps > MAX_STEPS {
        return failure("BadRequest", format!("steps must be at most {MAX_STEPS}")).to_string();
    }
    let problem = LandingProblem {
        vehicle: req.problem.vehicle.into(),
        position: req.problem.position,
        velocity: req.problem.velocity,
        steps: req.problem.steps,
    };
    let hint = req.hint.map(|h| Hint {
        final_time: h.final_time,
        mode: mode_from(h.mode),
    });
    match plan(&problem, hint) {
        Ok(r) => json!({
            "ok": true,
            "mode": mode_dto(r.mode),
            "miss": r.miss,
            "solves": r.solves,
            "trajectory": TrajectoryDto::from(r.trajectory),
        }),
        Err(e) => failure(error_kind(&e), format!("{e:?}")),
    }
    .to_string()
}

/// One flight, owned by the page.
#[wasm_bindgen(js_name = Flight)]
pub struct FlightJs {
    inner: Flight,
}

#[wasm_bindgen(js_class = Flight)]
impl FlightJs {
    /// A flight from `{position, velocity, vehicle, seed?, lag?, noise?}`;
    /// `undefined` if the configuration is not valid.
    pub fn create(config_json: &str) -> Option<FlightJs> {
        let c: ConfigDto = serde_json::from_str(config_json).ok()?;
        let noise = match c.noise.as_deref() {
            None => Noise::OFF,
            Some(name) => noise_level(name)?,
        };
        let inner = Flight::new(FlightConfig {
            position: c.position,
            velocity: c.velocity,
            vehicle: c.vehicle.into(),
            seed: c.seed,
            lag: c.lag,
            noise,
        })
        .ok()?;
        Some(FlightJs { inner })
    }

    pub fn step(&mut self, dt: f64) {
        self.inner.step(dt);
    }

    pub fn wants_replan(&self) -> bool {
        self.inner.wants_replan()
    }

    /// `{t, request}`: a sensor reading turned into a re-plan request.
    pub fn measure_request(&mut self) -> String {
        let m = self.inner.measure();
        let p = self.inner.problem(&m);
        let request = ReplanRequestDto {
            problem: ProblemDto {
                position: p.position,
                velocity: p.velocity,
                vehicle: VehicleDto::from(&p.vehicle),
                steps: p.steps,
            },
            hint: self.inner.hint(m.t).map(|h| HintDto {
                final_time: h.final_time,
                mode: mode_dto(h.mode),
            }),
        };
        json!({ "t": m.t, "request": request }).to_string()
    }

    /// Install the worker's answer to a request measured at sim time `t`.
    /// Unreadable answers are ignored.
    pub fn apply_replan(&mut self, response_json: &str, t: f64) {
        let Ok(v) = serde_json::from_str::<Value>(response_json) else {
            return;
        };
        if v["ok"] == true {
            let trajectory = serde_json::from_value::<TrajectoryDto>(v["trajectory"].clone());
            let mode = serde_json::from_value::<ModeDto>(v["mode"].clone());
            if let (Ok(trajectory), Ok(mode)) = (trajectory, mode) {
                let r = Replan {
                    trajectory: trajectory.into(),
                    mode: mode_from(mode),
                    miss: v["miss"].as_f64().unwrap_or(0.0),
                    solves: v["solves"].as_u64().unwrap_or(0) as usize,
                };
                self.inner.set_plan(r, t);
            }
        } else if v["ok"] == false {
            let e = match v["error"]["kind"].as_str() {
                Some("Infeasible") => GuidanceError::Infeasible,
                _ => GuidanceError::SolverFailed(v["error"]["message"].to_string()),
            };
            self.inner.plan_failed(&e);
        }
    }

    pub fn gust(&mut self, dir: f64) {
        self.inner.gust(dir);
    }

    pub fn engine_out(&mut self) -> bool {
        self.inner.engine_out()
    }

    pub fn set_lag(&mut self, seconds: f64) {
        if seconds.is_finite() {
            self.inner.set_lag(seconds);
        }
    }

    /// `"off" | "low" | "high"`; false for anything else.
    pub fn set_noise(&mut self, level: &str) -> bool {
        match noise_level(level) {
            Some(n) => {
                self.inner.set_noise(n);
                true
            }
            None => false,
        }
    }

    pub fn grab(&mut self, x: f64, y: f64, z: f64) {
        self.inner.grab([x, y, z]);
    }

    pub fn release(&mut self) {
        self.inner.release();
    }

    /// The vehicle now, for the scene and the panels.
    pub fn snapshot(&self) -> String {
        let f = &self.inner;
        let s = f.sim();
        let (min_thrust, max_thrust) = s.engine.limits();
        let (mode, miss) = match f.guide() {
            None => ("none", Value::Null),
            Some(Guide::Brake) => ("brake", Value::Null),
            Some(Guide::Plan {
                mode: Mode::Pinpoint,
                miss,
                ..
            }) => ("pinpoint", json!(miss)),
            Some(Guide::Plan {
                mode: Mode::Closest,
                miss,
                ..
            }) => ("closest", json!(miss)),
        };
        let outcome = f.outcome().map(|o| {
            json!({
                "landed": o.landed,
                "speed": o.speed,
                "vertical": o.vertical,
                "horizontal": o.horizontal,
                "miss": o.miss,
                "fuel_used": o.fuel_used,
            })
        });
        json!({
            "t": s.truth.t,
            "position": s.truth.position,
            "velocity": s.truth.velocity,
            "mass": s.truth.mass,
            "thrust": s.engine.thrust,
            "command": f.command(),
            "nav_position": f.nav().position,
            "engines": sim::engine::ENGINES - s.engine.failed,
            "min_thrust": min_thrust,
            "max_thrust": max_thrust,
            "lag": s.engine.lag,
            "landed": s.truth.landed,
            "mode": mode,
            "miss": miss,
            "plan_remaining": f.plan_remaining(),
            "outcome": outcome,
        })
        .to_string()
    }

    /// The plan being flown (positions relative to the real pad), or `null`.
    pub fn plan_json(&self) -> String {
        match self.inner.guide() {
            Some(Guide::Plan { trajectory, .. }) => {
                serde_json::to_string(&TrajectoryDto::from(trajectory.clone()))
                    .unwrap_or_else(|_| "null".into())
            }
            _ => "null".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> String {
        let p = LandingProblem::example();
        json!({
            "position": p.position,
            "velocity": p.velocity,
            "vehicle": VehicleDto::from(&p.vehicle),
            "seed": 7,
            "lag": 0.3,
            "noise": "low",
        })
        .to_string()
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("valid JSON")
    }

    #[test]
    fn bad_config_gives_none() {
        assert!(FlightJs::create("not json").is_none());
        let mut c = parse(&config());
        c["position"][2] = json!(-5.0);
        assert!(FlightJs::create(&c.to_string()).is_none());
        let mut c = parse(&config());
        c["noise"] = json!("extreme");
        assert!(FlightJs::create(&c.to_string()).is_none());
    }

    #[test]
    fn one_replan_round_trip() {
        let mut f = FlightJs::create(&config()).unwrap();
        let snap = parse(&f.snapshot());
        assert_eq!(snap["mode"], "none");
        assert_eq!(snap["engines"], 6);
        assert!(f.wants_replan());

        f.step(0.5);
        let req = parse(&f.measure_request());
        let t = req["t"].as_f64().unwrap();
        assert!((t - 0.5).abs() < 1e-9);
        let resp = parse(&replan(&req["request"].to_string()));
        assert_eq!(resp["ok"], true, "{resp}");
        assert_eq!(resp["mode"], "pinpoint");
        assert!(resp["solves"].as_u64().unwrap() > 3, "no hint: full search");

        f.apply_replan(&resp.to_string(), t);
        let snap = parse(&f.snapshot());
        assert_eq!(snap["mode"], "pinpoint");
        assert!(snap["plan_remaining"].as_f64().unwrap() > 30.0);
        let plan = parse(&f.plan_json());
        let z = plan["position"].as_array().unwrap().last().unwrap()[2]
            .as_f64()
            .unwrap();
        assert!(
            (z - crate::flight::TERMINAL_GATE).abs() < 1e-6,
            "plan drawn above the pad"
        );

        // The next request carries a warm-start hint.
        f.step(0.25);
        let req = parse(&f.measure_request());
        assert_eq!(req["request"]["hint"]["mode"], "pinpoint");
    }

    #[test]
    fn failures_reach_the_flight() {
        let mut f = FlightJs::create(&config()).unwrap();
        f.apply_replan(
            r#"{"ok":false,"error":{"kind":"Infeasible","message":"x"}}"#,
            0.0,
        );
        assert_eq!(parse(&f.snapshot())["mode"], "brake");
        // Garbage is ignored, not a crash.
        f.apply_replan("garbage", 0.0);
        assert_eq!(parse(&f.snapshot())["mode"], "brake");
        let bad = parse(&replan("{}"));
        assert_eq!(bad["ok"], false);
        assert_eq!(bad["error"]["kind"], "BadRequest");
    }

    #[test]
    fn snapshot_reports_outcome_after_touchdown() {
        let mut c = parse(&config());
        c["position"] = json!([0.0, 0.0, 3.0]);
        c["velocity"] = json!([0.0, 0.0, -10.0]);
        let mut f = FlightJs::create(&c.to_string()).unwrap();
        f.step(2.0);
        let snap = parse(&f.snapshot());
        assert_eq!(snap["landed"], true);
        assert_eq!(snap["outcome"]["landed"], false, "hit at ~10 m/s");
        assert!(snap["outcome"]["speed"].as_f64().unwrap() > 5.0);
    }

    #[test]
    fn controls_apply() {
        let mut f = FlightJs::create(&config()).unwrap();
        assert!(f.engine_out());
        assert_eq!(parse(&f.snapshot())["engines"], 5);
        assert!(f.set_noise("high"));
        assert!(!f.set_noise("loud"));
        f.gust(-1.0);
        f.grab(100.0, 0.0, 900.0);
        f.step(1.0);
        let snap = parse(&f.snapshot());
        assert_eq!(snap["t"], 0.0, "held: time stops");
        assert_eq!(snap["position"], json!([100.0, 0.0, 900.0]));
        f.release();
    }
}
