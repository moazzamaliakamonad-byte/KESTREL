# Architecture

Status: the `guidance` crate is **built** (M0). Everything else is
_(planned)_ and is updated to describe what was actually built when it lands.

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
| `sim` crate | Integrate vehicle dynamics; inject wind, failures, lag, noise | M2 (M1 plays back the plan instead) |
| `wasm` crate | JSON-in/JSON-out bridge exposing guidance to JavaScript | M1 — **built** |
| `web/` | Page, 3D view, controls, panels | M1 — **built** |

Proposed workspace layout (create only when the milestone needs it):

```text
kestrel/
├─ crates/
│  ├─ guidance/   # M0
│  ├─ sim/        # M1/M2
│  └─ wasm/       # M1
├─ web/           # M1
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

Per frame: `FlightLoop` advances the flight clock (reducer `tick`) and writes
`live.sample` (the plan sampled at `t`, or the start state while editing);
the booster, plume, dust, cameras and telemetry read it.

Per edit: dragging dispatches `setStart` → `useSolver` debounces 120 ms →
worker → response accepted only if it answers the newest request → preview
path redraws. Land is only allowed for a successful plan of the current start.

Coordinates: physics z-up metres; three.js y-up via `toScene([x,y,z]) = [x,z,−y]`.

Tests: Vitest unit tests in `web/src/lib/*.test.ts` (incl. a Rust-generated
fixture proving playback reproduces the solver's nodes); opt-in Playwright
end-to-end tests in `web/e2e/`.

## Data flow per guidance cycle with a closed loop _(planned, M2)_

1. Read noisy state: position `r`, velocity `v`, mass `m`.
2. Solve (warm-started landing time; a full search is ~250 ms, too slow per frame).
3. On infeasibility, solve the minimum-landing-error problem instead.
4. Hand trajectory to controller and UI.

## Open questions

- Firefox solve time (Chromium measured: ~244 ms with a GPU).
- Warm-starting the landing-time search for M2's re-planning several times a
  second.
- Which 3D library for the front end? Decide in M1.
