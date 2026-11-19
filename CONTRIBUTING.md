# Contributing to Kestrel

Thanks for wanting to help. Kestrel is a browser rocket-landing
simulator with real convex landing guidance in Rust → WebAssembly. Bug
reports, ideas, tests, sharper guidance and better visuals are all
welcome.

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to help

- **Report a bug.** Open an issue with the bug template. The most useful
  report says the vehicle, the settings (wind, noise, failed engines,
  start), what you did and when (the flight clock), and what happened.
  A flight that should have been caught and was not is a great bug.
- **Suggest an idea.** Open an issue with the feature template first, so
  the idea can be discussed before you spend time on code.
- **Pick an open item.** [docs/ROADMAP.md](docs/ROADMAP.md) lists what is
  open (unticked boxes), and [LIMITATIONS.md](LIMITATIONS.md) lists what
  is approximate or not measured.
- **Improve the docs.** If something here or in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) was unclear to you, it
  will be unclear to the next person too.

## Set up

One-time toolchain setup (Node 22+, Rust with the
`wasm32-unknown-unknown` target, `wasm-bindgen-cli` 0.2.130):
[docs/SETUP.md](docs/SETUP.md). Then:

```sh
cd web && npm install && npm run dev      # builds the WebAssembly, serves http://localhost:3000
```

After changing Rust code, `npm run wasm` (in `web/`) rebuilds the
WebAssembly so a running dev server picks it up.

## Before you open a pull request

Run what CI runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run lint && npm run typecheck && npm test
```

The whole Rust suite should stay under about two minutes; anything slow
is `#[ignore]`d and run on demand.

## Changing guidance or the simulator

Flights are sensitive: a change that helps one flight often loses
another. So a change to guidance, a controller or the simulator is kept
only when it is **measured on many flights**, before and after, on the
same build:

- Add a test that fails without your change and passes with it (a
  planner test in `crates/guidance/src/scvx/tests.rs`, or a closed-loop
  flight in `crates/wasm/tests/ship_loop.rs`).
- Run the boards your change can affect and put both numbers in the pull
  request. They are ignored tests, run in release mode:

  ```sh
  cargo test --release -p kestrel-wasm --test ship_loop wind_sweep -- --ignored --nocapture
  cargo test --release -p kestrel-wasm --test ship_loop approach_board -- --ignored --nocapture
  cargo test --release -p kestrel-wasm --test ship_loop booster_engine_out_board -- --ignored --nocapture
  cargo test --release -p kestrel-wasm --test ship_loop booster_browser_sweep -- --ignored --nocapture
  ```

  Each takes a few minutes. Compare flight by flight where you can
  (which flights were gained, which were lost), not just the totals.
- New behaviour goes behind a switch in `ShipFlightConfig`, on by
  default, so the test `unaware_flight_matches_today` can still fly the
  older code exactly.

A change that does not improve the boards is still worth reporting: the
project records what was tried and why it was dropped.

## Style

- **Small commits**, one logical change each, with a conventional prefix:
  `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `perf`.
- **Keep the docs current** in the same pull request: tick the ROADMAP
  box, update ARCHITECTURE if a part changed, add a numbered entry to
  [docs/DECISIONS.md](docs/DECISIONS.md) for a design decision (append,
  do not rewrite old ones).
- **No unmeasured claims.** A number in the docs says how it was measured
  (put it in [LIMITATIONS.md](LIMITATIONS.md)).
- **Simulation only.** Published figures are cited; unpublished ones are
  marked *illustrative*. Do not add third-party copyrighted assets
  (models, images, sounds); everything on screen is procedural.
- Rust: `cargo fmt`, clippy clean. Comments say *why*, with the measured
  reason when there is one.

## License

Kestrel is MIT licensed. By contributing you agree that your
contributions are licensed under the same [MIT License](LICENSE).
