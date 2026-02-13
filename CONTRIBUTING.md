# Contributing to Dripz

Thanks for taking a look. This document covers how to set up a working tree, the coding style we expect, and what the CI workflow is going to run against your PR.

## Local setup

```bash
git clone https://github.com/dripz-labs/dripz.git
cd dripz

# Rust
cargo build --workspace
cargo test  --workspace

# TypeScript
cd sdk-demo && npm install && npm test
```

The minimum supported toolchain is whatever `dtolnay/rust-toolchain@stable` resolves to in CI plus Node 20. We do not hard-pin a `rust-toolchain.toml` because crates we depend on (`anchor-lang`, `solana-program`) move their MSRV faster than we want to chase.

## Style

- Pure integer math in the curve and engine crates. No `f32` / `f64` is allowed to cross the on-chain boundary.
- Doc comments on every `pub` item. We run `#![deny(missing_docs)]` in `dripz-curves` and `dripz-snipeguard`.
- Errors as `thiserror` enums; no `String` errors from library code.
- TypeScript uses `decimal.js` for everything that might overflow `Number.MAX_SAFE_INTEGER`.

## Tests

- New curve families: add a module under `dripz-curves/src/` with at least four unit tests covering endpoints, monotonicity, an invalid-config rejection, and one boundary case.
- New engine behaviour: extend `dripz-engine/src/math.rs` tests and add a `state.rs` integration test if the change affects buy / sell quoting.
- Snipe guard: every new layer must come with a matching `enforce_buy` integration test in `dripz-snipeguard/src/lib.rs`.

## Commit messages

We use plain English commit subjects. Conventional Commits-style prefixes are not used in this repository. Aim for sentence case, present tense, and under 80 characters.

## PR checklist

- [ ] `cargo fmt` is clean
- [ ] `cargo test --workspace` passes
- [ ] `npm test` in `sdk-demo/` passes
- [ ] Public API changes are reflected in `docs/`
- [ ] No new floating-point math in `dripz-curves`, `dripz-engine`, or `dripz-snipeguard`
