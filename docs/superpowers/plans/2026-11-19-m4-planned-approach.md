# M4.2 Planned final approach Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (owner chose native execution). Steps use checkbox (`- [ ]`) syntax.

**Goal:** From the burn's gate onto the arms, fly a re-planned optimal approach (Kestrel's own solver) with the base and nose kept off the mast; the scripted terminal controller becomes the fallback.

**Architecture:** `guidance::approach` poses a 3-DoF point-mass SOCP (20 intervals, fixed final time, shrinking horizon) and solves it with `socp::Solver`, set up once and updated after. `ShipFlight` re-plans it every 0.25 s in the terminal phase; `ship_control::approach` tracks it.

**Spec:** `docs/superpowers/specs/2026-11-19-m4-planned-approach-design.md`

## Global Constraints

- N = 20 intervals; re-plan every 0.25 s; final-time candidates {5, 7, 9, 12, 15} s; tilt 6°; funnel 0.5 + 0.6 h m; sink cone 1.0 + 0.4 h m/s; side cone 0.6 + 0.15 h m/s; contact sink 1.0 m/s; a_max 90 % of lit thrust, a_min 105 % of lit minimum; keep-out margin 1 m + 2 × navigation position error (per noise level, measured).
- With `planned_approach: false` the flight is bit-identical to before (A/B on one build).
- Default `cargo test` under two minutes; sweeps `#[ignore]`. Small commits; never stage `learn/`, `project-ideas.md`, `CLAUDE.md`. Edit files with Python (UTF-8, `newline=''`), never PowerShell Set-Content.

## Review Focus

1. The vehicle reaches the terminal phase far from the gate (a fallback "brake" arrival, the booster's high arrival): the approach must refuse cleanly and the fallback fly, not a wild plan.
2. A solve that fails mid-approach: the last plan or the fallback, never a stale plan far from the vehicle.
3. The plan's end in the last half second: no re-plan with a degenerate horizon.
4. `planned_approach: false` is exactly the old flight.
5. The worker boundary: the approach runs in the flight (not the worker), so nothing crosses it; the snapshot carries what the page draws.

---

### Task 1: Navigation error per noise level (measure first)
- [ ] Ignored test `nav_error_by_noise` in `ship_loop.rs`: RMS of (estimate − truth) position over the terminal phase, ship and booster, noise low/high, seeds 7-9. Record; set `NAV_ERROR_LOW/HIGH` constants in `ship_flight.rs` from it.

### Task 2: The approach problem and planner (`crates/guidance/src/approach.rs`)
- Produces: `ApproachProblem { r0, v0, z_catch, sink, t_f, a_min, a_max, keep_x, base_arm, nose_arm }`, `ApproachPlan { t_f, r: Vec<Vec3>, v: Vec<Vec3>, a: Vec<Vec3> }`, `ApproachPlanner::new()`, `plan(&mut self, &ApproachProblem) -> Result<ApproachPlan, GuidanceError>`, `plan_free(&mut self, p, &[f64]) -> Result<ApproachPlan, GuidanceError>`, `sample(&self, tau) -> (Vec3, Vec3, Vec3)`, `setups()`.
- [ ] Tests first: `reaches_the_arms_from_the_gate`, `respects_every_constraint`, `plans_from_off_to_the_side`, `refuses_a_mast_collision_course`, `reuses_its_set_up_solver`, `free_time_keeps_the_cheapest`.
- [ ] Implement; tests pass; clippy; commit `feat: approach planner (3-DoF SOCP on Kestrel's own solver)`.

### Task 3: Tracking (`ship_control::approach`)
- [ ] Test `tracks_an_approach_plan` (shared model, no error: within 0.5 m of the plan's end, caught limits met).
- [ ] Implement; commit.

### Task 4: In the flight
- [ ] Tests first: `planned_approach_off_matches_today` (bit-exact numbers as in M4.1), `earth_planned_approach_is_caught`, `booster_planned_approach_is_caught`.
- [ ] `ShipFlightConfig.planned_approach`, the planner in `ShipFlight`, re-plan logic, fallback; commit.

### Task 5: Page
- [ ] Snapshot `approach` (positions, t_f, mode); config `planned_approach`; record decisions; dock switch; analysis nodes. Web tests; commit.

### Task 6: Measure and tune
- [ ] A/B board (approach off/on), wind sweep, smoothness stats (gate→catch time, peak sideways acceleration, contact speeds); tune one constant at a time; commit with numbers.

### Task 7: Docs
- [ ] D-047, ARCHITECTURE, LIMITATIONS, ROADMAP (4.2 ticked), `learn/` chapter 20; commit.
