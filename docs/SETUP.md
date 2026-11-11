# Development setup

Target machine: Windows 11 (also works on macOS/Linux).

## Status of this machine (checked 2026-11-11)

| Tool | Installed? |
| --- | --- |
| Git | yes (2.53) |
| Node.js | yes (v24) |
| Rust (`rustup`, `cargo`) | **no** — needed for M0 |

## Install Rust

1. Download and run `rustup-init.exe` from <https://rustup.rs>.
   On Windows it will ask for the MSVC build tools; accept.
2. Open a new terminal and check:
   ```sh
   cargo --version
   rustc --version
   ```
3. Add the WebAssembly target (needed from M1, but check it early in M0):
   ```sh
   rustup target add wasm32-unknown-unknown
   ```

## Later (M1)

- `wasm-pack` or `wasm-bindgen-cli` — chosen in M1, recorded in DECISIONS.md.

## Test policy

- `cargo test` must stay under two minutes for the whole suite.
- Slow tests (long simulations, benchmarks) go behind an opt-in flag
  (e.g. `#[ignore]` plus `cargo test -- --ignored`) and never run by default.
