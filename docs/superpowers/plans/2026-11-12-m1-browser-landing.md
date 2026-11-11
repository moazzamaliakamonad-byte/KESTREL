# M1 — Browser Landing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Next.js page where a visitor drags a booster's start position and velocity, the M0 guidance re-plans live in a Web Worker via WebAssembly, and Land plays a cinematic 3D landing with live telemetry.

**Architecture:** New Rust crate `crates/wasm` (`kestrel-wasm`) exposes two JSON-in/JSON-out functions over `guidance`. `web/` is a Next.js 16 static-export app: pure TypeScript logic in `src/lib` (unit-tested), a solver Web Worker, R3F scene components and a DOM HUD driven by one zustand store.

**Tech Stack:** Rust 1.99, wasm-bindgen 0.2.130 (+ cli 0.2.130), serde 1 / serde_json 1; Next.js 16.4, React 19.3, three 0.187, @react-three/fiber 9.8, @react-three/drei 10.7, @react-three/postprocessing 3.2, zustand 5.1, Vitest 5.1, Playwright 1.65 (opt-in).

**Spec:** `docs/superpowers/specs/2026-11-12-m1-browser-landing-design.md`

## Global Constraints

- Static export only (`output: 'export'`); no server code, no API routes.
- Every 3D/WASM component is a client component loaded with `ssr: false`.
- Coordinates: physics `(x, y, z)` z-up metres → three `(x, z, −y)`; 1 unit = 1 m.
- Generated WASM artifacts are gitignored: `web/src/wasm/`, `web/public/wasm/`.
- No third-party models, textures, fonts files or sounds; procedural/original only (fonts via `next/font/google` are fine).
- WASM functions never panic or throw; all failures are `{ok:false, error:{kind, message}}`.
- Fast tests by default: `cargo test` < 2 min total, `npm test` < 10 s; Playwright only via `npm run e2e`.
- Run cargo from PowerShell (Git Bash `link.exe` shadows MSVC); never round-trip source files through PowerShell 5.1 `Get-Content/Set-Content`.
- Small commits, conventional prefixes; never stage `learn/` or `project-ideas.md`. Add a short local `learn/build-log/` note per task.

## Review Focus

1. **Rapid dragging floods the worker** → only the newest response may change the preview (Task 5 test `drops_stale_responses`).
2. **Land pressed while a newer solve is pending** → no flight on a plan for an old start state (Task 4 test `land_ignored_when_preview_stale`).
3. **Playback time outside `[0, final_time]`** (tab resumes after sleep, ×4 overshoot) → clamped, touchdown state, never NaN (Task 3 test `clamps_time`).
4. **Pointer drag producing NaN/∞ or extreme coordinates** → clamped to finite allowed values (Task 4 test `clamp_handles_non_finite`).
5. **WASM fails to load** → `failed` state and fallback card, not a blank page (Task 4 test `worker_error_fails`, Task 9 UI).

---

### Task 1: WASM bridge crate

**Files:** Create `crates/wasm/Cargo.toml`, `crates/wasm/src/lib.rs`, `crates/wasm/src/dto.rs`, `crates/wasm/examples/fixture.rs`; modify `Cargo.toml` (workspace members), `docs/DECISIONS.md` (append D-005…D-011 from the spec table).

**Interfaces — Produces:**
- `#[wasm_bindgen] pub fn example_request() -> String`
- `#[wasm_bindgen] pub fn plan_landing(request_json: &str) -> String`
- Plain (non-wasm) Rust fns with the same behaviour used by both: `example_request_json() -> String`, `plan_landing_json(&str) -> String`.
- JSON shapes exactly as the spec's "WASM bridge" section. `vehicle` in the response is the vehicle actually used.
- `cargo run -p kestrel-wasm --example fixture > web/src/lib/__fixtures__/example-response.json` prints `plan_landing_json(&example_request_json())`.

Crate: `name = "kestrel-wasm"`, `[lib] crate-type = ["cdylib", "rlib"]`, deps `guidance = { path = "../guidance", package = "kestrel-guidance" }`, `wasm-bindgen = "=0.2.130"`, `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`. DTOs live in `dto.rs` (guidance stays serde-free).

