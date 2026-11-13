# M2 Closed Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fly the booster in a simulator with noisy sensing, MPC re-planning, a tracking controller, "break it" disturbances and a closest-landing fallback, with live panels.

**Architecture:** New `kestrel-sim` crate (truth, no guidance dependency); `guidance` gains `track` (controller), `replan` (warm-started re-plan) and a "closest" SOCP target; `wasm` exposes a `Flight` class (main thread) and `replan` (worker). The page steps `Flight` per frame and asks the worker for a re-plan every 0.25 s of sim time.

**Tech Stack:** Rust 2024, Clarabel 0.11.1, wasm-bindgen 0.2.130, Next.js 16, R3F 9, zustand 5, Vitest 5, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-14-m2-closed-loop-design.md`

## Global Constraints

- Simulation only; say so in UI and docs.
- `kestrel-sim` must not depend on `kestrel-guidance`.
- No new runtime dependencies in Rust (`sim` has none); no new npm dependencies.
- JSON across the WASM boundary; never panic or throw across it (D-009).
- Whole test suite < 2 min; slow tests `#[ignore]`.
- Numbers in LIMITATIONS.md only when measured, with method.
- Never stage `learn/` or `project-ideas.md`.
- Run cargo/npm from PowerShell.

## Review Focus

- Fuel runs out mid-flight → engine off, vehicle falls, outcome Crashed (no NaN, no panic). Test in Task 1 and Task 5.
- Vehicle pushed below the glide cone or past it → re-plan returns `closest`, not an error. Test in Task 4.
- Re-plan reply arrives after touchdown or after Reset → ignored. Test in Task 6.
- Re-plan with remaining mass ≈ dry mass (wet ≤ dry after relabel) → clean failure kind, controller brakes. Test in Task 4.
- Tab hidden / huge frame dt → sim advances at most 0.1 s × speed per frame. Test in Task 6.

---

### Task 1: `kestrel-sim` crate

**Files:** Create `crates/sim/{Cargo.toml,src/lib.rs,src/rng.rs,src/engine.rs,src/sensor.rs,src/sim.rs}`; modify root `Cargo.toml` members.

**Interfaces — Produces:**
- `rng::Rng { new(seed: u64), next_u64(), uniform() -> f64, gaussian() -> f64 }` (SplitMix64 + Box–Muller).
- `engine::Engine { new(min: f64, max: f64), failed: u8 (≤3), lag: f64, thrust: Vec3, limits() -> (f64, f64), fail_one() -> bool, step(cmd: Vec3, dt) }` — first-order lag toward `cmd`, then magnitude clamped to `limits()`; zero command stays zero (engine off).
- `sensor::Noise { pos_sigma, vel_sigma }`, `Noise::{OFF, LOW (2, 0.2), HIGH (8, 0.8)}`; `Sensor { noise, rng, measure(&Truth) -> Measurement { t, position, velocity, mass } }`.
- `sim::Params { gravity, dry_mass, alpha, min_thrust, max_thrust }`; `Truth { t, position, velocity, mass, landed: bool }`; `Gust { start, dir: f64 (±1), magnitude }` with force profile (2 s, 0.3 s ramps); `Sim { new(params, r, v, m), truth, engine, gusts, step(cmd: Vec3, dt) }` RK4 in 10 ms substeps; engine off when `mass ≤ dry_mass` or `landed`; touchdown when `z ≤ 0` sets `landed` and records `touchdown_velocity`.

- [ ] Tests (in-module): `free_fall_matches_formula`, `hover_holds_still` (cmd = m·g up, 10 s, |Δr| < 1e-6·…), `mass_follows_rocket_equation` (Δv vs `v_e ln(m0/m1)` in zero g < 1e-6 rel), `lag_step_reaches_63_percent_at_tau`, `engine_out_scales_limits` (5/6, 4/6, 3/6, no 4th), `gust_impulse` (∫F dt ≈ magnitude·(2 − 0.3) within 1 %), `noise_mean_and_sigma` (10⁵ samples, |mean| < 0.05σ, σ within 2 %), `same_seed_same_noise`, `empty_tank_engine_off`, `touchdown_latches`.
- [ ] Run `cargo test -p kestrel-sim` — fail, implement, pass.
- [ ] Commit `feat: kestrel-sim crate (truth dynamics, engine, gusts, sensor noise)`.

