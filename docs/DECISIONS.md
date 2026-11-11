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
- Status: proposed (confirm during M0)
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
