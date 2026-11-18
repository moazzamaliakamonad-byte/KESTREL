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

## Current state (M3 done: ship, booster and the one-page console)

What is simulated, and what is only shown:

- **Simulated:** the return of one vehicle at a time, from a chosen start
  in the air to the catch or loss. The start is a state the page
  sets (altitude, position, velocity), not the end of a simulated ascent.
- **Not simulated, and out of scope: the launch, ascent, stage separation
  and boostback** (Kestrel is the landing guidance, D-044).
  The opening sequence (countdown, liftoff, the stages parting at 64 km,
  the title card) is an illustration timed by hand. The screen says so
  while it plays.
- **Entry from orbit is a point mass on a round, non-rotating Earth**
  (the "From orbit" start, D-042). From 120 km at 7.5 km/s, 3 000-3 600 km
  out: the ship's position and velocity only (3-DoF), lift and drag from
  *illustrative* coefficients (lift-to-drag 0.46 at Mach 5 and above,
  fading to the belly flop's drag only below Mach 1.5), the 1976 standard
  atmosphere, gravity μ/r². No Earth rotation (no Coriolis, no rotating
  air), no attitude dynamics: the bank angle rolls at 15°/s toward what
  guidance asks. Entry guidance reads the true state (no entry
  navigation). At 25 km the 6-DoF flight takes over from the entry's
  position, velocity and attitude in the tower's frame (D-043); the
  attitude during the entry is a schedule (about 60° angle of attack,
  easing into the belly flop below Mach 5), not a dynamics result, and
  the body rates start from zero at the hand-over.
- **Heating matters in the entry, and only there.** Stagnation-point heat
  flux (Sutton–Graves, 4.5 m effective nose radius, *illustrative*) sets
  the tile temperature by radiative equilibrium (emissivity 0.85,
  *illustrative*). Above the limit (1 700 K, *illustrative*) for more than
  8 s the heat shield burns through and the flight is lost. No heat soak,
  no ablation, no tile-by-tile damage, one point on the vehicle. Nominal
  entry: peak 0.38 MW/m², 1 674 K, 2.9 g. After the hand-over heating is
  again an estimate for the picture.
- **The entry corridor** (`orbit_sweep`, `--ignored`, low noise, seed 7,
  2026-11-17): of 20 starts (3 000-3 600 km out, entry angle 0.8-1.5°
  down) 14 were caught. Too close and shallow: short of the hand-over
  window (one burned through); too far and steep: past it. The page's
  prediction flies each start. Guidance steers the heading at the tower
  and hands over within about 2.5 km of its aim when the start is on the
  ground track; with the aim moved 10 km sideways it handed over 6 km
  short, outside the window (not a supported start).
- **The globe is procedural**: noise land, sea, ice and clouds, not real
  geography. The site's coordinates (Starbase, Texas) only place the
  ground track. Around the site a 1 200 km regional ground continues the
  painted coast and fades into the globe; the 120 km painted coast lies
  flat on the tangent plane (150 m above the globe at its centre, about
  430 m at its edge 60 km out, where it fades out).
- **After the entry (and in every other start): flat ground, constant
  gravity, exponential atmosphere, no Mach effects.** The booster starts
  supersonic and the ship's 80 km start hypersonic with the same drag
  coefficients as at low speed. No plume–air
  interaction during the burn.
- **Aerodynamics are a simple model**: crossflow drag of a cylinder, a
  linear lift slope for the booster (slender-body value), flaps as drag
  plates, grid fins as a force proportional to deflection. Not wind-tunnel
  or flight data.
- **Engines share one gimbal direction and throttle**, act on the axis and
  have no start-up transient beyond a fixed delay. A failed engine removes
  its thrust but creates no off-axis torque. Failures are known to guidance
  at once. Shut-down engines do not relight.
- **The booster may end its final approach on two engines** when too light
  to hover on three at minimum throttle (a rule of this simulation; with
  the quicker approach it did not happen in the runs below).
- **The ship flies its burn on one engine** after a guided flip on all
  three (D-032). Two engines at their minimum throttle push harder than
  the nearly empty ship weighs, so they cannot fly the slow end of a
  landing (D-031). If the running engine fails, a shut-down engine is
  relit: it gives thrust after 1 s, during which the ship falls. Real
  Raptors relight in flight; whether a real ship would, and how fast, is
  not published.
- **Engine-out is survived 20 times in 20** (the running engine failed at
  49, 51, ... 67 s, noise off and low; measured 2026-11-17 after D-039 and
  D-040; 15 of 20 before both). Failures at 67 s, the last seconds before
  the gate, began just outside the engine-out envelope (-0.1 and
  -0.8 m/s) and were still caught: the arms accept 2 m/s. Other starts,
  seeds and failure times were not swept.
- **The engine-out envelope** (`guidance::envelope`, shown as
  "Engine-out" in the telemetry) is a point-mass estimate: if the running
  engine failed now, could a spare lit 1.3 s later (ignition plus throttle
  lag) at full thrust still stop the ship above the arms? It ignores
  attitude, sideways motion and sensing errors. In the sweep the latest
  failures began just outside it (by up to 0.8 m/s) and were caught;
  earlier, one that began 1.9 m/s outside was lost. The arms accept
  2 m/s, so a ship a little outside can still be caught. Below about 17 m
  above the arms nothing is survivable, not even hovering: the fall during
  the gap alone takes more height than is left.

- **The plan ends 10 m out and 20 m above the arms**; the last 20 m are
  flown by the terminal controller, which glides onto the arms (D-047:
  slides in and comes down in one movement; 9-11 s from the gate). Plans
  ending on the arms hit the mast under sensor noise (D-033), and a
  separately planned approach from the gate (`planned_approach`, off)
  caught fewer flights on every board (D-047: ship nominal 8/12, seeds
  28/40, booster 33/40, engine-out 11/20, against 12/12, 38/40, 40/40,
  20/20).
- **The catch is a test, not contact dynamics**: within 4 m of the catch
  axis, at most 2 m/s down and 1 m/s sideways, tilt under 5°, turning
  slower than 5°/s (all *illustrative*; D-026). The arms are drawn closing
  on the tower's centre line, not on the vehicle.
- **Sensor noise and the navigation filter** (D-040): guidance flies on a
  complementary filter's estimate (accelerometers, taken as exact, plus
  the noisy position and velocity readings), not on raw readings. Ship,
  seeds 11-30: 19 of 20 at high noise (11 of 20 on raw readings), 19 of
  20 at low (20 of 20 raw); seeds 7-10 at all levels 12 of 12 (11 raw).
  Booster, seeds 11-30: 20 of 20 at high noise (11 raw), 20 of 20 at low
  (19 raw). Real accelerometers drift and have noise; these do not. One
  seed per cell: differences of one or two flights are noise.
