# M0 — Offline 3-DoF Landing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Solve one fuel-optimal 3-DoF powered-descent landing offline in Rust, export it as CSV, plot it, and prove the constraints hold with tests.

**Architecture:** One library crate `kestrel-guidance` (`crates/guidance`, lib name `guidance`). Physical inputs (SI) → non-dimensional scaling → SOCP assembly (lossless convexification, Açıkmeşe & Ploen 2007) → Clarabel → re-dimensionalized `Trajectory`. Free final time via an outer 1-D search. No simulator, no WASM bindings yet (M1).

**Tech Stack:** Rust 1.99 (edition 2024), `clarabel = { version = "0.11.1", default-features = false }`, Python 3 + matplotlib for the plot script only.

**Spec:** `docs/ROADMAP.md` (M0 section), `docs/ARCHITECTURE.md`, `docs/DECISIONS.md` D-002/D-003. Maths reference: Malyuta et al. arXiv:2106.09125, LCvx section.

## Global Constraints

- Frame: pad at origin, `z` up, gravity `[0, 0, -g]`. SI units at every public API boundary; non-dimensional values never leave the crate.
- Vectors are `pub type Vec3 = [f64; 3]`. No linear-algebra crate.
- `cargo test` for the whole workspace under 2 minutes; anything slower is `#[ignore]`.
- Small commits, conventional prefixes (`feat`, `test`, `fix`, `docs`, `chore`). Never stage `learn/` or `project-ideas.md`.
- After each task, add a short local note to `learn/` (not committed) explaining just the Rust and maths that task used, for a Rust beginner.
- Simulation only; no unmeasured claims in docs. Measured numbers go in `LIMITATIONS.md` with how they were measured.
- Run `cargo` from PowerShell (Git Bash's `/usr/bin/link.exe` shadows MSVC; see `docs/SETUP.md`).

## Review Focus

1. **Initial state outside the glide-slope cone or below ground** → `Err(GuidanceError::InvalidProblem(..))` before solving, not a solver failure or panic. (Task 3)
2. **Final time longer than the propellant allows** (`m_wet − α·ρ₂·t ≤ 0` makes `ln` NaN) → `Err(InvalidProblem)`, never NaN in the SOCP. (Task 4)
3. **Too-short final time** (cannot stop in time) → `Err(GuidanceError::Infeasible)`, not a panic. (Task 5)
4. **NaN/inf anywhere in the initial state** → `Err(InvalidProblem)`. (Task 3)
5. **Lossless convexification failing silently** (`σ > ‖u‖`, i.e. thrust below ρ₁ while fuel is charged for ρ₁) → test asserts true thrust magnitude stays in `[ρ₁, ρ₂]`. (Task 5)

---

### Task 0: Toolchain gate

Blocked until the Windows SDK is installed (`docs/SETUP.md`).

- [ ] **Step 1:** In PowerShell: `cargo test -p kestrel-guidance` → Expected: compiles, links, `0 passed`.
- [ ] **Step 2:** Add dependency in `crates/guidance/Cargo.toml`: `clarabel = { version = "0.11.1", default-features = false }`.
- [ ] **Step 3:** `cargo build -p kestrel-guidance --target wasm32-unknown-unknown` → Expected: success. If it fails, stop and report: this decides D-003.
- [ ] **Step 4:** Update D-003 in `docs/DECISIONS.md` to `accepted` (or record the failure), tick the ROADMAP boxes "Install Rust", "Cargo workspace", "Confirm Clarabel builds". Commit: `chore: add clarabel dependency and confirm wasm build`.

### Task 1: Vehicle parameters

**Files:** Create `crates/guidance/src/params.rs`; modify `crates/guidance/src/lib.rs` (`pub mod params; pub use params::{Vec3, Vehicle, ParamError};`).

**Interfaces — Produces:**
- `pub type Vec3 = [f64; 3]`, `pub const G0: f64 = 9.80665`
- `pub struct Vehicle { wet_mass, dry_mass, isp, min_thrust, max_thrust, gravity, glide_slope, max_tilt: f64 }` (`Debug, Clone, PartialEq`; angles in radians; `glide_slope` = minimum elevation angle seen from the pad, enforced as `z ≥ tan(γ)·‖(x,y)‖`; `max_tilt` = max angle between thrust and +z)
- `pub enum ParamError { NotPositive(&'static str), DryMassNotBelowWetMass, MinThrustNotBelowMax, AngleOutOfRange(&'static str) }`
- `Vehicle::example_mars() -> Vehicle`, `Vehicle::alpha(&self) -> f64` (= `1/(isp·G0)`), `Vehicle::validate(&self) -> Result<(), ParamError>`

Example values (illustrative, not a real vehicle): `t_max = 6·3100·cos(27°)`; `wet 1905, dry 1505, isp 225, min_thrust 0.3·t_max, max_thrust 0.8·t_max, gravity 3.7114, glide_slope 4°, max_tilt 45°`.

- [ ] **Step 1: Failing tests** in `params.rs` `mod tests`: `example_is_valid` (`validate() == Ok(())`); `example_can_hover_when_full` (`max_thrust > wet_mass·gravity`); `rejects_negative_mass` (`NotPositive("wet_mass")`); `rejects_nan_isp` (`NotPositive("isp")`); `rejects_dry_heavier_than_wet`; `rejects_inverted_thrust_bounds`; `rejects_vertical_glide_slope` (γ = π/2 → `AngleOutOfRange("glide_slope")`); `alpha_matches_definition` (`alpha()·isp·G0 ≈ 1` within 1e-15).
- [ ] **Step 2:** `cargo test -p kestrel-guidance params` → FAIL (items not defined).
- [ ] **Step 3:** Implement. Validation order: positivity (finite and > 0) for the six scalars in field order, then dry < wet, then min < max, then `0 ≤ γ < π/2`, then `0 < θ ≤ π`.
- [ ] **Step 4:** Tests pass; `cargo clippy` clean.
- [ ] **Step 5:** Commit `feat: add vehicle parameters with validation`.

### Task 2: Non-dimensional scaling

**Files:** Create `crates/guidance/src/scaling.rs`.

**Interfaces — Produces:** `pub struct Scaling { pub length: f64, pub time: f64, pub mass: f64 }` with `Scaling::new(length: f64, gravity: f64, mass: f64) -> Scaling` (`time = sqrt(length/gravity)`), and `to_nd_length/from_nd_length`, `to_nd_velocity/from_nd_velocity`, `to_nd_accel/from_nd_accel`, `to_nd_time/from_nd_time`, `to_nd_force/from_nd_force`, all `(&self, f64) -> f64`; `nd_alpha(&self, alpha: f64) -> f64` (= `alpha·length/time`, the factor in `dz/dt' = −α'σ'`).
Units: velocity `L/T`, accel `L/T²`, force `M·L/T²`. Chosen so non-dimensional gravity is exactly 1.

- [ ] **Step 1: Failing tests:** `gravity_is_one_nd` (`to_nd_accel(g) == 1` within 1e-12); `round_trip_each_quantity` (each `from(to(x)) ≈ x`, rel 1e-12, for x = 1234.5); `nd_alpha_preserves_mass_decay` — for Γ = 10 kN, m = 1800 kg, check `α·(Γ/m)` (1/s) equals `nd_alpha(α)·to_nd_accel(Γ/m) / time` within 1e-12 relative.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Pass. **Step 5:** Commit `feat: add non-dimensional scaling`.

### Task 3: Landing problem and trajectory types

**Files:** Create `crates/guidance/src/problem.rs`; modify `lib.rs` (`pub mod problem; pub use problem::{GuidanceError, LandingProblem, Trajectory};`).

**Interfaces — Produces:**
- `pub struct LandingProblem { pub vehicle: Vehicle, pub position: Vec3, pub velocity: Vec3, pub steps: usize }` (`steps` = N intervals)
- `LandingProblem::example() -> LandingProblem`: `example_mars()`, position `[2000, 0, 1500]`, velocity `[-50, 0, -75]`, steps 50
- `LandingProblem::validate(&self) -> Result<(), GuidanceError>`
- `pub enum GuidanceError { InvalidParams(ParamError), InvalidProblem(&'static str), Infeasible, SolverFailed(String) }` (`Debug, Clone, PartialEq`)
- `pub struct Trajectory { pub t: Vec<f64>, pub position: Vec<Vec3>, pub velocity: Vec<Vec3>, pub mass: Vec<f64>, pub thrust: Vec<Vec3>, pub final_time: f64 }` — `t, position, velocity, mass` have N+1 entries (nodes), `thrust` has N (constant over each interval). `Trajectory::fuel_used(&self) -> f64` = `mass[0] − mass[N]`.

- [ ] **Step 1: Failing tests:** `example_is_valid`; `rejects_bad_vehicle` (→ `InvalidParams(..)`); `rejects_nan_state` (→ `InvalidProblem("state not finite")`); `rejects_below_ground` (z < 0 → `InvalidProblem("outside glide slope")`); `rejects_outside_glide_slope` (position `[10000, 0, 100]`); `rejects_too_few_steps` (steps < 2 → `InvalidProblem("steps < 2")`); `fuel_used_is_mass_difference`.
- [ ] **Step 2–4:** Fail → implement → pass. Glide-slope check: `z ≥ tan(γ)·hypot(x, y)`.
- [ ] **Step 5:** Commit `feat: add landing problem and trajectory types`.

### Task 4: SOCP assembly for a fixed final time

**Files:** Create `crates/guidance/src/socp.rs`.

**Interfaces:**
- Consumes: `LandingProblem`, `Scaling`, `Vehicle::alpha`.
- Produces: `pub(crate) struct Socp { pub p: CscMatrix<f64>, pub q: Vec<f64>, pub a: CscMatrix<f64>, pub b: Vec<f64>, pub cones: Vec<SupportedConeT<f64>>, pub layout: Layout, pub scaling: Scaling, pub dt_nd: f64 }`; `pub(crate) struct Layout { n: usize }` with index fns `r(k, i)`, `v(k, i)`, `z(k)` (k = 0..=N), `u(k, i)`, `sigma(k)` (k = 0..N), `len()`; `pub(crate) fn build(problem: &LandingProblem, final_time: f64) -> Result<Socp, GuidanceError>`.

Scaling choice: `length = max(‖r₀‖, 1 m)`, `gravity = vehicle.gravity`, `mass = wet_mass`. Work in log-mass `z = ln(m / wet_mass)` so `z₀ = 0`.

The problem (non-dimensional, `dt = t_f'/N`, `t_k = k·dt`, `g = [0,0,−1]`), all in Clarabel form `A x + s = b, s ∈ K`, cones appended in this order:

```text
Zero cone (equalities):
  r_0 = r₀', v_0 = v₀', z_0 = 0
  r_{k+1} = r_k + dt·v_k + ½dt²·(u_k + g)        k = 0..N-1   (exact for constant u)
  v_{k+1} = v_k + dt·(u_k + g)
  z_{k+1} = z_k − α'·dt·σ_k
  r_N = 0, v_N = 0
Nonnegative cone (inequalities), with z0_k = ln(1 − α'ρ₂'·t_k), z1_k = ln(1 − α'ρ₁'·t_k):
  z_N ≥ ln(dry/wet)
  z0_k ≤ z_k ≤ z1_k                               k = 0..N-1
  σ_k ≤ ρ₂'·e^{−z0_k}·(1 − (z_k − z0_k))          (linearised upper bound; conservative)
  u_k,z ≥ σ_k·cos θ                               (tilt)
  r_k,z ≥ 0                                       k = 0..N
Second-order cones:
  ‖u_k‖ ≤ σ_k                                     SOC(4) per k
  tan γ·‖(r_k,x, r_k,y)‖ ≤ r_k,z                  SOC(3) per k = 0..N  (skip when γ = 0)
  σ_k ≥ μ_k(1 − δ + δ²/2),  μ_k = ρ₁'e^{−z0_k}, δ = z_k − z0_k
      as  (μ_k/2)·δ² ≤ w,  w = σ_k − μ_k + μ_k·δ
      as  ‖(2·sqrt(μ_k/2)·δ, w − 1)‖ ≤ w + 1      SOC(3) per k   (conservative lower bound)
Objective: minimize −z_N   (P = 0)
```

If `1 − α'ρ₂'·t_k ≤ 0` for any k (final time beyond full-thrust burn-out), return `Err(InvalidProblem("final time exceeds propellant"))`.

- [ ] **Step 1: Failing tests:** `layout_indices_are_unique_and_dense` (every index in `0..len()` produced exactly once, N = 3); `dimensions_match_cones` (sum of cone dims == `b.len()` == `a.m`); `rejects_final_time_beyond_burnout` (t_f = 10× `(wet − dry)/(α·ρ₁)` → `InvalidProblem("final time exceeds propellant")`); `free_fall_satisfies_dynamics_rows` — build x with `u = 0` propagated by the same ZOH formulas from the example state, and check that `A·x` equals `b` on the dynamics rows (rows of the zero cone except the terminal rows) within 1e-12.
- [ ] **Step 2–4:** Fail → implement → pass. Build `A` with `CscMatrix::new_from_triplets(m, n, rows, cols, vals)`; `P = CscMatrix::zeros((n, n))`. Note: Clarabel wants `s = b − A·x ∈ K`, so `s ≥ 0` rows are written as `−(coeffs)·x + s = −const`; SOC rows as `s = (t, x…)` with `s₀ ≥ ‖s₁..‖`.
- [ ] **Step 5:** Commit `feat: assemble lossless-convexified landing SOCP`.

### Task 5: Solve for a fixed final time

**Files:** Create `crates/guidance/src/solve.rs`; integration tests `crates/guidance/tests/fixed_time.rs`.

**Interfaces — Produces:** `pub fn solve_fixed_time(problem: &LandingProblem, final_time: f64) -> Result<Trajectory, GuidanceError>`. Re-exported from `lib.rs`.

Status mapping: `Solved` → Ok; `PrimalInfeasible | AlmostPrimalInfeasible` → `Infeasible`; anything else → `SolverFailed(format!("{status:?}"))`. Settings: `DefaultSettings { verbose: false, ..Default::default() }`. Recover SI: `m_k = wet·e^{z_k}`, `T_k = m_k·u_k` (scaled back with `from_nd_force`), `t_k` dimensional.

Tests use the example with `final_time = 50.0` s (if the example turns out infeasible at 50 s, pick the midpoint of the feasible range found by a 1 s scan and write the value into this plan and the test).

- [ ] **Step 1: Failing tests** in `tests/fixed_time.rs`: `lands_on_pad_at_rest` (`‖r_N‖ < 0.01 m`, `‖v_N‖ < 0.01 m/s`); `starts_at_initial_state`; `thrust_magnitude_within_bounds` (each `‖T_k‖ ∈ [ρ₁(1−1e-4), ρ₂(1+1e-4)]` — this is the lossless check, Review Focus 5); `tilt_within_limit` (`T_z ≥ ‖T‖cos θ − 1e-6·ρ₂`); `glide_slope_respected` (all nodes, tol 1e-3 m); `never_below_dry_mass`; `mass_decreases_monotonically`; `too_short_time_is_infeasible` (t_f = 5 s → `Err(Infeasible)`, Review Focus 3).
- [ ] **Step 2–4:** Fail → implement → pass. Time the test file: must be < 10 s in debug.
- [ ] **Step 5:** Commit `feat: solve fixed-final-time landing with clarabel`.

### Task 6: Free final time

**Files:** Modify `crates/guidance/src/solve.rs`; tests in `crates/guidance/tests/free_time.rs`.

**Interfaces — Produces:** `pub fn solve(problem: &LandingProblem) -> Result<Trajectory, GuidanceError>`.

Bounds (Açıkmeşe & Ploen): `t_min = dry·‖v₀‖/ρ₂`, `t_max = (wet − dry)/(α·ρ₁)`, clipped to just below full-thrust burn-out. Search: evaluate 12 evenly spaced t_f in `[t_min, t_max]`; infeasible counts as +∞ fuel; take the best grid point and golden-section search on its two neighbours until bracket < 0.05 s or 30 iterations. All grid points infeasible → `Err(Infeasible)`.

- [ ] **Step 1: Failing tests:** `free_time_beats_grid` (fuel of `solve()` ≤ fuel of `solve_fixed_time` at t_f ∈ {40, 50, 60, 70} wherever those are Ok, + 0.1 kg tolerance); `unreachable_start_is_infeasible` (example with velocity `[0, 0, -400]` → `Err(Infeasible)`); `free_time_respects_bounds_too` (re-use the bound checks from Task 5 via a shared `tests/common/mod.rs::assert_constraints(&LandingProblem, &Trajectory)`, extracted from Task 5's tests as part of this task).
- [ ] **Step 2–4:** Fail → implement → pass. Whole suite still < 2 min.
- [ ] **Step 5:** Commit `feat: optimise final time with grid plus golden-section search`.

### Task 7: CSV export, plot, documentation

**Files:** Create `crates/guidance/examples/land_once.rs`, `scripts/plot_trajectory.py`; modify `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `LIMITATIONS.md`, `README.md` (status line).

- [ ] **Step 1:** `Trajectory::to_csv(&self) -> String` with a failing test first: header exactly `t,x,y,z,vx,vy,vz,mass,tx,ty,tz,thrust_mag`, N+1 data rows, last row's thrust columns empty (no thrust after the final node).
- [ ] **Step 2:** Example: `cargo run --release -p kestrel-guidance --example land_once > target/landing.csv` prints CSV to stdout, and solve time + fuel used + t_f to stderr.
- [ ] **Step 3:** `python scripts/plot_trajectory.py target/landing.csv target/landing.png`: 2×2 figure — x–z path with glide-slope line; thrust magnitude vs t with ρ₁, ρ₂ dashed; tilt angle vs t with θ dashed; mass vs t with dry mass dashed. Expected qualitatively (LCvx literature): thrust magnitude is max–min–max ("bang–bang") or close to it.
- [ ] **Step 4:** Compare with the LCvx example figure in Malyuta et al. (arXiv:2106.09125): copy that example's parameters into a second example `examples/malyuta_lcvx.rs`, plot, and record in `LIMITATIONS.md` what matched and what did not (qualitative only; no claim of a match that was not observed).
- [ ] **Step 5:** Record measured solve time (machine, release build, N = 50, method) in `LIMITATIONS.md`; tick M0 boxes; move ARCHITECTURE `guidance` from planned to built. Commit `feat: add csv export and landing plot` and `docs: mark M0 done with measured results` separately.
