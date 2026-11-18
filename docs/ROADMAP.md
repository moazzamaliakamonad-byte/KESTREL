# Roadmap

Rule: build the smallest end-to-end demo first, then widen. Do not build
infrastructure for a milestone that has not been reached. Tick boxes only when
the thing works and is committed.

Legend: `[ ]` not started, `[~]` in progress, `[x]` done.

## M0 — One 3-DoF landing, solved offline

Goal: prove the maths and the solver work before any browser code exists.

- [x] Install Rust toolchain (see [SETUP.md](SETUP.md))
- [x] Cargo workspace with a single `guidance` crate
- [x] Confirm Clarabel builds natively **and** for `wasm32-unknown-unknown`
      (see [DECISIONS.md](DECISIONS.md) D-003)
- [x] Vehicle/problem parameters struct with non-dimensional scaling
- [x] Discretize the convexified 3-DoF problem (fixed final time)
- [x] Solve it; check thrust bounds, glide slope and tilt limit are respected
- [x] Line search over final time (min fuel)
- [x] Export the trajectory (CSV or JSON) and plot it
- [x] Reproduce a figure from Malyuta et al. (arXiv 2106.09125) qualitatively
      (see [LIMITATIONS.md](../LIMITATIONS.md))
- [x] Unit tests: constraint satisfaction, lossless check (`‖T‖ = Γ` on the solution)
- [x] `learn/` chapter for M0 (local only)

## M1 — In the browser

Goal: one page that solves and animates one landing.

- [x] `wasm` crate exposing `plan_landing(json) -> json` (D-009)
- [x] Web page: Next.js static export + React Three Fiber (D-005, D-006)
- [x] 3D view of booster, pad and planned trajectory line, plus drag-to-edit
      start, live re-planning, cameras, telemetry HUD
- [x] Measure solve time in Chrome and Firefox; write it in LIMITATIONS.md
      (Chromium measured; Firefox dropped by the owner, 2026-11-18)
- [x] Demo GIF in README; one-command run instructions
- [x] `learn/` chapter for M1 (local only)

## M2 — Closed loop and "break it"

Goal: the demo that spreads.

- [x] Simulator separated from guidance; guidance sees only noisy state
      (`kestrel-sim` crate, D-012)
- [x] Re-solve a few times per second (MPC); tracking controller between solves
      (every 0.25 s of sim time, warm-started, D-013)
- [x] Controls: drag booster, wind gusts, engine failure, thrust lag, sensor noise
      (gusts are a sideways force, not an atmosphere model; see LIMITATIONS.md)
- [x] Infeasible case: fall back to minimum-landing-error problem and say so on screen
      (closest landing, D-014; braking when even that is impossible)
- [x] Panels: fuel, thrust, constraint margins, solve time
- [x] `learn/` chapter for M2 (local only)

## M3 — 6-DoF vehicles and flight-style solver

Goal (owner, 2026-11-15): a Starship-class simulation, as realistic as a
simulation can be, on one page with a mission-control interface.

- [x] 6-DoF rigid body with aerodynamics (`kestrel-model`, `sim::ship`)
- [x] Successive convexification (Szmuk & Açıkmeşe 2018) for the landing burn
- [x] Ship: belly flop, flip, tower catch (the Mars landing on legs was built, then removed on 2026-11-16 at the owner's request)
- [x] Booster: grid-fin descent, 13 → 3 engine landing burn, tower catch
- [x] Ship: steep fast entry start (80 km, 2.5 km/s) flies to the catch
- [x] One page: vehicle, start, engines, failures, wind, noise, lag,
      speed, cameras, physics options
- [x] Console redesign: telemetry rail, engine map, timeline, phone layout
- [x] Opening sequence (countdown, liftoff, stage separation, title card):
      an **illustration**, not simulated
- [x] After a booster catch, fly the ship's return to the same tower
- [x] Pause anywhere; analysis view (forces, plan, equations, graphs)
- [x] Analysis view as a blueprint: the vehicle alone as a line drawing on
      blue, every part clickable, the camera flies to it, its calculations
      open in the left panel; step a paused flight 0.1 s or 1 s at a time
- [x] Final approach to the catch about three times quicker (about 12 s,
      was about 35 s)
- [x] Countdown from ten: number on screen, number spoken and mission clock
      from one timeline; slower opening sequence
- [x] Flight report after each flight; keyboard help sheet
- [x] Guidance flies the ship's flip (planned from engine start, engine
      switch in the plan)
