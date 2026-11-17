# Architecture

Status: M0–M2 are **built**: guidance, simulator, WASM bridge and the
browser page with the closed loop. M3 (6-DoF) is _(planned)_.

## Big picture

```text
 ┌──────────────────────────── browser tab ────────────────────────────┐
 │                                                                     │
 │  UI / 3D view (JS/TS)  ── user interference ──▶  Simulator (Rust)   │
 │        ▲                                            │ true state    │
 │        │ planned trajectory,                        ▼               │
 │        │ margins, solve time                  Sensor model          │
 │        │                                            │ noisy state   │
 │        └───────────── Guidance (Rust → WASM) ◀──────┘               │
 │                       formulate SOCP → Clarabel → plan              │
 │                              │                                      │
 │                              ▼                                      │
 │                     Tracking controller ──▶ thrust command ──▶ Sim  │
 └─────────────────────────────────────────────────────────────────────┘
```

Key separation: **the simulator owns the truth; guidance only sees what the
sensors report.** This keeps the demo honest: if guidance looks good, it is
not because it secretly read the true state.

## Components

| Component | Responsibility | Milestone |
| --- | --- | --- |
| `guidance` crate | Build and solve the convex landing problem; return a trajectory | M0 — **built** |
| `sim` crate | Integrate vehicle dynamics; inject gusts, failures, lag, noise | M2 — **built** |
| `wasm` crate | JSON-in/JSON-out bridge exposing guidance to JavaScript | M1 — **built** |
| `web/` | Page, 3D view, controls, panels | M1 — **built** |

Workspace layout:

```text
kestrel/
├─ crates/
│  ├─ guidance/   # M0 (+ track, replan, closest target in M2)
│  ├─ sim/        # M2
│  └─ wasm/       # M1 (+ Flight, replan in M2)
├─ web/           # M1, M2
└─ docs/
```

## The `guidance` crate (built, M0)

Public API (`crates/guidance/src/lib.rs`), all SI units, pad at origin, `z` up:

- `Vehicle` (+ `validate`, `example_mars`) — masses, Isp, thrust bounds,
  gravity, glide slope, tilt limit.
- `LandingProblem { vehicle, position, velocity, steps }` (+ `validate`, `example`).
- `solve(&LandingProblem) -> Result<Trajectory, GuidanceError>` — free final
  time: 12-point grid, then golden-section search, with the propellant limit
  off; the limit is applied once at the best time found.
- `solve_fixed_time(&LandingProblem, t_f)` — one convex solve, followed by a
  check that the relaxation was tight (`‖u_k‖ = σ_k`).
- `Trajectory { t, position, velocity, mass, thrust, final_time }`
  (+ `fuel_used`, `to_csv`). Nodes: N + 1; thrust: N (one per interval).
- `GuidanceError::{InvalidParams, InvalidProblem, Infeasible, NotLossless,
  SolverFailed}`.

Internals, one responsibility per file:

| File | Responsibility |
| --- | --- |
| `params.rs` | vehicle constants and validation |
| `scaling.rs` | reference units so the solver sees numbers near 1 (gravity = 1) |
| `problem.rs` | problem/trajectory types, input validation, CSV |
| `socp.rs` | variable layout and the convexified problem as Clarabel's `A, b, cones` |
| `solve.rs` | Clarabel call, status mapping, back to SI, landing-time search |

Data flow for one fixed-time solve:

1. Validate the problem (vehicle, state finite, inside glide-slope cone).
2. Non-dimensionalize (length = start distance, mass = wet mass, gravity = 1).
3. Build the SOCP: variables `r, v, z = ln(m/m_wet)` per node and `u, σ` per
   interval; exact constant-acceleration dynamics per interval; lossless
   convexification with Taylor-approximated thrust bounds.
4. Solve with Clarabel; map status to `Ok` / `Infeasible` / `SolverFailed`.
5. Check the relaxation is tight on every interval, else `NotLossless`.
6. Convert back to SI.

Tests: unit tests per module, plus `tests/fixed_time.rs`, `tests/free_time.rs`
and `tests/robustness.rs`, which check every physical constraint on SI
outputs, including vertical descents, tilt limits and tight propellant.

## The browser app (built, M1)

