# M3 design — 6-DoF ship: belly flop, flip, tower catch (Earth) and legs (Mars)

Status: approved by the owner's standing delegation (they make no gate
reviews; decisions are reported). Simulation only.

## Requirements (2026-11-15)

- Continue with M3 (6-DoF, successive convexification, custom solver, benchmark).
- Make the vehicle like SpaceX's Starship upper stage, researched in depth:
  no landing legs on Earth, **caught by the tower's arms**; it falls
  **belly first** (heat-shield side into the wind), steered by **flaps**,
  then **flips upright** with its engines just before the catch; **cold-gas
  thrusters** for attitude; **several main engines**, not one.
- **Real scale.** Earth now, with a tower. Also a **Mars** landing on legs:
  two scenarios.

## Research summary (sources read 2026-11-15)

Public, secondary sources (Wikipedia, Everyday Astronaut 2021, Teslarati,
CNBC); several numbers are not published and are marked *illustrative*.

| Quantity | Value used | Source / status |
| --- | --- | --- |
| Ship height × diameter | 52 m × 9 m | Wikipedia (Block 2: 52.1 m) |
| Dry mass | 100 t | Wikipedia (Block 1 ~100 t; Block 2 85 t flagged) |
| Engines | 3 sea-level Raptors (gimbal, relight) + 3 vacuum Raptors (fixed, not used to land) | Wikipedia |
| Raptor 3 SL thrust | 2.45 MN (250 tf) | Wikipedia Raptor infobox |
| Throttle | 40–100 % | Wikipedia, Everyday Astronaut |
| SL Isp | 327 s | Wikipedia Raptor infobox |
| Gimbal range | ±15° | *illustrative* (not published) |
| Belly-flop terminal speed | ~90 m/s before the flip | Everyday Astronaut (Flight Club sim of SN8) |
| Flip start | ~500–550 m, two or three engines | Everyday Astronaut |
| Flaps | 4: two forward (leeward-shifted on Block 2), two aft | Wikipedia |
| RCS | cold gas from ullage gas: 4 below the payload bay, 2 on the LOX tank | Wikipedia |
| Heat shield | ~18 000 black hex tiles on the windward (belly) side | Wikipedia |
| Ship catch | first ship catch by the tower arms, Flight 15, 14 Nov 2026 | CNBC, Teslarati |
| Catch points | reinforced lifting points near the forward flaps | Wikipedia |
| Tower height | ~145 m | *approximate*, commonly reported; not verified |
| Arm height, catch speed, pin geometry, landing propellant | — | *illustrative*, not published |
| Mars legs | planned, design unpublished | *illustrative* (6 legs) |

The vehicle in Kestrel is a **generic "Starship-class" ship** built from
these public figures, drawn procedurally (D-010): not SpaceX's design, no
logos, no third-party assets.

## Scope

In:

1. `sim::ship`: 6-DoF rigid body (quaternion attitude) with mass-scaled
   inertia; Earth/Mars gravity and exponential atmosphere; crossflow aero
   model of a long cylinder; four flaps; three sea-level engines (shared
   gimbal, throttle, ignition delay, lag, shutdown, failure); six cold-gas
   RCS thrusters; wind gusts as real air velocity; outcomes: caught
   (tower arms), landed (legs), crashed (with a reason).
2. `guidance::scvx`: 6-DoF free-final-time successive convexification
   (Szmuk & Açıkmeşe 2018, arXiv:1802.03827): first-order-hold
   discretization, virtual control, trust region, Clarabel subproblems.
   Plans the flip and landing burn from engine ignition to a terminal gate.
3. `guidance::ship_control`: belly-flop flap/RCS attitude control with
   pitch steering toward the flip point; powered tracking controller
   (thrust vector + attitude via gimbal, RCS and flaps); terminal descent.
4. `wasm`: `ShipFlight` (main thread) and `plan_ship` (worker), same
   newest-wins MPC pattern as M2.
5. `web/`: scenario switch (Earth catch / Mars legs); procedural ship
   (steel body, tiled belly, flaps, 6 engine bells, RCS puffs, per-engine
   plumes, legs for Mars); Earth scene (coast, sea, tower with arms that
   close); new HUD: phase, engines lit, flap angles, attitude, dynamic
   pressure, catch/landing verdict.
