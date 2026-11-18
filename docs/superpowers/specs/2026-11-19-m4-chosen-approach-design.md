# M4.3 — A chosen approach bearing: design

Date: 2026-11-19. Status: accepted (owner delegates design approval).
Sub-project 3 of M4 (ROADMAP.md). Simulation only.

## Why

The owner: "it always falls the same way". Every flight lines up for the
same final approach: the gate 10 m (ship) or 14 m (booster) out along +x,
the fall aimed 450 m (ship) or 120 m (booster) out along +x, the ship's
nose toward −x. And M4.1 found the ship's worst loss: in a crosswind it
drifts ~500 m sideways in the belly flop, because it can only slide along
its nose, and no plan reaches the +x gate from there (5 of 18 caught at
15 m/s, 4 at 20, none at 25).

## Goal

Each flight chooses the bearing it approaches the tower from, within the
arms' open side (±60° of +x), toward **where its fall is taking it**: its
present position carried along its velocity and the estimated wind's
drift. The gate, the gate velocity, the fall's aim point, the ship's
belly-flop heading and the burn plan's tower-side keep-out all turn with
it. A bearing within ±10° of +x is flown as +x, so a still-air flight from
the default start is the old flight bit for bit.

## Design

- `Scenario::bearing` (rad, 0 = +x) and `Scenario::turned(ψ)`: the gate,
  gate velocity and aim rotated about the catch axis; the keep-out becomes
  the half-plane `b · r ≥ c` with `b = (cos ψ, sin ψ)` (the SCvx subproblem
  takes the direction).
- The ship's belly attitude is the old one turned by ψ about the vertical
  (nose along `−b`); its pitch slide works along `b` (the along-track
  miss); the across-track drift is what the bearing absorbs.
- `ShipFlight` chooses ψ once, in the unpowered fall, when the wind
  estimate first settles (1σ < 2 m/s) or at 70 % of the way to the
  ignition height, whichever is first; the predicted arrival is
  `r + v t + w (t − τ (1 − e^(−t/τ)))` across the hull (the drag time
  constant from the shared model); ψ = its bearing, clamped to ±60°, zero
  within ±10°. A flight-record entry says which bearing and why.
- The page draws the gate at the flight's bearing (the snapshot carries
  ψ).

## Success (measured)

- Still air: the boards unchanged (`approach_board`'s glide column:
  12/12, 38/40, 40/40, 20/20).
- Wind sweep: no fewer catches than 146/216 at any wind speed; the ship's
  crosswind catches (from +y) up.

## Non-goals

- Changing the bearing after it is chosen; the entry from orbit's ground
  track; the experimental planned approach (it keeps the +x geometry).
