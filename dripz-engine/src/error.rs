//! Error type returned by every engine function.

use thiserror::Error;

/// Errors emitted by the engine math and state helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    /// A balance or weight was zero where a strictly positive value was
    /// required.
    #[error("division by zero in weighted pool math")]
    DivisionByZero,

    /// An intermediate computation overflowed.
    #[error("arithmetic overflow in weighted pool math")]
    Overflow,

    /// A weight passed to the math layer was outside the supported precision.
    #[error("weight {got} is outside the supported precision {max}")]
    WeightOutOfRange {
        /// Offending weight.
        got: u64,
        /// Configured precision ceiling.
        max: u64,
    },

    /// The curve evaluator rejected the parameters.
    #[error("curve evaluation failed: {0}")]
    Curve(String),

    /// The configured swap fee was unrealistically large.
    #[error("swap fee {got_bps} bps exceeds the maximum {max_bps} bps")]
    SwapFeeTooHigh {
        /// Configured fee in basis points.
        got_bps: u16,
        /// Maximum supported value in basis points.
        max_bps: u16,
    },
}

impl From<dripz_curves::CurveError> for EngineError {
    fn from(value: dripz_curves::CurveError) -> Self {
        EngineError::Curve(value.to_string())
    }
}