6. Docs, measurements, `learn/` chapter.

Later in M3 (separate plan once the above works): custom conic solver with
no heap allocation in the loop and bounded iterations; published benchmark.

Out: hypersonic re-entry (plasma, heating), Mach-dependent aero,
supersonic-retropropulsion aero interaction, fuel slosh, structural
loads, tower arm dynamics, the 3-DoF booster UI (its Rust code stays).

## Earth catch geometry (illustrative)

Tower mast centred at x = −14 m, 145 m tall, 9 m square. Arms at
z = 110 m reach +x from the mast; the catch axis is x = y = 0. Catch
pins 40 m above the ship's base, so when caught the centre of mass is at
z = 110 − 18 = 92 m. Guidance gate: 6 m above that.

## Frames and state

Inertial: metres, `z` up, origin at the target point on the ground
(Earth: below the catch point; Mars: the pad). Body: `b_z` along the axis
toward the nose, `b_x` out of the heat-shield (belly), `b_y = b_z × b_x`.
State `x = [m, r(3), v(3), q(4), ω(3)]`, `q` scalar-first body→inertial,
`ω` in body axes.

## Vehicle (illustrative numbers, from the table)

- Length 52 m, radius 4.5 m; centre of mass 22 m above the base; engine
  gimbal point at the base (lever 22 m); inertia of a 52 × 9 m cylinder
  scaled with mass: `J = m · diag(230, 230, 15)` m² (axes x, y, z).
- Engines: 3 × 2.45 MN, 40–100 % each; `α = 1/(327·g0)`; lit together,
  shared gimbal ≤ 15°; ignition 1.0 s to light, then first-order lag
  (0.3 s default, settable); an engine shut down in flight stays off.
  Engine manager: light all healthy engines at ignition; shut one down
  when the command stays below 75 % of (lit − 1) engines' maximum for 1 s,
  never below 1 lit.
- RCS: 6 × 4 kN cold-gas (illustrative), on/off; 4 near the nose, 2 aft.
- Flaps: forward pair 22 m ahead of the CoM, aft pair 20 m behind,
  each with an exposure `e ∈ [0.1, 1]`; exposed area 40 m² (forward 25 m²).
- Earth: 100 t dry + 25 t propellant; start ~10 km at belly-flop speed.
  Mars: 100 t dry + 160 t cargo + 70 t propellant (heavy enough that one
  engine at minimum throttle can still hover in Mars gravity); start
  12 km at 450 m/s down (after entry; the hypersonic entry is not
  simulated).
- Ignition altitude: `max(h_floor, gate + 1.4 · v_z² / (2(0.7·a_max − g)))`,
  `h_floor` = 600 m on Earth (the flip needs room), 0 on Mars.

## Aerodynamics (sim and planner share the body model)

With air-relative velocity `w = v − v_wind`, axial `w_a = (w·b_z)`, normal
`w_n = w − w_a b_z`:

- `F = −½ρ (C_A S_A |w_a| w_a b_z + C_N S_N |w_n| w_n)`, with `S_A = πR²`,
  `S_N = L·D`, `C_A = 0.8`, `C_N = 0.6`. This gives ~90 m/s belly-flop
  speed at Earth sea level (matches the research row).
- Body normal force acts 2 m ahead of the CoM (mildly unstable: the flaps
  must fly it, as on the real vehicle). Flaps add drag
  `½ρ C_f S_f e |w_n| w_n` at their stations; differences between fore/aft
  pitch the ship, left/right roll it; yaw comes from the RCS.
- Atmosphere: Earth `ρ = 1.225·e^(−z/8500)`; Mars `ρ = 0.020·e^(−z/11100)`.
  No Mach effects (Mars belly-flop speeds are supersonic: approximation).
- The planner uses the body model without flaps or wind (flaps neutral),
  the controller absorbs the difference.

## Guidance: successive convexification (SCvx)

- Time-normalized `τ ∈ [0, 1]`, `dx/dτ = σ f(x, u)`, `σ` = flight time
  (free). Control `u = T_B` body-frame thrust (3). Non-dimensional units:
  mass = start mass, length = 100 m, time = √(100 m / g).