### Task 2: Tracking controller

**Files:** Create `crates/guidance/src/track.rs`; modify `lib.rs`, `problem.rs` (add `LandingProblem::from_state(vehicle, position, velocity, mass, steps)` setting `wet_mass = mass`).

**Interfaces — Produces:** `track::Gains { kp: 0.25, kd: 0.9 }` (Default); `track::command(plan: Option<&Trajectory>, tau: f64, position, velocity, mass, gravity, max_tilt, limits: (f64, f64), gains) -> Vec3`. Plan reference at `tau` uses the same exact constant-acceleration interpolation as `web/src/lib/playback.ts`; past the end / no plan → vertical descent `v_z = −max(0.5, 0.3·z)`; result tilt-clamped to `max_tilt` and magnitude clamped to `limits`. Also `track::brake(velocity, max_tilt, limits) -> Vec3` (max thrust against velocity, tilt-clamped).

- [ ] Tests: `zero_error_returns_plan_thrust` (example plan, τ = 3.3, < 1e-6 rel), `position_error_pushes_back`, `tilt_is_clamped`, `magnitude_is_clamped_both_ways`, `after_plan_descends_vertically`, `brake_opposes_velocity`, `from_state_relabels_wet_mass`.
- [ ] Commit `feat: tracking controller for flying a plan between solves`.

### Task 3: Closest-landing SOCP target

**Files:** Modify `socp.rs` (add `Target::{Pinpoint, Closest}` param to `build_with`; Closest: drop glide cones and `r_N,x/y = 0`, add variable `e ≥ ‖r_N,xy‖` at index `layout.len()`, objective `W·e − z_N` with `W = 10`), `solve.rs` (`pub fn solve_closest(problem, final_time) -> Result<Trajectory, GuidanceError>`; validation must accept states outside the glide cone for this target — add `LandingProblem::validate_state()` without the cone check).

- [ ] Tests (`tests/closest.rs`): `pinpoint_reachable_gives_zero_miss` (example, t = 40 s, miss < 1 m), `too_little_propellant_lands_short_at_rest` (example, dry mass 1880 kg: `solve` → Infeasible; closest → v_N ≈ 0, z_N ≈ 0, miss > 10 m, final mass ≥ dry), `outside_glide_cone_still_lands` (r = [3000,0,100], v = [-20,0,-5]).
- [ ] Commit `feat: closest-landing fallback problem`.

### Task 4: Warm-started re-plan

**Files:** Create `crates/guidance/src/replan.rs`; export from `lib.rs`.

**Interfaces — Produces:** `enum Mode { Pinpoint, Closest }`; `struct Replan { trajectory, mode, miss: f64, solves: usize }`; `fn replan(problem: &LandingProblem, hint: Option<f64>) -> Result<Replan, GuidanceError>` exactly as spec §Re-plan (h = clamp(0.05·hint, 0.25, 2); Closest grid = 8 times evenly in `[t_min, t_max]` using the same bounds as `solve`).

- [ ] Tests: `warm_matches_full_search` (hint = full optimum + 1.5 s: fuel within 1 %; solves = 3), `no_hint_uses_full_search`, `bad_hint_falls_back`, `outside_cone_goes_closest`, `nearly_empty_tank_fails_cleanly` (mass = dry + 0.01 → Err, no panic), `hopeless_is_error` (v = [0,0,-3000]).
- [ ] Commit `feat: warm-started re-plan with closest-landing fallback`.

### Task 5: WASM `Flight` + `replan` + native closed loop

**Files:** Create `crates/wasm/src/flight.rs`, `crates/wasm/tests/closed_loop.rs`; modify `crates/wasm/src/{lib.rs,dto.rs}`, `crates/wasm/Cargo.toml` (+ `kestrel-sim`).

