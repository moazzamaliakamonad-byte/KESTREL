# M1 — Browser landing: design

Date: 2026-11-12. Status: approved by owner ("go absolutely wild", decisions
delegated). Roadmap: `docs/ROADMAP.md` M1.

## Goal

A web page where a visitor drags a booster to any start position and
velocity, watches the real fuel-optimal guidance re-plan **live** in the
browser (the path re-bends while dragging), presses **Land**, and watches a
cinematic 3D landing with live telemetry. Everything runs client-side. The
solver is the M0 Rust crate compiled to WebAssembly.

## Non-goals (M2 or later)

- No physics simulator, disturbances, wind, engine failure, sensor noise, or
  re-planning during flight. Playback replays the solved plan exactly.
- No minimum-landing-error fallback. Impossible starts are shown as such.
- No 3D (out-of-plane) dragging: start position and velocity are edited in
  the vertical x–z plane (y = 0).
- No server, accounts or analytics.

## Decisions

| ID | Decision | Why |
| --- | --- | --- |
| D-005 | Next.js 16 (App Router, TypeScript), `output: 'export'` | Owner's choice; static export keeps it serverless and free to host |
| D-006 | React Three Fiber 9 + drei 10 + @react-three/postprocessing 3 on three 0.187 | Declarative 3D in React, the standard pairing with Next.js |
| D-007 | `wasm-bindgen` 0.2.130 + `wasm-bindgen-cli` 0.2.130 (`--target web`), no wasm-pack | One tool, version pinned to the library |
| D-008 | Solver runs in a module Web Worker; latest-request-wins | A full solve is ~250 ms native; the UI must never stall |
| D-009 | JSON strings across the WASM boundary | Simple, debuggable, no shared-memory lifetimes; payload is a few KB |
| D-010 | All visuals procedural or original (code-generated terrain, booster, plume) | No third-party or copyrighted assets (project rule 5) |
| D-011 | zustand for page state | Small, works across the React tree and the R3F render loop |

## Architecture

```text
crates/guidance (M0, unchanged)
crates/wasm  ──(wasm-bindgen)──▶  web/src/wasm/kestrel_wasm.js   (generated, gitignored)
                                  web/public/wasm/kestrel_wasm_bg.wasm (generated, gitignored)
web/ (Next.js)
  src/app/page.tsx            page shell (server component) → <KestrelApp> (client, ssr:false)
  src/lib/                    pure TypeScript, unit-tested: types, playback, state, drag limits, format
  src/workers/solver.worker.ts  loads WASM once; answers {id, request} with {id, response, ms}
  src/hooks/useSolver.ts      worker lifecycle, debounce, latest-wins
  src/components/scene/*      R3F: Terrain, Sky, Pad, GlideCone, Booster, Plume, Dust, PlannedPath,
                              DragHandles, Cameras, Effects
  src/components/hud/*        Telemetry, Charts, StatusPill, TouchdownCard, Controls
scripts/build-wasm.mjs        cargo build (wasm32, release) + wasm-bindgen; run by npm predev/prebuild
```

### WASM bridge (`crates/wasm`, crate `kestrel-wasm`)

Exports (all `&str` → `String`, never panic, never throw):

- `example_request() -> String`: the default request (M0 example state and
  vehicle), so the vehicle's numbers have one source of truth (Rust).
- `plan_landing(request_json) -> String`.

Request:

```json
{ "position": [x, y, z], "velocity": [vx, vy, vz], "steps": 50,
  "vehicle": { "wet_mass", "dry_mass", "isp", "min_thrust", "max_thrust",
               "gravity", "glide_slope", "max_tilt" } }
```

`steps` and `vehicle` optional (defaults: 50, `Vehicle::example_mars()`).
SI units, radians, pad at origin, z up.

Response, success:

```json
{ "ok": true,
  "trajectory": { "t": [...], "position": [[x,y,z],...], "velocity": [...],
                  "mass": [...], "thrust": [[...],...], "final_time": 36.62,
                  "fuel_used": 194.39 },
  "vehicle": { ...same fields as the request... } }
```

Response, failure:

```json
{ "ok": false, "error": { "kind": "Infeasible", "message": "..." } }
```

`kind` ∈ `BadRequest` (unparseable JSON), `InvalidParams`, `InvalidProblem`,
`Infeasible`, `NotLossless`, `SolverFailed`. Rust's `std::time::Instant`
panics on `wasm32-unknown-unknown`, so timing is measured in the worker with
`performance.now()`.

### Coordinates

Physics: metres, z up. three.js: y up. Mapping `(x, y, z) → (x, z, −y)`.
One scene unit = one metre. The booster model is 12 m tall and 3.7 m wide
(illustrative, not to scale with the 1.9 t example vehicle; stated in
LIMITATIONS). A screen-space halo marks it when far away.

### Playback maths (pure, unit-tested)

On interval k (duration `dt_k = t[k+1] − t[k]`, local time `τ`): thrust
acceleration is held constant, `u_k = T_k / m_k`, and gravity
`g = (0, 0, −vehicle.gravity)`:

```text
a_k   = u_k + g
r(τ)  = r_k + v_k·τ + ½·a_k·τ²
v(τ)  = v_k + a_k·τ
m(τ)  = m_k · (m_{k+1}/m_k)^(τ/dt_k)        (log-mass is linear in time)
T(τ)  = m(τ) · u_k                          (displayed thrust)
```

Clamped to `[0, final_time]`. At `τ = dt_k` the formulas must reproduce node
`k+1` (tested against a real solved fixture, rel. tol 1e-6).

Booster attitude: body axis along `T(τ)` (thrust direction), slerped with a
time constant of 0.15 s; when thrust is ~0 hold the last attitude.