- **Wind** is the same at every height; a gust is a 4 s trapezoid.
  Guidance knows neither in advance: it **estimates** the wind in flight
  (D-046) with a two-state Kalman filter on the air's force, made from
  the accelerometers (taken as exact), engine telemetry (the thrust
  delivered, taken as a 1 % reading, *illustrative*) and the surfaces'
  positions, at the navigation estimate. In a 15 m/s crosswind it is
  within 0.05 m/s of the truth after 10 s of noise-free fall (unit test)
  and within 0.25 m/s (ship) and 0.13 m/s (booster) by ignition at low
  noise (closed loop, seed 7), with a stated 1σ of 0.8-2.7 m/s (it is
  least sure along the ship's nose, where the air's force hardly depends
  on the wind). Correlated sensor errors count once per correlation
  time, so its 1σ is honest (unit test with a drifting attitude error:
  worst error 0.8σ). It follows a gust with a lag (unit test: within
  3 m/s of a 10 m/s step within 4 s). It learns only while the engines
  are off and holds its estimate through the burn (under thrust, errors
  in the thrust line looked like up to 10 m/s of wind in still air): a
  gust that begins in the burn is not tracked. Guidance flies only the
  part beyond two standard deviations: winds under about 1-3 m/s are
  flown as calm. The planner flies in the estimate; the
  controllers lean into it; the booster's burn plan still has no
  crossflow drag (D-031), so its controller adds that part.