**Interfaces — Produces (JS-visible):**
- `replan(json) -> json`: request `{position, velocity, mass, vehicle, steps?, hint?}` → `{ok:true, mode:"pinpoint"|"closest", trajectory, miss, solves}` | failure.
- `class Flight`: `new(json {position, velocity, vehicle, seed, lag, noise: "off"|"low"|"high"})`, `step(dt)`, `measure() -> json {t, position, velocity, mass, vehicle}` (vehicle limits already reduced for failures), `set_plan(json replanResponse, t_measured)`, `clear_plan()` (no plan → brake if `mode none`), `gust(dir)`, `engine_out() -> bool`, `set_lag(s)`, `set_noise(level)`, `grab(x, z)` / `release()`, `snapshot() -> json {t, position, velocity, mass, thrust, command, engines, landed, outcome?: {landed: bool, speed, vertical, horizontal, miss}, plan_tau, plan_t0}`.
- Rust-side `Flight` is a plain struct usable natively; the `#[wasm_bindgen]` wrapper is thin.
- Outcome rule: Landed iff `|v_z| ≤ 2` and horizontal speed ≤ 1 m/s at touchdown.

- [ ] Tests (`closed_loop.rs`, helper `fly(scenario) -> Outcome` that measures every 0.25 s, calls `replan` synchronously and installs the plan one period later; stops re-planning when < 2 s remain): `nominal_lands_on_pad` (speed < 1, miss < 2 m), `noise_and_lag_still_land` (high, 1 s), `two_gusts_still_land`, `engine_out_midflight_lands`, `two_engines_out_lands_or_closest`, `fuel_starved_start_lands_closest_reports_miss`, `three_engines_out_early_crashes_without_panic`. `#[ignore] monte_carlo` (50 seeds × noise high).
- [ ] Run, record outcomes for LIMITATIONS.
- [ ] Commit `feat: Flight closed loop and replan in the wasm bridge` (+ test commit if separate).

### Task 6: Worker re-plan protocol + MPC scheduler (TS)

**Files:** Modify `web/src/lib/{solverProtocol.ts,solverClient.ts,types.ts}`, `web/src/workers/solver.worker.ts`; create `web/src/lib/mpc.ts` + test, `web/src/lib/flightTypes.ts`.

**Interfaces — Produces:** `client.replan(req: ReplanRequest): Promise<{response: ReplanResponse, ms}>`; `createMpc({ period: 0.25, freezeBefore: 2 })` with `shouldRequest(simT, planRemaining): boolean`, `sent(simT)`, `received(epoch): boolean` (false for a stale epoch after reset/touchdown), `reset()`, `clampDt(dt, speed)` (≤ 0.1 × speed).
- [ ] Vitest: period respected, one in flight, frozen near end, stale epoch rejected, dt clamp.
- [ ] Commit `feat: worker replan message and MPC scheduler`.

### Task 7: Fly the closed loop in the page

**Files:** Create `web/src/lib/flightRuntime.ts` (loads the WASM on the main thread, owns the `Flight`), `web/src/hooks/useFlight.ts`; modify `state.ts` (flight phase holds `{mode, outcome, solves[], engines, lag, noise}`; actions `flightStarted`, `planInstalled`, `touchdown`), `FlightLoop.tsx` (step Flight, write `live.sample` from snapshot, add `live.plan`), `PlannedPath.tsx` (draw `live.plan` during flight), `KestrelApp.tsx`.
- [ ] Reducer tests updated/added; `npm test`, `npm run typecheck`, `npm run lint`.
- [ ] Manual: dev server, press Land, booster lands in closed loop; screenshot.
- [ ] Commit `feat: closed-loop flight in the browser`.

### Task 8: Break-it controls, margins, banners, outcome, trail

**Files:** Create `web/src/components/hud/{BreakIt.tsx,Margins.tsx,ModeBanner.tsx}`, `web/src/components/scene/Trail.tsx`, `web/src/lib/margins.ts` + test; modify `Hud.tsx`, `hud.module.css`, `DragHandles.tsx` (grab during flight), touchdown card → outcome card.
- [ ] `margins.test.ts`: glide margin at pad/edge, tilt margin, thrust % of available.
- [ ] Manual screenshots of each disturbance.
- [ ] Commit(s) `feat: break-it controls`, `feat: margins panel and mode banner`, `feat: flown-path trail and outcome card`.

### Task 9: Measure, e2e, docs, learn

- [ ] e2e: land → gust → outcome card shows Landed. `measure.spec.ts`: re-plan ms median/range during a flight.
- [ ] LIMITATIONS, ARCHITECTURE (sim built; loop described), DECISIONS (D-012 sim on main thread; D-013 warm-start 3-point re-plan; D-014 closest fallback objective; D-015 seeded noise), ROADMAP ticks, README, glossary; `learn/` M2 chapter (local).
- [ ] Commit `docs: M2 ...`.
