//! The ship's closed loop as JavaScript sees it: a `ShipFlight` class on the
//! main thread, `plan_ship` for the worker, `preview_ship` for the page's
//! "where will it go" line. JSON in and out; nothing here panics or throws
//! (D-009). Simulation only.
//!
//! Per re-plan: `flight.measure_request()` → worker `plan_ship(json)` →
//! `flight.apply_replan(json, t)`. The request carries every option to try
//! (leg + problem), so the worker holds no flight state.

use guidance::GuidanceError;
use guidance::scvx::{ShipPlan, ShipProblem};
use model::dynamics::{self, State};
use model::quat::{self, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sim::Noise;
use sim::ship::Outcome;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::ship_flight::{self, Guide, Kind, Leg, Phase, ShipFlight, ShipFlightConfig};
use crate::{error_kind, failure};

#[derive(Debug, Deserialize)]
struct ConfigDto {
    scenario: String,
    position: Vec3,
    velocity: Vec3,
    #[serde(default)]
    seed: u64,
    #[serde(default)]
    lag: f64,
    #[serde(default)]
    noise: Option<String>,
}

fn noise_level(name: &str) -> Option<Noise> {
    match name {
        "off" => Some(Noise::OFF),
        "low" => Some(Noise::LOW),
        "high" => Some(Noise::HIGH),
        _ => None,
    }
}

fn config(json: &str) -> Option<ShipFlightConfig> {
    let c: ConfigDto = serde_json::from_str(json).ok()?;
    let noise = match c.noise.as_deref() {
        None => Noise::OFF,
        Some(n) => noise_level(n)?,
    };
    Some(ShipFlightConfig {
        kind: Kind::parse(&c.scenario)?,
        position: c.position,
        velocity: c.velocity,
        seed: c.seed,
        lag: c.lag,
        noise,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct LegDto {
    engines: u8,
    gate: Vec3,
    gate_velocity: Vec3,
    last: bool,
}

impl From<Leg> for LegDto {
    fn from(l: Leg) -> LegDto {
        LegDto {
            engines: l.engines,
            gate: l.gate,
            gate_velocity: l.gate_velocity,
            last: l.last,
        }
    }
}

impl From<LegDto> for Leg {
    fn from(l: LegDto) -> Leg {
        Leg {
            engines: l.engines,
            gate: l.gate,
            gate_velocity: l.gate_velocity,
            last: l.last,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OptionDto {
    leg: LegDto,
    x0: Vec<f64>,
    thrust_limits: (f64, f64),
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestDto {
    scenario: String,
    options: Vec<OptionDto>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlanDto {
    t: Vec<f64>,
    x: Vec<Vec<f64>>,
    u: Vec<Vec3>,
    final_time: f64,
    iterations: usize,
    converged: bool,
    gate_error: f64,
    defect: f64,
}

impl From<&ShipPlan> for PlanDto {
    fn from(p: &ShipPlan) -> PlanDto {
        PlanDto {
            t: p.t.clone(),
            x: p.x.iter().map(|x| x.to_vec()).collect(),
            u: p.u.clone(),
            final_time: p.final_time,
            iterations: p.iterations,
            converged: p.converged,
            gate_error: p.gate_error,
            defect: p.defect,
        }
    }
}

fn state_of(v: &[f64]) -> Option<State> {
    let x: State = v.try_into().ok()?;
    x.iter().all(|c| c.is_finite()).then_some(x)
}

impl PlanDto {
    fn into_plan(self) -> Option<ShipPlan> {
        let x = self.x.iter().map(|v| state_of(v)).collect::<Option<Vec<_>>>()?;
        let n = self.t.len();
        if n < 2 || x.len() != n || self.u.len() != n || !self.final_time.is_finite() {
            return None;
        }
        Some(ShipPlan {
            t: self.t,
            x,
            u: self.u,
            final_time: self.final_time,
            iterations: self.iterations,
            defect: self.defect,
            gate_error: self.gate_error,
            converged: self.converged,
            switch_time: None,
        })
    }
}

fn request_of(kind: Kind, options: &[(Leg, ShipProblem)]) -> RequestDto {
    RequestDto {
        scenario: kind.as_str().to_string(),
        options: options
            .iter()
            .map(|(leg, p)| OptionDto {
                leg: (*leg).into(),
                x0: p.x0.to_vec(),
                thrust_limits: p.thrust_limits,
            })
            .collect(),
    }
}

fn problems_of(req: RequestDto) -> Option<Vec<(Leg, ShipProblem)>> {
    let kind = Kind::parse(&req.scenario)?;
    if req.options.is_empty() || req.options.len() > 4 {
        return None;
    }
    req.options
        .into_iter()
        .map(|o| {
            let mut scn = kind.scenario();
            let leg: Leg = o.leg.into();
            scn.gate = leg.gate;
            scn.gate_velocity = leg.gate_velocity;
            let x0 = state_of(&o.x0)?;
            let (lo, hi) = o.thrust_limits;
            if !(lo.is_finite() && hi.is_finite() && 0.0 < lo && lo < hi) {
                return None;
            }
            Some((leg, ShipProblem::new(scn, x0, (lo, hi))))
        })
        .collect()
}

/// Solve a request from [`ShipFlightJs::measure_request`]:
/// `{ok: true, index, leg, plan}` or `{ok: false, error}`.
#[wasm_bindgen]
pub fn plan_ship(request_json: &str) -> String {
    let Some(options) = serde_json::from_str::<RequestDto>(request_json).ok().and_then(problems_of) else {
        return failure("BadRequest", "could not read ship request".into()).to_string();
    };
    match ship_flight::plan(&options) {
        Ok((i, p)) => json!({
            "ok": true,
            "index": i,
            "leg": LegDto::from(options[i].0),
            "plan": PlanDto::from(&p),
        }),
        Err(e) => failure(error_kind(&e), format!("{e:?}")),
    }
    .to_string()
}

fn outcome_str(o: Option<Outcome>) -> (Value, Value) {
    match o {
        None => (Value::Null, Value::Null),
        Some(Outcome::Caught) => (json!("caught"), Value::Null),
        Some(Outcome::Landed) => (json!("landed"), Value::Null),
        Some(Outcome::Crashed(r)) => (json!("crashed"), json!(r.as_str())),
    }
}

/// Where a start leads: fly the whole flight now (noise off, solves with no
/// latency) and return the path every 0.5 s, the phase changes and the end.
#[wasm_bindgen]
pub fn preview_ship(config_json: &str) -> String {
    let Some(mut c) = config(config_json) else {
        return failure("BadRequest", "could not read ship config".into()).to_string();
    };
    c.noise = Noise::OFF;
    let Ok(mut f) = ShipFlight::new(c) else {
        return failure("BadRequest", "start not allowed".into()).to_string();
    };
    let mut path = vec![dynamics::position(&f.sim().x)];
    let mut phases = vec![json!({"t": 0.0, "phase": f.phase().as_str()})];
    let mut since = 0.0;
    let mut last_phase = f.phase();
    while f.phase() != Phase::Done && f.sim().t < 400.0 {
        f.step(0.05);
        since += 0.05;
        if f.wants_replan() {
            let (t0, options) = f.problem();
            match ship_flight::plan(&options) {
                Ok((i, p)) => f.set_plan(p, t0, options[i].0),
                Err(e) => f.plan_failed(&e),
            }
        }
        if since >= 0.5 || f.phase() == Phase::Done {
            path.push(dynamics::position(&f.sim().x));
            since = 0.0;
        }
        if f.phase() != last_phase {
            last_phase = f.phase();
            phases.push(json!({"t": f.sim().t, "phase": last_phase.as_str(), "position": dynamics::position(&f.sim().x)}));
        }
    }
    let (outcome, reason) = outcome_str(f.sim().outcome);
    json!({"ok": true, "path": path, "phases": phases, "outcome": outcome, "reason": reason, "t": f.sim().t}).to_string()
}

/// The scenario's fixed numbers the page draws and edits with.
#[wasm_bindgen]
pub fn ship_scenario(kind: &str) -> String {
    let Some(k) = Kind::parse(kind) else {
        return failure("BadRequest", "unknown scenario".into()).to_string();
    };
    let s = k.scenario();
    let (position, velocity) = k.default_start();
    json!({
        "ok": true,
        "scenario": k.as_str(),
        "planet": match s.planet { model::Planet::Earth => "earth", model::Planet::Mars => "mars" },
        "gravity": s.planet.gravity(),
        "start": {"position": position, "velocity": velocity},
        "ship": {
            "length": s.ship.length,
            "radius": s.ship.radius,
            "cm_height": s.ship.cm_height,
            "pin_height": s.ship.pin_height,
            "leg_drop": s.ship.leg_drop,
            "leg_radius": s.ship.leg_radius,
            "engine_max": s.ship.engine_max,
            "engines": s.ship.engine_count,
            "fore_station": s.ship.fore.station,
            "aft_station": s.ship.aft.station,
            "flap_y": s.ship.flap_y,
            "gimbal_max_deg": s.ship.gimbal_max.to_degrees(),
        },
        "start_mass": s.start_mass,
        "dry_mass": s.dry_mass,
        "gate": s.gate,
        "rest_cm_z": s.rest_cm_z(),
        "catch": s.catch.map(|c| json!({
            "arm_z": c.arm_z, "mast_x": c.mast_x, "mast_half": c.mast_half,
            "tower_top": c.tower_top, "capture_radius": c.capture_radius,
        })),
    })
    .to_string()
}

/// One ship flight, owned by the page.
#[wasm_bindgen(js_name = ShipFlight)]
pub struct ShipFlightJs {
    inner: ShipFlight,
}

#[wasm_bindgen(js_class = ShipFlight)]
impl ShipFlightJs {
    /// `{scenario: "earth"|"mars", position, velocity, seed?, lag?, noise?}`;
    /// `undefined` if not valid.
    pub fn create(config_json: &str) -> Option<ShipFlightJs> {
        let inner = ShipFlight::new(config(config_json)?).ok()?;
        Some(ShipFlightJs { inner })
    }

    pub fn step(&mut self, dt: f64) {
        self.inner.step(dt);
    }

    pub fn wants_replan(&self) -> bool {
        self.inner.wants_replan()
    }

    /// `{t, request}` for the worker.
    pub fn measure_request(&mut self) -> String {
        let (t, options) = self.inner.problem();
        json!({"t": t, "request": request_of(self.inner.kind, &options)}).to_string()
    }

    /// Install the worker's answer to a request measured at sim time `t`.
    /// Unreadable answers are ignored.
    pub fn apply_replan(&mut self, response_json: &str, t: f64) {
        let Ok(v) = serde_json::from_str::<Value>(response_json) else {
            return;
        };
        if v["ok"] == true {
            let leg = serde_json::from_value::<LegDto>(v["leg"].clone());
            let plan = serde_json::from_value::<PlanDto>(v["plan"].clone()).ok().and_then(PlanDto::into_plan);
            if let (Ok(leg), Some(plan)) = (leg, plan)
                && t.is_finite()
            {
                self.inner.set_plan(plan, t, leg.into());
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

    /// The ship now, for the scene and the panels.
    pub fn snapshot(&self) -> String {
        let f = &self.inner;
        let s = f.sim();
        let x = s.x;
        let v = dynamics::velocity(&x);
        let q = dynamics::attitude(&x);
        let w_air = quat::sub(v, s.wind_at(s.t));
        let rho = s.scn.planet.density(x[dynamics::R + 2]);
        let thrust = s.cluster.thrust;
        let tn = quat::norm(thrust);
        let gimbal = if tn > 0.0 { (thrust[2] / tn).clamp(-1.0, 1.0).acos().to_degrees() } else { 0.0 };
        let (outcome, reason) = outcome_str(s.outcome);
        let (guide, leg_engines) = match (f.guide(), f.leg()) {
            (Guide::Plan { .. }, Some(l)) => ("plan", l.engines),
            (Guide::Brake, _) => ("brake", 0),
            _ => ("none", 0),
        };
        json!({
            "t": s.t,
            "phase": f.phase().as_str(),
            "position": dynamics::position(&x),
            "velocity": v,
            "quat": q,
            "omega": dynamics::rates(&x),
            "tilt_deg": quat::tilt(q).to_degrees(),
            "mass": x[dynamics::M],
            "propellant": s.propellant(),
            "engines": f.engine_states(),
            "thrust": thrust,
            "thrust_mag": tn,
            "gimbal_deg": gimbal,
            "flaps": s.flaps,
            "rcs": s.rcs_firing,
            "q_dyn": 0.5 * rho * quat::dot(w_air, w_air),
            "legs": s.legs_down,
            "arms": s.arms_closed,
            "wind": s.wind_at(s.t),
            "guide": guide,
            "leg_engines": leg_engines,
            "ignition_z": f.ignition_z(),
            "grabbed": f.grabbed(),
            "outcome": outcome,
            "reason": reason,
            "end_velocity": s.end_velocity,
        })
        .to_string()
    }

    /// The current plan for drawing: `{t0, final_time, position[], quat[]}` or null.
    pub fn plan_json(&self) -> String {
        match self.inner.guide() {
            Guide::Plan { plan, t0 } => json!({
                "t0": t0,
                "final_time": plan.final_time,
                "position": plan.x.iter().map(dynamics::position).collect::<Vec<_>>(),
                "quat": plan.x.iter().map(dynamics::attitude).collect::<Vec<_>>(),
                "thrust": plan.u.iter().map(|u| quat::norm(*u)).collect::<Vec<_>>(),
            })
            .to_string(),
            _ => "null".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("JSON")
    }

    const EARTH: &str = r#"{"scenario":"earth","position":[700,0,5000],"velocity":[-25,0,-75],"noise":"low"}"#;

    #[test]
    fn bad_configs_are_refused() {
        assert!(ShipFlightJs::create("{}").is_none());
        assert!(ShipFlightJs::create(r#"{"scenario":"venus","position":[0,0,5000],"velocity":[0,0,0]}"#).is_none());
        assert!(ShipFlightJs::create(r#"{"scenario":"earth","position":[0,0,10],"velocity":[0,0,0]}"#).is_none());
        assert_eq!(parse(&plan_ship("garbage"))["ok"], false);
        assert_eq!(parse(&preview_ship("{}"))["ok"], false);
    }

    #[test]
    fn request_round_trip_through_the_worker() {
        let mut f = ShipFlightJs::create(EARTH).unwrap();
        // Fly the belly flop and the flip (no plans needed).
        while !f.wants_replan() {
            f.step(0.1);
            assert!(f.inner.sim().t < 200.0, "never asked for a plan");
        }
        let req = parse(&f.measure_request());
        let t = req["t"].as_f64().unwrap();
        let answer = plan_ship(&req["request"].to_string());
        let v = parse(&answer);
        assert_eq!(v["ok"], true, "{answer}");
        f.apply_replan(&answer, t);
        assert_eq!(parse(&f.snapshot())["guide"], "plan");
        assert_ne!(f.plan_json(), "null");
        // Garbage is ignored.
        f.apply_replan("garbage", t);
        assert_eq!(parse(&f.snapshot())["guide"], "plan");
    }

    #[test]
    fn snapshot_has_the_panels_fields() {
        let f = ShipFlightJs::create(EARTH).unwrap();
        let s = parse(&f.snapshot());
        for k in ["t", "phase", "position", "quat", "engines", "flaps", "rcs", "q_dyn", "propellant"] {
            assert!(!s[k].is_null(), "missing {k}");
        }
        assert_eq!(s["phase"], "belly-flop");
        assert_eq!(parse(&ship_scenario("mars"))["planet"], "mars");
    }
}
