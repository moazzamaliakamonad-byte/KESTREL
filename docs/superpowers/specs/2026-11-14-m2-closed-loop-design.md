# M2 — Closed loop and "break it": design

Date: 2026-11-14. Status: approved (decisions delegated by the owner).
Roadmap: `docs/ROADMAP.md` M2.

## Goal

Press **Land** and the booster now flies in a **physics simulator**, not a
playback. Guidance only sees noisy sensor readings, re-plans a few times a
second in the worker, and a tracking controller flies the latest plan between
solves. The visitor can break things mid-flight — grab the booster, gust,
kill engines, add thrust lag, add sensor noise — and watch guidance recover,
or fall back to "land as close as possible" and say so. Panels show fuel,
thrust, constraint margins and solve time live.

## Non-goals

- No attitude dynamics, aerodynamics or 6-DoF (M3).
- No state estimator (Kalman filter): the controller and guidance use raw noisy
  measurements. Engine failures are known to guidance instantly (perfect fault
  detection).
- Gusts are an illustrative sideways force, not a Mars atmosphere model.
- No out-of-plane editing (start still edited in the x–z plane); disturbances
  may push the vehicle out of plane and guidance handles full 3D.

## Architecture

```text
crates/sim       NEW. Truth only; does not depend on guidance.
                 state (r, v, m) · engine (lag, failed engines, limits) ·
                 disturbance force (gusts) · sensor (seeded Gaussian noise) · RK4 step
crates/guidance  + track.rs   tracking controller: plan + measured state → thrust command
                 + replan.rs  warm-started re-plan: 3 solves near the expected landing
                              time, else full search, else closest-landing fallback
                 + socp: "closest" target (free horizontal end point, no glide cone,
                   minimise miss distance then fuel)
crates/wasm      + Flight (wasm-bindgen class): sim + sensor + controller + current plan;
                   step(dt), measure(), set_plan(), disturbance setters, snapshot()
                 + replan(json) for the worker
                 + tests/closed_loop.rs: native closed-loop scenarios (the loop the page runs)
web/             Flight lives on the main thread (same .wasm, second instance);
                 the solver worker gains a `replan` message.
                 lib/mpc.ts: when to re-plan (sim-time period, one solve in flight)
                 BreakIt panel, Margins panel, flown-path trail, outcome card
```

Why the sim runs on the main thread: it is cheap (point mass, RK4 at 100 Hz)
and must advance every frame; guidance is slow and runs asynchronously in the
worker. The latency between measuring and receiving a plan is therefore real,
as it would be on a vehicle. A plan is time-stamped with the sim time of the
measurement it was solved from, so the controller reads the plan at
`τ = t − t_measured`; latency is compensated by construction.

## Loop, per frame (page)

1. `flight.step(frameDt × speed)` → fixed 10 ms substeps: sensor → controller
   (current plan) → engine (lag, limits) → dynamics with disturbance force.
2. If no solve is in flight and ≥ 0.25 s of sim time passed since the last
   request, `flight.measure()` (noisy r, v, m and the sim time) → worker
   `replan` with the last landing-time estimate as a hint.
3. On reply, `flight.set_plan(plan, t_measured)`; record solve ms and mode.
4. `flight.snapshot()` → `live` for the scene and HUD.
5. Touchdown (z ≤ 0): outcome **Landed** if vertical speed ≤ 2 m/s and
   horizontal ≤ 1 m/s, else **Crashed**; report miss distance from the pad.

Re-planning stops when the plan has < 2 s to go (tiny horizons are near
vertical, where lossless convexification is not guaranteed; see M0).

## Re-plan (`guidance::replan`)

Input: problem from the measured state (wet mass ← current mass, limits
reduced for failed engines), optional hint `t_hint`.

1. **Warm start** (if hint): fixed-time solves at `t_hint − h, t_hint,
   t_hint + h`, `h = clamp(0.05·t_hint, 0.25, 2) s`, dry mass enforced; best
   feasible wins. Successive re-plans walk toward the optimum.