- **The ship cannot cross a crosswind in its belly flop**: the hull and
  flaps are symmetric about the long axis, so a roll makes no sideways
  force, and it can only slide along its nose. In a 15 m/s crosswind it
  drifts about 500 m sideways before ignition. So it turns its whole
  approach toward that drift (D-048: gate, aim, heading, keep-out;
  within 60° of the arms' open side, for a predicted drift of 400 m or
  more): 15 m/s crosswinds caught 11 of 18 (5 before). At 20 and 25 m/s
  the drift outgrows the 60° and most are still lost (4 and 0 of 18).
  The booster keeps the +x approach (its lean steers across its fall).
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
- **Shake, smoke, plume shapes, clouds, sky colours and the blueprint
  drawing** are visual only. The vehicles' shapes are by eye from public
  photographs.
- **The flight record** keeps every plan, failure, phase and engine change
  and the full state every 0.1 s; a replayed instant is the nearest
  recorded frame (up to 0.05 s away). Why a plan was asked for is worked
  out on the page from the plan and the state (drift, age, an event), the
  same rules as the flight's, not read back from it.
- **Analysis view numbers** are the snapshot's, or the model's formulas
  evaluated on it by the page (`parts.ts`): a flap's or fin's force is
  recomputed, not read from the integrator; "each engine" is the total
  divided by the engines lit. Stepping a paused flight lets plans arrive
  with no delay in sim time, so a stepped flight is not the same flight as
  one left running.
- **End-to-end tests** (`web/e2e/console.spec.ts`, opt-in, Playwright):
  a booster flight caught, a ship flight with its running engine failed
  in the landing burn and caught, pause → analysis view → open a part →
  step → resume, and reset mid-flight. 4 of 4 passed on 2026-11-17 against
  the dev server with the real GPU (Chromium, this laptop, ×1, about 4
  minutes). They check what a visitor sees, not the physics; a slow
  machine can make a flight fail (solve delay counts in sim time).

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
| No state estimator (raw sensor readings) | M3: a complementary filter on the 6-DoF page (D-040); the 3-DoF page still uses raw readings |

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

**Native scenarios** (`cargo test -p kestrel-wasm --release --test ship_loop --
--nocapture`, seed 7, low noise, 0.3 s throttle lag, solve latency modelled
as 0.5 s of sim time). Read on 2026-11-17 after the guidance redesign
(D-031 to D-036):

| Scenario | Outcome | Flight time | Propellant used | Plans / failed |
| --- | --- | --- | --- | --- |
| Booster, default start (14 km, 420 m/s) | caught | 64.5 s | 85.8 of 120 t | 15 / 0 |
| Booster, a centre engine out at ignition | caught | 64.6 s | 85.6 t | 14 / 0 |
| Booster, gusts at 8 s and 20 s | caught | 65.2 s | 86.4 t | 15 / 0 |
| Booster, every solve taking 1.0 s | caught | 63.4 s | 84.6 t | — |
| Ship, default (5 km) | caught | 85.8 s | 21.7 of 40 t | 16 / 0 |
| Ship, engine out at ignition | caught | 86.0 s | 21.0 t | 16 / 13 |
| Ship, gusts in the belly flop | caught | 82.4 s | 20.9 t | 19 / 0 |
| Ship, the running engine fails at 55 s | caught (1 relight) | 79.8 s | 19.6 t | 16 / 0 |
| Ship, the running engine fails at 61 s | caught (1 relight) | 86.4 s | 22.4 t | 19 / 1 |

**Robustness board** (`score.py`-style runs of `trace_earth` and
`engine_out_window`, `--ignored`): 12 nominal ship flights (seeds 7-10,
noise off, low, high) and 20 engine-out flights (the running engine failed
at 49, 51, … 67 s, noise off and low). Current configuration (2026-11-17,
after D-040): 12 of 12 and 20 of 20 (11 and 18 without the navigation
filter, with the fix of D-040). The decision log (D-032, D-033, D-034, D-039, D-040) lists

the configurations compared. Single seeds per cell: not a statistical claim.

**Planner timing** (2026-11-17, i7-1185G7 laptop on AC power, about 30 %
background load). Ship flip problem, native, per SCvx iteration:
discretization 2-3 ms, convex subproblem 22 ms with 24 nodes, 9.5 ms with
16 (now used), 6.7 ms with 12. Whole plans of the reference flights in
WebAssembly under Node: ship median 115-134 ms (min 53, max 319), booster
median 145-200 ms (max 519 ms with the 8-iteration cap). In a Chromium
worker on a bare page: ship 134 ms, booster 166 ms (medians). On the 3D
page the same plans were 3-4 times slower before rendering was lightened
(ship median 439 ms); after: 89 ms in one run and 276-460 ms in four later
runs, booster 498-597 ms (13-15 plans per flight, all four flights caught).
These swing with the machine's background load; a desktop with a separate
graphics card was not measured.

