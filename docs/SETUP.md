# Development setup

Target machine: Windows 11 (also works on macOS/Linux).

## Status of this machine (checked 2026-11-11)

| Tool | Installed? |
| --- | --- |
| Git | yes (2.53) |
| Node.js | yes (v24) |
| Rust (`rustup`, `cargo`) | yes (1.99.1, `x86_64-pc-windows-msvc`) |
| `wasm32-unknown-unknown` target | yes |
| MSVC linker (VS 2026 Community) | present |
| Windows SDK | **no** — linking fails until installed |

## Windows: the C++ build tools are required

The MSVC Rust toolchain links with Microsoft's `link.exe` and the Windows SDK
libraries (`kernel32.lib` etc.). Without the SDK, `cargo check` works for
plain code, but `cargo build`, `cargo test` and any dependency with a build
script or proc macro fail. Install the "Desktop development with C++"
workload:

```sh
"C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify --installPath "C:\Program Files\Microsoft Visual Studio\18\Community" --add Microsoft.VisualStudio.Workload.NativeDesktop --includeRecommended
```

**Git Bash gotcha:** Git ships its own `/usr/bin/link.exe` (a Unix tool) that
shadows Microsoft's. Run `cargo` from PowerShell or cmd, or a Git Bash where
`/usr/bin` comes after the MSVC paths. Also, `cargo` may not be on Git Bash's
`PATH`; it lives in `~/.cargo/bin`.

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
