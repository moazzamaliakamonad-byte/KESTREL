# Limitations

Every claim in this project must be measured before it is written down. This
file lists what Kestrel does **not** do, and anything that is known to be
approximate. Update it whenever a milestone lands.

## Scope

- **Simulation only.** Not flight software. No flight hardware, no
  hardware-in-the-loop testing, no certification, no safety case.
- **No real vehicle data.** Vehicle parameters are illustrative, loosely in the
  range of published examples, not taken from any real booster.

## Current state (M2 done)

- **The flight is simulated.** A point-mass simulator (RK4,
  10 ms) flies the vehicle; guidance sees only sensor readings and re-plans
  every 0.25 s of sim time; a PD tracking controller flies the latest plan in
  between. What you see is the simulator's truth, not the plan.
- **Gusts are illustrative.** A gust is a sideways force of 15 % of the
  healthy maximum thrust (≈2 kN) for 2 s, not an atmosphere or wind model
  (Mars air is far too thin to push this hard).
- **Engine failures are known to guidance instantly** (perfect fault
  detection): the next re-plan uses the reduced limits. Each failure scales
  both the minimum and maximum thrust by (6 − k)/6; at most three.
- **Sensor model.** Position and velocity errors are first-order
  Gauss–Markov per axis (correlation time 2 s; σ low 2 m / 0.2 m/s, high
  8 m / 0.8 m/s), shrinking linearly below 200 m to 10 % (a stand-in for a
  landing radar). Mass and time are exact. There is no state estimator; the
  controller and guidance use the readings directly.
- **Thrust lag** is a first-order lag on the thrust vector (0, 0.3 or 1 s);
  guidance does not model it.
- **Guidance keeps margins** (D-015): it plans to a gate 15 m above the pad
  (a fixed descent law lands from there), with 85 % of the available thrust
  and a propellant reserve. Nominal flights therefore burn more propellant
  than the open-loop optimum (229 kg vs 194 kg for the example start).
- **"Landed"** means touchdown at ≤ 2 m/s vertical and ≤ 1 m/s horizontal;
  anything faster is reported as a crash. There is no leg, tip-over or
  attitude model.
- **Some disturbance combinations crash**, by physics or by limits of this
  guidance: two engines out from the example start is unlandable (about
  1 m/s² of braking where ~2.8 km would be needed); high noise + 1 s lag +
  an engine failure was observed to end in a crash in the browser. Guidance
  then says so ("braking") rather than pretending.
- **Mid-flight grab** freezes sim time while held and teleports the vehicle
  (velocity unchanged).
- The start state is edited in the vertical x–z plane only (y = 0).
- **Displayed attitude is cosmetic.** A 3-DoF plan has no vehicle attitude;
  the booster is drawn pointing along the thrust, eased to upright over the
  last 40 m. The plan itself may end with thrust tilted up to the tilt limit
  (e.g. ~44° in the example). A real "touch down upright" constraint is
  future work (M3).
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
| Thrust direction follows a first-order lag (no gimbal or attitude dynamics) | M3 |
| No state estimator (raw sensor readings) | not planned |
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

### Closed loop (M2)

**Re-plan time in the browser.** Same machine, mains power, Chromium 158
with the real GPU (`PW_GPU=1 npx playwright test --grep "re-plan time
during"`): one nominal landing from the example start at ×1, every re-plan
timed in the worker (`performance.now()` around `replan`).

| Run | Re-plans | Median | p90 | Max |
| --- | --- | --- | --- | --- |
| 1 | 139 | 30 ms | 47 ms | 288 ms |
| 2* | 139 | 35 ms | 46 ms | 308 ms |
| 3* | 138 | 43 ms | 57 ms | 296 ms |

\* Runs 2 and 3 overlapped with a native Monte-Carlo run using the CPU.
The maximum is the first plan (a full landing-time search); warm re-plans are
three fixed-time solves. In the same session a full search for the editing
preview took a median of 356 ms (10 solves, 252–571 ms), slower than the M1
measurement (244 ms), with the dev server and builds running alongside.

**Native closed-loop scenarios** (`cargo test --release -p kestrel-wasm
--test closed_loop -- --nocapture`; seeded, so these numbers repeat exactly;
the page's loop with solve latency modelled as one 0.25 s period).

| Scenario | Outcome | Touchdown speed | From pad | Propellant | Re-plans |
| --- | --- | --- | --- | --- | --- |
| Nominal (example start) | landed | 0.55 m/s | 0.0 m | 228.6 kg | 139 |
| High noise + 1 s lag (seed 1) | landed | 0.36 m/s | 1.9 m | 291.8 kg | 188 |
| Gusts → at 5 s, ← at 15 s | landed | 0.55 m/s | 0.0 m | 235.2 kg | 144 |
| One engine out at 10 s | landed | 0.56 m/s | 0.0 m | 295.9 kg | 191 |
| Dry mass 1670 kg (pinpoint impossible) | landed, closest mode | 0.55 m/s | 246.6 m | 223.4 kg | 140 |

Unlandable cases (`-- --ignored`), which must crash cleanly:

| Scenario | Outcome | Vertical / horizontal at impact | Propellant |
| --- | --- | --- | --- |
| Two engines out at 8 s and 12 s | crash (braking) | 38.6 / 63.2 m/s | 117.1 kg |
| Three engines out at 1, 1.5, 2 s | crash | 75.3 / 54.4 m/s | 64.6 kg |

**Monte Carlo, high noise** (`cargo test --release -p kestrel-wasm --test
closed_loop monte_carlo -- --ignored --nocapture`): 50 seeds, nominal start,
no lag. **50/50 landed**; touchdown speed median 0.58 m/s, max 1.01 m/s;
distance from the pad median 1.0 m, max 2.9 m. One start, one vehicle and one
noise level: not a statistical claim about other conditions.

Not measured: noise combined with lag and failures over many seeds; any
start other than the example; speeds ×2 and ×4, where the worker's solve
time counts ×2 or ×4 in sim time, so guidance effectively reacts later.

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