Single runs of other starts (`start_probe`, `--ignored`; **read before the
retune and not repeated since**, except the 80 km entry, which was re-run
and caught): ship from 25 km
caught (207 s, 12.7 t left), from 45 km caught (245 s, 6.9 t left), from
80 km at 2.5 km/s caught (204 s, 12.3 t left; top speed 2 553 m/s).
Booster from 20 km at 550 m/s caught (8.3 t left), from 6 km caught
(17.2 t left), from 10 km on the tower side caught (11.7 t left); from
16 km starting 1.4 km out with 50 m/s of drift: out of propellant; from
25, 30, 45, 60 and 70 km: out of propellant.

**Sensitivity to solve time and frame rate** (`booster_browser_sweep`,
`--ignored`): booster default start, three seeds per row. 2026-11-17: 32 of
33 caught (lost: fixed 1 s latency, seed 23, since the 8-iteration cap,
D-036). The table below is an earlier reading; its 0.8 s row is no longer in
the test.

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

- After the retune (2026-11-16, one run each, ×1, opening sequence
  skipped, while a voice-model install was running on the machine): booster
  default start caught at 72.7 s with 25.1 t left (25 plans; contact at
  1.66 m/s down, 0.14 m/s sideways, 0.44 m off the axis); ship default
  start caught at 87.9 s (13 plans). The booster's page flight is 8 s
  longer than the native one; not investigated.
- One ship flight at ×4 that had been paused, stepped and resumed: hit the
  tower ("no plan, braking"). High speeds multiply the solve delay in sim
  time; ×4 was already unreliable before (out of propellant, earlier note).
- Earlier readings (before the retune): booster caught at ×1 (81.3 s) and
  ×2 (86.0 s), solve times 64–70 ms after the worker's warm-up solve
  (without the warm-up the first solve took 1 407 ms and the flight hit the
  tower); ship caught at ×2 (100.5 s).
- Commentary: 84 clips after the countdown was split into one clip per
  number; nobody has listened to all of them. The alignment of the spoken
  count with the number on screen is by construction (one clock, clips
  preloaded) and was not measured with a recording.

Not measured: a benchmark of 6-DoF solve times; Firefox or Safari; phones
(the layout was checked at phone width only as a screenshot); frame rates;
any Monte Carlo of the M3 flights; ×2 and faster after the retune; how
the blueprint view performs on a slow GPU.

### Wind (M4.1)

**Wind sweep** (`wind_sweep`, `--ignored`; before: 2026-11-18, after:
2026-11-19): both vehicles,
steady wind 0, 5, 10, 15, 20, 25 m/s blowing from +x (the sea), −x (the
tower side) and +y (across), noise low and high, seeds 7, 8, 9, 0.3 s
throttle lag, solve latency one re-plan period (0.5 s). Caught, of 18
per cell (3 directions × 2 noise levels × 3 seeds), before M4.1 (the
code before any change, which the wind-blind switch still flies bit for
bit) and after; mean
sideways distance from the approach gate when the final approach began:

| Wind | Booster before | after | gate error | Ship before | after | gate error |
| --- | --- | --- | --- | --- | --- | --- |
| 0 m/s | 18 | 18 | 4.9 → 4.9 m | 18 | 18 | 4.0 → 4.0 m |
| 5 m/s | 18 | 17 | 4.7 → 3.7 m | 15 | 17 | 5.0 → 5.0 m |
| 10 m/s | 16 | 16 | 3.9 → 3.3 m | 13 | 15 | 5.8 → 4.4 m |
| 15 m/s | 11 | 15 | 4.0 → 4.0 m | 1 | 5 | 4.1 → 3.9 m |
| 20 m/s | 0 | 14 | 6.7 → 4.0 m | 0 | 4 | 5.5 → 4.3 m |
| 25 m/s | 0 | 5 | 8.2 → 3.2 m | 0 | 0 | 9.0 → 3.4 m |

After M4.2 and M4.3 (the glide, the chosen bearing; `wind_sweep`,
2026-11-19): 152 of 216 (booster 18, 17, 16, 15, 15, 5; ship 18, 17, 16,
11, 4, 0 at 0-25 m/s), on Clarabel.

