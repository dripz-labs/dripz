//! `dripz-curves` -- time-weighted LBP curve library.
//!
//! Implements five curve families used by the Dripz framework to describe how a
//! Liquidity Bootstrapping Pool's token weight evolves over its launch window.
//! All weights are expressed in micro-units (1e6 = 100%) so that on-chain Solana
//! programs and off-chain simulators agree on the integer representation.
//!
//! References:
//! - Balancer V2 weighted pool whitepaper (spot price = `(B_i/W_i)/(B_o/W_o)`)
//! - Copper LBP whitepaper (fair price discovery)
//! - Liquid Token Launch (Dutch auction)
//!
//! Each submodule provides:
//! - `weight_token_micro(...)` -- weight in micro-units at elapsed time `t`
//! - configuration helpers and validators
//!
//! The crate is `no_std`-friendly in spirit (only uses core arithmetic and the
//! standard library for tests). Floating point is avoided on-chain; an
//! integer Taylor approximation is used for the exponential curve.

#![deny(missing_docs)]
#![allow(clippy::needless_doctest_main)]

pub mod common;
pub mod dutch;
pub mod error;
pub mod exponential;
pub mod fair;
pub mod linear;
pub mod step;

pub use common::{CurveKind, CurveParams, WEIGHT_PRECISION_MICRO};
pub use dutch::DutchCurve;
pub use error::CurveError;
pub use exponential::ExponentialCurve;
pub use fair::FairCurve;
pub use linear::LinearCurve;
pub use step::{StepCurve, StepLevel};

/// Trait implemented by every curve. Pure integer arithmetic so on-chain
/// programs can re-use the same module without floating point.
pub trait Curve {
    /// Returns the token weight in micro-units (1e6 == 100%) at `elapsed_secs`.
    fn weight_token_micro(&self, elapsed_secs: u64) -> Result<u64, CurveError>;

    /// Returns the quote (counter-asset) weight in micro-units. Always
    /// `WEIGHT_PRECISION_MICRO - weight_token_micro`.
    fn weight_quote_micro(&self, elapsed_secs: u64) -> Result<u64, CurveError> {
        let token = self.weight_token_micro(elapsed_secs)?;
        Ok(WEIGHT_PRECISION_MICRO.saturating_sub(token))
    }

    /// Curve kind discriminant for serialization / on-chain account state.
    fn kind(&self) -> CurveKind;
}

/// Dispatching enum so callers can keep heterogeneous curve definitions in a
/// single collection (the CLI tools and back-tests rely on this).
#[derive(Debug, Clone)]
pub enum AnyCurve {
    /// Linear decay curve.
    Linear(LinearCurve),
    /// Exponential decay curve.
    Exponential(ExponentialCurve),
    /// Step (piecewise-constant) curve.
    Step(StepCurve),
    /// Dutch auction curve (constant weight; price falls linearly).
    Dutch(DutchCurve),
    /// Fair-discovery curve (demand-responsive).
    Fair(FairCurve),
}

impl AnyCurve {
    /// Constructs an `AnyCurve` from the typed params produced by a CLI / SDK.
    pub fn from_params(params: &CurveParams) -> Result<Self, CurveError> {
        match params.kind {
            CurveKind::Linear => Ok(AnyCurve::Linear(LinearCurve::new(
                params.start_weight_token_micro,
                params.end_weight_token_micro,
                params.duration_secs,
            )?)),
            CurveKind::Exponential => Ok(AnyCurve::Exponential(ExponentialCurve::new(
                params.start_weight_token_micro,
                params.end_weight_token_micro,
                params.duration_secs,
                params.exponential_k_micro.unwrap_or(3_000_000),
            )?)),
            CurveKind::Step => Ok(AnyCurve::Step(StepCurve::with_default_levels(
                params.start_weight_token_micro,
                params.end_weight_token_micro,
                params.duration_secs,
            )?)),
            CurveKind::Dutch => Ok(AnyCurve::Dutch(DutchCurve::new(
                params.start_weight_token_micro,
                params.duration_secs,
                params.dutch_price_max_micro.unwrap_or(1_000_000),
                params.dutch_price_min_micro.unwrap_or(100_000),
            )?)),
            CurveKind::Fair => Ok(AnyCurve::Fair(FairCurve::new(
                params.start_weight_token_micro,
                params.end_weight_token_micro,
                params.duration_secs,
                params.fair_alpha_micro.unwrap_or(300_000),
            )?)),
        }
    }

    /// Returns the discriminant.
    pub fn kind(&self) -> CurveKind {
        match self {
            AnyCurve::Linear(_) => CurveKind::Linear,
            AnyCurve::Exponential(_) => CurveKind::Exponential,
            AnyCurve::Step(_) => CurveKind::Step,
            AnyCurve::Dutch(_) => CurveKind::Dutch,
            AnyCurve::Fair(_) => CurveKind::Fair,
        }
    }

    /// Forwards to the underlying curve.
    pub fn weight_token_micro(&self, elapsed_secs: u64) -> Result<u64, CurveError> {
        match self {
            AnyCurve::Linear(c) => c.weight_token_micro(elapsed_secs),
            AnyCurve::Exponential(c) => c.weight_token_micro(elapsed_secs),
            AnyCurve::Step(c) => c.weight_token_micro(elapsed_secs),
            AnyCurve::Dutch(c) => c.weight_token_micro(elapsed_secs),
            AnyCurve::Fair(c) => c.weight_token_micro(elapsed_secs, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_curve_dispatch_linear() {
        let params = CurveParams::linear(990_000, 500_000, 604_800);
        let curve = AnyCurve::from_params(&params).expect("constructable");
        assert_eq!(curve.kind(), CurveKind::Linear);
        let w0 = curve.weight_token_micro(0).unwrap();
        let wt = curve.weight_token_micro(604_800).unwrap();
        assert!(w0 > wt);
    }

    #[test]
    fn any_curve_quote_complement_holds() {
        let curve = LinearCurve::new(800_000, 200_000, 1_000).unwrap();
        let t = 500;
        let w_token = curve.weight_token_micro(t).unwrap();
        let w_quote = curve.weight_quote_micro(t).unwrap();
        assert_eq!(w_token + w_quote, WEIGHT_PRECISION_MICRO);
    }

    #[test]
    fn dutch_curve_constant_weight_through_time() {
        let curve = DutchCurve::new(500_000, 1_000, 1_000_000, 100_000).unwrap();
        let w0 = curve.weight_token_micro(0).unwrap();
        let w_end = curve.weight_token_micro(1_000).unwrap();
        assert_eq!(w0, w_end);
    }
}
