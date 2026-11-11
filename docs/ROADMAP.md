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
- [ ] Vehicle/problem parameters struct with non-dimensional scaling
- [ ] Discretize the convexified 3-DoF problem (fixed final time)
- [ ] Solve it; check thrust bounds, glide slope and tilt limit are respected
- [ ] Line search over final time (min fuel)
- [ ] Export the trajectory (CSV or JSON) and plot it
- [ ] Reproduce a figure from Malyuta et al. (arXiv 2106.09125) qualitatively
- [ ] Unit tests: constraint satisfaction, lossless check (`‖T‖ = Γ` on the solution)
- [ ] `learn/` chapter for M0

## M1 — In the browser

Goal: one page that solves and animates one landing.

- [ ] `wasm` crate exposing `solve(state, params) -> trajectory`
- [ ] Minimal web page (build tool chosen at the time; record in DECISIONS.md)
- [ ] 3D view of booster, pad and planned trajectory line
- [ ] Measure solve time in Chrome and Firefox; write it in LIMITATIONS.md
- [ ] Demo GIF in README; one-command run instructions
- [ ] `learn/` chapter for M1

## M2 — Closed loop and "break it"

Goal: the demo that spreads.

- [ ] Simulator separated from guidance; guidance sees only noisy state
- [ ] Re-solve a few times per second (MPC); tracking controller between solves
- [ ] Controls: drag booster, wind gusts, engine failure, thrust lag, sensor noise
- [ ] Infeasible case: fall back to minimum-landing-error problem and say so on screen
- [ ] Panels: fuel, thrust, constraint margins, solve time
- [ ] `learn/` chapter for M2

## M3 — 6-DoF and flight-style solver

- [ ] 6-DoF rigid body with aerodynamics
- [ ] Successive convexification (Szmuk & Açıkmeşe 2018)
- [ ] Custom solver: no heap allocation in the loop, bounded iterations
- [ ] Published benchmark of solve times (method stated)
- [ ] `learn/` chapter for M3

## Change log

- 2026-11-11 — Roadmap written from the project brief.