- [ ] **Step 1: Failing tests** in `lib.rs`: `example_request_round_trips` (parse `example_request_json()`; position `[2000,0,1500]`, velocity `[-50,0,-75]`, steps 50); `defaults_fill_missing_fields` (`{"position":[2000,0,1500],"velocity":[-50,0,-75]}` → ok, `trajectory.t.len() == 51`); `matches_native_solve` (fuel_used and final_time equal `guidance::solve(&LandingProblem::example())` within 1e-9); `garbage_is_bad_request` (`"not json"` → kind `BadRequest`); `infeasible_kind` (velocity `[0,0,-3000]` → `Infeasible`); `invalid_problem_kind` (position `[0,0,-5]` → `InvalidProblem`); `nan_is_handled` (`"velocity":[NaN…]` is not valid JSON → `BadRequest`; `1e308` speed → ok:false, any kind, no panic).
- [ ] **Step 2:** `cargo test -p kestrel-wasm` → FAIL (not defined).
- [ ] **Step 3:** Implement. Error `message` = `format!("{e:?}")` of the guidance error.
- [ ] **Step 4:** Tests pass; `cargo build -p kestrel-wasm --release --target wasm32-unknown-unknown` succeeds; clippy clean.
- [ ] **Step 5:** Generate the fixture (path above; create the directory). Commit `feat: add wasm bridge crate with json api` and `docs: record M1 decisions D-005 to D-011`.

### Task 2: Next.js app scaffold and WASM build pipeline

**Files:** Create `web/` via `npx create-next-app@16.4.6 web --ts --eslint --app --src-dir --no-tailwind --import-alias "@/*" --use-npm --yes` (default bundler, Turbopack; if the worker or WASM import fails under Turbopack in Task 8, switch the dev/build scripts to `--webpack` and ledger it); `scripts/build-wasm.mjs`; `web/vitest.config.ts`; modify `web/next.config.ts`, `web/package.json`, `.gitignore`.

**Interfaces — Produces:**
- `npm run wasm` (in `web/`) → runs `node ../scripts/build-wasm.mjs`: `cargo build -p kestrel-wasm --release --target wasm32-unknown-unknown`, then `wasm-bindgen --target web --out-dir web/src/wasm --out-name kestrel_wasm target/wasm32-unknown-unknown/release/kestrel_wasm.wasm`, then moves `kestrel_wasm_bg.wasm` to `web/public/wasm/`. Fails loudly if either tool is missing.
- Scripts: `predev`/`prebuild` = `npm run wasm`; `test` = `vitest run`; `typecheck` = `tsc --noEmit`; `e2e` = `playwright test` (Task 10).
- `next.config.ts`: `output: 'export'`, `basePath`/`assetPrefix` from `process.env.NEXT_PUBLIC_BASE_PATH ?? ''`, `images.unoptimized: true`.
- `web/src/lib/env.ts`: `export const BASE_PATH: string`.
- Install deps at the Tech Stack versions (exact): three, @react-three/fiber, @react-three/drei, @react-three/postprocessing, postprocessing, zustand; dev: vitest, @types/three.

- [ ] **Step 1:** Failing Vitest test `web/src/lib/env.test.ts`: `BASE_PATH` is `''` when the env var is unset.
- [ ] **Step 2:** `npm test` → FAIL (module missing). **Step 3:** implement. **Step 4:** `npm test` PASS; `npm run typecheck`, `npm run lint` clean; `npm run build` produces `web/out/index.html` and `web/out/wasm/kestrel_wasm_bg.wasm`.
- [ ] **Step 5:** Commit `chore: scaffold next.js app and wasm build pipeline` (verify `git status` shows no `web/node_modules`, `web/.next`, `web/out`, generated wasm).

### Task 3: Types and playback maths

**Files:** Create `web/src/lib/types.ts`, `web/src/lib/playback.ts`, `web/src/lib/playback.test.ts`.

