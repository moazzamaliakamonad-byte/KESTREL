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
