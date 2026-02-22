//! Shared constants, the curve kind enum, and a parameter bag used by every
//! curve constructor and by the CLI front-end.

use crate::error::CurveError;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 1e6 == 100% weight. Matches the on-chain Anchor program precision.
pub const WEIGHT_PRECISION_MICRO: u64 = 1_000_000;

/// Lower bound on the duration. A pool with a sub-second window would be
/// meaningless and divides cleanly into the integer arithmetic below.
pub const MIN_DURATION_SECS: u64 = 60;

/// Discriminant for every curve family. Stable wire format (u8) so we can use
/// this in account state and JSON without versioning surprises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[repr(u8)]
pub enum CurveKind {
    /// Linear decay.
    Linear = 0,
    /// Exponential decay (Taylor expanded integer approximation).
    Exponential = 1,
    /// Step (piecewise-constant) curve.
    Step = 2,
    /// Dutch auction (constant weight, linear price decay).
    Dutch = 3,
    /// Fair discovery (demand responsive).
    Fair = 4,
}

impl CurveKind {
    /// Stable wire-byte for serialization.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Parses the wire-byte.
    pub fn from_u8(byte: u8) -> Result<Self, CurveError> {
        match byte {
            0 => Ok(Self::Linear),
            1 => Ok(Self::Exponential),
            2 => Ok(Self::Step),
            3 => Ok(Self::Dutch),
            4 => Ok(Self::Fair),
            other => Err(CurveError::UnknownKind(other)),
        }
    }
}

/// Parameter bundle that the CLI / SDK populates before constructing an
/// `AnyCurve`. Optional fields are interpreted as "use the family default".
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CurveParams {
    /// Curve family.
    pub kind: CurveKind,
    /// Starting token weight in micro-units (e.g. 990_000 = 99%).
    pub start_weight_token_micro: u64,
    /// Ending token weight in micro-units (e.g. 500_000 = 50%).
    pub end_weight_token_micro: u64,
    /// LBP duration in seconds.
    pub duration_secs: u64,
    /// Exponential steepness `k`, scaled by 1e6. Only used by `Exponential`.
    pub exponential_k_micro: Option<u64>,
    /// Maximum price (quote per token) for Dutch auctions, micro-scaled.
    pub dutch_price_max_micro: Option<u64>,
    /// Minimum price (quote per token) for Dutch auctions, micro-scaled.
    pub dutch_price_min_micro: Option<u64>,
    /// `alpha` factor used by the fair-discovery curve, micro-scaled.
    pub fair_alpha_micro: Option<u64>,
}

impl CurveParams {
    /// Convenience constructor for a linear curve.
    pub fn linear(start: u64, end: u64, duration_secs: u64) -> Self {
        Self {
            kind: CurveKind::Linear,
            start_weight_token_micro: start,
            end_weight_token_micro: end,
            duration_secs,
            exponential_k_micro: None,
            dutch_price_max_micro: None,
            dutch_price_min_micro: None,
            fair_alpha_micro: None,
        }
    }

    /// Convenience constructor for an exponential curve.
    pub fn exponential(start: u64, end: u64, duration_secs: u64, k_micro: u64) -> Self {
        Self {
            kind: CurveKind::Exponential,
            start_weight_token_micro: start,
            end_weight_token_micro: end,
            duration_secs,
            exponential_k_micro: Some(k_micro),
            dutch_price_max_micro: None,
            dutch_price_min_micro: None,
            fair_alpha_micro: None,
        }
    }

    /// Sanity checks shared by every curve. Family-specific validators run
    /// inside each curve constructor.
    pub fn validate(&self) -> Result<(), CurveError> {
        if self.duration_secs < MIN_DURATION_SECS {
            return Err(CurveError::DurationTooShort {
                got: self.duration_secs,
                min: MIN_DURATION_SECS,
            });
        }
        if self.start_weight_token_micro > WEIGHT_PRECISION_MICRO
            || self.end_weight_token_micro > WEIGHT_PRECISION_MICRO
        {
            return Err(CurveError::WeightOutOfRange {
                got: self
                    .start_weight_token_micro
                    .max(self.end_weight_token_micro),
                max: WEIGHT_PRECISION_MICRO,
            });
        }
        if self.start_weight_token_micro <= self.end_weight_token_micro
            && self.kind != CurveKind::Dutch
        {
            return Err(CurveError::NonDecreasingWeights);
        }
        Ok(())
    }
}

/// Linear interpolation in u128 fixed-point.
pub fn linear_interp(start: u64, end: u64, t: u64, duration: u64) -> u64 {
    if duration == 0 {
        return start;
    }
    let clamped = t.min(duration);
    let diff = start.saturating_sub(end) as u128;
    let scaled = (diff * clamped as u128) / duration as u128;
    start.saturating_sub(scaled as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u8_roundtrip() {
        for kind in [
            CurveKind::Linear,
            CurveKind::Exponential,
            CurveKind::Step,
            CurveKind::Dutch,
            CurveKind::Fair,
        ] {
            let byte = kind.as_u8();
            let restored = CurveKind::from_u8(byte).unwrap();
            assert_eq!(kind, restored);
        }
    }

    #[test]
    fn linear_interp_endpoints() {
        assert_eq!(linear_interp(1_000_000, 500_000, 0, 1_000), 1_000_000);
        assert_eq!(linear_interp(1_000_000, 500_000, 1_000, 1_000), 500_000);
        let mid = linear_interp(1_000_000, 500_000, 500, 1_000);
        assert!(mid > 740_000 && mid < 760_000);
    }

    #[test]
    fn validate_rejects_short_duration() {
        let params = CurveParams::linear(990_000, 500_000, 10);
        assert!(matches!(
            params.validate(),
            Err(CurveError::DurationTooShort { .. })
        ));
    }

    #[test]
    fn validate_rejects_non_decreasing_weights_for_non_dutch() {
        let mut params = CurveParams::linear(500_000, 600_000, 120);
        assert!(matches!(
            params.validate(),
            Err(CurveError::NonDecreasingWeights)
        ));
        params.kind = CurveKind::Dutch;
        // Dutch is allowed to keep constant weight.
        assert!(params.validate().is_ok());
    }
}
