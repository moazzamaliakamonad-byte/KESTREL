# M3 part 2 design — Super Heavy-class booster, full customization, mission-control page

Status: approved by the owner's standing delegation (decisions are
reported, not gated). Simulation only.

## What the owner asked for (2026-11-15), in order

1. The **Super Heavy booster**, as realistic as a simulation can be: its
   return (falls engines-first, steers with grid fins, landing burn on 13
   engines then the 3 centre ones, diverts to the tower, caught by the
   arms). Show the **full stack** standing on the launch mount.
2. **One page, full customization**: vehicle, planet (Mars is ship-only),
   start conditions, engines, failures, wind/gusts, sensor noise, throttle
   lag, sim speed, cameras, physics options.
3. A **top-class, futuristic interface** (mission-control HUD), readable on
   phones.
4. Later (not in this spec): full mission — launch, hot staging, both returns.

Purpose: a showcase. Success = a visitor can pick either vehicle, change
anything, break it, and watch guidance cope, on one page that looks like a
flight console; every number is either cited or labelled illustrative.

## Research (read 2026-11-15)

Wikipedia "SpaceX Super Heavy", "SpaceX Starship (spacecraft)", "SpaceX
Raptor"; press coverage of Flight 5 (NPR, The Register) for the landing
sequence.

