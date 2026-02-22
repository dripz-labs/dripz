//! Error type shared by every curve constructor and evaluator.

use thiserror::Error;

/// Errors returned by curve constructors and `weight_token_micro` calls.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CurveError {
    /// The duration was below the minimum allowed value.
    #[error("duration {got}s is below the minimum of {min}s")]
    DurationTooShort {
        /// The supplied duration in seconds.
        got: u64,
        /// The required minimum in seconds.
        min: u64,
    },

    /// A weight value exceeded `WEIGHT_PRECISION_MICRO`.
    #[error("weight {got} exceeds the precision ceiling {max}")]
    WeightOutOfRange {
        /// Offending weight value.
        got: u64,
        /// The maximum supported weight.
        max: u64,
    },

    /// Start weight was not strictly greater than end weight.
    #[error("start weight must be strictly greater than end weight for decaying curves")]
    NonDecreasingWeights,

    /// The wire-format curve byte is not recognised.
    #[error("unknown curve kind byte: {0}")]
    UnknownKind(u8),

    /// Step curve received an invalid configuration (e.g. empty levels).
    #[error("invalid step curve configuration: {0}")]
    InvalidStepConfig(&'static str),

    /// Dutch auction parameters were inconsistent.
    #[error("invalid dutch auction parameters: {0}")]
    InvalidDutchConfig(&'static str),

    /// An arithmetic operation overflowed.
    #[error("arithmetic overflow in curve evaluation")]
    Overflow,
}
