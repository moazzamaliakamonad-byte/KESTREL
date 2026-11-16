# Decision log

One entry per significant decision. Newest at the bottom. Never delete an
entry; if a decision is reversed, add a new one that supersedes it.

Template:

```text
## D-NNN — Title
- Date:
- Status: proposed | accepted | superseded by D-NNN
- Context: what forced a choice
- Decision: what we chose
- Alternatives: what else was considered and why not
- Consequences: what this makes easier / harder
```

---

## D-001 — Project name "Kestrel"
- Date: 2026-11-11
- Status: accepted
- Context: Needed a name for project 2 (rocket landing in the browser).
- Decision: Kestrel, after the falcon that hovers in place then lands precisely.
- Consequences: Crate names use the `kestrel-` prefix if published.

## D-002 — Rust compiled to WebAssembly
- Date: 2026-11-11
- Status: accepted (from the project brief)
- Context: Optimization must run in the browser at interactive rates, and the
  code should be credible to engineers who care about numerical software.
- Decision: Core (guidance, simulator) in Rust, compiled to WASM. Front end in
  JS/TS.
- Alternatives: Pure JS/TS (no mature conic solver, weaker numerics); C++ via
  Emscripten (heavier toolchain, less safe).
- Consequences: Need `wasm-pack` or `wasm-bindgen` tooling; solver must be
  pure Rust or WASM-compatible.

## D-003 — Start with the Clarabel conic solver
- Date: 2026-11-11
- Status: accepted 2026-11-12 (0.11.1 compiles natively and for
  `wasm32-unknown-unknown`; solve speed in a browser not yet measured).
  Default features must stay enabled: 0.11.1 fails to compile without them.
- Context: The convexified landing problem is an SOCP. Writing a robust
  interior-point solver first would delay every demo.
- Decision: Use Clarabel (pure Rust, interior point, supports second-order
  cones) for M0–M2. Custom embedded-grade solver is an M3 stretch.
- To confirm: builds for `wasm32-unknown-unknown`; solve time acceptable.
- Alternatives: ECOS / SCS via C bindings (painful under WASM); OSQP (QP only,
  no general SOC constraints).
- Consequences: Dependency on an external crate's API and numerics.

