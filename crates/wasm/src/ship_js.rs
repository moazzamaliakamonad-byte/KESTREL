//! The ship's and the booster's closed loop as JavaScript sees it: a `ShipFlight` class on the
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
use model::{Scenario, Tuning, VehicleKind};
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
    /// Physics options; every field optional.
    #[serde(default)]
    tuning: TuningDto,
    /// Landing propellant, kg.
    #[serde(default)]
    propellant: Option<f64>,
    /// Steady wind, m/s (x, y).
    #[serde(default)]
    wind: Option<[f64; 2]>,
    #[serde(default)]
    gust_speed: Option<f64>,
    /// Landing engines failed from the start.
    #[serde(default)]
    failed: Vec<usize>,
    /// Start from orbit (the ship only).
    #[serde(default)]
    entry: Option<EntryDto>,
    /// Guidance uses its wind estimate (default true; D-046).
    #[serde(default)]
    wind_aware: Option<bool>,
}

/// An entry from orbit as the page sends it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct EntryDto {
    /// Distance from the entry interface to the tower, km.
    range_km: f64,
    /// Flight-path angle at the entry interface, degrees (negative: down).
    gamma_deg: f64,
}

/// The entry now, for the page (null when there is none in progress).
fn entry_json(f: &ShipFlight) -> Value {
    let Some(e) = f.entry() else {
        return Value::Null;
    };
    use model::entry as me;
    let s = &e.sim;
    let r = me::position(&s.x);
    let (lat, lon) = me::lat_lon(r);
    let (slat, slon) = me::lat_lon(e.site);
    let alt = me::altitude(&s.x);
    let l = me::loads(&s.aero, &s.x, s.mass);
    let q = s.heat_flux();
    let cmd = e.last;
    json!({
        "t": s.t,
        "altitude": alt,
        "speed": s.speed(),
        "mach": l.mach,
        "q_dyn": l.q_dyn,
        "density": l.rho,
        "gamma_deg": me::flight_path_angle(&s.x).to_degrees(),
        "heat_flux": q,
        "tile": me::tile_temperature(q),
        "tile_limit": me::TILE_LIMIT,
        "overheat": s.overheat,
        "overheat_limit": me::OVERHEAT_TIME,
        "heat_load": s.heat_load,
        "g": l.lift.hypot(l.drag) / model::vehicle::G0,
        "lift_to_drag": if l.drag > 0.0 { l.lift / l.drag } else { 0.0 },
        "bank_deg": s.bank.to_degrees(),
        "bank_cmd_deg": s.bank_cmd.to_degrees(),
        "reversals": e.guide.reversals,
        "heat_limited": cmd.is_some_and(|c| c.heat_limited),
        "heading_error_deg": cmd.map_or(0.0, |c| c.heading_error.to_degrees()),
        "predicted_overshoot": cmd.map_or(0.0, |c| c.prediction.overshoot),
        "predicted_time": cmd.map_or(0.0, |c| c.prediction.time),
        "to_site": me::surface_distance(r, e.site),
        "to_aim": me::surface_distance(r, e.guide.target.aim),
        "lat": lat.to_degrees(),
        "lon": lon.to_degrees(),
        "site": [slat.to_degrees(), slon.to_degrees()],
        "handover_altitude": s.handover_altitude,
        "position": r,
        "velocity": me::velocity(&s.x),
        "peak_tile": s.peak_tile,
        "peak_g": s.peak_g,
    })
}

/// What the entry did, after the hand-over (null without one).
fn entry_summary_json(f: &ShipFlight) -> Value {
    match f.entry_summary() {
        Some(s) => json!({
            "duration": s.duration,
            "peak_heat_flux": s.peak_heat_flux,
            "peak_tile": s.peak_tile,
            "heat_load": s.heat_load,
            "peak_g": s.peak_g,
            "reversals": s.reversals,
            "miss": s.miss,
        }),
        None => Value::Null,
    }
}

