# Limitations

Every claim in this project must be measured before it is written down. This
file lists what Kestrel does **not** do, and anything that is known to be
approximate. Update it whenever a milestone lands.

## Scope

- **Simulation only.** Not flight software. No flight hardware, no
  hardware-in-the-loop testing, no certification, no safety case.
- **No real vehicle data.** Vehicle parameters are illustrative, loosely in the
  range of published examples, not taken from any real booster.

## Current state (pre-M0)

- Nothing is implemented yet. No solve times, success rates or accuracy figures
  have been measured, so none are claimed.

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

_None yet. Each entry here must state hardware, browser, problem size and how
it was measured._