On Kestrel's own solver, which flies since M4.4 (D-049; same day, same
build otherwise): 150 of 216 (booster 18, 17, 16, 14, 16, 5; ship 18, 16,
17, 9, 4, 0). Paired flight by flight: 4 caught only on Clarabel, 2 only
on Kestrel. The ship losses traced had the same plans and failures on
both solvers and parted only in the last seconds (small numerical
differences grow); one booster loss (15 m/s from +x, low noise, seed 9)
had subproblems stopped at the 40-iteration cap with the primal residual
stalled at 1e-4..1e-6, where Clarabel reports them infeasible.

110 of 216 caught before, 144 after (final guidance, 2026-11-19; a first
version that flew the raw estimate caught 149 but lost still-air flights,
D-046). One flight was caught before and is lost now: the booster in a
5 m/s wind from +y at high noise, seed 7 (before, a marginal catch 18.4 m
off the gate at 7.6 m/s sideways; now it never reaches the gate and
hits the tower). The criterion "at least as many at every wind speed" is
missed by that one flight. Before, the booster in 20 m/s and
more either hit the tower or reached the gate and ran out of
propellant fighting the wind in the final slide; the ship lost most
crosswind flights in its belly flop (above). Gate errors are means over
the flights that reached the gate, so they mix different flights before
and after. Three seeds per cell: not a statistical claim.

**Robustness boards, wind awareness off and on, still air**
(`wind_aware_board`, `--ignored`, same build; "off" is exactly the
guidance before M4.1, checked bit for bit by `unaware_flight_matches_today`):

| Board | Wind-blind | Wind-aware |
| --- | --- | --- |
| Ship nominal (seeds 7-10, noise off/low/high) | 12/12 | 12/12 |
| Ship seeds 11-30 (noise low/high) | 38/40 | 38/40 |
| Booster seeds 11-30 (noise low/high) | 40/40 | 40/40 |
| Ship engine-out window (49-67 s, noise off/low) | 20/20 | 20/20 |

The same two flights are lost either way (ship seed 17 at low noise and
seed 24 at high: they hit the tower), with the same propellant left: in
still air the wind guidance flies is zero (D-046). Single seeds per
cell: not a statistical claim.

**Controllers alone** (unit tests, the shared model as the truth, no
noise): the booster's descent in a 15 m/s crosswind, largest sideways
error on the way down 17.6 m without the estimate, 7.8 m with it (both
arrive within 3 m); the ship over the arms in a 10 m/s wind, mean
distance from the axis in the second 10 s 1.81 m without, 1.04 m with.
Tried and removed: a belly-flop roll against a crosswind (no effect, see
above) and a belly-flop drift term along the track (3 m worse).

### The final approach (M4.2)

`approach_board` (`--ignored`, 2026-11-19, same build, wind awareness
on): the boards of the wind section, the terminal controller with its
old stop-and-wait and with the glide (D-047). Mean over the caught
flights of the time from the gate to the catch and of the contact speeds.

| Board | Stop-and-wait | Glide |
| --- | --- | --- |
| Ship nominal (12) | 12/12 · 11.4 s · 1.50 down · 0.34 side | 12/12 · 9.1 s · 1.23 · 0.24 |
| Ship seeds 11-30 (40) | 38/40 · 13.9 s · 1.42 · 0.42 | 38/40 · 10.9 s · 1.25 · 0.31 |
| Booster seeds 11-30 (40) | 40/40 · 13.1 s · 1.61 · 0.40 | 40/40 · 10.3 s · 1.17 · 0.27 |
| Ship engine-out window (20) | 20/20 · 11.4 s · 1.46 · 0.31 | 20/20 · 9.9 s · 1.11 · 0.28 |

Speeds in m/s (the catch allows 2 down, 1 sideways). The same two ship
flights are lost both ways (seed 17 low and seed 24 high: they hit the
tower before the final approach). Wind sweep with the glide: 146 of 216
(144 without), no fewer at any wind speed. The navigation filter's
horizontal error in the final approach (`nav_error_by_noise`): ship 0.22
/ 0.94 m RMS, booster 0.28 / 1.07 m, at low / high noise (seeds 7-9).

### Solve-time benchmark (M4.4, Kestrel's solver flying)

