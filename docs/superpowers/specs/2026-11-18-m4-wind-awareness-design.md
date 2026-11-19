# M4.1 — Wind awareness: design

Date: 2026-11-18. Status: proposed (owner review).
Milestone M4, "smarter guidance", has three sub-projects, each with its own
spec, plan and measurement, in this order:

1. **Wind awareness** (this spec): guidance estimates the wind in flight and
   uses it everywhere it models the air.
2. **A planned final approach**: the optimizer plans onto the arms, with a
   keep-out constraint and margins from the navigation uncertainty; the
   scripted slide-and-sink becomes a fallback.
3. **A chosen approach**: each flight picks its approach bearing within the
   arms' open side, from where it comes from and the wind, so flights differ
   and the page says why.

Simulation only, like everything in Kestrel.

## Why

Guidance was not smart enough in three ways: the last approach, every
flight falling the same way, and no reaction to wind. Today the simulator blows wind and gusts at the vehicle, but guidance
never knows: the planner's model is called with zero wind
(`scvx/discretize.rs`, `Extra::default()`), the controllers model the air
"no wind known" (`ship_control::aero_torque`), and the fall's steering aims
as if the air were still. Guidance only fights the wind after it has been
pushed off its plan. Sub-projects 2 and 3 need a wind estimate (a planned
approach with drag, a bearing into the wind), so this comes first.

## Goal and success

Guidance carries an estimate of the steady wind and its uncertainty, made
only from what the vehicle can sense, and every model of the air in
guidance uses it.

Success, measured (numbers go to LIMITATIONS.md with the method):

- **Estimator:** in a steady 15 m/s crosswind, falling engines-off at the
  default starts with noise off, the estimate is within 1 m/s of the truth
  within 10 s of the start, and its 1σ covers the error at least 2 times in
  3 over the fall. At high noise the error and the time are measured and
  reported, not promised.
- **Closed loop, the wind sweep** (new, `--ignored`): ship and booster,
  steady wind 0, 5, 10, 15, 20, 25 m/s, blowing from +x, −x and +y, noise
  low and high, three seeds (7, 8, 9). Run once *before* any change (the
  baseline) and once after. Pass: at every wind speed, at least as many
  catches as the baseline, summed over directions, noise and seeds; and the
  sideways error at the approach gate lower on average at 10 m/s and above.
  The numbers are reported whatever they are.
- **No regression** on the existing boards: 12 of 12 nominal ship flights,
  20 of 20 engine-out, 32 of 33 booster timing sweep, the booster seeds
  11-30 at low and high noise, the ship seeds 11-30, the default flights of
  `ship_loop` (caught, propellant within 2 t of today). The entry corridor
  (14 of 20) is unaffected: the entry is not in this change.
- The default `cargo test` stays under two minutes.

## Non-goals

- Wind that changes with height, turbulence, or a weather forecast. The
  simulator's wind stays the same at every height, with 4 s gusts
  (LIMITATIONS.md says so). The estimator follows gusts with a lag; it does
  not predict them.
- Vertical wind. The estimate is horizontal (2 numbers).
- Wind in the entry from orbit (it has no wind in the simulator either).
- Any change to the approach geometry or the terminal controller's law
  beyond adding the wind's force (sub-projects 2 and 3).

## Design

### 1. The estimator (`crates/guidance/src/wind.rs`, new)

An extended Kalman filter with two states: the horizontal wind
`w = (w_x, w_y)`, m/s.

- **Process model:** a random walk, `w_k+1 = w_k + noise`, process noise
  chosen so the estimate can follow a 4 s gust in about 2 s (a time
  constant, tuned and logged in the decision). Initial estimate 0 with 1σ of
  15 m/s (no forecast).
- **Measurement:** the aerodynamic force the vehicle actually felt, from
  the accelerometers: `F_meas = m · Δv/h − m · g − R(q) · T − F_surfaces`,
  where `Δv/h` is the accelerometers' reading over the step, `T` the
  engines' delivered thrust (engine telemetry: chamber pressure and gimbal
  angle, which real engines report), and `F_surfaces` the shared model's
  flap or grid-fin force at their *measured* positions. The prediction is
  the shared model's body force at the airspeed `v_est − w`:
  `F_pred(w) = aero::body_force(ρ, q_est, v_est − w)`, plus the surfaces'
  force recomputed with the same airspeed. Three equations (the force's
  three components), two unknowns.
- **Linearization:** `H = ∂F_pred/∂w` by central differences of the shared
  model around the current estimate (3×2), each step.
- **Measurement noise:** what the readings' errors do to `F_pred`: the
  attitude noise (`att_sigma`) pushed through `∂F_pred/∂q` (finite
  differences, the three attitude axes), plus the velocity estimate's error
  through `∂F_pred/∂v`, plus a floor. So the filter trusts the measurement
  more at high dynamic pressure, where the wind shows, and almost ignores it
  near hover, where it does not: no special cases for "too slow to tell".
- **While the engines burn** the same equations hold, but thrust telemetry
  is less certain than the air: the thrust reading's error (a percentage of
  thrust, *illustrative*, taken as 1 %) is added to the measurement noise
  along the thrust line. The filter keeps running; near hover it mostly
  coasts on what it learned during the fall.
- **Output:** `WindEstimate { w: [f64; 2], sigma: [f64; 2] }` (1σ per
  axis), and the innovation (measured minus predicted force) for the page.