**Interfaces — Produces:**
- `types.ts`: `type Vec3 = [number, number, number]`; `interface Vehicle {wet_mass; dry_mass; isp; min_thrust; max_thrust; gravity; glide_slope; max_tilt}`; `interface Trajectory {t: number[]; position: Vec3[]; velocity: Vec3[]; mass: number[]; thrust: Vec3[]; final_time: number; fuel_used: number}`; `type ErrorKind = 'BadRequest'|'InvalidParams'|'InvalidProblem'|'Infeasible'|'NotLossless'|'SolverFailed'`; `type PlanResponse = {ok: true; trajectory: Trajectory; vehicle: Vehicle} | {ok: false; error: {kind: ErrorKind; message: string}}`; `interface PlanRequest {position: Vec3; velocity: Vec3; steps?: number; vehicle?: Vehicle}`.
- `playback.ts`: `interface Sample {t; position: Vec3; velocity: Vec3; mass: number; thrust: Vec3; thrustMag: number; done: boolean}`; `sampleAt(traj: Trajectory, vehicle: Vehicle, t: number): Sample` (spec "Playback maths"; `done = t >= final_time`; after the last interval thrust = `[0,0,0]`); `toScene(p: Vec3): Vec3` (`[x, z, -y]`).

- [ ] **Step 1: Failing tests** (fixture from Task 1): `reproduces_nodes` (for every k, `sampleAt(t[k])` position/velocity/mass within 1e-6 relative-or-1e-6 absolute of node k); `ends_at_rest_on_pad` (`sampleAt(final_time)` ‖position‖ < 0.01, ‖velocity‖ < 0.01, done); `mass_never_increases` (200 samples); `thrust_within_limits` (for samples strictly inside intervals, `thrustMag` in `[min_thrust·0.995, max_thrust·1.001]`); `clamps_time` (t = −5 → equals t = 0 sample; t = final_time + 100 → equals final_time sample; t = NaN → equals t = 0 sample); `to_scene_maps_axes` (`[1,2,3]` → `[1,3,-2]`).
- [ ] **Step 2–4:** Fail → implement → pass. **Step 5:** Commit `feat: add trajectory types and playback interpolation`.

### Task 4: Page state, drag limits, formatting

**Files:** Create `web/src/lib/state.ts`, `web/src/lib/drag.ts`, `web/src/lib/format.ts` and their `.test.ts`.

**Interfaces — Produces:**
- `drag.ts`: `const LIMITS = {maxAbsX: 4000, maxZ: 3000, coneMargin: 1.05, coneOffset: 20, maxSpeed: 150, handleSeconds: 2}`; `clampPosition(x: number, z: number, glideSlope: number): {x: number; z: number}`; `clampVelocity(vx: number, vz: number): {vx: number; vz: number}`; `velocityFromHandle(pos: {x,z}, tip: {x,z}): {vx, vz}` (`(tip − pos)/handleSeconds`, then clamped). Non-finite inputs → treated as 0.
- `state.ts`: `type Phase = 'loading'|'editing'|'flying'|'touchdown'|'failed'`; `interface AppState {phase; start: {position: Vec3; velocity: Vec3}; latestRequestId: number; requestedAt: number | null; preview: {requestId: number; response: PlanResponse} | null; flight: {response: PlanResponse & {ok: true}; t: number} | null; speed: 1|2|4; failure: string | null}`; `type Action = {type:'ready'; start} | {type:'setStart'; start} | {type:'requested'; id; at: number} | {type:'response'; id; response} | {type:'land'} | {type:'tick'; dt} | {type:'reset'} | {type:'setSpeed'; speed} | {type:'workerError'; message}`; `reduce(s: AppState, a: Action): AppState`; `initialState: AppState` (phase `loading`); `canLand(s): boolean` (= editing ∧ preview ok ∧ preview.requestId === latestRequestId).
- `format.ts`: `fmtNumber(v: number, digits: number): string` (non-finite → `'—'`), `fmtTime(s): string` (`T+12.3 s`), `fmtMass(kg)`, `fmtForce(N)` (`13.26 kN`), `fmtSpeed(mps)`.

