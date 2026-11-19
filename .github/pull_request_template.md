## What and why

## How it was checked
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
- [ ] `npm run lint`, `npm run typecheck`, `npm test` (in `web/`)
- [ ] A test that fails without this change and passes with it

## Measurements (guidance or simulator changes)
Boards before and after on the same build, e.g. `wind_sweep`,
`approach_board`, `booster_engine_out_board` (see CONTRIBUTING.md):

| Board | Before | After |
| --- | --- | --- |

## Docs
- [ ] ROADMAP / ARCHITECTURE / DECISIONS / LIMITATIONS updated where they are affected
