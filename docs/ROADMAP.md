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
- [~] Measure solve time in Chrome and Firefox; write it in LIMITATIONS.md
      (Chromium measured; Firefox not yet)
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

## M3 — 6-DoF and flight-style solver

- [ ] 6-DoF rigid body with aerodynamics
- [ ] Successive convexification (Szmuk & Açıkmeşe 2018)
- [ ] Custom solver: no heap allocation in the loop, bounded iterations
- [ ] Published benchmark of solve times (method stated)
- [ ] `learn/` chapter for M3

## Change log

- 2026-11-11 — Roadmap written from the project brief.
- 2026-11-12 — M0 done: offline 3-DoF fuel-optimal landing, free final time,
  qualitative match with Malyuta et al. eq. 108 (see LIMITATIONS.md).
- 2026-11-12 — M1 done (except a Firefox measurement): browser page with the
  solver in WebAssembly, live re-planning while dragging, 3D playback, HUD.
- 2026-11-14 — M2 done: closed loop with a separate simulator, noisy sensing,
  warm-started re-planning, tracking controller, closest-landing fallback,
  "break it" controls and live margins.
