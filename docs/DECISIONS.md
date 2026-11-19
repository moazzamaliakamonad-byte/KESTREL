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

## D-031 — Engine-out: relight a spare (supersedes D-027's open question)
- Date: 2026-11-17
- Status: accepted
- Context: the owner asked for the ship to stay on two engines "till the
  last stages of final approach", and then explained the goal: "if the
  running engine fails the ship will just fall". Two engines were tried at
  length (D-027 and since): at their 40 % minimum throttle two Raptors push
  a nearly empty ship at about 1.6 times its weight, so they cannot fly the
  slow end of a landing at all; every 2-engine plan near the gate was
  infeasible.
- Decision: keep one engine for the burn and solve the actual problem: a
  shut-down engine is a spare. When fewer engines are lit than the burn
  needs, the flight software relights one (it gives thrust after its 1 s
  ignition time), asks for a new plan at once, plans from where the ship
  will be when the spare lights, and, after an engine loss only, may use
  98 % of the engine's thrust instead of the usual 75 % margin.
- Measured (`engine_out_window`, the running engine failed at one of ten
  times between 49 and 67 s, noise off and low): 15 of 20 caught; before,
  none. Failures at 65 s and 67 s (the last seconds before the gate) were
  all lost, as were some at 49-63 s: in traced cases the ship tilted during
  the second without thrust and its base swung into the mast. See
  LIMITATIONS.md.

## D-032 — The ship's flip is planned and flown by guidance
- Date: 2026-11-17
- Status: accepted (replaces the scripted flip of the M3 ship plan)
- Context: owner: "starship's guidance don't work until it flips". The flip
  was a scripted manoeuvre; guidance planned only from the end of it. The
  M3 build ledger records why: a planned engine switch inside one plan
  "made re-plans infeasible".
- Decision: from the moment the engines reach thrust, guidance plans the
  flip on every lit engine, a switch to one engine at about the end of the
  flip (node chosen from the tilt still to go, at about 35 deg/s), and the
  burn; the flight flies it and makes the switch when the plan says. The
  scripted flip remains only until the first plan arrives. After the flip
  the plan keeps within 30 deg of upright (the flip may overshoot vertical
  to brake the 40 m/s it builds up toward the tower).
- Measured (12 nominal flights, seeds 7-10, noise off/low/high, and 20
  engine-out flights): with the scripted flip 6 of 12 and 5 of 20 caught;
  guided 11 of 12 and 14 of 20 (24 nodes). It is the largest single gain
  of the redesign.

## D-033 — Ship approach: plan to 10 m out, 20 m up (supersedes D-026's gate)
- Date: 2026-11-17
- Status: accepted
- Context: owner: "instead of slide in it should just come in optimised way
  to the chopsticks". Plans ending on the arms (2 m out, 5 m up) were tried
  first and caught 9 of 12 nominal flights: any sideways braking next to
  the tower tilts the ship, and a tilted ship's base swings into the mast
  (5 m from the hull at the catch axis).
- Decision: the plan, fuel-optimal and flown from the flip, ends 10 m out
  from the catch axis (away from the mast) and 20 m above the caught
  position, moving in at 1 m/s and down at 2 m/s; the terminal controller
  flies those last 20 m. Narrow approach funnel (3 m at the gate, 0.6 m
  per metre above), side-speed limit 2 m/s + 0.15 per metre.
- Measured candidates (nominal / engine-out, of 12 / 20): gate on the arms
  9/5; 6 m out 20 m up 10/9; 10 m out 20 m up 11/14; 12 m out 9/15; the
  old 14 m out 14 m up 9/12. Funnel 10 m/1.0 instead of 3 m/0.6: 10/13.
  Tilt cap of 12 deg for plans made near the tower: no gain (11/13).
- Consequences: median ship flight 81 s (86 s before the redesign, 99 s on
  2026-11-15); 4-10 s from the gate to the catch.

