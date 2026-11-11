# Kestrel

**Rocket landing you can break.** The real class of algorithm used for
propulsive rocket landing, running live in a browser tab. Drag the booster
anywhere, add wind, kill an engine mid-descent, and watch it compute a new
fuel-optimal trajectory in real time.

> _Demo GIF / live link goes here once M1 ships._

Named after the kestrel, the small falcon that hovers motionless in a headwind
before it drops onto a precise spot.

## Status

**M0 done (offline, not yet in a browser).** The guidance solves a
fuel-optimal 3-DoF landing in about 8–9 ms per convex solve on a laptop CPU,
and picks the landing time itself. Measured numbers and every known
approximation: [LIMITATIONS.md](LIMITATIONS.md). Next: M1, the browser page.
See [docs/ROADMAP.md](docs/ROADMAP.md).

![M0 example landing: path, thrust (max–min–max), tilt and mass](docs/img/m0-example-landing.png)

Try it (needs Rust; Windows also needs the Visual Studio C++ workload, see
[docs/SETUP.md](docs/SETUP.md)):

```sh
cargo test
cargo run --release -p kestrel-guidance --example land_once > target/landing.csv
python scripts/plot_trajectory.py target/landing.csv target/landing.png   # needs matplotlib
```

## How it works (short version)

1. **Guidance problem.** Find the thrust profile that lands at the target with
   zero velocity using the least fuel, within thrust limits, a glide-slope cone
   and a thrust-tilt limit.
2. **Lossless convexification.** A change of variables turns the non-convex
   problem into a convex second-order cone program (SOCP) with the same
   optimum when the theory's conditions hold, so it solves fast and
   reliably. Kestrel checks this on every solve and refuses plans where it
   failed (see [LIMITATIONS.md](LIMITATIONS.md)).
3. **Solver.** A pure-Rust conic solver, compiled to WebAssembly.
4. **Closed loop.** Re-solve from the current (noisy) state several times per
   second (model predictive control).
5. **Simulator and front end.** Disturbances you control, and a 3D view where
   the planned trajectory visibly re-bends when you interfere.

Longer version: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Install

_One-command install goes here once there is something to install._

## Honest ceiling

This is a **simulation only**. Real flight software needs flight hardware,
hardware-in-the-loop testing and certification, none of which this project has.
See [LIMITATIONS.md](LIMITATIONS.md).

## License

MIT. See [LICENSE](LICENSE).
