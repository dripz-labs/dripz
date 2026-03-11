//! `dripz-engine` -- weighted pool math used by every Dripz LBP.
//!
//! This crate implements Balancer V2's weighted-pool formulas using only
//! integer arithmetic (Q-1.64 fixed point) so the same expressions run inside
//! the on-chain Anchor program and inside the off-chain simulator. The
//! crate also exposes a `PoolState` helper that applies one of the
//! `dripz-curves` curves before quoting buys and sells.

#![deny(missing_docs)]

pub mod error;
pub mod math;
pub mod state;

pub use error::EngineError;
pub use math::{compute_buy_out, compute_sell_out, spot_price_micro};
pub use state::{PoolConfig, PoolState, QuoteResult};
