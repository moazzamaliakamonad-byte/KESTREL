# M4.3 Chosen approach bearing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (native). Checkbox steps.

**Goal:** Each flight approaches the tower from the bearing its fall is taking it to (±60°, zero within ±10°); gate, aim, ship heading and the burn's keep-out turn with it.

**Spec:** `docs/superpowers/specs/2026-11-19-m4-chosen-approach-design.md`

## Global Constraints
- ψ clamped to ±60°, flown as 0 within ±10° (bit-identical still-air default flights); chosen once.
- Boards and sweep as the spec; suite < 2 min; Python edits (UTF-8, newline='').

## Review Focus
1. ψ = 0 must be exactly the old flight (rotation by 0 exact; `unaware_flight_matches_today` and the glide boards).
2. The keep-out half-plane turns with ψ; the mast must stay clear at ±60° (sim contact).
3. The page's gate marker at the flight's bearing.

### Task 1: `Scenario::turned(ψ)` and the turned keep-out (model, scvx subproblem) — tests: turning by 0 is identity; by 90° the gate is on +y; the subproblem's keep-out uses `b · r`.
### Task 2: The ship's belly attitude and pitch slide along the bearing (ship_control) — tests: at ψ the nose points along −b; the slide steers the along-track miss.
### Task 3: Choosing ψ in the flight (ship_flight) — tests: still air default start ψ = 0; a 15 m/s crosswind from +y chooses ψ < −10°; `earth_crosswind_is_caught` at 15 m/s from +y.
### Task 4: Page (snapshot `bearing`, gate drawn turned, record entry) — web tests.
### Task 5: Measure (glide board, wind sweep); docs (D-048, LIMITATIONS, ARCHITECTURE, ROADMAP, learn 21).