/// Physics options as the page sends them (angles in degrees). A missing
/// field keeps the scenario's value.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct TuningDto {
    #[serde(default)]
    density_scale: Option<f64>,
    #[serde(default)]
    gimbal_max_deg: Option<f64>,
    #[serde(default)]
    throttle_min: Option<f64>,
    #[serde(default)]
    isp: Option<f64>,
    #[serde(default)]
    ignition_time: Option<f64>,
}

impl TuningDto {
    fn onto(&self, base: Tuning) -> Tuning {
        Tuning {
            density_scale: self.density_scale.unwrap_or(base.density_scale),
            gimbal_max: self.gimbal_max_deg.map_or(base.gimbal_max, f64::to_radians),
            throttle_min: self.throttle_min.unwrap_or(base.throttle_min),
            isp: self.isp.unwrap_or(base.isp),
            ignition_time: self.ignition_time.unwrap_or(base.ignition_time),
        }
    }

    fn of(t: &Tuning) -> TuningDto {
        TuningDto {
            density_scale: Some(t.density_scale),
            gimbal_max_deg: Some(t.gimbal_max.to_degrees()),
            throttle_min: Some(t.throttle_min),
            isp: Some(t.isp),
            ignition_time: Some(t.ignition_time),
        }
    }
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
    let kind = Kind::parse(&c.scenario)?;
    let base = ShipFlightConfig::new(kind);
    Some(ShipFlightConfig {
        position: c.position,
        velocity: c.velocity,
        seed: c.seed,
        lag: c.lag,
        noise,
        tuning: Some(c.tuning.onto(kind.scenario().tuning())),
        propellant: c.propellant,
        wind: c.wind.unwrap_or([0.0; 2]),
        gust_speed: c.gust_speed.unwrap_or(base.gust_speed),
        failed: c.failed,
        entry: c.entry.map(|e| ship_flight::EntryConfig {
            range: e.range_km * 1e3,
            gamma: e.gamma_deg.to_radians(),
            ..Default::default()
        }),
        wind_aware: c.wind_aware.unwrap_or(true),
        ..base
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
    /// A planned engine switch (the ship's flip): the range after it, and
    /// the node it happens at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    late_limits: Option<(f64, f64)>,
    #[serde(default)]
    switch_node: usize,
    /// The rest of the problem the flight chose: the tilt cap and from
    /// which node it and the upright constraint apply (None: the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_tilt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tilt_from: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upright_from: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    approach: Option<(f64, f64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    side_speed: Option<(f64, f64)>,
    /// The wind the plan flies in (guidance's estimate, D-046); absent:
    /// still air.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wind: Option<Vec3>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestDto {
    scenario: String,
    /// The flight's physics options: the worker must plan with them.
    #[serde(default)]
    tuning: TuningDto,
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
    /// Plan time of its engine switch, s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    switch_time: Option<f64>,
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
            switch_time: p.switch_time,
        }
    }
}

fn state_of(v: &[f64]) -> Option<State> {
    let x: State = v.try_into().ok()?;
    x.iter().all(|c| c.is_finite()).then_some(x)
}

impl PlanDto {
    fn into_plan(self) -> Option<ShipPlan> {
        let x = self
            .x
            .iter()
            .map(|v| state_of(v))
            .collect::<Option<Vec<_>>>()?;
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
            switch_time: self.switch_time.filter(|s| s.is_finite() && *s >= 0.0),
        })
    }
}

