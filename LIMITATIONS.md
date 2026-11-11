# Limitations

Every claim in this project must be measured before it is written down. This
file lists what Kestrel does **not** do, and anything that is known to be
approximate. Update it whenever a milestone lands.

## Scope

- **Simulation only.** Not flight software. No flight hardware, no
  hardware-in-the-loop testing, no certification, no safety case.
- **No real vehicle data.** Vehicle parameters are illustrative, loosely in the
  range of published examples, not taken from any real booster.

## Current state (M0 done)

- Guidance solves offline in Rust, natively. **Nothing runs in a browser yet**
  (M1). The crate compiles for `wasm32-unknown-unknown`, but no browser solve
  time has been measured.
- No closed loop, no disturbances, no simulator yet (M2).
- Fuel versus landing time is assumed unimodal for the golden-section search.
  This holds for the cases tested and is cited by Malyuta et al. for this
  problem class; it is not proven in this code.

## Known approximations in the M0 guidance

- **Thrust drifts slightly within each time slice.** Each interval holds
  thrust *per kilogram* (`u = T/m`) constant while mass falls, so the actual
  thrust `m·u` drifts within the interval. Reported thrust uses the mass at
  the interval's start. Measured on the example at N = 50 (t_f = 50 s): the
  thrust implied by the propellant burned in an interval differs from the
  reported thrust by up to 0.18%. At minimum-thrust intervals the true thrust
  can therefore dip up to ~0.2% below ρ₁ near the interval's end. Shrinks
  with more intervals.
- **Thrust limits use conservative Taylor approximations** (tangent line for
  the maximum, quadratic for the minimum, around the full-thrust mass
  profile). Plans never exceed the true limits; a little performance may be
  given up.
- Constraints are enforced at nodes (glide slope, ground) and per interval
  (thrust, tilt), not continuously between nodes.

## Known modelling simplifications (planned)

These are deliberate and will be revisited at the milestone noted.

| Simplification | Until |
| --- | --- |
| 3-DoF point-mass dynamics (no attitude, no rotation) | M3 |
| Flat, non-rotating ground; constant gravity | indefinitely |
| No aerodynamic forces | M3 |
| Thrust direction changes instantly | M2 (thrust lag) / M3 |
| Perfect state knowledge | M2 (sensor noise) |
| No fuel slosh | stretch goal |

## Measured results

Each entry states hardware, problem size and how it was measured.

### Solve time (native, M0)

- Machine: Intel Core i7-1185G7 @ 3.00 GHz, 32 GB RAM, Windows 11.
- Build: `cargo run --release -p kestrel-guidance --example land_once`,
  Rust 1.99.1, Clarabel 0.11.1, default settings.
- Problem: `LandingProblem::example()`, N = 50 intervals (557 variables).
- Method: wall clock (`std::time::Instant`) around the call, 3 runs.
- **One fixed-time solve: 8.3–12.6 ms. Full free-time search (~25 solves):
  290–323 ms.**
- Result (identical in all runs): landing time 36.62 s, 194.39 kg propellant.

The full search is far too slow to re-run every frame; M2 will need to warm-
start from the previous landing time instead of searching from scratch.

### Comparison with Malyuta et al. (arXiv:2106.09125, eq. 108, Fig. 21)

`examples/malyuta_lcvx.rs` uses the paper's parameters (N = 75, Δt ≈ 1 s as
in the paper).

| | Paper | Kestrel |
| --- | --- | --- |
| Optimal time of flight | 75 s | 75.55 s |
| Minimum-thrust segment | "about 40 to 70 seconds" | ~37.3 s to ~66.5 s |
| Thrust profile | max–min–max | max–min–max |
| Glide slope activations | twice (mid-flight, touchdown) | twice (visually, same places) |
| Propellant used | not stated | 337.79 kg |

Agreement is qualitative. Our model differs from the paper's: no planet
rotation (paper includes Coriolis and centrifugal terms), an exact glide-slope
cone instead of four half-spaces, and no 500 km/h speed limit (reported
inactive in the paper). The comparison was made against the paper's text and
figure description, not against its plotted data.
Plot: [docs/img/m0-malyuta-comparison.png](docs/img/m0-malyuta-comparison.png).
