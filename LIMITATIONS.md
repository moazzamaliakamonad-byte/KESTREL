# Limitations

Every claim in this project must be measured before it is written down. This
file lists what Kestrel does **not** do, and anything that is known to be
approximate. Update it whenever a milestone lands.

## Scope

- **Simulation only.** Not flight software. No flight hardware, no
  hardware-in-the-loop testing, no certification, no safety case.
- **No real vehicle data.** The M3 vehicles use published outer figures
  (height, diameter, engine count and thrust, specific impulse, throttle
  range, empty mass, number of grid fins) from public sources, cited in
  `docs/superpowers/specs/`. Everything else (aerodynamic coefficients,
  centre of mass, inertia, gimbal range, landing propellant, fin size,
  catch geometry, tower dimensions) is not published and is *illustrative*:
  chosen to be plausible. The page marks each number one way or the other.
  The models are generic "Starship-class" shapes, not anyone's design.

## Current state (M3: ship, booster and the one-page console)

What is simulated, and what is only shown:

- **Simulated:** the return of one vehicle at a time, from a chosen start
  in the air to the catch, landing or loss. The start is a state the page
  sets (altitude, position, velocity), not the end of a simulated ascent.
- **Not simulated: the launch, ascent, stage separation and boostback.**
  The opening sequence (countdown, liftoff, the stages parting at 64 km,
  the title card) is an illustration timed by hand. The screen says so
  while it plays.
- **Not simulated: orbital entry.** The ship's "Entry" start is a steep
  fall from 80 km at 2.5 km/s (about Mach 7), not an entry from orbit
  (7.8 km/s, a shallow path over thousands of kilometres on a round
  planet). In the dense air it decelerates far harder than a real vehicle
  or crew could take; nothing limits it.
- **Heating is an estimate for the picture only.** The glow and the
  "Heating" readout use the Sutton–Graves stagnation-point formula with a
  4.5 m nose radius. Heat does nothing to the vehicle. In every start the
  page offers it stays below about 0.15 MW/m² (a real orbital entry is in
  the MW/m² range).
- **Flat ground, constant gravity, exponential atmosphere, no Mach
  effects.** The booster starts supersonic and the ship's entry start
  hypersonic with the same drag coefficients as at low speed. No plume–air
  interaction during the burn.
- **Aerodynamics are a simple model**: crossflow drag of a cylinder, a
  linear lift slope for the booster (slender-body value), flaps as drag
  plates, grid fins as a force proportional to deflection. Not wind-tunnel
  or flight data.
- **Engines share one gimbal direction and throttle**, act on the axis and
  have no start-up transient beyond a fixed delay. A failed engine removes
  its thrust but creates no off-axis torque. Failures are known to guidance
  at once. Shut-down engines do not relight.
- **The booster ends its final approach on two engines** when too light to
  hover on three at minimum throttle (a rule of this simulation).
- **The final approach is slow.** The terminal controller takes about 25 s
  to slide over the axis and settle; real catches are far quicker. It costs
  propellant: the booster's nominal flight ends with about 15 t of 120 t.
- **The tower's arms do not move to meet the vehicle** (they close when it
  is within reach); "caught" is a test of position, speed, tilt and rate at
  arm height, not contact dynamics.
- **Wind** is the same at every height; a gust is a 4 s trapezoid. The
  planner knows neither.
- **Start envelope.** Booster: caught from 6–20 km at 300–600 m/s in the
  starts tried; from 25 km and above it reaches the tower area but the
  burn's plan fails and it runs out of propellant. A sideways divert of
  more than about 150 m during the 13-engine burn is refused. Ship: caught
  from 5, 25, 45 and 80 km (single runs each). The prediction shown before
  a flight says whether a chosen start works.
- **Sound** is synthesized (filtered noise and low oscillators) and mixed
  by rules chosen by ear: distance attenuation, a low-pass that tightens
  with distance, a delay of distance / 343 m/s. Levels are not physical.
- **Commentary** lines are triggered by simulated events; the voice is a
  text-to-speech model's (D-023), pre-rendered.
- **Shake, smoke, plume shapes, clouds, sky colours and the "blueprint"
  look** are visual only.
- **Mars:** ship only; 160 t of cargo is assumed so one engine can hover.
- The page has no end-to-end test yet (the old Playwright tests drove the
  M2 page and no longer apply); it was checked by hand and with scripted
  headless runs.

