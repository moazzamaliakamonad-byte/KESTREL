# M4.1 Wind awareness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Guidance estimates the horizontal wind in flight from its sensors and uses it in the planner, the controllers and its predictions.

**Architecture:** A 2-state extended Kalman filter (`guidance::wind`) turns the accelerometers' reading, engine telemetry and the shared aerodynamic model into a wind estimate with 1σ. `ShipFlight` updates it every control step and hands it to `ShipProblem` (the planner's dynamics) and to the controllers through an `Air` value. Everywhere the wind enters as *model with wind minus model without*, so at a zero estimate guidance is exactly today's code.

**Tech Stack:** Rust 2024 (crates `model`, `sim`, `guidance`, `wasm`), wasm-bindgen 0.2.130, Next.js/TypeScript (web), Vitest.

**Spec:** `docs/superpowers/specs/2026-11-18-m4-wind-awareness-design.md`

## Global Constraints

- Simulation only; say so in every user-facing text that matters.
- Default `cargo test` under two minutes; sweeps are `#[ignore]`.
- No unmeasured claims: every number goes to LIMITATIONS.md with its method.
- Thrust telemetry error taken as 1 % of thrust (*illustrative*).
- Estimate clamped to ±60 m/s; initial 1σ 15 m/s; settled when 1σ < 2 m/s.
- Gust record entry: the estimate moves more than 4 m/s within 3 s, at most one per 4 s.
- Wind sweep: ship and booster; 0, 5, 10, 15, 20, 25 m/s; from +x, −x, +y; noise low and high; seeds 7, 8, 9.
- Small commits, conventional prefixes; never stage `learn/`, `project-ideas.md`, `CLAUDE.md`. Run cargo/npm from PowerShell.

## Review Focus

1. **Still air with noise:** the estimate wanders a few m/s from noise alone; guidance must not get worse in still air → the boards (12/12, 20/20, seeds 11-30) are rerun in Task 7.
2. **Engine state changes** (ignition, failure, relight): telemetry vs truth mismatch for a step must not throw the estimate → Task 2 test `thrust_step_does_not_throw_the_estimate`.
3. **Near hover** the wind is nearly unobservable: the estimate must coast, not drift → Task 2 test `coasts_when_the_air_is_still_relative`.
4. **wind_aware = false** must fly exactly today's flight → Task 3 test `unaware_flight_matches_today`.
5. **Grab and drop** on the page: the estimate survives `grab`/`release` → Task 3 test.

---

### Task 1: Baseline wind sweep (before any change)

**Files:**
- Modify: `crates/wasm/tests/ship_loop.rs` (add `wind_sweep`, `#[ignore]`)

**Interfaces:**
- Produces: `fn wind_sweep()` printing one line per flight and a summary table: kind, wind speed, from, noise, seed, outcome, flight time, propellant left (t), sideways error at the approach gate (m, `|r_xy − gate_xy|` when the phase first becomes `Terminal`, or "—"), sideways speed there (m/s). Later tasks add the estimate's error at ignition.

- [ ] **Step 1:** Write `wind_sweep`: for each (kind, speed, from, noise, seed), build `ShipFlightConfig { seed, lag: 0.3, noise, wind, ..new(kind) }` where `wind` = −speed·(unit vector of "from"); fly with the existing `request`/`deliver` loop; record the numbers above. Run the cells on `std::thread::scope` threads (one per (kind, noise), 4 threads) and print sorted.
- [ ] **Step 2:** Run `cargo test --release -p kestrel-wasm --test ship_loop wind_sweep -- --ignored --nocapture > target/wind-baseline.txt` (background). Expected: completes; table printed.
- [ ] **Step 3:** Commit the test (`test: wind sweep (ignored)`); keep `target/wind-baseline.txt` for Task 7.

### Task 2: The wind filter

**Files:**
- Create: `crates/guidance/src/wind.rs`; Modify: `crates/guidance/src/lib.rs` (`pub mod wind;`)