2. **Full search** (M0 `solve`) if none of those worked.
3. **Closest landing** if that is infeasible or the state is outside the glide
   cone: drop the glide cone and the horizontal end point; minimise
   `W·‖r_N,xy‖ − z_N` (miss first, then fuel) on 8 landing times; keep the
   best. Reported as mode `closest` with the miss distance.
4. Otherwise mode `none`: the controller brakes (full thrust against the
   velocity, within the tilt limit) and the page says a crash is unavoidable.

Output: `{ok, mode: "pinpoint"|"closest", trajectory, miss, solves}` or a
failure.

## Tracking controller (`guidance::track`)

`a_cmd = u_plan(τ) + Kp·(r_plan(τ) − r̂) + Kd·(v_plan(τ) − v̂)` with
`Kp = 0.25 s⁻²`, `Kd = 0.9 s⁻¹` (ω ≈ 0.5 rad/s, ζ ≈ 0.9); thrust
`T = m̂·a_cmd`, then tilt clamped to `max_tilt` and magnitude clamped to the
**available** `[T_min, T_max]`. Past the plan's end (or with no plan): descend
vertically at `v_z = −max(0.5, 0.3·z)` m/s above the current point.

## Simulator (`kestrel-sim`)

- State `r, v, m`; `ṙ = v`, `v̇ = T/m + F_ext/m − g ẑ`, `ṁ = −α‖T‖`. RK4,
  10 ms. Engine off when the tank is empty or after touchdown.
- Engine: six engines; each failure scales both thrust limits by
  `(6 − k)/6` (k ≤ 3). Thrust vector follows the command with first-order lag
  `τ_lag ∈ {0, 0.3, 1.0} s`, then clamped to the available limits.
- Gust: sideways force `0.15·T_max` (≈ 2 kN) for 2 s with 0.3 s ramps,
  left or right (illustrative).
- Sensor: Gaussian noise, levels off / low / high: position σ 0 / 2 / 8 m,
  velocity σ 0 / 0.2 / 0.8 m/s; mass exact. SplitMix64 + Box–Muller, seeded,
  so runs are reproducible.
- Grab: while the visitor holds the booster, sim time is frozen and position
  follows the pointer; release resumes with the old velocity and forces an
  immediate re-plan.

## UI

- **Break it** panel (flight only): Gust ← / Gust →, Engine out (shows
  engines 6/6 → 3/6), Thrust lag Off/0.3 s/1 s, Sensor noise Off/Low/High.
  Lag and noise can also be preset before Land.
- **Margins** panel: glide-slope margin (elevation − γ, degrees), tilt margin
  (limit − actual thrust tilt), thrust as % of available max with the band,
  propellant left, solve time (last and sparkline) and re-plan rate (Hz),
  guidance mode badge.
- Banner when mode is `closest`: "No pinpoint landing from here — guiding to
  the closest reachable point (≈ N m from the pad)"; when `none`: "No safe
  landing exists — braking".
- Scene: planned path = latest plan from the vehicle's position; a trail of
  the actually flown path; booster, plume and dust driven by the **actual**
  thrust.
- Outcome card: Landed / Crashed, touchdown speed, miss distance, propellant
  used, re-plans and median solve time.

## Testing

- `sim`: free fall, hover equilibrium, mass vs rocket equation, lag step
  response (63 % at τ), engine-out limits, gust impulse, noise mean/σ and
  seed determinism.
- `guidance`: controller (zero error → plan thrust; clamps), warm re-plan fuel
  within 1 % of the full search on the example, closest landing outside the
  glide cone and with too little propellant (lands at v = 0, miss > 0).
- `wasm/tests/closed_loop.rs` (native, same loop as the page, solve latency
  modelled as one re-plan period): nominal; high noise + 1 s lag; two gusts;
  one and two engines out mid-flight; a start where only closest landing works.
  Assertions on outcome, touchdown speed and miss. A Monte-Carlo sweep is
  `#[ignore]`.
- Web: Vitest for `mpc.ts` scheduling, margin maths, reducer changes;
  Playwright e2e extended to "land, gust, still lands".

## Measured claims to record (LIMITATIONS.md)

Re-plan time and rate in the browser; closed-loop touchdown speed and miss for
each native scenario (seeded, so reproducible).
