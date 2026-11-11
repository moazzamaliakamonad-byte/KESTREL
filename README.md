# Kestrel

**Rocket landing you can break.** The real class of algorithm used for
propulsive rocket landing, running live in a browser tab. Drag the booster
anywhere, add wind, kill an engine mid-descent, and watch it compute a new
fuel-optimal trajectory in real time.

> _Demo GIF / live link goes here once M1 ships._

Named after the kestrel, the small falcon that hovers motionless in a headwind
before it drops onto a precise spot.

## Status

Pre-M0. Documentation and planning only; no code yet. See
[docs/ROADMAP.md](docs/ROADMAP.md).

## How it works (short version)

1. **Guidance problem.** Find the thrust profile that lands at the target with
   zero velocity using the least fuel, within thrust limits, a glide-slope cone
   and a thrust-tilt limit.
2. **Lossless convexification.** A change of variables turns the non-convex
   problem into a convex second-order cone program (SOCP) with the same
   optimum, so it solves fast and reliably.
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