- [x] Engine-out recovery: a spare relights, guidance re-plans at once
      (15 of 20 mid-burn failures caught)
- [x] Plans ending 10 m out and 20 m above the arms; faster planner
      (ship 2.3x per iteration); events re-plan at once
- [x] Flight record: every decision with its reason, replay any instant;
      units and number sources on hover
- [x] Ship engine-out in the last seconds before the gate: an emergency
      lower gate and the navigation filter below (20 of 20 engine-out
      flights caught, was 15); an
      engine-out envelope says when a loss can no longer be recovered,
      on screen and in the sweep (D-039, D-040)

- [x] Sensor noise "high": a navigation filter (accelerometers fused with
      the readings); at high noise on new seeds ship 19 of 20 (11 without
      it) and booster 20 of 20 (11 without) (D-040)

- [x] Commentary (rendered clips) and camera-aware procedural sound
- [x] Orbital entry (round planet, heating that matters): a guided
      entry from 120 km and 7.5 km/s on a round Earth, tiles that can
      burn through, hand-over to the landing (D-042)
- [x] Custom solver: no heap allocation in the loop, bounded iterations
      (`crates/socp`; a choice on the page; Clarabel stays the default,
      D-041)
- [x] Published benchmark of solve times, both solvers, native and in
      the browser (LIMITATIONS.md, method stated)
- [x] Update the end-to-end tests and the demo recording for the new page
- [x] `learn/` chapter for M3 (local only)

## Publishing on GitHub

Not a milestone: getting the finished project public (D-045).

- [x] Checks on every push: Rust format, clippy (warnings are errors),
      tests; web lint, types, unit tests (`.github/workflows/ci.yml`)
- [x] Publish `main` to GitHub Pages under the repository's base path
      (built and flown locally under `/Kestrel/` before the first push)
- [x] Clippy clean; `CLAUDE.md` no longer tracked
- [x] History kept as it is (owner's call: the commit email and the
      two early commits that hold `CLAUDE.md` may be public)
- [ ] Repository created and pushed; Pages source set to "GitHub Actions"
- [ ] Live link and CI badge at the top of the README; `repository` in
      `Cargo.toml`

## Change log

- 2026-11-11 — Roadmap written from the project brief.
- 2026-11-12 — M0 done: offline 3-DoF fuel-optimal landing, free final time,
  qualitative match with Malyuta et al. eq. 108 (see LIMITATIONS.md).
- 2026-11-12 — M1 done (except a Firefox measurement): browser page with the
  solver in WebAssembly, live re-planning while dragging, 3D playback, HUD.
- 2026-11-14 — M2 done: closed loop with a separate simulator, noisy sensing,
  warm-started re-planning, tracking controller, closest-landing fallback,
  "break it" controls and live margins.
- 2026-11-15 — M3: 6-DoF ship (belly flop, flip, tower catch, Mars legs) with
  successive convexification; the M2 booster page removed.
- 2026-11-16 — M3: Super Heavy-class booster (grid fins, 13 → 3 engines,
  tower catch); one-page console with full customization, launch and
  staging sequence (illustration), analysis view, commentary and sound.
- 2026-11-16 — M3: Mars removed; final approach retuned (catch about 12 s
  after the approach gate); blueprint analysis view with clickable parts
  and per-part calculations; ten-second countdown on one clock; ship model
  reshaped toward the real vehicle; kestrel mark.
- 2026-11-17 — M3: guided flip, engine-out relight, approach gate 10 m out
  and 20 m up, planner 2.3x faster for the ship, event re-plans, flight
  record with replay, unit and source tooltips.
- 2026-11-18 — M3 done. The simulated ascent, hot staging and boostback
  removed from the roadmap by the owner: Kestrel is the landing guidance
  (D-044); the opening sequence stays an illustration.
- 2026-11-18 — Prepared for GitHub: CI and a GitHub Pages deploy, clippy
  clean, the agent's rules file kept local (D-045).