- [ ] **Step 1: Failing tests:** drag — `clamp_keeps_inside_cone` (x = 3000, z = 10 → z ≥ 1.05·tan(γ)·3000 + 20), `clamp_bounds_box`, `clamp_handles_non_finite` (NaN/∞ → finite in bounds), `velocity_speed_capped` (|v| ≤ 150), `velocity_from_handle` (pos (0,1000), tip (100,900) → (50, −50)); state — `starts_loading`, `ready_enters_editing`, `response_for_latest_sets_preview`, `land_requires_ok_latest_preview`, `land_ignored_when_preview_stale` (requested id 2 after preview id 1 → land is no-op), `tick_advances_and_touches_down` (tick past final_time → phase touchdown, t clamped), `tick_respects_speed` (speed 4 → t advances 4·dt), `reset_returns_to_editing_keeping_start`, `worker_error_fails` (any phase → failed, failure message kept); format — `non_finite_is_dash`, `force_in_kN`, `time_format`.
- [ ] **Step 2–4:** Fail → implement → pass. **Step 5:** Commit `feat: add page state machine, drag limits and formatters`.

### Task 5: Solver worker and client

**Files:** Create `web/src/workers/solver.worker.ts`, `web/src/lib/solverClient.ts`, `web/src/lib/solverClient.test.ts`, `web/src/lib/store.ts`.

**Interfaces:**
- Consumes: Task 1 JSON API; Task 4 `reduce`, `Action`.
- Produces:
  - Worker protocol: in `{id: number; request: PlanRequest} | {id: 0; kind: 'example'}`; out `{id; response: PlanResponse; ms: number} | {id: 0; example: PlanRequest} | {id: -1; fatal: string}`. Worker calls `init({module_or_path: \`${BASE_PATH}/wasm/kestrel_wasm_bg.wasm\`})` once; posts `{id:-1, fatal}` on init failure.
  - `class LatestOnly { next(): number; accept(id: number): boolean }` (accepts only the most recently issued id, once).
  - `createSolverClient(worker: Pick<Worker,'postMessage'|'addEventListener'>, onResult: (id, response, ms) => void, onFatal: (msg) => void): {loadExample(): Promise<PlanRequest>; request(req: PlanRequest): number}` — debounce is the caller's job.
  - `store.ts`: zustand store `useApp` wrapping `reduce` with `dispatch(action)`, plus non-reducer fields `lastSolveMs: number | null`, `vehicle: Vehicle | null`.

- [ ] **Step 1: Failing tests** with a fake worker (`EventTarget` + recorded `postMessage`): `drops_stale_responses` (issue ids 1,2,3; responses arrive 3,1,2 → only 3 delivered), `delivers_fatal`, `load_example_resolves`.
- [ ] **Step 2–4:** Fail → implement → pass. The worker file itself is verified in Task 8's in-browser check (no unit test for WASM inside Vitest).
- [ ] **Step 5:** Commit `feat: add solver worker and latest-only client`.

### Task 6: Scene environment

**Files:** Create `web/src/components/scene/{Stage.tsx, Sky.tsx, Terrain.tsx, Pad.tsx, GlideCone.tsx, Effects.tsx}`, `web/src/lib/terrain.ts` + `terrain.test.ts`, `web/src/components/KestrelApp.tsx`; modify `web/src/app/page.tsx`, `web/src/app/layout.tsx`, `web/src/app/globals.css`.

**Interfaces — Produces:**
- `terrain.ts`: `heightAt(x: number, y: number, seed: number): number` — deterministic fractal value noise (5 octaves, amplitude 40 m, base wavelength 900 m) plus 12 seeded craters (radius 40–220 m, rim + bowl), multiplied by a smooth flat-pad mask (0 within 120 m of origin, 1 beyond 300 m).
- `<Stage>`: R3F `<Canvas shadows dpr={[1, 2]} gl={{ antialias: false }}>` with camera far = 20000, fog colour from sky horizon; children slot.
- `<Effects quality>`: Bloom (luminanceThreshold 0.9, intensity 1.2), ToneMapping ACES, Vignette, SMAA; `quality='low'` drops Bloom. `<Stage>` wraps drei `PerformanceMonitor`: on decline lower DPR to 1, on a second decline set quality `'low'`.
- `<Pad blinking>`: perimeter lights blink at 2 Hz when `blinking` (flying and `t > final_time − 8`).
- `KestrelApp` is the client root loaded from `page.tsx` via `next/dynamic(..., {ssr: false})`.

