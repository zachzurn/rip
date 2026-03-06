# Rip Javascript (npm rip-receipt)

WASM bindings + JS wrapper for the Rip receipt markup renderer. Published to npm as [`rip-receipt`](https://www.npmjs.com/package/rip-receipt).

## Prerequisites

- [Rust](https://rustup.rs/)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

## Project structure

```
rip_wasm/
  src/lib.rs         # Rust wasm-bindgen bindings (low-level sync functions)
  js/rip.mjs         # JS async wrapper (npm entry point)
  js/rip.d.ts        # TypeScript declarations
  package.json       # npm package metadata (source of truth)
  build_pkg.sh       # Build + assemble script
  tests/             # wasm-bindgen integration tests
  test_smoke.mjs     # Node.js smoke tests for the JS wrapper
  browser_demo/      # Browser demo (client-side WASM rendering)
  node_demo/         # Node.js demo (server-side rendering via POST)
  pkg/               # Build output (git-ignored, this is what gets published)
```

## Build

```bash
bash rip_wasm/build_pkg.sh
```

This runs `wasm-pack build`, copies the JS wrapper and package.json into `pkg/`, and cleans up. The result in `pkg/` is the publishable npm package.

## Test

```bash
# Rust-side WASM integration tests (16 tests)
wasm-pack test --node rip_wasm

# JS wrapper smoke tests (23 tests)
node rip_wasm/test_smoke.mjs
```

## Publish

```bash
# 1. Bump the version in package.json
# 2. Build
bash rip_wasm/build_pkg.sh

# 3. Publish
cd rip_wasm/pkg && npm publish
```

## Demos

```bash
# Browser demo — WASM runs client-side, live editor with preview
node rip_wasm/browser_demo/serve.mjs    # http://localhost:3333

# Node demo — markup is POSTed to the server, rendered server-side
node rip_wasm/node_demo/serve.mjs       # http://localhost:3334
```
