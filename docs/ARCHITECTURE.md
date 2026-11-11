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
| `sim` crate | Integrate vehicle dynamics; inject wind, failures, lag, noise | M2 (simple version in M1) |
| `wasm` crate | Thin bindings exposing guidance + sim to JavaScript | M1 |
| `web/` | Page, 3D view, controls, panels | M1 |

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

## Data flow per guidance cycle in the browser _(planned)_

1. Read noisy state: position `r`, velocity `v`, mass `m`.
2. Solve (warm-started landing time; a full search is ~300 ms, too slow per frame).
3. On infeasibility, solve the minimum-landing-error problem instead.
4. Hand trajectory to controller and UI.

## Open questions

- How fast is Clarabel in a browser? (It compiles for `wasm32-unknown-unknown`;
  speed not yet measured.)
- Run guidance in a Web Worker so a slow solve never freezes rendering?
  Likely yes; decide in M1.
- Which 3D library for the front end? Decide in M1.