## D-034 — Planner speed and robustness, per vehicle
- Date: 2026-11-17
- Status: accepted
- Measured where the time goes (ship flip problem, native): the convex
  subproblem about 22 ms per SCvx iteration, discretization 3 ms. So:
  - Ship: 16 nodes instead of 24 and an interior-point tolerance of 1e-6
    instead of 1e-8: 9.5 ms per subproblem (2.3 times faster), same
    iterations and fuel; 32 closed-loop flights caught as many (25 of 32
    both). 12 nodes: 3.3 times faster, 23 of 32.
  - Ship: a solve still needing virtual control (defect > 1e-2) after 7
    iterations is abandoned as infeasible (a failed solve took over 1 s).
  - Booster: kept at 24 nodes and 1e-8 (16 nodes or 1e-6 each failed its
    slow-solver flights), abandoning nothing early (that lost 2 of 33 in
    the browser-timing sweep), but capped at 8 iterations (D-036).
  - Both: a plan is accepted only if, re-flown on the true dynamics, it
    ends within 15 % of the distance it had to cover from its gate (3 to
    30 m), instead of always 30 m: plans missing by 20 m next to the tower
    had been accepted.

## D-035 — Events ask for a plan at once
- Date: 2026-11-17
- Status: accepted
- Context: owner: "replan extremely fast as soon as something happens".
  Re-plans were triggered by drift (5 m, 2 m/s) or age (3 s), at most every
  0.5 s of sim time.
- Decision: a gust, an engine failure, a relight, or a changed wind, lag or
  noise setting calls `request_replan()`; the page sends the request on
  the next frame, skipping the 0.5 s minimum gap.

## D-036 — Booster solves capped at 8 iterations
- Date: 2026-11-17
- Status: accepted
- Context: booster plans that ran to the 15-iteration limit were slowly
  converging, not improving, and took up to 1 s natively and about 3 s on
  the 3D page, arriving stale.
- Decision: at most 8. Worst plan on the reference flight (Node,
  WebAssembly) 519 ms instead of 1 041 ms. The browser-timing sweep, which
  models a fixed latency and so cannot credit faster solves, loses its
  most extreme case (1 s latency, seed 23): 32 of 33.

## D-037 — The picture gives way to guidance
- Date: 2026-11-17
- Status: accepted
- Context: the same plan took a median 134 ms in a Chromium worker on a
  bare page and 439 ms on the 3D page (i7-1185G7 laptop, integrated
  graphics): the renderer and the planner share one power and memory
  budget.
- Decision: no 4x multisampling (SMAA instead, as before 2026-11-16), pixel
  ratio at most 1.5, and when the median of recent solves passes 250 ms the
  picture steps down (pixel ratio 1, then no bloom). Ship median on the
  page: 89 ms in one run after the change; later runs 276-460 ms with the
  machine at 30 % background load (timings on this laptop vary widely,
  LIMITATIONS.md).