Same method as the M3 benchmark below (native: `bench_solvers`, the
requests of one recorded flight per vehicle, each solved 5 times per
solver, median kept), 2026-11-19, same laptop, one run; Kestrel's solver
with the infeasibility certificate and best-iterate fallback (D-049):

| Requests | Solver | Median | p90 | Max |
| --- | --- | --- | --- | --- |
| Booster, 13 | Clarabel | 49.5 ms | 69.3 ms | 70.4 ms |
| Booster, 13 | Kestrel | 53.6 ms | 75.1 ms | 76.4 ms |
| Ship, 17 | Clarabel | 35.6 ms | 56.9 ms | 57.9 ms |
| Ship, 17 | Kestrel | 31.5 ms | 49.6 ms | 53.4 ms |

All solved by both. In the browser (`E2E_BASE_URL=http://localhost:3000
PW_GPU=1 npx playwright test e2e/measure.spec.ts`, dev build, headless
Chromium 158, integrated GPU, one flight each): booster caught, 14 plans,
median 217 ms, p90 432 ms, max 601 ms; ship caught, 14 plans, median
173 ms, p90 452 ms, max 729 ms. Single runs on a different day from the
M3 table: not a statistical claim, and not a like-for-like comparison
with it. The 3-DoF closed-loop tests (`closed_loop`) took 53 s on
Kestrel's solver against 85-90 s on Clarabel the same day.

### Solve-time benchmark (M3, both solvers)

What is timed: a whole plan (every SCvx iteration: discretization and
the convex subproblem), for every plan request the reference flights
make. Clarabel 0.11.1 against Kestrel's own solver (`crates/socp`,
D-041), same requests, same guidance.

**Native** (`cargo run --release -p kestrel-wasm --example bench_solvers`;
2026-11-17; i7-1185G7, 32 GB, Windows 11, mains power, about 12-28 %
background load; the flights are recorded once with Clarabel (low noise,
seed 7, 0.3 s lag, 0.5 s solve latency) and each recorded request is
solved 5 times per solver, median kept; three runs):

| Requests | Solver | Median | p90 | Max |
| --- | --- | --- | --- | --- |
| Booster, 13 | Clarabel | 66-70 ms | 73-80 ms | 78-96 ms |
| Booster, 13 | Kestrel | 88-99 ms | 112-129 ms | 117-146 ms |
| Ship, 17 | Clarabel | 54-58 ms | 88-98 ms | 93-114 ms |
| Ship, 17 | Kestrel | 48-53 ms | 81-85 ms | 97-105 ms |

Both solved every request. Kestrel's iterations cost about as much as
Clarabel's (about 0.6 ms each for the ship's 1 741-row KKT system) but it
needs more of them (15-25 per subproblem). Without keeping set-up solvers
between plans it was slower still (4-9 ms of ordering per plan).

**In the browser** (`E2E_BASE_URL=http://localhost:3000 PW_GPU=1
MEASURE_SOLVER=... npx playwright test e2e/measure.spec.ts`: one flight
per vehicle at ×1 on the full 3D page, dev build, headless Chromium 158
with the laptop's integrated GPU, each plan timed in the worker; single
runs):

| Flight | Solver | Plans | Median | p90 | Max | Outcome |
| --- | --- | --- | --- | --- | --- | --- |
| Booster | Clarabel | 11 | 342 ms | 623 ms | 646 ms | caught |
| Ship | Clarabel | 15 | 295 ms | 625 ms | 1 879 ms | caught |
| Booster | Kestrel | 17 | 302 ms | 983 ms | 1 330 ms | hit the tower |
| Ship | Kestrel | 13 | 373 ms | 582 ms | 748 ms | caught |

On the page plans take 4-6 times longer than natively: rendering and
planning share one laptop's power and memory budget (D-037). The booster
is sensitive to late plans (D-036); its loss with Kestrel's solver came
with its slowest plans (about 1 s). One flight per row: not a statistical
claim. Not measured: other machines, Firefox, Safari, phones.

**Run-time bound.** Kestrel's solver stops after 40 interior-point
iterations, each a fixed amount of work for a given problem shape, so
its worst case per subproblem is bounded; the SCvx loop around it is
capped at 15 iterations (ship) and 8 (booster). Measured worst cases are
in the tables; the bound itself was not timed.

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