## The earlier 3-DoF page (M0–M2; code kept, no longer served)

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

### Ship and booster closed loop (M3)

**Native scenarios** (`cargo test -p kestrel-wasm --test ship_loop --
--nocapture`, seed 7, low noise, 0.3 s throttle lag, solve latency modelled
as 0.5 s of sim time; test profile is optimized). Read on 2026-11-16:

| Scenario | Outcome | Flight time | Propellant used | Plans / failed |
| --- | --- | --- | --- | --- |
| Booster, default start (14 km, 420 m/s) | caught | 85.3 s | 104.5 of 120 t | 15 / 0 |
| Booster, a centre engine out at ignition | caught | 86.1 s | 105.4 t | 15 / 0 |
| Booster, gusts at 8 s and 20 s | caught | 85.3 s | 104.5 t | 15 / 0 |
| Booster, every solve taking 1.0 s | caught | 85.0 s | 104.4 t | — |
| Ship, Earth default (5 km) | caught | 98.8 s | 25.7 of 40 t | 11 / 0 |
| Ship, Earth, engine out at ignition | caught | 101.6 s | 26.7 t | 12 / 0 |
| Ship, Earth, gusts in the belly flop | caught | 105.7 s | 27.7 t | 10 / 3 |
| Ship, Mars default | landed | 106.5 s | 86.8 of 100 t | 30 / 0 |

Single runs of other starts (`start_probe`, `--ignored`): ship from 25 km
caught (207 s, 12.7 t left), from 45 km caught (245 s, 6.9 t left), from
80 km at 2.5 km/s caught (204 s, 12.3 t left; top speed 2 553 m/s).
Booster from 20 km at 550 m/s caught (8.3 t left), from 6 km caught
(17.2 t left), from 10 km on the tower side caught (11.7 t left); from
16 km starting 1.4 km out with 50 m/s of drift: out of propellant; from
25, 30, 45, 60 and 70 km: out of propellant.

**Sensitivity to solve time and frame rate** (`booster_browser_sweep`,
`--ignored`): booster default start, three seeds per row.

| Solve latency | Reported solve time | Sim step per frame | Caught |
| --- | --- | --- | --- |
| 0.1 s | 0.15 s | 1/60 s | 3 / 3 |
| 0.25–0.3 s | 0.3 s | 1/60, 1/15, 0.1, 0.2 s | 12 / 12 |
| 0.5 s | 0 (no prediction) | 1/60 s | 3 / 3 |
| 0.5 s | 0.5–0.55 s | 1/60, 1/15, 0.133, 0.2 s | 12 / 12 |
| 1.0 s | 1.0 s | 1/60 s | 3 / 3 (one with 7 t left) |
| 0.8 s | 0.85 s | 1/15 s | 1 / 3 (two hit the tower) |

One start, one noise level, three seeds: not a statistical claim.

**In the browser** (headless Chromium via Playwright with the real GPU,
dev build, this laptop, other work running; scripted flights with the
opening sequence skipped; 2026-11-16):

- Booster default start: caught at ×1 (81.3 s) and ×2 (86.0 s); solve times
  shown by the page 64–70 ms after the worker's warm-up solve. Before the
  warm-up existed the first solve took 1 407 ms and the flight hit the tower.
- Ship, Earth default at ×2: caught (100.5 s; last solve 89 ms).
- **Ship, Mars default: fails in the browser.** At ×1 and at ×2, on the
  idle machine, the ship reached the final descent and ran out of
  propellant a few metres above the ground. The page showed solve times of
  1.8–3.4 s for Mars plans (the Earth flights: 64–89 ms): at that delay
  plans arrive stale, guidance brakes and hovers, and the propellant goes.
  Natively, with the delay modelled as 0.5 s, the same flight lands
  (table above). Open: make the Mars plans converge faster or tolerate the
  delay. Until then the Mars flight on the page is not reliable.
- Ship, Earth default at ×4 while the machine was busy rendering audio
  (solves of 1–1.7 s, i.e. 4–7 s of sim time): out of propellant. High
  speeds multiply the solve delay in sim time.
- Commentary: 77 clips, 1.8 MB in total; the page fetched and decoded them
  (checked for two lines; nobody has listened to all 77).

Not measured: a benchmark of 6-DoF solve times; Firefox or Safari; phones
(the layout was checked at phone width only as a screenshot); frame rates;
any Monte Carlo of the M3 flights; noise "high" with the booster.

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