- `K = 24` nodes, first-order hold on `u`; `A, B⁻, B⁺, S, z` from RK4
  integration of the variational equations; Jacobians by central finite
  differences of `f`.
- Subproblem (one Clarabel SOCP): dynamics with virtual control `ν`
  (L1-penalized, weight 1e3); initial state fixed; final state: gate
  position, gate velocity (0, 0, −1 m/s), `q_x = q_y = 0` (upright, roll
  free), `ω = 0`; `m ≥ m_dry`; `‖u‖ ≤ T_max(lit)`;
  `T_min ≤ ûᵀu` (linearized lower bound around the reference);
  `‖u‖ cos 15° ≤ u_z` (gimbal); `‖ω‖ ≤ 30°/s`; `z ≥ z_floor`
  (Earth: catch height, never under the arms; Mars: 0); Earth tower
  clearance `x ≥ −3 m` (the mast stands on the −x side); quadratic trust
  region `w_tr Σ ‖δx‖² + ‖δu‖² + δσ²`; objective `−m_K + w_ν‖ν‖₁ + trust`.
- Stop when `‖ν‖₁ < 1e-4` (non-dim) and the step is < 1e-3, at most
  15 iterations (first plan) or 4 (warm re-plan from the previous plan).
- Result checked by re-integrating the nonlinear dynamics with the plan's
  controls: final-gate error reported; plan rejected if > 10 m or
  constraints are violated beyond tolerance.

## Flight phases (closed loop)

1. **Belly flop**: engines off; attitude target belly-down, axis along the
   ground track; pitch offset up to ±12° steers downrange so the ship
   reaches the ignition altitude over the flip point.
2. **Ignition / flip / landing burn**: at the ignition altitude (rule
   above), all healthy engines light; the SCvx plan
   (re-solved every 0.5 s of sim time, warm-started) is flown by the
   tracking controller; engines shut down per the engine manager.
3. **Terminal**: below the gate, a 1 m/s descent law (Earth) over the
   catch point, arms close when the pins are within 8 m above the arms;
   Mars legs deploy at ignition and touch down.
- Outcomes. **Caught**: pins reach arm height with ≤ 2 m/s down, ≤ 1 m/s
  sideways, ≤ 1.5 m from the catch axis, tilt ≤ 3°, |ω| ≤ 2°/s.
  **Landed** (Mars): feet touch with ≤ 2 m/s down, ≤ 1 m/s sideways,
  tilt ≤ 5°. Anything else is **crashed** with a reason (hit the tower,
  missed the arms, too fast, tipped over, hit the ground).
- Re-plan fails → keep flying the last plan; no plan at all → brake
  (full thrust against velocity, upright).

## Break it (kept from M2, adapted)

Gust = wind of 15 m/s (Earth) along ±x for 4 s (real aero force now);
engine out (one of three, up to two); thrust lag; sensor noise (position,
velocity as M2; attitude 0.2° / 0.05°/s at high); grab mid-flight.

## Web

- Scenario switch top-left: **Earth — tower catch** (default) / **Mars — legs**.
- Editing: drag the ship start in the x–z plane (Earth: 3–12 km up,
  ±3 km downrange); velocity handle as before. Preview = predicted
  belly flop (cheap glide simulation) + one SCvx plan from the predicted
  ignition state.
- Cameras re-tuned for 52 m vehicle and 145 m tower.

## Testing

- `sim::ship`: free fall, torque-free rotation preserves |q| and energy,
  terminal speed ≈ 90 m/s, engine ignition delay and shutdown, RCS torque
  sign, flap pitch sign, catch/landing/crash classification.
- `scvx`: discretization matches nonlinear propagation for the
  reference; converges on the Earth and Mars examples; plan satisfies
  thrust, gimbal, ω, floor and clearance; re-integrated final error < 10 m.
- Closed loop (native, in `wasm/tests`): nominal Earth catch, nominal Mars
  landing, Earth with one engine out at ignition, gust during belly flop.
  Slow cases `#[ignore]`.
- Vitest for new pure TS modules; Playwright smoke test.
