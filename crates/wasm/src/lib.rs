//! WebAssembly bridge for Kestrel guidance. Two functions, JSON in and JSON
//! out, that never panic and never throw: every failure comes back as
//! `{"ok": false, "error": {"kind": ..., "message": ...}}`.

mod dto;
pub mod flight;
mod flight_js;
pub mod ship_flight;
pub mod ship_js;

use dto::{RequestDto, TrajectoryDto, VehicleDto};
use guidance::{GuidanceError, LandingProblem, Vehicle, solve};
use serde_json::{Value, json};
use wasm_bindgen::prelude::wasm_bindgen;

/// Largest accepted `steps`; bigger problems would exhaust browser memory
/// long before they finished.
const MAX_STEPS: usize = 1000;

/// The default request (example state and vehicle), so the page takes the
/// vehicle's numbers from Rust rather than duplicating them.
#[wasm_bindgen]
pub fn example_request() -> String {
    example_request_json()
}

/// Plan a fuel-optimal landing. See the M1 spec for the JSON shapes.
#[wasm_bindgen]
pub fn plan_landing(request_json: &str) -> String {
    plan_landing_json(request_json)
}

pub fn example_request_json() -> String {
    let p = LandingProblem::example();
    let req = RequestDto {
        position: p.position,
        velocity: p.velocity,
        steps: Some(p.steps),
        vehicle: Some(VehicleDto::from(&p.vehicle)),
    };
    serde_json::to_string(&req).expect("plain numbers always serialize")
}

pub fn plan_landing_json(request_json: &str) -> String {
    let response = match serde_json::from_str::<RequestDto>(request_json) {
        Err(e) => failure("BadRequest", format!("could not read request: {e}")),
        Ok(req) => respond(req),
    };
    response.to_string()
}

fn respond(req: RequestDto) -> Value {
    let steps = req.steps.unwrap_or(50);
    if steps > MAX_STEPS {
        return failure("BadRequest", format!("steps must be at most {MAX_STEPS}"));
    }
    let vehicle = req
        .vehicle
        .map(Vehicle::from)
        .unwrap_or_else(Vehicle::example_mars);
    let problem = LandingProblem {
        vehicle,
        position: req.position,
        velocity: req.velocity,
        steps,
    };
    match solve(&problem) {
        Ok(traj) => json!({
            "ok": true,
            "trajectory": TrajectoryDto::from(traj),
            "vehicle": VehicleDto::from(&problem.vehicle),
        }),
        Err(e) => failure(error_kind(&e), format!("{e:?}")),
    }
}

pub(crate) fn error_kind(e: &GuidanceError) -> &'static str {
    match e {
        GuidanceError::InvalidParams(_) => "InvalidParams",
        GuidanceError::InvalidProblem(_) => "InvalidProblem",
        GuidanceError::Infeasible => "Infeasible",
        GuidanceError::NotLossless { .. } => "NotLossless",
        GuidanceError::SolverFailed(_) => "SolverFailed",
    }
}

pub(crate) fn failure(kind: &str, message: String) -> Value {
    json!({ "ok": false, "error": { "kind": kind, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(json: &str) -> Value {
        serde_json::from_str(&plan_landing_json(json)).expect("response is JSON")
    }

    fn kind(v: &Value) -> &str {
        assert_eq!(v["ok"], false, "expected failure, got {v}");
        v["error"]["kind"].as_str().unwrap()
    }

    #[test]
    fn example_request_round_trips() {
        let v: Value = serde_json::from_str(&example_request_json()).unwrap();
        assert_eq!(v["position"], serde_json::json!([2000.0, 0.0, 1500.0]));
        assert_eq!(v["velocity"], serde_json::json!([-50.0, 0.0, -75.0]));
        assert_eq!(v["steps"], 50);
        assert!(v["vehicle"]["max_thrust"].as_f64().unwrap() > 13000.0);
    }

    #[test]
    fn defaults_fill_missing_fields() {
        let v = plan(r#"{"position":[2000,0,1500],"velocity":[-50,0,-75]}"#);
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(v["trajectory"]["t"].as_array().unwrap().len(), 51);
        assert_eq!(v["trajectory"]["thrust"].as_array().unwrap().len(), 50);
    }

    #[test]
    fn matches_native_solve() {
        let v = plan(&example_request_json());
        let native = solve(&LandingProblem::example()).unwrap();
        let fuel = v["trajectory"]["fuel_used"].as_f64().unwrap();
        let tf = v["trajectory"]["final_time"].as_f64().unwrap();
        assert!((fuel - native.fuel_used()).abs() < 1e-9);
        assert!((tf - native.final_time).abs() < 1e-9);
        assert_eq!(v["vehicle"]["wet_mass"], 1905.0);
    }

    #[test]
    fn garbage_is_bad_request() {
        assert_eq!(kind(&plan("not json")), "BadRequest");
        assert_eq!(kind(&plan(r#"{"position":[1,2]}"#)), "BadRequest");
    }

    #[test]
    fn infeasible_kind() {
        let v = plan(r#"{"position":[2000,0,1500],"velocity":[0,0,-3000]}"#);
        assert_eq!(kind(&v), "Infeasible");
    }

    #[test]
    fn invalid_problem_kind() {
        let v = plan(r#"{"position":[0,0,-5],"velocity":[0,0,0]}"#);
        assert_eq!(kind(&v), "InvalidProblem");
        assert!(!v["error"]["message"].as_str().unwrap().is_empty());
    }

    #[test]
    fn nan_is_handled() {
        // NaN is not valid JSON.
        let v = plan(r#"{"position":[2000,0,1500],"velocity":[NaN,0,0]}"#);
        assert_eq!(kind(&v), "BadRequest");
        // Absurdly large values must fail cleanly, not panic.
        let v = plan(r#"{"position":[2000,0,1500],"velocity":[1e308,0,0]}"#);
        assert_eq!(v["ok"], false);
    }
}