### Page state machine (pure reducer, unit-tested)

```text
editing ──drag──▶ editing (solve requested, debounced 120 ms; preview path updates on response)
editing ──Land (last response ok)──▶ flying(t=0) ──t ≥ final_time──▶ touchdown
editing ──Land (last response error)──▶ editing (button disabled; no-op)
flying/touchdown ──Reset──▶ editing (same start state)
any ──WASM load failure──▶ failed (fallback screen)
```

Responses carry the request id; a response whose id is not the latest is
dropped. While a solve is pending the previous preview stays visible, dimmed.
Playback speed ×1/×2/×4; Space = Land/Reset.

### Drag limits (pure, unit-tested)

- Position (x–z plane): `|x| ≤ 4000 m`, `z ≤ 3000 m`, and above the
  glide-slope cone with a 5% margin: `z ≥ 1.05·tan(γ)·|x| + 20 m`.
- Velocity handle: arrow tip at `position + velocity·2 s`; speed ≤ 150 m/s.

## Visual design

- **Sky:** physically-inspired Mars dusk gradient (butterscotch high, blue
  halo around the low sun), drei `Sky`-style custom shader; stars faintly at
  the zenith.
- **Terrain:** 8 km × 8 km plane, fractal noise heightmap with a flattened
  disc around the pad, crater stamps; vertex-coloured regolith with a
  normal-detail shader. Soft directional shadows (sun), hemisphere fill,
  exponential fog for depth.
- **Pad:** concrete disc, target ring, perimeter lights that blink before
  touchdown.
- **Glide-slope cone:** translucent additive funnel (fresnel shader), fades
  with distance.
- **Booster:** original procedural model: tapered body with panel lines,
  interstage band, four grid fins, four landing legs (deploy below 100 m over
  2 s), engine bell. PBR materials, emissive engine bell.
- **Plume:** two nested cone meshes with an animated noise shader (core +
  outer glow), length and brightness ∝ `‖T‖/max_thrust`, slight flicker.
  Point light at the nozzle scaled the same way.
- **Dust:** instanced particle ring at the ground below the booster when
  altitude < 60 m, count and speed ∝ thrust and proximity.
- **Planned path:** tube with a time gradient (cyan → white); progress head
  during flight; infeasible preview = red, pulsing; pending = dimmed.
- **Cameras:** chase (behind/above, damped), pad (low, looking up), orbit
  (free). Auto: orbit while editing, chase during flight, pad for the last
  8 s, slow orbit on touchdown. Manual override buttons.
- **Effects:** bloom (plume, pad lights, path), ACES tone mapping, vignette,
  SMAA. drei `PerformanceMonitor` lowers DPR, then disables bloom, if FPS
  drops; `prefers-reduced-motion` disables camera shake and auto camera
  moves.
- **HUD (DOM overlay):** glass panels. Mission clock, altitude, speed,
  vertical speed, thrust (N and % of max), propellant remaining, solve time.
  Two sparkline charts (canvas 2D): thrust with the ρ₁–ρ₂ band and a moving
  cursor; mass with the dry-mass line. Status pill. Touchdown card: touchdown
  speed, propellant used, landing time, solve time. Help text for first
  visit. Sound toggle (off by default): WebAudio brown-noise rumble, low-pass
  ∝ thrust.
- **Title/credits:** "Kestrel" wordmark, one-line pitch, "simulation only"
  note, link to the repo (placeholder until a remote exists).

## Error handling

| Situation | Behaviour |
| --- | --- |
| `Infeasible` | Red pulsing preview path from the start; status "No landing exists from here: too fast, too low or too little propellant." Land disabled. |
| `NotLossless` | Status "Couldn't certify a flyable plan from here; try another start." Land disabled. |
| `InvalidProblem` / `InvalidParams` / `BadRequest` | Status shows the message; should not occur through the UI (drag limits). |
| `SolverFailed` | Status "Solver failed (…)"; Land disabled. |
| WASM fails to load / worker error | Full-page fallback card with the reason and a reload button. |
| Solve slower than 3 s | Status "Still planning…"; no timeout kill in M1. |

## Testing

- **Rust (`crates/wasm`, native `cargo test`):** request defaults, example
  request round-trips, success response matches `guidance::solve` on the same
  problem, each error kind maps correctly, garbage JSON → `BadRequest`, no
  panic on NaN/huge values.
- **TypeScript (Vitest, `npm test` in `web/`, < 10 s):** playback at node
  times reproduces nodes (fixture generated from Rust and committed);
  monotonic mass; thrust display within limits; reducer transitions and
  latest-wins; drag clamps; formatters.
- **Type check + lint:** `npm run typecheck`, `npm run lint`.
- **Build:** `npm run build` produces `web/out/` (static export).
- **End-to-end (Playwright, opt-in `npm run e2e`):** page loads, the preview
  path appears, Land → "Touchdown" within 60 s at ×4. Not run by default
  (downloads a browser).
- **Measured:** WASM solve time in Chrome (median of 10, example problem,
  same laptop as M0) recorded in LIMITATIONS.md.

## Acceptance criteria

1. `npm run dev` in `web/` builds the WASM and serves the page.
2. Dragging the booster or velocity handle re-plans live; the path updates
   within ~0.5 s of releasing the mouse, while the scene stays responsive.
3. Land flies the plan; touchdown speed shown ≤ 0.1 m/s; telemetry matches
   the trajectory.
4. Impossible starts show the red path and message; nothing crashes.
5. All Rust and TypeScript tests pass; typecheck and lint clean; static
   export builds.
6. README leads with a demo GIF or screenshot of the page and a one-command
   run; LIMITATIONS has the measured browser solve time.