- [ ] **Step 1: Failing tests:** `height_is_deterministic`, `pad_is_flat` (|heightAt| < 0.01 for r < 120 m), `height_is_finite_everywhere` (grid over ±4000 m).
- [ ] **Step 2–4:** Fail → implement → pass; then build the scene components. Terrain mesh 8 km, 256×256 segments, vertex colours by height and slope (rust → dark basalt). Sky: shader gradient per spec with sun at azimuth 230°, elevation 6°. GlideCone: open cone of half-angle `90° − γ` from vertical, height 3000 m, fresnel additive material, opacity 0.12.
- [ ] **Step 5:** `npm run typecheck`, `npm run lint`, `npm test` clean; `npm run build` ok. Commit `feat: add mars environment scene`.

### Task 7: Booster, plume, dust

**Files:** Create `web/src/components/scene/{Booster.tsx, Plume.tsx, Dust.tsx}`, `web/src/lib/attitude.ts` + `attitude.test.ts`, `web/src/lib/legs.ts` + `legs.test.ts`.

**Interfaces — Produces:**
- `attitude.ts`: `targetQuaternion(thrust: Vec3, previous: [x,y,z,w]): [x,y,z,w]` — rotates scene +Y onto `toScene(thrust)`; if ‖thrust‖ < 1 N returns `previous`. `smoothQuaternion(current, target, dt, tau = 0.15)`.
- `legs.ts`: `legDeploy(altitude: number): number` — 0 above 100 m, 1 at or below `100 − 2 s × 25 m/s = 50 m`, smoothstep between.
- `<Booster sample>` (12 m tall, ⌀3.7 m, four grid fins, four legs using `legDeploy`, emissive bell); `<Plume intensity>` (`intensity = thrustMag / max_thrust`; nested additive cones with animated noise shader, length 6–28 m, point light); `<Dust intensity altitude>` (instanced 600 particles, active below 60 m).

- [ ] **Step 1: Failing tests:** attitude — `vertical_thrust_is_identity`, `tilted_thrust_rotates_up_axis` (thrust (1,0,1) → rotated +Y equals normalized toScene), `zero_thrust_keeps_previous`, `smoothing_converges` (after 2 s of 1/60 steps, angle to target < 0.5°); legs — `stowed_high`, `deployed_low`, `monotonic`.
- [ ] **Step 2–4:** Fail → implement → pass; then components. **Step 5:** checks clean; commit `feat: add booster, engine plume and dust`.

### Task 8: Planned path, drag handles, cameras, flight loop

**Files:** Create `web/src/components/scene/{PlannedPath.tsx, DragHandles.tsx, Cameras.tsx, FlightLoop.tsx}`, `web/src/lib/camera.ts` + `camera.test.ts`; modify `KestrelApp.tsx`.

**Interfaces:**
- Consumes: Tasks 3–7.
- Produces:
  - `camera.ts`: `type CameraMode = 'orbit'|'chase'|'pad'`; `autoMode(phase: Phase, t: number, finalTime: number, reducedMotion: boolean): CameraMode` (editing → orbit; flying → chase until `finalTime − 8`, then pad; touchdown → orbit; reducedMotion never auto-switches away from orbit).
  - `<PlannedPath response pending>` (tube along `toScene(position[k])`, cyan→white by t; red pulsing when `ok:false` using a straight dashed line from start toward the pad; opacity 0.4 when `pending`).
  - `<DragHandles>`: in `editing`, pointer-drag the booster on the x–z plane (scene plane z = 0) through `clampPosition`, and the velocity arrow tip through `velocityFromHandle`; each change → `setStart`, then a 120 ms debounced `request`.
  - `<FlightLoop>`: `useFrame` dispatches `tick(dt)` while flying (dt capped at 0.1 s).

