# Changelog

## v0.1.0 -- initial public release

- New `dripz-curves` crate covering the five curve families (Linear, Exponential, Step, Dutch Auction, Fair Discovery). All evaluators use integer fixed-point arithmetic so the on-chain Anchor program and the off-chain simulator agree on the same numbers.
- New `dripz-engine` crate with the Balancer V2 weighted-pool spot-price formula, buy / sell quoters, and a `PoolState` helper that refreshes weights from a curve.
- New `dripz-snipeguard` crate composing the per-tx max-buy cap, commit-reveal flow, and per-wallet rolling-window guard.
- New `dripz` CLI binary (`dripz-cli-demo`) with `design`, `simulate`, `backtest`, and `guard` subcommands.
- New TypeScript reference simulator under `sdk-demo/` powering the Curve Designer UI parity tests.
- Workspace builds clean on stable Rust with `cargo build --workspace`. CI runs format, build, and `cargo test --workspace` on every push.
- Documentation under `docs/`: architecture, curve specification, security model, anti-snipe and anti-MEV walk-throughs.