Spec: `docs/superpowers/specs/2026-11-12-m1-browser-landing-design.md`.

```text
crates/wasm  plan_landing(json) / example_request()  ──wasm-bindgen──▶ web/src/wasm/ (generated)
web/src/workers/solver.worker.ts   loads the WASM once; answers {id, request} with {id, response, ms}
web/src/lib/                        pure, unit-tested TypeScript:
  solverClient.ts   newest-request-wins client     state.ts    page state machine (reducer)
  playback.ts       exact interpolation of a plan   drag.ts     drag limits (glide cone, box, speed)
  camera.ts         automatic camera choice         status.ts   status line text
  terrain.ts        procedural height field         attitude.ts, legs.ts, format.ts
  store.ts          zustand store wrapping the reducer
  live.ts           per-frame booster sample (read by the scene without React re-renders)
web/src/hooks/useSolver.ts          worker lifecycle; debounced (120 ms) re-solve on start changes
web/src/components/scene/           R3F: Stage, Sky, Terrain, Pad, GlideCone, Booster, Plume, Dust,
                                    PlannedPath, DragHandles, Cameras, FlightLoop, Effects
web/src/components/hud/             DOM overlay: brand, status, telemetry, charts, controls,
                                    touchdown card, failure fallback
```

Per frame: `FlightLoop` writes `live.sample` (the start state while editing;
since M2 the simulated vehicle during a flight, see below); the booster,
plume, dust, cameras and telemetry read it. (In M1 it replayed the plan;
`playback.ts` is still used to draw plans and charts.)

Per edit: dragging dispatches `setStart` → `useSolver` debounces 120 ms →
worker → response accepted only if it answers the newest request → preview
path redraws. Land is only allowed for a successful plan of the current start.

Coordinates: physics z-up metres; three.js y-up via `toScene([x,y,z]) = [x,z,−y]`.