**Interfaces:**
- Produces:
  - `pub struct WindInputs<'a> { pub scn: &'a Scenario, pub x: &'a State, pub dv: Vec3, pub h: f64, pub thrust_body: Vec3, pub surfaces: [f64; FLAPS], pub att_sigma: f64, pub vel_sigma: f64 }` (x = navigation estimate after the step; dv = accelerometers' Δv over the step; thrust and surfaces as used during the step).
  - `pub struct WindFilter` with `new() -> WindFilter`, `update(&mut self, i: &WindInputs)`, `estimate(&self) -> WindEstimate`, `wind(&self) -> Vec3` (z = 0).
  - `pub struct WindEstimate { pub w: [f64; 2], pub sigma: [f64; 2], pub innovation: Vec3 }`.
  - Constants: `INITIAL_SIGMA = 15.0`, `WALK` (m/s/√s, start 3.0, tuned in Task 7), `THRUST_TELEMETRY = 0.01`, `FORCE_FLOOR` (N, start 2e3), `LIMIT = 60.0`.
- Algorithm: predict `P += WALK²·h·I`. Measurement `z = m·(dv/h − g_vec)`. Prediction `hx(w, q, v) = body_force(ρ, q, v − w).0 + surfaces(ρ, q, v − w) + rotate(q, thrust_body)` where `surfaces` is `flap_force` (ship) or `fin_force` (booster). `H = ∂hx/∂w` (3×2, central differences, 0.5 m/s). `R = Jq·att_sigma²·Jqᵀ + Jv·vel_sigma²·Jvᵀ + (THRUST_TELEMETRY·|T|)²·t̂t̂ᵀ + FORCE_FLOOR²·I` with `Jq` from small body-axis rotations (3 columns) and `Jv` from velocity perturbations. Update with `S = HPHᵀ + R`, `K = PHᵀS⁻¹` (3×3 inverse by cofactors), Joseph form for `P`, then clamp `w` to ±LIMIT and symmetrize `P`. Skip the update when `h <= 0` or ρ·|v|² is tiny (< 1 Pa·2), predict only.

- [ ] **Step 1: Failing tests** in `wind.rs`, using a local truth integrator like `ship_control::tests::fly` (shared model as truth with `Extra { wind }`):
  - `converges_in_a_steady_crosswind`: booster default start, true wind (0, −15), noise off, 0.01 s steps, engines off, fins neutral: after 10 s `|w − truth| < 1.0` and both sigmas < 2.0.
  - `ship_belly_flop_converges`: ship default start, wind (10, 5): after 10 s error < 1.0.
  - `follows_a_step_change`: steady 0 for 5 s then (−10, 0): within 4 s of the step, error < 3.0.
  - `coasts_when_the_air_is_still_relative`: dv chosen so ρ|v|² ≈ 0 (hover, v = 0, z = 1 000 m, thrust = weight): 30 s of updates move a converged estimate (8, 0) by < 0.5.
  - `thrust_step_does_not_throw_the_estimate`: converged at (5, 0), one step whose measured dv corresponds to 30 % less thrust than telemetry: estimate moves < 2.0.
  - `covariance_stays_symmetric_positive`: 10 000 random-input updates: P symmetric to 1e-9, eigenvalues > 0.
- [ ] **Step 2:** `cargo test -p kestrel-guidance wind` → FAIL (module missing).
- [ ] **Step 3:** Implement as above.
- [ ] **Step 4:** `cargo test -p kestrel-guidance wind` → PASS; clippy clean.
- [ ] **Step 5:** Commit `feat: wind filter (2-state EKF on the aerodynamic force)`.

### Task 3: The filter in the flight

**Files:**
- Modify: `crates/wasm/src/ship_flight.rs` (`ShipFlightConfig.wind_aware: bool` default true; `ShipFlight.wind: WindFilter`; update in `control_step` after `sim.step`; `pub fn wind_estimate(&self) -> WindEstimate`; `pub fn air(&self) -> Air` = estimate if `wind_aware`, else still air)
- Test: `crates/wasm/tests/ship_loop.rs`

**Interfaces:**
- Consumes: Task 2 `WindFilter`, `WindInputs`.
- Produces: `ShipFlight::wind_estimate()`, `ShipFlight::air() -> guidance::ship_control::Air` (`Air` defined in Task 5; in this task add `pub struct Air { pub wind: Vec3 }` with `Air::STILL` to `ship_control.rs` unused yet).
- The inputs: `dv = imu_dv`, `h = imu_h`, `thrust_body = sim.cluster.thrust` and `surfaces = sim.flaps` read *before* `sim.step` (what acted during it), `x = est` after navigation of the next step is not available, so use the estimate carried forward (`estimate_now()`), `att_sigma = att_sigma().0`, `vel_sigma` from `noise`.

- [ ] **Step 1: Failing tests:** `estimate_settles_in_flight` (booster, wind (0, −15), noise low: by ignition the estimate is within 2 m/s); `unaware_flight_matches_today` (wind 0 noise low, `wind_aware: false` → same outcome, same t to 1e-9, same propellant as a flight built without the field set… i.e. today's code path); `estimate_survives_grab` (grab, release: estimate unchanged).
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p kestrel-wasm` → PASS (whole suite, time noted).
- [ ] **Step 5:** Commit `feat: the flight carries a wind estimate`.

### Task 4: The planner knows the wind

**Files:**
- Modify: `crates/guidance/src/scvx/mod.rs` (`ShipProblem.wind: Vec3`, default zero), `crates/guidance/src/scvx/discretize.rs` (`f_nd` and the Jacobians pass `Extra { wind, ..Default::default() }`; thread `wind` from the problem), `crates/wasm/src/ship_flight.rs` (`problem()` sets `p.wind = self.air().wind` on every option)

**Interfaces:**
- Produces: `ShipProblem.wind` (inertial, m/s, z = 0).

- [ ] **Step 1: Failing tests** (`scvx` tests): `wind_changes_the_dynamics` (f_nd with wind (10, 0, 0) differs from still air on the velocity rows for the ship's belly attitude); `zero_wind_plan_is_unchanged` (a default ship plan with `wind = 0` equals the plan before the field existed — compare to the existing test's numbers, final time to 1e-12).
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p kestrel-guidance scvx` → PASS.
- [ ] **Step 5:** Commit `feat: the planner flies in the estimated wind`.

### Task 5: The controllers know the wind

**Files:**
- Modify: `crates/guidance/src/ship_control.rs`, call sites in `crates/wasm/src/ship_flight.rs`

**Interfaces:**
- Produces: `pub struct Air { pub wind: Vec3 }`, `Air::STILL`; new last parameter `air: &Air` on `descent`, `belly_flop`, `unpowered`, `powered`, `flip`, `steady_burn`, `terminal`. Helper `fn wind_force(scn, x, air) -> (Vec3, Vec3)` = body (+ surfaces at trim) force and torque at `v − w` minus at `v`.
- Behaviour (each zero when `w = 0`):
  - `aero_torque` adds the wind's torque difference.
  - `descent`: `a_want` minus the horizontal wind force difference / m at the commanded lean (one fixed-point pass); `back` stays along `−v` (the lean already measures from it).
  - `belly_flop`: predicted position `r + v·t_go + w·(t_go − τ(1 − e^(−t_go/τ)))`, with `τ = m / (∂F_x/∂v_x)` from the shared model at the belly attitude (finite difference), floored at 1 s.
  - `fly_force` callers (`powered`, `terminal`, `flip`, `steady_burn`): the wanted inertial force minus the wind force difference.

- [ ] **Step 1: Failing tests:** `descent_leans_into_a_crosswind` (booster at the aim, no position error, wind (0, 15): the commanded axis tilts so the lift points −y… i.e. against the push); `belly_flop_aims_upwind` (wind toward +x: commanded pitch slides the ship toward −x compared with still air); `terminal_leans_into_the_wind_at_once` (hovering on the axis, wind (10, 0): first-step thrust has a −x component); `still_air_is_unchanged` (each controller with `Air::STILL` returns exactly the old output on a fixed state — snapshot the old values first).
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement; pass `self.air()` at every call site.
- [ ] **Step 4:** `cargo test --workspace` → PASS.
- [ ] **Step 5:** Commit `feat: the controllers lean into the estimated wind`.

### Task 6: Predictions in the wind, and crosswind flights in the suite

**Files:**
- Modify: `crates/wasm/src/ship_flight.rs` (`predict(scn, x, thrust, seconds, wind)`; callers pass the estimate), `crates/wasm/tests/ship_loop.rs`

- [ ] **Step 1: Failing tests:** `earth_crosswind_is_caught` and `booster_crosswind_is_caught` (15 m/s from +y, noise low, seed 7; caught). Run first against the baseline numbers of Task 1 to confirm they are meaningful (if the baseline already catches them, raise to the lowest speed the baseline lost, and say so in the test's comment).
- [ ] **Step 2:** Implement `predict` with wind.
- [ ] **Step 3:** `cargo test --workspace` → PASS; note the suite time (< 2 min).
- [ ] **Step 4:** Commit `test: crosswind flights caught`.

### Task 7: Measure, tune, compare

- [ ] **Step 1:** Rerun `wind_sweep` → `target/wind-after.txt`; add the estimate's error at ignition to its output.
- [ ] **Step 2:** Rerun the boards: `trace_earth` 12-flight board, `engine_out_window`, `booster_browser_sweep`, the seeds 11-30 runs, `ship_loop` defaults. Compare with LIMITATIONS.md.
- [ ] **Step 3:** If a pass criterion fails, tune (`WALK`, `FORCE_FLOOR`, the feed-forward), one constant at a time, rerunning the failing board; log each change. Stop when the criteria pass or the cause is understood and written down.
- [ ] **Step 4:** Commit any tuning (`perf:` / `fix:`) with before/after in the message.

### Task 8: The page

**Files:**
- Modify: `crates/wasm/src/ship_js.rs` (snapshot gets `windEst: [x, y]`, `windSigma: [x, y]`, `windTrue: [x, y]`; config accepts `windAware`), `web/src/lib/ship/types.ts`, `store.ts` (option `windAware`, default true, in `configOf`), `record.ts` (decisions "wind estimated" on settling, "gust felt" per the constraint), `phrases.ts` + `commentary.ts` (two lines), `components/console/Telemetry.tsx` (row "Wind (est.)": speed, from-direction, ±1σ), `components/console/Dock.tsx` (Environment: "Wind awareness" switch), `components/stage/Analysis.tsx` (two arrows, "estimated" and "true wind — guidance does not know this"), `lib/ship/parts.ts` if a part sheet shows aerodynamics (its formulas use the airspeed with the estimate).
- Tests: `web/src/lib/ship/record.test.ts` (settle and gust decisions from a synthetic stream), a formatting test for the wind row.

- [ ] **Step 1:** Failing web tests → `npm test` FAIL.
- [ ] **Step 2:** Implement the bridge and the page.
- [ ] **Step 3:** `npm run wasm; npm run lint; npm run typecheck; npm test` → PASS.
- [ ] **Step 4:** Screenshot a 15 m/s crosswind booster flight in the analysis view (`scripts/screenshot.mjs`) and look at it.
- [ ] **Step 5:** Commit `feat: the page shows guidance's wind estimate`.

### Task 9: Voice clips

- [ ] **Step 1:** Install `kokoro-js` and `@breezystack/lamejs` in a scratch folder; run `web/scripts/make-voice.mjs`; check the new clips exist and `manifest.json` lists them.
- [ ] **Step 2:** Commit `chore: voice clips for the wind lines`.

### Task 10: Docs

- [ ] D-046 in DECISIONS.md (the filter, its inputs, the "difference" rule, tuning with numbers); ARCHITECTURE.md (`guidance::wind`, the data flow line); LIMITATIONS.md (wind section rewritten: the sweep before/after, the estimator's accuracy, thrust telemetry assumption, wind constant with height); ROADMAP.md (M4 section, three sub-projects, 4.1 ticked); README status line; `learn/` chapter 19 (Kalman filter from scratch, observability, exercises; local only).
- [ ] Commit `docs: M4.1 wind awareness (D-046), measured`.
