# Kestrel

**Rocket landing you can break.** The real class of algorithm used for
propulsive rocket landing, running live in a browser tab. Drag the booster
anywhere and watch it compute a new fuel-optimal trajectory in real time;
wind, engine failures and mid-flight interference come next (M2).

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
velocity, press **Space** to land.

## Status

- **M0 done:** fuel-optimal 3-DoF landing guidance in Rust (lossless
  convexification, Clarabel), checked against a published example.
- **M1 done:** the browser page. The solver runs as WebAssembly in a Web
  Worker, re-plans continuously while you drag (one solve at a time, always
  for the newest position), and the landing plays back in a
  procedural 3D Mars scene with live telemetry.
- **Next, M2:** closed loop with disturbances and the "break it" controls.

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
4. **Front end.** Next.js (static export) + React Three Fiber. The playback
   uses the plan's exact constant-acceleration segments, so the animation is
   the plan, not an approximation of it.
5. **Next: closed loop.** Re-solve from the current (noisy) state several
   times per second (model predictive control) while you interfere.

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
cargo test                      # Rust: guidance + WASM bridge
cd web && npm test              # TypeScript unit tests (Vitest)
cd web && npm run e2e           # opt-in: real-browser end-to-end tests (Playwright)
```

## Honest ceiling

This is a **simulation only**. Real flight software needs flight hardware,
hardware-in-the-loop testing and certification, none of which this project has.
See [LIMITATIONS.md](LIMITATIONS.md).

## License

MIT. See [LICENSE](LICENSE).