fn request_of(kind: Kind, scn: &Scenario, options: &[(Leg, ShipProblem)]) -> RequestDto {
    RequestDto {
        scenario: kind.as_str().to_string(),
        tuning: TuningDto::of(&scn.tuning()),
        options: options
            .iter()
            .map(|(leg, p)| OptionDto {
                leg: (*leg).into(),
                x0: p.x0.to_vec(),
                thrust_limits: p.thrust_limits,
                late_limits: p.late_limits,
                switch_node: p.switch_node,
                max_tilt: p.max_tilt,
                tilt_from: Some(p.tilt_from),
                upright_from: p.upright_from,
                approach: Some(p.approach),
                side_speed: Some(p.side_speed),
                wind: Some(p.wind),
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
            let base = kind.scenario();
            let tuning = req.tuning.onto(base.tuning());
            let mut scn = base.tuned(&tuning);
            let leg: Leg = o.leg.into();
            scn.gate = leg.gate;
            scn.gate_velocity = leg.gate_velocity;
            let x0 = state_of(&o.x0)?;
            let (lo, hi) = o.thrust_limits;
            if !(lo.is_finite() && hi.is_finite() && 0.0 < lo && lo < hi) {
                return None;
            }
            let mut p = ShipProblem::new(scn, x0, (lo, hi));
            if let Some((a, b)) = o.late_limits {
                if !(a.is_finite() && b.is_finite() && 0.0 < a && a < b)
                    || o.switch_node < 1
                    || o.switch_node >= p.nodes
                {
                    return None;
                }
                p.late_limits = Some((a, b));
                p.switch_node = o.switch_node;
            }
            if let Some(t) = o.max_tilt {
                if !(t.is_finite() && t > 0.0) {
                    return None;
                }
                p.max_tilt = Some(t);
            }
            p.tilt_from = o.tilt_from.unwrap_or(2).min(p.nodes - 1);
            p.upright_from = o.upright_from.filter(|u| *u < p.nodes);
            if let Some((r, k)) = o.approach {
                if !(r.is_finite() && k.is_finite() && r > 0.0 && k > 0.0) {
                    return None;
                }
                p.approach = (r, k);
            }
            if let Some((a, b)) = o.side_speed {
                if !(a.is_finite() && b.is_finite() && a > 0.0 && b >= 0.0) {
                    return None;
                }
                p.side_speed = (a, b);
            }
            if let Some(w) = o.wind {
                if !w.iter().all(|c| c.is_finite() && c.abs() <= 100.0) {
                    return None;
                }
                p.wind = w;
            }
            Some((leg, p))
        })
        .collect()
}

/// Solve a request from [`ShipFlightJs::measure_request`]:
/// `{ok: true, index, leg, plan}` or `{ok: false, error}`.
#[wasm_bindgen]
pub fn plan_ship(request_json: &str) -> String {
    let Some(options) = serde_json::from_str::<RequestDto>(request_json)
        .ok()
        .and_then(problems_of)
    else {
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
        Some(Outcome::Crashed(r)) => (json!("crashed"), json!(r.as_str())),
    }
}

/// Solve one representative problem (the booster's first landing-burn
/// plan) and throw the answer away. A worker calls this once when it
/// starts: the first solve in a fresh WebAssembly instance is several
/// times slower than the rest, and the booster's burn is too short to
/// spend that on the plan it is waiting for. Returns true if it solved.
#[wasm_bindgen]
pub fn warmup_ship() -> bool {
    let Ok(mut f) = ShipFlight::new(ShipFlightConfig::new(Kind::Booster)) else {
        return false;
    };
    while !f.wants_replan() && f.phase() != Phase::Done && f.sim().t < 120.0 {
        f.step(0.1);
    }
    let (_, options) = f.problem();
    ship_flight::plan(&options).is_ok()
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
    let kind = f.kind;
    let mut path = vec![dynamics::position(&f.sim().x)];
    let mut phases = vec![json!({"t": 0.0, "phase": f.phase().label(kind)})];
    let mut since = 0.0;
    let mut last_phase = f.phase();
    // The entry (from orbit): its ground track every 10 s, then the
    // 6-DoF flight as usual.
    let mut track: Vec<[f64; 3]> = Vec::new();
    let mut next_track = 0.0;
    while f.phase() == Phase::Entry {
        if let Some(e) = f.entry()
            && e.sim.t >= next_track
        {
            let (lat, lon) = model::entry::lat_lon(model::entry::position(&e.sim.x));
            track.push([lat.to_degrees(), lon.to_degrees(), e.sim.altitude()]);
            next_track += 10.0;
        }
        f.step(1.0);
    }
    if f.phase() != Phase::Done {
        path = vec![dynamics::position(&f.sim().x)];
        phases.push(json!({"t": f.sim().t, "phase": f.phase().label(kind), "position": dynamics::position(&f.sim().x)}));
        last_phase = f.phase();
    }
    let limit = f.sim().t + 400.0;
    while f.phase() != Phase::Done && f.sim().t < limit {
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
            phases.push(json!({"t": f.sim().t, "phase": last_phase.label(kind), "position": dynamics::position(&f.sim().x)}));
        }
    }
    let (outcome, reason) = outcome_str(f.sim().outcome);
    json!({
        "ok": true, "path": path, "phases": phases, "outcome": outcome, "reason": reason, "t": f.sim().t,
        "entry": entry_summary_json(&f), "entry_track": track,
        "propellant": f.sim().propellant(),
    })
    .to_string()
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
        "vehicle": vehicle_str(s.ship.kind),
        "propellant": s.start_mass - s.dry_mass,
        "aim": s.aim,
        "tuning": TuningDto::of(&s.tuning()),
        "gravity": s.planet.gravity(),
        "start": {"position": position, "velocity": velocity},
        "ship": {
            "length": s.ship.length,
            "radius": s.ship.radius,
            "cm_height": s.ship.cm_height,
            "pin_height": s.ship.pin_height,
            "engine_max": s.ship.engine_max,
            "engines": s.ship.engine_count,
            "final_engines": s.ship.final_engines,
            "isp": s.ship.isp,
            "throttle_min": s.ship.throttle_min,
            "fin_station": s.ship.fins.map(|f| f.station),
            "fin_max_deg": s.ship.fins.map(|f| f.max_deflection.to_degrees()),
            "fore_station": s.ship.fore.station,
            "aft_station": s.ship.aft.station,
            "flap_y": s.ship.flap_y,
            "gimbal_max_deg": s.ship.gimbal_max.to_degrees(),
        },
        // The model's own coefficients, so the page can show each part's
        // force with the same formula the simulator integrates.
        "model": {
            "c_a": s.ship.c_a,
            "c_n": s.ship.c_n,
            "c_n_alpha": s.ship.c_n_alpha,
            "cp_offset": s.ship.cp_offset,
            "c_flap": s.ship.c_flap,
            "fore_area": s.ship.fore.area,
            "aft_area": s.ship.aft.area,
            "fin_area": s.ship.fins.map(|f| f.area),
            "fin_c_delta": s.ship.fins.map(|f| f.c_delta),
            "fin_radius": s.ship.fins.map(|f| f.radius),
            "rcs_thrust": s.ship.rcs_thrust,
            "rcs_fore": s.ship.rcs_fore,
            "rcs_aft": s.ship.rcs_aft,
            "inertia_per_kg": s.ship.inertia_per_kg,
            "gimbal_rate_deg": s.ship.gimbal_rate.to_degrees(),
            "ignition_time": s.ship.ignition_time,
        },
        "limits": {
            "catch_down": sim::ship::CATCH_DOWN,
            "catch_side": sim::ship::CATCH_SIDE,
            "catch_tilt_deg": sim::ship::CATCH_TILT_DEG,
            "catch_rate_deg": sim::ship::CATCH_RATE_DEG,
            "arms_close_above": sim::ship::ARMS_CLOSE_ABOVE,
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

fn vehicle_str(k: VehicleKind) -> &'static str {
    match k {
        VehicleKind::Ship => "ship",
        VehicleKind::Booster => "booster",
    }
}

/// One flight (ship or booster), owned by the page.
#[wasm_bindgen(js_name = ShipFlight)]
pub struct ShipFlightJs {
    inner: ShipFlight,
}

#[wasm_bindgen(js_class = ShipFlight)]
impl ShipFlightJs {
    /// `{scenario: "earth"|"booster", position, velocity, seed?, lag?,
    /// noise?, tuning?, propellant?, wind?, gust_speed?, failed?}`;
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
        json!({"t": t, "request": request_of(self.inner.kind, &self.inner.sim().scn, &options)})
            .to_string()
    }

    /// Install the worker's answer to a request measured at sim time `t`.
    /// Unreadable answers are ignored.
    pub fn apply_replan(&mut self, response_json: &str, t: f64) {
        let Ok(v) = serde_json::from_str::<Value>(response_json) else {
            return;
        };
        if v["ok"] == true {
            let leg = serde_json::from_value::<LegDto>(v["leg"].clone());
            let plan = serde_json::from_value::<PlanDto>(v["plan"].clone())
                .ok()
                .and_then(PlanDto::into_plan);
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

    /// Re-plan at the next chance (something happened).
    pub fn request_replan(&mut self) {
        self.inner.request_replan();
    }

    /// A gust blowing toward the horizontal direction `(dx, dy)`.
    pub fn gust_toward(&mut self, dx: f64, dy: f64) {
        self.inner.gust_toward(dx, dy);
    }

    pub fn set_gust_speed(&mut self, speed: f64) {
        self.inner.set_gust_speed(speed);
    }

    /// How long solves are taking, s of sim time (the page measures it).
    pub fn set_solve_time(&mut self, seconds: f64) {
        self.inner.set_solve_time(seconds);
    }

    /// Steady wind, m/s.
    pub fn set_wind(&mut self, x: f64, y: f64) {
        self.inner.set_wind(x, y);
    }

    pub fn engine_out(&mut self) -> bool {
        self.inner.engine_out()
    }

    /// Fail landing engine `i` (0-based; on the booster 0–2 are the centre
    /// three). False if it cannot fail (already failed, or the last one).
    pub fn fail_engine(&mut self, i: usize) -> bool {
        self.inner.fail_engine(i)
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
        let rho = s.scn.density(x[dynamics::R + 2]);
        let (t_min, t_max) = s.cluster.limits();
        // Aerodynamic force now (body plus control surfaces), inertial, N.
        let (f_body, _) = model::aero::body_force(&s.scn.ship, rho, q, w_air);
        let (f_surf, _) = match s.scn.ship.kind {
            VehicleKind::Ship => model::aero::flap_force(&s.scn.ship, rho, q, w_air, s.flaps),
            VehicleKind::Booster => model::aero::fin_force(&s.scn.ship, rho, q, w_air, s.flaps),
        };
        // Stagnation-point heating, W/m² (Sutton–Graves, 4.5 m nose radius:
        // an estimate for the picture; heating is not part of the dynamics).
        let k_sg = 1.7415e-4;
        let heat_flux = k_sg * (rho / s.scn.ship.radius).sqrt() * quat::norm(w_air).powi(3);
        let speed_of_sound = 340.0 - 0.004 * x[dynamics::R + 2].clamp(0.0, 11_000.0);
        let thrust = s.cluster.thrust;
        let tn = quat::norm(thrust);
        let gimbal = if tn > 0.0 {
            (thrust[2] / tn).clamp(-1.0, 1.0).acos().to_degrees()
        } else {
            0.0
        };
        let (outcome, reason) = outcome_str(s.outcome);
        let (guide, leg_engines) = match (f.guide(), f.leg()) {
            (Guide::Plan { .. }, Some(l)) => ("plan", l.engines),
            (Guide::Brake, _) => ("brake", 0),
            _ => ("none", 0),
        };
        // During the entry the clock is the entry's (the 6-DoF flight has not begun).
        let t = f.entry().map_or(s.t, |e| e.sim.t);
        let mut out = json!({
            "t": t,
            "vehicle": vehicle_str(s.scn.ship.kind),
            "phase": f.phase().label(f.kind),
            "thrust_limits": [t_min, t_max],
            "mach": quat::norm(w_air) / speed_of_sound,
            "accel_g": quat::norm(thrust) / (x[dynamics::M] * model::vehicle::G0),
            "dry_mass": s.scn.dry_mass,
            "aero_force": quat::add(f_body, f_surf),
            "thrust_inertial": quat::rotate(q, thrust),
            "weight": x[dynamics::M] * s.scn.planet.gravity(),
            "density": rho,
            "heat_flux": heat_flux,
            "command": {"thrust": f.last_command().thrust_body, "surfaces": f.last_command().flaps, "rcs": f.last_command().rcs},
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
            "arms": s.arms_closed,
            "wind": s.wind_at(s.t),
            "guide": guide,
            "leg_engines": leg_engines,
            "ignition_z": f.ignition_z(),
            "grabbed": f.grabbed(),
            "outcome": outcome,
            "reason": reason,
            "end_velocity": s.end_velocity,
            "entry": entry_json(f),
            "entry_summary": entry_summary_json(f),
            "engine_out_margin": f.engine_out_margin(),
        });
        // Guidance's own view of the air (D-046).
        let est = f.wind_estimate();
        out["wind_est"] = json!(est.w);
        out["wind_sigma"] = json!(est.sigma);
        out["wind_aware"] = json!(f.wind_aware());
        // The approach's bearing (M4.3, D-048).
        out["bearing"] = json!(f.bearing());
        out["bearing_chosen"] = json!(f.bearing_chosen());
        out["earth_frame"] = f.earth_frame().map_or(
            Value::Null,
            |fr| json!({"site": fr.site, "x": fr.x, "y": fr.y}),
        );
        // During the entry the vehicle is where the entry has it, in the
        // tower frame (the 6-DoF flight has not begun): the page draws one
        // continuous flight through the hand-over.
        if let (Some(e), Some((p, vel, qq))) = (f.entry(), f.entry_local()) {
            let l = model::entry::loads(&e.sim.aero, &e.sim.x, e.sim.mass);
            out["position"] = json!(p);
            out["velocity"] = json!(vel);
            out["quat"] = json!(qq);
            out["omega"] = json!([0.0, 0.0, 0.0]);
            out["tilt_deg"] = json!(quat::tilt(qq).to_degrees());
            out["heat_flux"] = json!(e.sim.heat_flux());
            out["density"] = json!(l.rho);
            out["mach"] = json!(l.mach);
            out["q_dyn"] = json!(l.q_dyn);
            out["wind"] = json!([0.0, 0.0, 0.0]);
            out["aero_force"] = json!(quat::scale(model::entry::unit(vel), -l.drag * e.sim.mass));
        }
        out.to_string()
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

    const EARTH: &str =
        r#"{"scenario":"earth","position":[700,0,5000],"velocity":[-25,0,-75],"noise":"low"}"#;

    #[test]
    fn bad_configs_are_refused() {
        assert!(ShipFlightJs::create("{}").is_none());
        assert!(
            ShipFlightJs::create(
                r#"{"scenario":"venus","position":[0,0,5000],"velocity":[0,0,0]}"#
            )
            .is_none()
        );
        assert!(
            ShipFlightJs::create(r#"{"scenario":"earth","position":[0,0,10],"velocity":[0,0,0]}"#)
                .is_none()
        );
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

    /// The page's planner runs in a worker: the estimated wind must cross
    /// that boundary in the request, or the browser plans in still air
    /// while every native test plans in the wind (found in review).
    #[test]
    fn the_estimated_wind_reaches_the_worker() {
        let windy = r#"{"scenario":"booster","position":[900,0,14000],"velocity":[-30,0,-420],"noise":"low","wind":[0,-15]}"#;
        let mut f = ShipFlightJs::create(windy).unwrap();
        while !f.wants_replan() {
            f.step(0.05);
            assert!(f.inner.sim().t < 200.0, "never asked for a plan");
        }
        let flown = f.inner.air().wind;
        assert!(flown[1] < -10.0, "flight flies {flown:?}");
        let req = parse(&f.measure_request());
        let dto: RequestDto = serde_json::from_value(req["request"].clone()).unwrap();
        let problems = problems_of(dto).expect("problems");
        for (_, p) in &problems {
            assert_eq!(p.wind, flown, "the worker's problem lost the wind");
        }
        // A request without the field (an older page) plans in still air.
        let mut old = req["request"].clone();
        for o in old["options"].as_array_mut().unwrap() {
            o.as_object_mut().unwrap().remove("wind");
        }
        let dto: RequestDto = serde_json::from_value(old).unwrap();
        assert_eq!(problems_of(dto).unwrap()[0].1.wind, [0.0; 3]);
    }

    #[test]
    fn booster_config_options_reach_the_flight_and_the_worker() {
        let cfg = r#"{"scenario":"booster","position":[1200,0,14000],"velocity":[-30,0,-420],
            "tuning":{"gimbal_max_deg":10,"density_scale":1.2},"propellant":120000,
            "wind":[5,-3],"gust_speed":25,"failed":[4,7]}"#;
        let mut f = ShipFlightJs::create(cfg).unwrap();
        let s = parse(&f.snapshot());
        assert_eq!(s["vehicle"], "booster");
        assert_eq!(s["phase"], "descent");
        assert_eq!(s["engines"].as_array().unwrap().len(), 13);
        assert_eq!(s["engines"][4], "failed");
        assert_eq!(s["engines"][7], "failed");
        assert_eq!(s["propellant"], 120000.0);
        assert_eq!(s["wind"], json!([5.0, -3.0, 0.0]));
        assert!(f.fail_engine(0) && !f.fail_engine(0));
        let scn = &f.inner.sim().scn;
        assert!((scn.ship.gimbal_max.to_degrees() - 10.0).abs() < 1e-9);
        assert_eq!(scn.density_scale, 1.2);
        // The request carries the options to the worker.
        let req = parse(&f.measure_request());
        assert_eq!(req["request"]["tuning"]["density_scale"], 1.2);
        let info = parse(&ship_scenario("booster"));
        assert_eq!(info["vehicle"], "booster");
        assert_eq!(info["ship"]["engines"], 13);
    }

    #[test]
    fn warmup_solves_a_plan() {
        assert!(warmup_ship());
    }

    #[test]
    fn snapshot_has_the_panels_fields() {
        let f = ShipFlightJs::create(EARTH).unwrap();
        let s = parse(&f.snapshot());
        for k in [
            "t",
            "phase",
            "position",
            "quat",
            "engines",
            "flaps",
            "rcs",
            "q_dyn",
            "propellant",
            "aero_force",
            "heat_flux",
            "mach",
            "weight",
        ] {
            assert!(!s[k].is_null(), "missing {k}");
        }
        assert_eq!(s["phase"], "belly-flop");
        assert_eq!(parse(&ship_scenario("mars"))["ok"], false);
    }

    /// Guidance's wind estimate reaches the page, with its 1-sigma and
    /// whether guidance uses it; the page can switch that off.
    #[test]
    fn snapshot_carries_the_wind_estimate() {
        let windy = r#"{"scenario":"booster","position":[900,0,14000],"velocity":[-30,0,-420],"noise":"low","wind":[0,-15]}"#;
        let mut f = ShipFlightJs::create(windy).unwrap();
        for _ in 0..600 {
            f.step(1.0 / 60.0);
        }
        let s = parse(&f.snapshot());
        let est = s["wind_est"].as_array().expect("wind_est");
        assert!((est[1].as_f64().unwrap() + 15.0).abs() < 2.0, "{est:?}");
        assert!(s["wind_sigma"][0].as_f64().unwrap() < 2.0);
        assert_eq!(s["wind_aware"], true);
        assert_eq!(s["bearing"], 0.0);
        assert_eq!(s["bearing_chosen"], false);
        let blind = r#"{"scenario":"booster","position":[900,0,14000],"velocity":[-30,0,-420],"wind":[0,-15],"wind_aware":false}"#;
        let f = ShipFlightJs::create(blind).unwrap();
        assert_eq!(parse(&f.snapshot())["wind_aware"], false);
    }
}