## D-038 — A flight record; units and sources on hover
- Date: 2026-11-17
- Status: accepted
- Decision: every decision (plans with their reason, solve time and shape;
  failures; phases; engine changes; relights; the arms; the outcome; the
  visitor's own actions) is recorded with its numbers, and the full state
  every 0.1 s; the analysis view lists them and replays any instant. Every
  unit on screen explains itself on hover, and every "published" or
  "illustrative" tag gives its source or reason. Dry masses are now marked
  illustrative: SpaceX has not published them (they were marked published).

## D-039 — Late engine-out: an emergency gate and an envelope
- Date: 2026-11-17
- Status: accepted
- Context: failures of the running engine in the last seconds before the
  approach gate were all lost (D-031). Traced: after the 1 s relight gap
  the ship is too low and fast for a plan to the usual gate (20 m above
  the arms), every plan fails, and the braking fallback stops it too low
  or beside the mast.
- Decision: after an engine loss the planner tries, last, a gate 12 m
  lower (8 m above the caught position) at full thrust. Engine-out sweep:
  17 of 20 caught (15 before); nominal flights unchanged (10 of 12).
- Also added: `guidance::envelope`, the point-mass bound on the sink speed
  from which a relight can still stop the ship above the arms; shown on
  the page and printed by the sweep at each failure. It separates losses
  physics allows from losses it does not.
- Tried and rejected:
  - The envelope as a constraint in the planner (descend no faster than an
    engine loss can be survived, written exactly as tangent half-planes of
    a concave bound, soft): plans stopped converging and swung near the
    tower. 3 of 12 nominal, 5-12 of 20 engine-out, with shared or separate
    slack and three penalty weights. The idea is sound; the slow descent
    it demands next to the tower costs more than it saves with this
    controller.
  - Preferring a converged plan from any option over an unconverged one
    after a failure: 15 of 20.

## D-040 — A navigation filter between the sensors and guidance
- Date: 2026-11-17
- Status: accepted
- Context: under "high" sensor noise the ship hit the tower in 1-2 of 4
  runs. Traced: the position reading wanders by up to 8 m (correlated over
  2 s); the controller chased it, really moving the ship sideways, and
  drift-triggered re-plans pulled different ways. M0-M2 recorded "no state
  estimator" as a deliberate simplification.
- Decision: a complementary filter, the simplest navigation filter: every
  10 ms the estimate is carried forward by the accelerometers (the true
  velocity change over the step, taken as exact) and pulled toward the
  position and velocity readings with time constants per vehicle: ship
  1 s and 0.5 s, booster 4 s and 2 s. Guidance and every controller use
  the estimate. A plan request uses the estimate carried forward to the
  moment of the request.
- Correction (same day): the first version handed plan requests the
  estimate from the previous 10 ms step, stamped "now". At the booster's
  400 m/s that is 4 m: the page's prediction (zero-latency plans) failed
  275 of 303 plans and the ship's hit the tower. The noisy scoreboard did
  not show it, and the numbers first logged here were measured with it.
  A test now flies both predictions (`tests/preview.rs`). Re-measured
  after the fix (all below).
- Measured, ship (seeds 7-10 x off/low/high, engine-out sweep of 20,
  seeds 11-30 at high and low noise):

  | Filter | Nominal | Engine-out | High noise | Low noise |
  | --- | --- | --- | --- | --- |
  | none (raw readings) | 11/12 | 18/20 | 11/20 | 20/20 |
  | 1 s / 0.5 s (chosen) | 12/12 | 20/20 | 19/20 | 19/20 |
  | 2 s / 1 s | 12/12 | 17/20 | 18/20 | 20/20 |
  | 4 s / 2 s | 12/12 | 20/20 | 19/20 | 18/20 |
  | 2 s / 0.5 s | 12/12 | 18/20 | 15/20 | 20/20 |
  | 4 s / 1 s | 12/12 | 16/20 | 18/20 | 20/20 |

- Final engine-out sweep with the chosen filter: 20 of 20.
- Measured, booster (seeds 11-30): raw readings 11/20 at high noise and
  19/20 at low; 4 s / 2 s 20/20 and 20/20. Browser-timing sweep 33 of 33
  (32 before).
- One seed per cell; differences of one or two flights are noise.

## D-041 — Kestrel's own convex solver, beside Clarabel
- Date: 2026-11-17
- Status: accepted
- Context: the brief's stretch goal and the roadmap: a solver "with no
  heap allocation and bounded run time, as flight code needs".
- Decision: `crates/socp`, an interior-point solver for the same problem
  form as Clarabel: Nesterov-Todd scaling, Mehrotra predictor-corrector,
  Ruiz equilibration, a sparse LDLᵀ factorization (QDLDL's algorithm) of
  the regularized KKT matrix with a minimum-degree ordering, one step of
  iterative refinement, at most 40 iterations. Everything is allocated in
  `Solver::new`; `solve` and `update` allocate nothing (a counting
  allocator test checks it). Guidance keeps one set-up solver per problem
  shape per thread, so after the first plan of each shape no setup runs.
- Clarabel stays the default; Kestrel's own is a choice on the page
  ("Guidance solver") and natively (`KESTREL_SOLVER=kestrel`). Its
  iterations cost about the same as Clarabel's but it takes more of them:
  booster plans about 30-45 % slower (median 88-99 ms against 66-70 ms
  natively), ship plans 5-10 % faster (48-53 against 54-58 ms; LIMITATIONS.md,
  "Solve-time benchmark"). The booster is the vehicle sensitive to solve
  time (D-036). Flight outcomes with the final guidance (D-040) are the same with either:
  ship nominal 12/12, engine-out sweep 20/20, seeds 11-30 at high and low
  noise 38/40 (Clarabel 38/40); booster 40/40 (Clarabel 40/40);
  booster browser-timing sweep 33/33; every regression test passes with
  both. Their answers differ by about 10⁻⁶ relative (at most 7·10⁻⁴), so
  single flights near a knife edge can go either way.
- Tried: without equilibration the solver stalled (maximum iterations or
  numerical errors) on the hard subproblems after an engine loss at
  ignition, 34 of 37 plans failing; with it, every subproblem solved.
  Setting up the solver anew for every plan cost 4-9 ms per plan (the
  ordering); keeping set-up solvers removed it.
- Speed: see LIMITATIONS.md, "Solve-time benchmark".

## D-042 — Entry from orbit: a guided 3-DoF entry that hands over to the landing
- Date: 2026-11-17
- Status: accepted
- Context: the roadmap's "orbital entry (round planet, heating that
  matters)". The ship's highest start was a fall from 80 km at 2.5 km/s.
- Decision:
  - Truth: a point mass on a round, non-rotating Earth (μ/r² gravity,
    1976 standard atmosphere), lift and drag at a Mach-dependent angle of
    attack (*illustrative*: lift-to-drag 0.46 hypersonic, drag only below
    Mach 1.5), bank angle rolling at 15°/s (`model::entry`, `sim::entry`).
  - Heating that matters: Sutton-Graves stagnation heating, tile
    temperature by radiative equilibrium; above 1 700 K for 8 s the heat
    shield burns through (all *illustrative*).
  - Guidance (`guidance::entry`): a numerical predictor-corrector in the
    family of FNPEG (Lu 2014). Every second it flies the rest of the entry
    ahead with a constant bank and bisects the bank until the hand-over
    lands on the aim point; bank reversals keep the heading on the tower
    within a deadband (10° fast, 0.25° slow); near the tile limit a
    heating override lifts the ship.
  - Hand-over at 25 km to the existing 6-DoF flight, in the tower's frame,
    aiming 27.5 km before the tower: start probes found 20.5-28 km caught
    (the belly flop carries the ship about 20 km on) and the entry ends
    about 2.5 km short of its aim.
  - The page: a "From orbit" start with distance and angle, the entry at
    ×16 on a procedural globe, its own telemetry, real time at the
    hand-over.
- Measured: nominal (3 300 km, -1.0°) entry 624 s, peak 1 674 K and 0.38
  MW/m², 2.9 g, hand-over 25.1 km out, caught at 827 s; 14 of 20 starts
  in the corridor sweep caught (LIMITATIONS.md).
- Tried: a 1 650 K tile limit (every start in the swept range came within
  50 K of it or above); aiming the heading at the aim point instead of the
  tower (the ship arrived with 88 m/s sideways, which the belly flop
  cannot remove, and was lost); the hand-over aimed at 3 km and 25 km out.

## D-043 — One continuous flight from orbit to the arms
- Date: 2026-11-18
- Status: accepted
- Context: owner: "I don't like how the simulation from orbital entry to
  next phase just switches". The entry was drawn in its own Earth-centred
  scene and the landing in the tower's flat one; at the hand-over the
  picture cut to a different world and camera, the ship's attitude was
  reset to belly down, and time dropped from ×16 to ×1 at once.
- Decision:
  - One frame for both: the tower's frame on the round Earth
    (`model::entry::EarthFrame`, exact 3D, x following the ship by a
    fraction of a degree). During the entry the page is given the ship's
    position, velocity and attitude in it; the 6-DoF flight starts from
    exactly those numbers, attitude included.
  - The entry attitude eases from about 60° angle of attack (hypersonic)
    into the belly-flop pose (body level, belly down) between Mach 5 and
    Mach 1.5, so the hand-over attitude is the one the belly flop flies.
  - The page draws a round Earth whose top lies just under the flat
    landing ground, a regional ground that continues the painted coast,
    and a sky and haze that follow the true altitude and local vertical;
    the flat horizon planes give way to it on flights from orbit.
  - One camera through the hand-over: during the entry it rides beside
    the ship (following it exactly, smoothing only its offset) and over
    the last 10 km eases into the chase framing the landing starts with;
    leaving the entry is never a cut, and the director holds the chase
    shot for 30 s after.
  - Time eases from ×16 to ×1 with altitude (55, 42, 34, 28 km).
- Measured: the corridor sweep is unchanged (14 of 20); the nominal
  flight from orbit is caught at 826 s; frames filmed across the
  hand-over show the same ground, attitude and framing either side.

## D-044 — Scope: the landing guidance only; no simulated ascent
- Date: 2026-11-18
- Status: accepted
- Context: the M3 roadmap still listed a simulated ascent, hot staging and
  boostback ("the full mission"). Owner: "remove the simulated ascent we
  only made the landing guide system that's it".
- Decision: the item is removed from the roadmap. Kestrel simulates and
  guides the return: from a start in the air (or from orbit, D-042) to the
  catch or loss. The opening sequence (countdown, liftoff, stage
  separation) stays an illustration timed by hand and says so on screen.
- Consequences: D-022's "it must be replaced when the ascent is simulated"
  no longer applies: nothing replaces it. With this, M3 is done.

## D-045 — Publishing: CI on every push, the page on GitHub Pages
- Date: 2026-11-18
- Status: accepted
- Context: the project goes public on GitHub. Most visitors will not
  clone and build a Rust + WebAssembly project, so it needs a live page;
  nothing checked the code except the owner's machine. The owner: the
  coding agent's rules file (`CLAUDE.md`) "should not be pushed".
- Decision:
  - One workflow (`.github/workflows/ci.yml`) on every push and pull
    request: `cargo fmt --check`, `cargo clippy -D warnings`,
    `cargo test`; the WebAssembly build, `npm run lint`, `typecheck`,
    `test`, and the static site build. The opt-in slow tests
    (`#[ignore]`, Playwright) stay off, as the test policy says.
  - Pushes to `main` publish `web/out` to GitHub Pages. The page is
    static (D-005), so Pages needs no server; the base path is the
    repository name, set through `NEXT_PUBLIC_BASE_PATH`.
  - `wasm-bindgen` comes as a prebuilt binary at the pinned 0.2.130
    (D-007) instead of `cargo install`, which would compile it on every
    run.
  - `CLAUDE.md` joins `learn/` and `project-ideas.md` as local-only and
    gitignored.
- Consequences: the first deploy needs one manual step on GitHub
  (Settings → Pages → Source: GitHub Actions). The workflow has only run
  on this machine's build so far: the base-path build was served under
  `/Kestrel/` and flown in Chromium (page, solver worker and landing burn
  ran, no load errors); the Linux runner itself is unproven until the first push.

## D-046 — Guidance estimates the wind and flies with it (M4.1)
- Date: 2026-11-18
- Status: accepted
- Context: the owner: guidance "is not as smart as it should be", naming
  among other things that it does not react to wind. The simulator blew
  wind and gusts, but guidance modelled still air everywhere: the
  planner's dynamics were called with zero wind, the controllers'
  torque model had "no wind known", and guidance only fought the wind
  after being pushed off its plan. Measured before the change (wind
  sweep, LIMITATIONS.md): 110 of 216 flights caught; none of either
  vehicle at 20 m/s or more.
- Decision:
  - `guidance::wind`, a two-state extended Kalman filter on the
    horizontal wind. Measurement: the air's force, from the
    accelerometers less gravity and the engines' delivered thrust
    (engine telemetry: chamber pressure and gimbal angle, which real
    engines report; a 1 % reading, *illustrative*). Prediction: the
    shared aerodynamic model (body and surfaces at their measured
    positions) at the airspeed `v − w`. Its noise comes from the
    readings' own errors (attitude and velocity through the model's
    sensitivities, the thrust line, a floor), so it trusts the air at
    speed and coasts near hover with no special case. A random walk lets
    it follow gusts; innovations beyond a 3-degree-of-freedom gate are
    set aside unless they persist 0.3 s (a glitch, not a gust).
  - It learns while the engines are off and holds its estimate while
    they give thrust. Running through the burn (the first design) it
    wandered up to 10 m/s in still air at high noise and 4.9 m/s with no
    noise at all: beside meganewtons of thrust the air's kilonewtons are
    lost, and the thrust line's own errors (the vehicle turning within a
    step, the attitude estimate) are correlated, so the filter grew sure
    of wind that was not there, and the still-air boards lost flights
    (ship seeds 38 → 36 of 40, booster 40 → 39, engine-out window 20 →
    18). The simulator's wind is the same at every height, so the fall's
    estimate stays valid; a gust that begins in the burn is not tracked.
  - Errors that stay alike over time count once per correlation time,
    not once per step (their variance times 2τ/h: the attitude reading's
    2 s, the navigation velocity's time constant). Counting every 10 ms
    reading as new evidence, the ship's belly-flop estimate wandered by
    5 m/s while claiming ±1 (unit test: worst error 3.6σ before, 0.8σ
    after).
  - Guidance flies only the **significant** wind: the estimate shrunk by
    two of its standard deviations along it, and none when it cannot be
    told from calm. Without this the knife-edge flights flipped on noise:
    an engine-out flight with no sensor noise at all was lost with an
    estimate of 0.02 m/s. With it, a still-air flight is (almost always)
    the wind-blind one, and winds under about 2σ (1-3 m/s) are not flown.
  - One rule everywhere: the wind enters as *the model in the estimated
    wind minus the model in still air*, so a zero estimate is exactly
    the old guidance. A switch (`wind_aware`, the page's "Guidance uses
    its wind estimate") flies the old guidance on the same build, bit for
    bit (`unaware_flight_matches_today`).
  - Where it is used: the planner's dynamics (`ShipProblem.wind`); the
    booster's descent (it leans into a crosswind before being pushed);
    the final approach (a feed-forward of the wind's force); the
    booster's burn tracking adds only the crossflow its plan leaves out
    (D-031: the burn is planned without it; the ship's plan models it
    all); every torque model; the short predictions (the booster's first
    plan, a spare's spin-up). Not used: the ignition height and fall time
    (vertical), the engine-out envelope, the entry.
  - On the page: a telemetry row, two arrows in the analysis view (the
    estimate and the true wind, labelled as not known to guidance), and
    flight-record entries when the estimate settles (1σ under 2 m/s) and
    when a gust is felt (more than 4 m/s within 3 s, at most every 4 s).
    No spoken lines: the voice model could not be fetched on this
    network, and the fallback voice would not match.
- Tried and removed: a belly-flop roll against a crosswind (no effect:
  hull and flaps are symmetric about the long axis, so a roll makes no
  sideways force) and a belly-flop drift term along the track (3 m
  worse than the feedback alone).
- Consequences: the wind sweep catches 144 of 216 (110 before; the
  booster at 20 m/s 14 of 18, none before), with a lower gate error at
  10 m/s and above; one marginal flight (booster, 5 m/s, high noise) is
  lost that was caught. The still-air boards are identical with and
  without the estimate (same flights lost, same propellant left), so
  nothing that worked in still air changed (LIMITATIONS.md has the
  tables). The ship still
  loses most strong crosswinds: it drifts ~500 m sideways in the belly
  flop, which only a heading choice can fix (M4.3). The default test
  suite stays under a minute.

## D-047 — The final approach: a glide, not a plan (M4.2)
- Date: 2026-11-19
- Status: accepted
- Context: the owner: the last approach "looks robotic". From the burn's
  gate (D-033) the terminal controller slid over the catch axis, stopped
  5 m above the arms until centred, then sank. The goal was a planned,
  re-planned approach onto the arms instead.
- Tried first, and kept as an option: `guidance::approach`, a 3-DoF
  point-mass SOCP from the estimate onto the caught position (20
  intervals, Kestrel's own solver set up once and updated after), keeping the base and nose off the mast (the
  tilt linearised at hover), with a margin of 1 m plus twice the
  navigation error (measured: 0.2-0.3 m RMS at low noise, 0.9-1.1 m at
  high). Getting a vehicle on its last engine near hover to follow it
  took, from traces: an acceleration linear between nodes (constant per
  interval and re-planned faster than an interval, it never tilted), a
  tilt-rate limit, starting from the last command (not the measured
  tilt), soft comfort limits (fuel-optimal plans ride their limits and
  re-plans failed; Clarabel agreed those problems were infeasible), a
  gentle objective, slower toward the mast near the axis, re-planning
  only on drift and at most every 0.5 s (many re-plans a second fed back
  on themselves: 18 degrees of tilt), and flying it by commanded velocity
  like the terminal controller (feeding its acceleration and tilt rate
  forward swung 5 m across the axis in the closed loop).
- Measured (`approach_board`, same build): it lost on every board.
  Scripted / planned: ship nominal 12/12 / 8/12 (gate to catch 11.4 s /
  22.9 s), ship seeds 11-30 38/40 / 28/40, booster seeds 11-30 40/40 /
  33/40, ship engine-out window 20/20 / 11/20. Most losses followed the
  planner giving up mid-way and handing a half-flown state to the
  scripted fallback. It stays (`planned_approach`, off by default, not on
  the page).
- Decision: the terminal controller **glides**. While still off the axis
  the height it may come down to falls with the distance from the axis
  (1.5 m per metre beyond 1 m, at most the old 5 m), so it slides in and
  comes down in one movement; held a few metres off but steady (a strong
  crosswind) it creeps down as before; the final settle slides in at most
  0.4 m/s and arrives at 1.0 m/s (1.3 before). Each of those three came
  from a measured loss: two high-noise touchdowns too fast (one sideways,
  one down), and 25 m/s crosswinds hovering dry.
- Consequences (`approach_board`, then `wind_sweep`): every board catches
  what it did (ship 12/12, 38/40, booster 40/40, engine-out 20/20, the
  same two flights lost); gate to catch 9.1-10.9 s instead of 11.4-13.9
  s; at contact 1.11-1.25 instead of 1.42-1.61 m/s down and 0.24-0.31
  instead of 0.31-0.42 m/s sideways. Wind sweep 146 of 216 (144 before),
  no fewer at any wind speed. A sink cap that went negative below the
  rest height panicked in the sweep; fixed and tested.

## D-048 — The ship chooses its approach bearing (M4.3)
- Date: 2026-11-19
- Status: accepted
- Context: the owner: "it always falls the same way": every flight lined
  up for the same +x approach (gate, aim, the ship's nose toward −x). And
  M4.1 found the ship's crosswind loss: the belly flop steers along its
  nose (the pitch slides it there) but cannot cross it (hull and flaps are
  symmetric about the long axis), so a crosswind drifted it ~500 m across
  its approach and no plan reached the +x gate.
- Decision: `Scenario::turned(ψ)` turns the gate, its velocity and the aim
  about the catch axis; the belly attitude turns with ψ (nose toward the
  tower from that side) and its pitch slides along the bearing; the
  burn's tower-side keep-out becomes the half-plane `b · r ≥ c`. The ship
  chooses ψ once, below 6 km (when the wind estimate settles, or 70 % of
  the way down): the drift across the +x track by ignition (its sideways
  speed and the wind's, relaxing with the hull's drag; from thin air the
  relaxation is slow, so the choice waits for 6 km), and if at least
  400 m, ψ = atan2(drift, distance to the aim), within ±60° of the arms'
  open side. Smaller drifts the burn takes up, as before. The booster
  steers across its fall with its lean and keeps +x. At ψ = 0 everything
  is exactly the old geometry (still-air flights bit for bit). The page
  draws the gate at the bearing and records "Approach turned".
- Tried: choosing ψ toward the unsteered landing point (the along-track
  momentum made a still-air flight look like a far-side approach: +60°,
  out of propellant); a 250 m threshold (a flight from orbit 193 m across
  ran dry when it turned; and 10 m/s crosswinds, ~300 m, dropped from 16
  to 14 of 18).
- Consequences (`wind_sweep`): the ship in 15 m/s crosswinds is caught 11
  of 18 (5 before), none fewer at any wind speed; sweep 152 of 216 (146
  before). Still air is unchanged (the bearing stays +x). The ship at 20
  m/s (4 of 18) and 25 m/s (0) is still mostly lost: at ±60° the bearing
  is clamped and the drift is more than it absorbs.

## D-049 — Kestrel's own solver is the only one that flies (M4.4)
- Date: 2026-11-19
- Status: accepted
- Context: the owner: the own solver "was the whole point of this
  project", and "why we even have another solver in there?". Since D-041
  Clarabel flew by default and the 3-DoF guidance called it directly.
  Flying the boards on Kestrel's solver as it was found two gaps:
  infeasible 3-DoF problems ran to the iteration cap instead of being
  reported infeasible, and booster plans at 1e-8 lost precision near the
  optimum (an iterate at 2e-8 relative gap, then a 5e-6 step, the
  residual from 1e-10 to 1e-5, then NaN), so one booster flight got no
  plan at all and ran dry.
- Decision: `crates/socp` gains (1) a primal infeasibility certificate
  (Farkas: `‖Aᵀy + Gᵀz‖ ≤ 1e-6 (−bᵀy − hᵀz)`, sound unless every
  feasible point has a scaled norm of 1e6 or more; measured: feasible
  landing problems kept the ratio above 1, infeasible ones stalled at
  1e-7..1e-8) and (2) `AlmostSolved`, the best iterate within Clarabel's
  default reduced tolerances when the iteration stops short. The 3-DoF
  guidance is built in the solver's own types and solved by it; its
  propellant limit is decided from the landing-time search (the only
  constraint the search leaves out is final mass ≥ dry mass), since a
  marginally short propellant load left no certificate strong enough.
  Clarabel becomes optional (`reference-solver` feature), enabled only by
  kestrel-wasm's dev-dependencies: the benchmark, `SOCP_COMPARE`, and the
  test that the switches-off flight equals the pre-M4 code (flown on
  Clarabel). The solver choice is per thread. The page's solver switch
  is removed.
- Also found on the way: the glide could settle into the arms while
  sliding back across the axis (a prediction, 1.02 m/s sideways); below
  its funnel floor and still sliding it now climbs back (GLIDE_RISE).
- Consequences: the page's WebAssembly is 0.80 MB (1.13 MB with
  Clarabel). Glide board unchanged flight for flight (12/12, 38/40,
  40/40, 20/20); wind sweep 150 of 216 (152 on Clarabel the same day;
  4 flights caught only on Clarabel, 2 only on Kestrel: not significant).
  Native plan times: booster median 53.6 ms (Clarabel 49.5), ship 31.5 ms
  (35.6). On the page both vehicles were caught. Open: some booster
  subproblems stall with the primal residual at 1e-4..1e-6 at the
  40-iteration cap, where Clarabel reports infeasible; telling marginal
  infeasibility from near-optimality reliably needs a homogeneous
  embedding (Clarabel's method), not looser tolerances.

## D-050 — Booster engine-out mid-burn: measured, guidance unchanged (M4)
- Date: 2026-11-19
- Status: accepted
- Context: ROADMAP: "two outer engines lost at 30 s hit the tower" (seen
  on the page on 2026-11-18, M3 code on Clarabel). Measure, then fix.
- Measured: a 320-flight board (`booster_engine_out_board`: one, two or
  three outer engines or one centre engine lost at 26-33 s, five seeds,
  0.5 s solves, frames of 1/60 and 1/15 s): 313 caught; that case caught
  in all 10 of its flights. The 7 losses come from a sideways swing on the
  last legs: the tracking controller leans further than the plan, the
  next plan starts from that lean, and with half a second of latency the
  swing grows; carried past the tower-side keep-out, no plan can exist.
- Tried, measured, not kept: (1) a quadratic price on sideways speed in
  the booster's planner, 316 of 320 (5 flights caught only with it, 2 only
  without), and isolated plans from the traced states barely changed even
  at 100 times the weight: the plans are bound by attitude and constraints,
  not under-priced, so this was noise, not a fix. (2) The keep-out
  loosened (node slack, comfort price) when the start is inside it or
  within 1 m of its edge, so a plan always exists: 302 of 320. Those
  plans were poor (one 39 s long, unconverged), and with no plan the
  existing fallback (brake upright toward the axis) did better.
- Decision: guidance unchanged; the board stays as an ignored test; the
  swing is a separate item (the controller's lean), to be measured on
  every board before any change.
