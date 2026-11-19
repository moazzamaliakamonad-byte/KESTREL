# M4.2 — A planned final approach: design

Date: 2026-11-19. Status: accepted (owner delegates design approval).
Sub-project 2 of M4 "smarter guidance" (ROADMAP.md). Simulation only.

## Why

The last approach looked scripted. Today the burn's plan ends at
a gate (ship: 10 m out from the catch axis, 20 m above the caught
position; booster: 14 m out, 12 m up) and a hand-tuned controller
(`ship_control::terminal`) flies the rest: slide over the axis at a capped
speed, wait, sink. Plans that ended on the arms were tried (D-033) and lost
3 of 12 flights: sideways braking next to the tower tilts the vehicle, and
a tilted vehicle's **base** swings into the mast 5 m from the hull.

## Goal and success

From the gate onto the arms, guidance flies an optimal plan that it keeps
re-planning, with the base and nose kept off the mast, instead of the
scripted slide and sink. The scripted controller remains the fallback
when a plan cannot be found.

Success (measured, in LIMITATIONS.md):

- Still air, the boards flown with the planned approach off and on (same
  build): nominal ship 12/12, ship seeds 11-30 and booster seeds 11-30 at
  low/high noise, ship engine-out window: **no fewer catches** than off.
- Wind sweep (wind awareness on): no fewer catches than M4.1's 144/216.
- Smoother: over the nominal flights, the time from the gate to the catch
  and the largest sideways acceleration in that leg are reported both
  ways; the planned approach must not take longer on average.
- Contact within the catch limits by margin: mean contact sink speed and
  sideways speed reported both ways.

## Design

### 1. The approach problem (`guidance::approach`, new)

A three-degree-of-freedom point mass (the centre of mass), from the
present estimate to the caught position, over `N = 20` intervals of a
fixed final time `t_f`, solved by **Kestrel's own solver**
(`crates/socp`), the solver set up once and updated after (no allocation
after the first plan; M4.4's direction).

- Variables: position and velocity at every node, the commanded
  acceleration `a_k` (thrust / mass) on every interval, its magnitude
  bound `σ_k`, and its change between intervals.
- Dynamics: constant acceleration per interval (exact for a point mass):
  `r⁺ = r + v dt + ½(a − g ẑ) dt²`, `v⁺ = v + (a − g ẑ) dt`.
- Ends: start at the estimate; end at the caught position `(0, 0,
  rest_cm_z)` sinking at 1.0 m/s, sideways speed zero.
- Thrust: `‖a‖ ≤ σ ≤ a_max` (90 % of what the lit engines give, room for
  feedback), `a_z ≥ a_min` (105 % of their minimum; with the tilt cap
  this is a conservative, convex stand-in for `‖a‖ ≥ a_min`).
- Tilt: `‖(a_x, a_y)‖ ≤ tan θ · a_z`, θ = 6° (the catch allows 5° at
  contact; the funnel below keeps the end upright).
- **Mast keep-out, for the base and the nose**: the axis tilts with the
  thrust, so the base is at `x − l_b a_x / g` and the nose at
  `x + l_n a_x / g` (l_b, l_n: centre of mass to base and nose; the tilt
  linearised at hover, conservative while `a_z ≥ g`). Both stay at
  `x ≥ mast face + hull radius + margin`.
- **Margin from the navigation uncertainty**: 1 m plus twice the
  navigation filter's position error (measured for each noise level and
  set as a constant per level, LIMITATIONS.md).
- A funnel onto the axis: `‖(x, y)‖ ≤ 0.5 + 0.6 (z − z_c)`, so the pins
  arrive over the arms; a sink-rate cone `v_z ≥ −(1.0 + 0.4 (z − z_c))`;
  a side-speed cone `‖(v_x, v_y)‖ ≤ 0.6 + 0.15 (z − z_c)` (it slows its
  slide as it comes down).
- Cost: propellant (`Σ σ_k dt`) plus a small penalty on the change of
  acceleration between intervals (no bang-bang chatter: smooth, not
  robotic).
- Final time: the first plan searches `t_f` ∈ {5, 7, 9, 12, 15} s and
  keeps the cheapest that solves; every later plan keeps the same arrival
  time (a shrinking horizon) and searches again only if that fails.

### 2. In the flight (`ship_flight.rs`)

- In the terminal phase, every 0.25 s of sim time (and at once when it
  begins), plan from the navigation estimate. A plan is accepted only if
  solved; else the last one is kept if the vehicle is within 3 m and
  1 m/s of it, else the scripted terminal controller flies (fallback).
- When less than 0.5 s of the plan is left, no more plans: the last one
  is flown to its end, then the scripted controller (it is on the arms
  by then).
- `ShipFlightConfig.planned_approach` (default true) for A/B runs and a
  page switch.

### 3. Tracking (`ship_control::approach`, new)

Like the terminal controller, with the plan as its reference: acceleration
= the plan's + position and velocity feedback (critically damped; the
terminal controller's bandwidth limits and sideways caps, so the attitude
loop stays stable), the estimated wind's force fed forward, thrust through
`fly_force` with the attitude tilted toward the wanted force.

### 4. On the page

The snapshot carries the approach plan (its positions, final time, and
whether it is flying it or the fallback); the analysis view draws its
nodes; the flight record gets "Approach planned: 8.4 s onto the arms"
(first plan) and "Approach fallback: no plan, scripted descent" if it
falls back; a settings switch "Planned final approach".

## Non-goals

- The burn's gate stays where D-033 put it (moving it is 4.3's business
  with the approach bearing).
- No 6-DoF approach plan: the attitude is not planned, only the thrust
  direction it follows (and the keep-out allows for its tilt).

## Testing

- Unit (`approach.rs`): from the ship's gate the plan reaches the caught
  position and velocity; every node respects the tilt, the funnel, the
  sink and side-speed cones and the base/nose keep-out; 8 m off to the
  side still plans; heading for the mast too fast is refused; the second
  plan reuses the set-up solver; the free-time search keeps the cheapest.
- Unit (`ship_control.rs`): tracking an approach plan in the shared
  model with no error follows it to within 0.5 m.
- Closed loop: ship and booster nominal flights caught with the planned
  approach (in the suite); the boards and the sweep A/B (`--ignored`).
- Web: record decisions, the switch.