- Pure and deterministic: a struct with `new`, `update(&mut self, inputs,
  h)`, `estimate()`. No allocation. Unit-tested against the shared model,
  like the tests in `ship_control.rs`.

### 2. Wiring (`crates/wasm/src/ship_flight.rs`)

- `ShipFlight` owns a `WindFilter`. In `control_step`, after the simulator
  step, it updates with: the accelerometers' `Δv` and `h` (already kept as
  `imu_dv`, `imu_h`), the engines' delivered thrust and the surfaces'
  positions used during that step (read from the simulator, as telemetry),
  the navigation estimate, and the mass.
- `wind_estimate()` for the page and the tests; written into the snapshot.
- A switch, `wind_aware: bool` in the flight's options (default on), so the
  sweep can fly the same flight with and without it, and the page can show
  the difference.

### 3. Where guidance uses it

Every place guidance models the air, `v` becomes the airspeed `v − w`:

1. **The planner.** `ShipProblem` gets `wind: Vec3` (horizontal, z = 0),
   set from the estimate at each plan request; `discretize::f_nd` passes it
   as `Extra { wind, .. }`. The ship's plans then include the wind's drag
   and the force on the flip. The booster plans without crossflow drag
   (`c_n = 0`, D-031's reason), so for it the wind changes only the axial
   drag; its controllers carry the rest.
2. **The controllers** (`ship_control.rs`): a `wind: Vec3` argument on
   `descent`, `belly_flop`, `unpowered`, `powered`, `flip`, `steady_burn`
   and `terminal` (via one `Air` value passed down, not seven loose
   parameters).
   - `aero_torque`, the model of the body's own torque, uses the airspeed.
   - **Booster descent:** the "engines first" axis points into the
     *relative* wind (airspeed), and the lean is solved for the wanted
     sideways acceleration *minus* what the air does at zero lean, so it
     leans into a crosswind before being pushed.
   - **Ship belly flop:** the predicted miss includes the drift toward the
     wind's speed over the remaining fall (a first-order relaxation whose
     time constant comes from the shared drag model at the current height
     and speed).
   - **Terminal and powered:** the modelled wind force is added as a
     feed-forward, so the thrust leans into the wind at once, not after a
     position error has built up.
3. **`predict()`** (the first booster plan's lead, the spare's spin-up)
   integrates with the estimated wind.
4. **Unchanged:** the ignition height and fall time (vertical; a horizontal
   wind barely changes them), the engine-out envelope, the entry.

### 4. On the page

- Telemetry: a "Wind (estimated)" row: speed, the direction it blows from,
  and ±1σ. In the analysis view, two arrows at the vehicle: the estimated
  wind and the true wind, labelled as such (the page knows the truth; the
  label says guidance does not).
- Flight record: a decision when the estimate first settles (1σ under
  2 m/s: "wind estimated 12 m/s from the east, ±1.5") and when the
  estimate moves by more than 4 m/s within 3 s (a gust: "gust felt,
  +8 m/s from the south"; at most one such entry per 4 s).
- Caption lines for both, with spoken clips regenerated by
  `web/scripts/make-voice.mjs` (the speech-synthesis fallback until then).
- A "Wind awareness" switch in the dock's Environment section, next to the
  wind settings (default on), for comparison flights.

### 5. Errors and edge cases

- Thrust telemetry or the model diverging from the truth (a failed engine
  in its first moments, before `fail(i)` is seen): the innovation is large
  for a step; the noise model's thrust term absorbs most of it. A large
  innovation while an engine state changes does not count as a gust.
- The estimate is clamped to ±60 m/s (the simulator's wind is clamped to
  ±40 m/s) and its covariance kept symmetric and positive.
- Grabbed and dropped (the page's drag-to-move): the filter keeps its
  estimate (the wind did not change); the navigation reset is enough.

## Testing

- Unit (`wind.rs`): converges in a steady wind with the shared model as
  the truth; follows a step change; ignores measurement noise with
  near-zero dynamic pressure; its covariance stays positive; a burning,
  upright vehicle near hover changes the estimate little.
- Unit (`ship_control.rs`): the booster's lean in a crosswind points the
  right way at zero position error; the terminal phase's thrust leans into
  a steady wind on its first step.
- Closed loop (`crates/wasm/tests/ship_loop.rs`): a default ship flight and
  a default booster flight in a 15 m/s crosswind, caught, in the normal
  suite; `wind_sweep` (`--ignored`) as above, printing a table.
- Existing boards rerun after the change (all `--ignored`).
- Web: unit tests for the new telemetry formatting and record decisions;
  the e2e suite rerun once.

## Docs

D-046 (the estimator and where it is used), ARCHITECTURE (the new module,
the data flow), LIMITATIONS (wind section rewritten with the sweep and the
estimator's numbers; thrust telemetry taken as a 1 % reading), ROADMAP (M4
with the three sub-projects), glossary (Kalman filter, innovation,
observability, airspeed), and a `learn/` chapter (local only).

## Risks

- **Attitude noise looks like wind.** At high noise, 0.2° of attitude error
  at 400 m/s is a few m/s of apparent crosswind. The measurement noise model
  is meant to cover it; the sweep at high noise shows whether it does.
- **The booster's plan has no crossflow drag**, so in a strong crosswind the
  plan and the controller disagree more. If the sweep shows this, it is
  reported, and planning with crossflow drag is revisited in sub-project 2.
- **Retuning:** feed-forward changes what the controllers see; the existing
  boards are the guard. Any constant that has to change is logged in the
  decision with before/after numbers.