Tests: Vitest unit tests in `web/src/lib/*.test.ts` (incl. a Rust-generated
fixture proving playback reproduces the solver's nodes); opt-in Playwright
end-to-end tests in `web/e2e/`.

## The closed loop (built, M2)

Spec: `docs/superpowers/specs/2026-11-14-m2-closed-loop-design.md`.

```text
crates/sim       truth only, no guidance dependency
  sim.rs     point mass r, v, m; RK4 in 10 ms substeps; gusts; touchdown latch
  engine.rs  six engines: first-order lag, failures scale both limits by (6−k)/6
  sensor.rs  Gauss–Markov navigation error (τ 2 s), range-scaled below 200 m; seeded
  rng.rs     SplitMix64 + Box–Muller
crates/guidance  (additions)
  track.rs   a = u_plan(τ) + Kp(r_plan − r̂) + Kd(v_plan − v̂), clamped to tilt cone and
             thrust band; descent law after the plan; brake = full thrust up
  replan.rs  warm start (3 solves around the hint) → full search → closest landing
  socp.rs    Target::Closest: free horizontal end point, no glide cone, min 10·miss − z_N
crates/wasm  (additions)
  flight.rs     Flight = Sim + Sensor + controller + latest plan; problem() adds the
                terminal gate (15 m) and propellant reserve; plan() the 85 % thrust margin
  flight_js.rs  JS class `Flight` (main thread) and `replan(json)` (worker)
  tests/closed_loop.rs  the page's loop, natively, with solve latency = one period
web/src/lib
  flightRuntime.ts  owns the Flight; steps it per frame; schedules re-plans
  mpc.ts            0.25 s sim-time period, one solve in flight, epochs drop stale replies
  flightSnapshot.ts snapshot → scene sample and (rare) store patches
  margins.ts        glide-slope / tilt margins, thrust share
web/src/components  hud/BreakIt, hud/Margins, hud/ModeBanner, scene/Trail, scene/GrabHandle
```

Per frame (`FlightLoop` → `flightRuntime.frame`):

1. `flight.step(min(dt, 0.1 s) × speed)`: every 10 ms the sensor reads the
   truth (with error), the controller computes thrust from that reading and
   the plan, the engine lags toward it, the simulator integrates. Held still
   until the first plan arrives, and while the visitor grabs the booster.
2. If no solve is in flight, 0.25 s of sim time has passed and
   `flight.wants_replan()` (plan > 2 s left, sensed altitude ≥ 25 m):
   `flight.measure_request()` → worker `replan` → `flight.apply_replan(answer,
   t_measured)`. An `Infeasible` answer switches to braking; other failures
   keep the current plan.
3. `flight.snapshot()` → `live` (scene, telemetry, margins); mode, engines,
   miss and outcome go to the store only when they change.

Per re-plan (worker, `guidance::replan` via `flight::plan`): margined problem
(85 % thrust) first; three fixed-time solves around the previous landing time;
else the full search; else (or if outside the glide cone) the closest landing;
retried at full thrust if that is what it takes.

## The 6-DoF vehicles and the console (built, M3)

Specs: `docs/superpowers/specs/2026-11-15-m3-ship-design.md` (ship) and
`2026-11-15-m3-booster-and-console-design.md` (booster, page).
The page no longer uses the 3-DoF pieces above (`Flight`, `replan`,
`components/scene`, `components/hud` except the error fallback); they remain, tested, as the earlier milestones' code.

```text
crates/model      what truth and guidance must agree on (no dependencies)
  vehicle.rs   Ship (one struct, two vehicles: VehicleKind::Ship / Booster), GridFins,
               Planet (Earth), Catch, Scenario::{earth, booster}, Tuning (physics options)
  aero.rs      body force: crossflow drag + linear lift slope; flap_force (ship);
               fin_gain / fin_force / fin_allocate (booster's three grid fins)
  dynamics.rs  x' = f(x, T): state [m, r, v, q, w] (14), thrust at the gimbal point
  quat.rs      quaternion algebra
crates/sim
  ship.rs      ShipSim: RK4 in 10 ms substeps; flaps or fins (slew, clamp); RCS; steady
               wind + gusts; contact with mast, arms, ground; Outcome (caught/crashed)
  raptors.rs   Cluster: N landing engines (ship 3; booster 13, centre three first),
               ignition delay, shared gimbal with slew, lag, shutdown, fail(i)
crates/guidance
  scvx/        successive convexification: discretize.rs (first-order hold, sensitivities
               by RK4, finite-difference Jacobians), subproblem.rs (Clarabel SOCP with
               virtual control, trust region, soft comfort constraints), mod.rs (iteration,
               cold/warm references, acceptance by re-integration)
  ship_control.rs  belly_flop (ship) / descent (booster) -> unpowered(); flip; steady_burn;
               powered (plan tracking); terminal; ignition_altitude; fall_time
  envelope.rs  engine-out envelope: the largest sink speed from which a relit spare can
               still stop the ship above the arms (point-mass bound, D-039)
crates/wasm
  ship_flight.rs  ShipFlight: phases (unpowered -> flip/start-up -> burn legs -> terminal);
               the ship plans from engine start (flip on every engine, planned switch
               to one), relights a spare after a failure, request_replan() on events,
               emergency full-thrust option and a lower emergency gate after an engine
               loss; engine_out_margin() for the page;

               legs (full engine set to a mid gate, final engines to the approach gate),
               when to re-plan, prediction of the booster's first plan, options
  ship_js.rs   JS class ShipFlight; plan_ship / preview_ship (worker); ship_scenario
               (incl. the model's coefficients and the catch limits, for the page's formulas);
               warmup_ship; snapshot (state, forces, heating estimate, engines)
  tests/ship_loop.rs  the page's loop natively: nominal, engine out, gusts, slow solver,
               and an ignored sweep over browser timing conditions
web/src/lib/ship
  parts.ts     the parts a visitor can open in the analysis view and, for each, the
               model's formulas with the snapshot's numbers (pure, tested)
  launch.ts    the opening sequence's one clock: count, mission time, cues to speak
  store.ts     zustand: vehicle -> flight kind, start, options, view (incl. open part), mode
               (editing -> launch -> flying -> done), presets, configOf() for Rust
  record.ts    the flight record: decisions (with reasons and numbers) from the state
               stream and from guidance's answers, and the state every 0.1 s
  shown.ts     the state the page shows: live, or the replayed instant (view.scrubT)
  runtime.ts   owns the ShipFlight; steps it per frame (not while paused or held);
               records decisions; events (gust, failure, setting) re-plan at once;
               stepShip() advances a paused flight for the analysis view;
               schedules plans; publishes to live.ts; reports solve times to the flight
  live.ts      per-frame snapshot, trail, plan, history samples, caption, hold flag
  commentary.ts callouts(prev, next) -> lines; Commentator plays clips (fallback: speech)
  phrases.ts   the finite list of lines and their clip ids
  audio.ts     mix(hearing) -> gains, cutoff, delay; FlightAudio (Web Audio graph)
  launch.ts    timing of the opening sequence (illustration)
  engines.ts, frames.ts, types.ts
web/src/components/stage    R3F scene
  Stage, World (sky by camera altitude, clouds, painted coast, tower, haze), Site (launch
  site), Vehicles (ship and booster models, plumes, heat shell), Vehicle (flown vehicle,
  full stack and its launch/staging animation, parked booster), Fx (smoke, paths),
  Cameras (director, chase, tracking, onboard, tower, free, launch), Analysis (vectors,
  cone, plan nodes, labels), Post (bloom, tone mapping, blueprint transition), Sound
web/src/components/console  DOM overlay
  Console (top bar, timeline, action bar, caption, verdict, keys), Dock (Mission, Vehicle,
  Environment, View), Telemetry (rail, attitude ball, engine map, graphs), AnalysisPanel,
  Launch (opening overlay, phase banner, analysis intro), useEngine (workers, preview)
web/scripts/make-voice.mjs  renders phrases.ts to public/voice/*.mp3 (dev tool)
web/e2e/console.spec.ts     opt-in Playwright tests of the page (catch, engine out, analysis, reset);
                            E2E_BASE_URL=http://localhost:3000 runs them against a dev server
```

Per frame while flying (`Stage` -> `runtime.shipFrame`): step the flight
(`min(dt, 0.1 s) x speed`; each 10 ms substep reads the sensors, runs the
phase's controller, steps the truth); if the flight wants a plan and none is
being solved, `measure_request()` -> flight worker `plan_ship` ->
`apply_replan(answer, t)`; snapshot -> `live`, callouts, history, store
patches when something discrete changed.

Two workers: one answers the flight's plan requests, the other flies whole
flights ahead of time for the prediction shown while editing, so a slow
prediction never delays a plan. Each solves one throw-away plan when it
starts (`warmup_ship`): the first solve in a fresh instance is several times
slower than the rest.

Modes: `editing` (stack on the mount, prediction) -> `launch` (opening
sequence; the flight starts 1.6 s into the title card and is held until the
card clears) -> `flying` -> `done` (after a booster catch: continue with the
ship, the booster parked beside the pad, or end).

## Open questions

- Firefox solve time (Chromium measured: ~244 ms with a GPU).
- M3: solve times of the 6-DoF planner are not benchmarked yet (only spot
  readings, LIMITATIONS.md).
- Lossless convexification is not guaranteed when the ground constraint is
  active (closest landings near the ground); such landing times are skipped.

### Analysis view (blueprint)

`components/stage/Blueprint.tsx` holds one switch, `skin.on`. `Post.tsx`
sweeps a blue shockwave over the film picture and sets the switch when it
has covered the screen; from then on `World` hides the scenery, `Backdrop`
draws the blue, `BlueprintSkin` swaps every opaque mesh of the vehicle for
a translucent shell with station rings, stringers and edge lines, and the
effects (plumes, heating, smoke) hide. `Part` wraps the named regions of
the models (`Vehicles.tsx`) so a click opens them (`view.part`);
`Analysis.tsx` draws the force vectors and a marker per part; `Cameras.tsx`
flies the free camera to the open part; `console/AnalysisPanel.tsx` shows
`partSheet()` for it.

### Guidance on the page, end to end

`ShipFlight.wants_replan()` (drift, age, or an event) → `measure_request()`
(the problem as JSON: options, engine switch, tilt cap, funnel, side-speed
limit) → the worker's `plan_ship` (SCvx: ship 16 nodes, 1e-6 tolerance, up
to 15 iterations, abandoned after 7 if still infeasible; booster 24 nodes,
up to 8) → `apply_replan()`. Each answer is also written to the flight
record with the reason it was asked for. `Stage.GiveWay` lowers the
picture's cost when recent solves are slow (D-037).