## D-004 — Small, frequent commits with conventional prefixes
- Date: 2026-11-11
- Status: accepted (owner's request)
- Decision: One logical change per commit. Prefixes: `feat`, `fix`, `docs`,
  `test`, `refactor`, `chore`, `perf`. Docs updated in the same or the next
  commit as the code they describe.
- Consequences: History reads as a step-by-step build log, which doubles as
  learning material.

## D-005 — Next.js for the web front end
- Date: 2026-11-12
- Status: accepted (owner's choice: "top tier")
- Context: M1 needs a browser page; the owner preferred Next.js over Vite or plain HTML.
- Decision: Next.js 16 (App Router, TypeScript) with `output: 'export'` (static files, no server).
- Alternatives: Vite + TypeScript (lighter; equally capable for a client-only app); plain HTML (too manual for M2).
- Consequences: 3D and WASM must be client components loaded with `ssr: false`; deploys to any static host.

## D-006 — React Three Fiber for 3D
- Date: 2026-11-12
- Status: accepted
- Decision: three 0.187 via @react-three/fiber 9, drei 10 and @react-three/postprocessing 3.
- Consequences: declarative scene in React; render-loop logic in `useFrame`.

## D-007 — wasm-bindgen CLI, no wasm-pack
- Date: 2026-11-12
- Status: accepted
- Decision: `wasm-bindgen` 0.2.130 library and CLI (pinned equal), `--target web`, driven by `scripts/build-wasm.mjs`.
- Alternatives: wasm-pack (one more tool and abstraction).
- Consequences: contributors install `wasm-bindgen-cli` at exactly 0.2.130.

## D-008 — Solver in a Web Worker, latest request wins
- Date: 2026-11-12
- Status: accepted
- Decision: the WASM solver runs in a module worker; only the newest request's response is used.
- Consequences: dragging never freezes rendering; stale plans can't be flown.

## D-009 — JSON strings across the WASM boundary
- Date: 2026-11-12
- Status: accepted
- Decision: `plan_landing(json) -> json`; failures are `{ok:false, error:{kind, message}}`, never panics or exceptions.
- Consequences: easy to test natively and debug; payload a few KB.

## D-010 — Procedural, original visuals only
- Date: 2026-11-12
- Status: accepted
- Decision: terrain, sky, booster, plume, dust and sound are generated in code; no third-party assets.

## D-011 — zustand for page state
- Date: 2026-11-12
- Status: accepted
- Decision: one zustand store wrapping a pure, unit-tested reducer.
- Consequences: usable from both React components and the R3F render loop.

## D-012 — Simulator in its own crate, stepped on the main thread
- Date: 2026-11-14
- Status: accepted
- Context: M2 needs the truth separated from guidance, advancing every frame,
  while a re-plan takes tens of milliseconds.
- Decision: new `kestrel-sim` crate (no dependency on guidance). The page runs
  a second instance of the same .wasm on the main thread for the `Flight`
  (simulator + sensors + controller); guidance stays in the worker. A plan is
  stamped with the sim time of the measurement it was solved from and read at
  `t − t_measured`, so solve latency is real and compensated.
- Alternatives: everything in the worker (solves would stall the simulation);
  simulator in TypeScript (D-002 puts the core in Rust; no native tests).
- Consequences: the page loads the WASM twice (~650 KB, cached); the closed
  loop is testable natively (`crates/wasm/tests/closed_loop.rs`).

## D-013 — Warm-started re-plan: three solves around the expected landing time
- Date: 2026-11-14
- Status: accepted
- Context: a full landing-time search is ~25 solves (~250 ms); MPC needs
  several re-plans a second.
- Decision: try landing times `t_hint − h, t_hint, t_hint + h` (h = 5 %,
  0.25–2 s) from the previous plan; keep the cheapest. Full search only if
  none works. Successive re-plans walk toward the optimum.
- Measured: median 30–43 ms per re-plan in Chromium (LIMITATIONS.md).
- Consequences: a re-plan may be slightly off the fuel optimum (≤ 1 % on the
  example in tests).

## D-014 — Closest-landing fallback with one weighted objective
- Date: 2026-11-14
- Status: accepted
- Context: when no pinpoint landing exists, the brief asks to fall back to a
  minimum-landing-error problem and say so.
- Decision: same SOCP with the horizontal end point free and the glide cone
  dropped; minimise `10·e − z_N` with `e ≥ ‖r_N,xy‖` (non-dimensional), i.e.
  miss first, then fuel. Searched over 10 landing times (+4 refinements), or
  3 around the previous one. When even that is infeasible: brake (full
  thrust straight up while descending) and say a crash is unavoidable.
- Alternatives: Blackmore et al.'s two-step problem (two solves per time).
- Consequences: near the ground with the ground constraint active the
  relaxation is often loose (NotLossless) at short landing times; those
  times are skipped.

## D-015 — Guidance plans with margins
- Date: 2026-11-14
- Status: accepted
- Context: fuel-optimal plans ride exactly on the thrust limit and empty the
  tanks; in the closed loop the first disturbance then made every re-plan
  infeasible (found in the native scenarios).
- Decision: plan to a terminal gate 15 m above the pad (the controller's
  descent law lands from there); plan with 85 % of the available maximum
  thrust, retrying at 100 % only if no landing exists otherwise or that
  reaches the pad; keep a propellant reserve (2 % of capacity + 12 s of
  hovering) the plan never uses.
- Consequences: more propellant than the open-loop optimum (nominal 229 kg
  vs 194 kg); landings survive noise, 1 s lag, gusts and one engine out.

## D-016 — Navigation error is slowly drifting and range-scaled
- Date: 2026-11-14
- Status: accepted
- Context: white Gaussian noise per 10 ms reading (8 m σ) saturated the
  controller (±3.8 kN of jitter) while the vehicle averaged it away; real
  navigation filters output errors that wander slowly, and landing radars
  get more precise near the ground.
- Decision: first-order Gauss–Markov error per axis (τ = 2 s) with the
  stated σ (low: 2 m, 0.2 m/s; high: 8 m, 0.8 m/s), scaled by
  `clamp(z / 200 m, 0.1, 1)`. Seeded (SplitMix64 + Box–Muller), reproducible.
- Consequences: "high" noise is survivable; the sensed position is drawn as
  a magenta ring so visitors can see it wander.

## D-017 — One vehicle model, two parameter sets; the booster is a third flight kind
- Date: 2026-11-15
- Status: accepted
- Context: the booster needs the same rigid-body equations, planner,
  powered and terminal controllers, contact logic and closed loop as the ship.
- Decision: `model::Ship` gains `VehicleKind`, the number of landing engines
  (3 / 13), the engines flown to the end (1 / 3), a linear lift slope and
  grid-fin data. New for the booster only: fin aerodynamics, the unpowered
  descent law, its scenario and start attitude. Flight kinds are `earth`,
  `mars` (ship) and `booster`; the page shows vehicle × planet.
- Alternatives: a separate booster simulator and planner (duplicates about
  2 000 lines that must stay in agreement).
- Consequences: every shared change is tested on all three kinds; a few
  rules are per-kind where the ship's tuned flights must not move (noted in
  the code where they are).

## D-018 — Three grid fins, not four
- Date: 2026-11-15
- Status: accepted
- Context: the owner's brief said four grid fins. The current (Block 3)
  vehicle has three in a 90/90/180° arrangement (Wikipedia, read
  2026-11-15); Blocks 1 and 2 had four.
- Decision: model the current vehicle: three fins at body +y, −y, +x.
  Pitch from the ±y pair, yaw from the single fin, roll from all three;
  cold-gas thrusters take what the fins cannot give.

## D-019 — The booster's burn is planned without crossflow drag and within 15° of vertical
- Date: 2026-11-15
- Status: accepted
- Context: with the full aerodynamic model the fuel-optimal 13-engine burn
  turned the booster broadside at 400 m/s to brake on air drag (cheap on
  paper). Forbidding that with a tilt limit left two mirror-image optima
  (leaning either way earns the same drag) and successive convexification
  hopped between them without converging.
- Decision: a soft 15° tilt limit on the booster's plans, and the planner's
  copy of the model has no crossflow drag (the linear lift term stays). The
  tracking controller absorbs the difference.
- Consequences: plans converge in 3–5 iterations. A sideways divert of more
  than about 150 m during the 13-engine burn is refused as infeasible.

## D-020 — Ignition altitude from integrating the fall, not from the present speed
- Date: 2026-11-15
- Status: accepted
- Context: "stop from the present speed at 70 % thrust" lit the booster's
  engines 12 km up from a 45 km start (it was falling three times faster
  than it would be once the air had slowed it), and lit the ship's at once
  on a fast entry.
- Decision: booster: integrate the fall with drag downward and the stopping
  burn (thrust plus drag) upward from the gate, in 1-D; light where they
  meet. Ship: take the later of the old rule and the same rule applied to
  the speed the fall predicts (on Mars the fall speeds up, so the old rule
  wins and its tuned flights are unchanged).
- Consequences: the ship flies a steep entry from 80 km at 2.5 km/s to the
  catch; booster starts above about 20 km still fail (LIMITATIONS.md).

## D-021 — Solve latency: warm-up, steady burn, predict the first plan only
- Date: 2026-11-16
- Status: accepted
- Context: in the browser the first solve took 1.4 s (cold WebAssembly);
  the booster's 13-engine leg lasts 9 s at 4 g. Braking flat out while
  waiting left the booster stopped a kilometre up with a plan that no
  longer fitted.
- Decision: each worker solves one throw-away plan when it starts; while
  waiting for its first plan the booster burns steadily at 60 %; the first
  plan is solved from the state predicted for the expected solve time
  (which the page measures and reports) and stamped with that time; an
  option the final engines plainly cannot fly is not solved.
- Alternatives: predicting every re-plan along the current plan (tried: each
  re-plan then fought the last and the booster hit the tower).
- Consequences: caught with solves up to 0.5 s of sim time at frame steps up
  to 0.2 s (native sweep, LIMITATIONS.md).

## D-022 — The launch and staging are an illustration; the simulation is the return
- Date: 2026-11-16
- Status: accepted
- Context: the page showed the stack on the pad and then, on Fly, a booster
  already falling. A simulated ascent, hot staging and boostback is a later
  roadmap item.
- Decision: an opening sequence timed by hand (countdown, liftoff, stage
  separation at 64 km, title card) with a caption that says it is an
  illustration; cuts fade through black; the simulated flight starts behind
  the title card. After a booster catch the visitor may fly the ship's
  return (the booster is then shown parked) or end.
- Consequences: nothing in the sequence is physics; it must be replaced
  when the ascent is simulated.

## D-023 — Commentary from a fixed phrase list, rendered once by a neural voice
- Date: 2026-11-16
- Status: accepted
- Context: the browser's built-in speech voices sounded robotic (owner).
- Decision: every line comes from a finite list (`phrases.ts`); each is
  rendered once to an MP3 clip by Kokoro-82M (Apache-2.0, run locally with
  `kokoro-js`; `web/scripts/make-voice.mjs`) and played back instantly; the
  browser voice is the fallback for a missing clip. The clips are generated
  output, committed under `web/public/voice/`; the model and tools are not
  dependencies of the site.
- Alternatives: an in-browser neural model (tens of MB to download and
  seconds per line: too slow for live calls); a cloud voice (network, keys).
- Consequences: a new line needs the script run again; a test checks every
  line the commentary can produce is in the list.

## D-024 — Sound is synthesized and mixed for the camera
- Date: 2026-11-16
- Status: accepted
- Decision: engine roar, crackle, rumble and airflow are generated with Web
  Audio (filtered noise and low oscillators); `mix()` turns thrust, distance,
  dynamic pressure and air density into gains, a low-pass cutoff and a
  travel delay (distance / 343 m/s); onboard is structure-borne (deep, no
  delay). Sonic boom, catch and loss are one-off sounds, delayed likewise.
- Consequences: no audio assets; levels and timbre are by ear, not
  measured (LIMITATIONS.md).

## D-025 — Mars removed
- Date: 2026-11-16
- Status: accepted (supersedes the Mars part of the M3 ship spec)
- Context: the owner asked for Mars to be removed completely. It also did
  not work on the page (plans took 2–3 s to solve in the browser and the
  flight ran out of propellant).
- Decision: the Mars scenario, the legs, the `landed` outcome and the
  planet choice are deleted from model, simulator, guidance, the wasm API
  and the page. `Planet` keeps one variant (Earth). The M0–M2 3-DoF code
  keeps its `Vehicle::example_mars` test vehicle (the paper's numbers); it
  is not served.
- Consequences: the terminal controller no longer has to work in Mars
  gravity, which allowed D-026.

## D-026 — A quick final approach: slide in while descending, arms take metres
- Date: 2026-11-16
- Status: accepted
- Context: the owner: close to the tower "it takes so much time to be
  caught while in real life it is very fast". Measured: about 35 s between
  the approach gate and the catch for both vehicles. The terminal law
  hovered 20 m up until the axis was within 3 m, then crept to within 1 m
  at a few tenths of a metre per second.
- Decision: (1) the plan's gate is nearer the catch: 14 m out, 12 m
  (booster) or 14 m (ship) above the caught position, with 2 m/s toward
  the axis; (2) the terminal law descends to 5 m while it slides, sideways
  speed from a square-root braking curve far out and a low linear gain
  close in; (3) the arms accept the vehicle within 4 m of the catch axis
  (was 1.5 m), tilt 5° and rate 5°/s (were 3° and 2°/s), contact speed
  limits unchanged (2 m/s down, 1 m/s sideways).
- Why (3) is fair: real catch arms close onto the vehicle and it lands
  anywhere along their rails; a 1.5 m circle was this simulation's own
  earlier choice, not a published figure. All of it is *illustrative*.
- Alternatives tried: a square-root braking law all the way in (weaved
  across the axis and hit the tower: the sideways loop runs through the
  attitude loop and cannot be stiff); gates 3–10 m out (hit the tower
  under sensor noise or slow solves); no inward speed at the gate (2 s
  slower).
- Consequences: measured in LIMITATIONS.md (about 12 s). The vehicle may
  hang up to 4 m off the tower's centre line; the arms are drawn closing on
  the centre line, so a far-off catch looks slightly off.

## D-027 — The ship still drops to one engine right after the flip
- Date: 2026-11-16
- Status: accepted for now (open on the roadmap)
- Context: the owner: the ship "turns down to 1 or 2 engines too quickly".
  Cause: at 870 m and 90 m/s three engines at minimum throttle push a
  nearly empty ship at about 2 g, more than the burn needs, so the first
  plan that fits is the one-engine plan.
- Tried: preferring plans on three, then two engines; igniting lower (500–
  700 m) with the aim point moved in. Result: 0–2 catches in 4 runs per
  setting (tower strikes, propellant exhaustion). Reverted.
- Decision: leave the ship's burn as it was. The quicker final approach
  shortens the one-engine time by about 22 s. A proper fix needs the flip,
  the aim point and the ignition rule retuned together.

## D-028 — Analysis view: the scene becomes the drawing
- Date: 2026-11-16
- Status: accepted (replaces the screen-space "blueprint" filter of D-022's page)
- Context: the owner wanted everything but the vehicle to disappear, the
  vehicle as a wireframe blueprint on blue, any part clickable with a zoom,
  and detailed calculations on the left.
- Decision: swap materials in the scene instead of filtering the picture
  (see ARCHITECTURE.md). Per-part numbers use the model's own formulas:
  Rust sends its coefficients with the scenario and `parts.ts` evaluates
  them on the snapshot, so nothing shown is a separate estimate. A paused
  flight can be stepped (0.1 s, 1 s).
- Consequences: per-engine thrust is total / lit (the simulator has one
  throttle for all); a flap's or fin's force is recomputed on the page
  from the same formula, not read back from the integrator.

## D-029 — One clock for the countdown
- Date: 2026-11-16
- Status: accepted
- Context: the owner: countdown, counter and voice did not align, and the
  opening sequence's text came too quickly. The count was one clip
  ("Five. Four. …") played beside a display dividing 6.5 s by five.
- Decision: `launchState()` gives the count, the mission clock and the
  engine power from one elapsed time; each number is its own clip, cued on
  its second and preloaded at the click; other lines are cues at least 2 s
  apart. Ten-second count, 51.5 s sequence (was 33.5 s).

## D-030 — Ship model reshaped toward the real vehicle; kestrel mark
- Date: 2026-11-16
- Status: accepted (within D-010: procedural, no logos, no third-party assets)
- Decision: tangent-ogive nose with a rounded tip; forward flaps on the
  nose and long aft flaps, leading edges swept, steel hinge fairings;
  tiles on the windward nose; leeward raceway and payload slot; dark
  engine shielding on the booster. Proportions by eye from public
  photographs, not from drawings. The header mark and tab icon are a
  hovering kestrel drawn for this project.
