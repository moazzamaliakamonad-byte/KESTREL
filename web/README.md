# Kestrel web

The browser front end: Next.js (static export) + React Three Fiber, with the
Rust guidance compiled to WebAssembly and run in a Web Worker.

```sh
npm install
npm run dev        # builds the WASM solver first, then serves http://localhost:3000
npm test           # unit tests (Vitest)
npm run build      # static site in out/
```

Needs Rust with the `wasm32-unknown-unknown` target and
`wasm-bindgen-cli` 0.2.130 (see ../docs/SETUP.md).
