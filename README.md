# Kestrel

**Rocket landing you can break.** The real class of algorithm used for
propulsive rocket landing, running live in a browser tab. Drag the booster
anywhere and watch it compute a new fuel-optimal trajectory in real time.
Then press Land and break it: gusts, engine failures, thrust lag, sensor noise,
or grab the booster mid-flight. Guidance re-plans several times a second from
noisy sensor readings, and when no pinpoint landing exists any more it says so
and lands as close as it can.

![Kestrel demo: dragging the booster re-plans the landing live, then it flies the plan to touchdown](docs/img/m1-demo.gif)

<sub>Recorded with a frame-capture script, which slows the solver; the solve
times shown in the GIF (1–2 s) are inflated by it. Measured in Chromium on a
laptop GPU: **244 ms median** per full plan when plugged in, ~700 ms on battery
(see [LIMITATIONS.md](LIMITATIONS.md)).</sub>

Named after the kestrel, the small falcon that hovers motionless in a headwind
before it drops onto a precise spot.

## Run it

Needs Node 22+, Rust with the `wasm32-unknown-unknown` target and
`wasm-bindgen-cli` 0.2.130 (one-time setup: [docs/SETUP.md](docs/SETUP.md)).

```sh
cd web && npm install && npm run dev      # → http://localhost:3000
```

`npm run dev` compiles the Rust solver to WebAssembly first, then serves the
page. Drag the ring to move the booster, drag the orange dot to set its
velocity, press **Space** to land. During the flight use the **Break it**
panel (left), or drag the ring on the booster; the **Guidance** panel (right)
shows the constraint margins and how long each re-plan took.

## Status

- **M0 done:** fuel-optimal 3-DoF landing guidance in Rust (lossless
  convexification, Clarabel), checked against a published example.
- **M1 done:** the browser page. The solver runs as WebAssembly in a Web
  Worker, re-plans continuously while you drag (one solve at a time, always
  for the newest position), and the landing plays back in a
  procedural 3D Mars scene with live telemetry.
- **M2 done:** closed loop. A simulator (Rust, separate from guidance) flies
  the vehicle; guidance sees only noisy sensor readings and re-plans every
  0.25 s of sim time (**30–43 ms median per re-plan** in Chromium, measured);
  a tracking controller flies the latest plan between solves. Disturbances
  from the "Break it" panel; a closest-landing fallback when the pad is out
  of reach.

Measured numbers and every known approximation:
[LIMITATIONS.md](LIMITATIONS.md). Plan: [docs/ROADMAP.md](docs/ROADMAP.md).

## How it works (short version)

1. **Guidance problem.** Find the thrust profile that lands at the target with
   zero velocity using the least fuel, within thrust limits, a glide-slope cone
   and a thrust-tilt limit.
2. **Lossless convexification.** A change of variables turns the non-convex
   problem into a convex second-order cone program (SOCP) with the same
   optimum when the theory's conditions hold, so it solves fast and
   reliably. Kestrel checks this on every solve and refuses plans where it
   failed (see [LIMITATIONS.md](LIMITATIONS.md)).
3. **Solver.** A pure-Rust conic solver (Clarabel), compiled to WebAssembly
   and run in a Web Worker so the page never freezes.
4. **Closed loop.** A simulator owns the true state; guidance only gets
   sensor readings. Every 0.25 s of sim time it re-plans from the latest
   reading (model predictive control), warm-started from the previous plan's
   landing time (3 small solves instead of ~25). Between re-plans a tracking
   controller flies the plan. If no pinpoint landing exists, guidance solves
   a second problem: land at rest as close to the pad as possible.
5. **Front end.** Next.js (static export) + React Three Fiber. The simulator
   runs on the page's main thread (WebAssembly), guidance in a Web Worker.

Longer version: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Offline (Rust only)

```sh
cargo test
cargo run --release -p kestrel-guidance --example land_once > target/landing.csv
python scripts/plot_trajectory.py target/landing.csv target/landing.png   # needs matplotlib
```

![M0 example landing: path, thrust (max–min–max), tilt and mass](docs/img/m0-example-landing.png)

## Tests

```sh
cargo test                      # Rust: guidance, simulator, WASM bridge, closed-loop scenarios
cd web && npm test              # TypeScript unit tests (Vitest)
cd web && npm run e2e           # opt-in: real-browser end-to-end tests (Playwright)
```

## Honest ceiling

This is a **simulation only**. Real flight software needs flight hardware,
hardware-in-the-loop testing and certification, none of which this project has.
See [LIMITATIONS.md](LIMITATIONS.md).

## License

MIT. See [LICENSE](LICENSE).
