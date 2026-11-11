# Architecture

Status: **planned**. Nothing here exists yet. Sections are marked _(planned)_
until the code is written, then updated to describe what was actually built.

## Big picture

```text
 ┌──────────────────────────── browser tab ────────────────────────────┐
 │                                                                     │
 │  UI / 3D view (JS/TS)  ── user interference ──▶  Simulator (Rust)   │
 │        ▲                                            │ true state    │
 │        │ planned trajectory,                        ▼               │
 │        │ margins, solve time                  Sensor model          │
 │        │                                            │ noisy state   │
 │        └───────────── Guidance (Rust → WASM) ◀──────┘               │
 │                       formulate SOCP → Clarabel → plan              │
 │                              │                                      │
 │                              ▼                                      │
 │                     Tracking controller ──▶ thrust command ──▶ Sim  │
 └─────────────────────────────────────────────────────────────────────┘
```

Key separation: **the simulator owns the truth; guidance only sees what the
sensors report.** This keeps the demo honest: if guidance looks good, it is
not because it secretly read the true state.

## Components _(planned)_

| Component | Responsibility | Milestone |
| --- | --- | --- |
| `guidance` crate | Build and solve the convex landing problem; return a trajectory | M0 |
| `sim` crate | Integrate vehicle dynamics; inject wind, failures, lag, noise | M2 (simple version in M1) |
| `wasm` crate | Thin bindings exposing guidance + sim to JavaScript | M1 |
| `web/` | Page, 3D view, controls, panels | M1 |

Proposed workspace layout (create only when the milestone needs it):

```text
kestrel/
├─ crates/
│  ├─ guidance/   # M0
│  ├─ sim/        # M1/M2
│  └─ wasm/       # M1
├─ web/           # M1
└─ docs/
```

## Data flow per guidance cycle _(planned)_

1. Read noisy state: position `r`, velocity `v`, mass `m`.
2. Non-dimensionalize (scale length, time and mass so values are near 1).
3. Build SOCP matrices for `N` time steps.
4. Solve. On infeasibility, solve the minimum-landing-error problem instead.
5. Re-dimensionalize; hand trajectory to controller and UI.

## Open questions

- Does Clarabel compile to `wasm32-unknown-unknown` cleanly, and how fast is
  it there? (Decides D-003.)
- Run guidance in a Web Worker so a slow solve never freezes rendering?
  Likely yes; decide in M1.
- Which 3D library for the front end? Decide in M1.
