# Limitations

Every claim in this project must be measured before it is written down. This
file lists what Kestrel does **not** do, and anything that is known to be
approximate. Update it whenever a milestone lands.

## Scope

- **Simulation only.** Not flight software. No flight hardware, no
  hardware-in-the-loop testing, no certification, no safety case.
- **No real vehicle data.** Vehicle parameters are illustrative, loosely in the
  range of published examples, not taken from any real booster.

## Current state (M1 done)

- The browser page **plays back the plan**; there is no physics simulator,
  no disturbances and no re-planning during flight yet (M2). What you see
  flying is exactly the solver's plan.
- The start state is edited in the vertical x–z plane only (y = 0).
- **Displayed attitude is cosmetic.** A 3-DoF plan has no vehicle attitude;
  the booster is drawn pointing along the thrust, eased to upright over the
  last 40 m. The plan itself may end with thrust tilted up to the tilt limit
  (e.g. ~44° in the example). A real "touch down upright" constraint is
  future work (M2/M3).
- The booster model (12 m × 3.7 m) is illustrative, not to scale with the
  1.9 t example vehicle's mass, and not any real vehicle.
- Tested in Chromium 158 (Playwright) only. Other browsers are untested.
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
  profile). The constraints the solver sees are stricter than the true
  limits; a little performance may be given up.
- **Lossless convexification is checked, not assumed.** The relaxation
  `‖u‖ ≤ σ` is only valid when it is tight at the optimum, and the theory's
  conditions can fail, e.g. on purely vertical descents, where the solver
  returned plans with thrust down to 0–71% of the minimum (found in the M0
  review). Every solve now checks `‖u_k‖ ≥ σ_k·(1 − 10⁻⁴)` on every interval
  and returns `GuidanceError::NotLossless` instead of a plan if it fails.
  The landing-time search skips such times. **Consequence:** in those cases
  the returned plan is valid but only optimal among landing times where the
  relaxation was tight; it may use slightly more fuel than the true optimum.
- **Cases checked by tests:** the example, vertical descents from rest and
  near the ground, tilt limits 10°/45°/90°, an upward-moving start, a
  tight-propellant case (195 kg), and absurd speeds. Every `Ok` plan in these
  passes all physical checks. Other inputs are not yet covered by a
  randomised test (planned before M2).
- **Landing-time search** assumes fuel versus landing time is unimodal. It
  searches with the propellant limit off (so narrow feasible windows can't be
  skipped) and applies the limit once at the best time.
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
- Method: wall clock (`std::time::Instant`) around the call, 3 runs
  (re-measured after the review fixes, 2026-11-12).
- **One fixed-time solve: 7.9–9.3 ms. Full free-time search (~25 solves):
  226–259 ms.**
- Result (identical in all runs): landing time 36.62 s, 194.39 kg propellant.

The full search is far too slow to re-run every frame; M2 will need to warm-
start from the previous landing time instead of searching from scratch.

### Solve time in the browser (M1)

Same machine (i7-1185G7, 32 GB, Windows 11), example problem, N = 50, full
free-time search (one `plan_landing` call), WASM built `--release`.

| Where | Method | Median | Range |
| --- | --- | --- | --- |
| Chromium 158, real GPU (`PW_GPU=1 npm run measure`) | 10 re-plans timed in the worker (`performance.now()`) while the 3D page renders | **244 ms** | 218–295 ms |
| Node 24 V8 (`npm run bench`) | 10 runs after 2 warm-ups, no rendering | 258 ms | 241–297 ms |
| Chromium 158, software WebGL (`npm run measure`) | as the first row, but rendering on the CPU | 1718 ms | 1499–4444 ms |

The rows above were measured on mains power. Re-measured 2026-11-13 **on
battery** (80%, discharging) with the dev server running, after the review
fixes, three runs each:

| Where | Median | Range |
| --- | --- | --- |
| Chromium 158, real GPU, per solve | 676–707 ms | 358–866 ms |
| Node 24 V8, per solve | 305–307 ms | 291–384 ms |
| Chromium 158, real GPU, **after a drag**: eight start changes 140 ms apart, then time until "Ready" | 859–890 ms | — |

So speed depends strongly on the power state (integrated GPU and CPU share
the budget). After a drag, at most one stale solve finishes before the newest
one starts (one solve in flight), so the wait is about one to two solves. The
spec's "~0.5 s after release" is met only on mains power; it has not been
re-measured plugged in after the fix.

So in a browser with a GPU the solver runs at about native speed (M0: 226–259
ms). On machines without GPU acceleration, rendering competes for the CPU and
a re-plan can take ~2 s. The demo GIF was recorded while capturing frames,
which also slows solving; its on-screen solve times are not representative.

### Comparison with Malyuta et al. (arXiv:2106.09125, eq. 108, Fig. 21)

`examples/malyuta_lcvx.rs` uses the paper's parameters (N = 75, Δt ≈ 1 s as
in the paper).

| | Paper | Kestrel |
| --- | --- | --- |
| Optimal time of flight | 75 s | 75.56 s |
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