- [ ] **Step 1: Failing tests:** `editing_is_orbit`, `flying_is_chase_then_pad`, `touchdown_is_orbit`, `reduced_motion_stays_orbit`.
- [ ] **Step 2–4:** Fail → implement → pass; then components and wiring.
- [ ] **Step 5 (in-browser check):** `npm run build && npx serve web/out` (or `npm run dev`); with Playwright's Chromium (installed in Task 10's Step 1 if not yet) take screenshots of: initial state with the preview path, a dragged infeasible start (red path), mid-flight. Look at them. Expected: path visible, booster visible, no console errors. Commit `feat: add live planned path, drag editing, cameras and flight loop`.

### Task 9: HUD, sound, fallback

**Files:** Create `web/src/components/hud/{Hud.tsx, Telemetry.tsx, Sparkline.tsx, StatusPill.tsx, TouchdownCard.tsx, Controls.tsx, Fallback.tsx, Help.tsx}`, `web/src/lib/status.ts` + `status.test.ts`, `web/src/lib/sound.ts`, `web/src/components/hud/hud.module.css`.

**Interfaces — Produces:**
- `status.ts`: `statusText(s: AppState, now: number): {tone: 'info'|'ok'|'warn'|'error'; text: string}` with the exact messages from the spec's Error handling table, plus `'Planning…'` (pending), `'Still planning…'` (pending for more than 3000 ms since `requestedAt`), `'Ready: press Land'`, `'Landing'`, `'Touchdown'`.
- `Help.tsx` also renders the "Kestrel" wordmark and one-line pitch.
- `sound.ts`: `createRumble(): {setLevel(x: number): void; dispose(): void}` (WebAudio brown noise → lowpass 80–600 Hz by level → gain); created only after a user toggle.
- Controls: Land / Reset (Space), speed ×1/×2/×4, camera override, sound toggle (off by default), "simulation only" note and repo link placeholder `#`.

- [ ] **Step 1: Failing tests:** `infeasible_message`, `not_lossless_message`, `pending_shows_planning`, `slow_solve_message` (pending, now − requestedAt = 3500 → `'Still planning…'`), `ready_message`, `touchdown_message`.
- [ ] **Step 2–4:** Fail → implement → pass; then components (glass panels: `backdrop-filter: blur(14px)`, 1px border `rgba(255,255,255,.12)`; sparklines on `<canvas>` with ρ₁–ρ₂ band and cursor).
- [ ] **Step 5:** In-browser screenshots (editing, flying, touchdown, fallback forced by renaming the wasm file). Commit `feat: add telemetry hud, controls and sound`.

### Task 10: End-to-end, measurement, docs

**Files:** Create `web/playwright.config.ts`, `web/e2e/landing.spec.ts`, `web/e2e/measure.spec.ts`; modify `README.md`, `LIMITATIONS.md`, `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/SETUP.md`; add `docs/img/m1-*.png` (and `docs/img/m1-demo.gif` if ffmpeg is available).

- [ ] **Step 1:** Install `@playwright/test@1.65.0` (dev) and Chromium (`npx playwright install chromium`). `landing.spec.ts`: page loads; status reaches `Ready: press Land` within 20 s; press Space; set ×4; `Touchdown` visible within 60 s; no console errors. Run `npm run e2e` → PASS.
- [ ] **Step 2:** `measure.spec.ts` (tagged `@measure`, excluded from the default e2e run): in the worker, solve the example 10 times, report the median and range of `ms`. Record in LIMITATIONS with machine, browser version and method.
- [ ] **Step 3:** Screenshots for the README (and a GIF from a Playwright video if ffmpeg exists; otherwise screenshots only and say so).
- [ ] **Step 4:** Docs: README leads with the image/GIF and `cd web && npm install && npm run dev`; SETUP adds Node and `wasm-bindgen-cli 0.2.130`; ARCHITECTURE moves `wasm` and `web/` to built; ROADMAP ticks M1 boxes; LIMITATIONS adds M1 limits (playback only, x–z editing, illustrative booster scale, measured browser time).
- [ ] **Step 5:** Commit `test: add end-to-end landing test and browser timing` and `docs: mark M1 done with measured browser results`.