| Quantity | Value used | Source / status |
| --- | --- | --- |
| Booster height × diameter | 72.3 m × 9 m | Wikipedia, Block 3 infobox |
| Engines | 33 Raptor 3; the inner 13 gimbal (3 centre + 10), 20 outer fixed | Wikipedia ("electric actuators on the thirteen engines in the middle and center rings") |
| Total thrust | 80.8 MN → 2.45 MN per engine | Wikipedia, Block 3 infobox |
| Sea-level Isp | 330 s | Wikipedia, Block 3 infobox |
| Throttle | 40–100 % | Wikipedia (Raptor) |
| Empty mass | 275 t | Wikipedia, Block 1/2 infobox; Block 3 not published → *carried over* |
| Ascent propellant | 3 650 t | Wikipedia, Block 3 (drawn/described only; ascent is not simulated) |
| Grid fins | **3**, electric, in a 90/90/180° arrangement; two carry the catch hardpoints | Wikipedia. (Block 1/2 had 4; the owner's brief said 4 — the current vehicle has 3, so 3 are modelled.) |
| Attitude thrusters | cold gas from ullage gas | Wikipedia |
| Landing propellant | header tanks (LOX header tank; methane via the downcomer) | Wikipedia; quantity not published → *illustrative* 120 t |
| Landing burn | 13 engines light, then all but the centre 3 shut down; the 3 steer it into the arms | Wikipedia, NPR / The Register (Flight 5) |
| Burn altitude / speed, catch speed, fin size and deflection, gimbal range, arm heights, aerodynamic coefficients, centre of mass, inertia | — | not published → *illustrative* |
| Full stack height | 124.4 m (72.3 + 52.1) | Wikipedia |

## Decisions

- **One vehicle model, two parameter sets.** `model::Ship` gains a
  `VehicleKind` (ship / booster), the number of landing engines (3 / 13),
  the engines flown to the end (1 / 3), a linear lift slope and grid-fin
  data. The rigid-body equations, the SCvx planner, the powered and
  terminal controllers, the simulator's contact logic and the closed loop
  are shared. New for the booster: the fin aerodynamics, the unpowered
  descent controller, its scenario and its start attitude.
- **Third flight kind** `"booster"` next to `"earth"` and `"mars"` (ship).
  The page presents it as vehicle × planet with Mars + booster disabled.
- **Engines**: `Cluster` holds as many engines as the vehicle lands on.
  Index 0–2 are the centre three, so "shut down to 3" keeps them. Any
  engine can be failed by index (click it on the engine map).
- **Too light to hover**: if the lit engines' minimum thrust exceeds the
  weight in the terminal phase, one more engine is shut down (the booster
  ends on two when nearly empty). Illustrative behaviour.
- **Offshore aim, then divert**: the unpowered descent aims 120 m out from
  the tower (+x); the landing-burn plan carries it in. The real vehicle
  does the same for range safety; the distance is illustrative.
- **Tuning** (physics options) is one serializable struct applied to the
  scenario, sent with every plan request so the worker plans with the same
  numbers the simulator flies.
- **Stack on the mount**: shown while editing (before a flight), hidden in
  flight: the returning vehicle is the one that left.

## Booster model (illustrative unless in the table)

- 72.3 m × 9 m; centre of mass 26 m above the base; inertia per kg
  `(440, 440, 18)` m²; 275 t empty + 120 t landing propellant.
- 13 landing engines × 2.45 MN, 40–100 %, Isp 330 s, shared gimbal ≤ 15°.
- Aerodynamics, engines first: axial `C_A = 1.0` (terminal speed ≈ 320 m/s
  near the ground); crossflow `C_N = 0.6`; **plus a linear lift term**
  `F = −½ρ S_A C_Nα |w_a| w_n`, `C_Nα = 2 /rad` (slender-body theory),
  acting 10 m above the centre of mass with the crossflow force (stable:
  it weathercocks engines-first). The lift term is in the shared model, so
  the planner sees it. No Mach effects, no plume–airflow interaction.
- Grid fins: three at body +y, −y, +x, 60 m above the base, 6 m from the
  axis, 20 m² each, ±20°, force `½ρ w_a² S c_δ δ` (`c_δ = 2 /rad`)
  tangential. Pitch from the ±y pair, yaw from the +x fin, roll from all;
  what the fins cannot give goes to the cold-gas thrusters.
- Catch: hardpoints 62 m above the base; arms at 95 m (the arms ride the
  tower: 110 m for the ship).

## Booster flight

1. **Descent**: engines off, engines first. Guidance leans the top away
   from where it wants to go by up to 10° (lift ≈ g per radian near
   terminal speed) to null the predicted miss at the ignition altitude.
2. **Landing-burn start-up**: the 13 engines light (ignition altitude from
   the same stopping rule as the ship); the attitude is brought upright.
3. **Landing burn**: SCvx on 13 engines to an intermediate gate (50 m/s
   down), then SCvx on the centre 3 to the approach gate 12 m out from the
   catch axis.
4. **Final approach**: the shared terminal controller slides over the axis
   and sinks onto the arms at 1 m/s.

Start (default): 14 km up, 0.9 km out, 420 m/s down. Supersonic part of
the fall has no Mach model (stated in LIMITATIONS.md).

## Customization

| Group | Options |
| --- | --- |
| Mission | vehicle, planet, altitude, downrange, crossrange (booster), fall speed, sideways speeds, random seed |
| Vehicle | landing propellant, engines failed before ignition (by index), gimbal limit, minimum throttle, Isp, ignition delay, throttle lag |
| Environment | steady wind speed and direction, gust speed and direction, air-density scale |
| Sensors | noise off / low / high |
| Failures (live) | fail any engine (click), gust now, grab the vehicle |
| View | camera (chase, tower, pad, onboard, free), sim speed 0.25–8×, pause, plan / trail / preview lines, labels |

## Page

One page. Layout: top bar (wordmark, mission clock, phase timeline,
"simulation" tag); left dock with tabs for the groups above (a bottom sheet
on phones); right telemetry rail (altitude / speed, attitude ball, engine
map, propellant, throttle, surfaces, guidance and solve time); bottom
action bar (fly / reset, speed, camera). Dark glass panels, one accent
colour, monospace numerals, short eased transitions, `prefers-reduced-motion`
respected. 3D: procedural booster (33 bells, 3 grid fins, chines, hot-stage
ring), the full stack on the mount, arms at the vehicle's catch height.
No logos, no third-party assets.

## Testing

- model: fin force/torque signs; lift slope sign and stability; terminal
  speed of the booster 280–360 m/s near the ground.
- sim: 13-engine cluster ignition/shutdown/failure by index; fins slew and
  clamp; booster caught / missed classification.
- control: descent holds attitude and steers to the aim point.
- closed loop: booster nominal catch; one centre engine out; gust during
  descent. Ship tests unchanged.
- web: vitest for the store (vehicle × planet rules, clamps, tuning);
  build + typecheck + lint; a browser screenshot check on desktop and phone
  widths.

## Out of scope

Ascent, hot staging, boostback (item 4, later); Mach-dependent aero;
plume interaction; per-engine gimbal and off-axis thrust torque; slosh;
arm dynamics.
