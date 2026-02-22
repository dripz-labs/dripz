//! Fair-discovery curve.
//!
//! Inspired by Copper's adaptive LBP design: the base curve is a linear decay,
//! but the realised buy pressure during the window slows or accelerates the
//! decay. When demand is strong, the curve flattens (keeping prices high
//! longer); when demand is weak, it falls faster to find the clearing price.
//!
//! `W_token(t, demand) = base(t) + alpha * realised_buy_pressure(t)`,
//! clamped to `[end_weight, start_weight]`.

use crate::common::{linear_interp, CurveKind, WEIGHT_PRECISION_MICRO};
use crate::error::CurveError;

/// Fair-discovery curve. Unlike the other curves it does *not* implement
/// `Curve` directly because it needs an extra input (realised buy pressure).
#[derive(Debug, Clone, Copy)]
pub struct FairCurve {
    start_weight_token_micro: u64,
    end_weight_token_micro: u64,
    duration_secs: u64,
    alpha_micro: u64,
}

impl FairCurve {
    /// Constructs a fair-discovery curve.
    ///
    /// `alpha_micro` controls how strongly buy pressure slows the decay. A
    /// value of `300_000` (`alpha = 0.3`) keeps the weight elevated by up to
    /// 30% if every token in the vault has already been sold.
    pub fn new(
        start: u64,
        end: u64,
        duration_secs: u64,
        alpha_micro: u64,
    ) -> Result<Self, CurveError> {
        if start > WEIGHT_PRECISION_MICRO || end > WEIGHT_PRECISION_MICRO {
            return Err(CurveError::WeightOutOfRange {
                got: start.max(end),
                max: WEIGHT_PRECISION_MICRO,
            });
        }
        if start <= end {
            return Err(CurveError::NonDecreasingWeights);
        }
        if duration_secs == 0 {
            return Err(CurveError::DurationTooShort {
                got: duration_secs,
                min: 1,
            });
        }
        if alpha_micro > 1_000_000 {
            return Err(CurveError::InvalidStepConfig(
                "alpha_micro must be <= 1_000_000 (1.0)",
            ));
        }
        Ok(Self {
            start_weight_token_micro: start,
            end_weight_token_micro: end,
            duration_secs,
            alpha_micro,
        })
    }

    /// Evaluates the curve at `elapsed_secs` given a measured buy-pressure
    /// signal scaled by 1e6 (0 == no buys, 1_000_000 == every token sold).
    pub fn weight_token_micro(
        &self,
        elapsed_secs: u64,
        realised_buy_pressure_micro: u64,
    ) -> Result<u64, CurveError> {
        let base = linear_interp(
            self.start_weight_token_micro,
            self.end_weight_token_micro,
            elapsed_secs,
            self.duration_secs,
        );
        let boost = (self.alpha_micro as u128 * realised_buy_pressure_micro as u128)
            / WEIGHT_PRECISION_MICRO as u128;
        let candidate = base as u128 + boost;
        let clamped = candidate.clamp(
            self.end_weight_token_micro as u128,
            self.start_weight_token_micro as u128,
        );
        Ok(clamped as u64)
    }

    /// Returns the configured `alpha` micro factor.
    pub fn alpha(&self) -> u64 {
        self.alpha_micro
    }

    /// Returns the configured duration.
    pub fn duration(&self) -> u64 {
        self.duration_secs
    }

    /// Discriminant helper.
    pub fn kind(&self) -> CurveKind {
        CurveKind::Fair
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_falls_back_to_linear_with_no_demand() {
        let curve = FairCurve::new(900_000, 500_000, 1_000, 300_000).unwrap();
        let no_demand = curve.weight_token_micro(500, 0).unwrap();
        let linear_midpoint = linear_interp(900_000, 500_000, 500, 1_000);
        assert_eq!(no_demand, linear_midpoint);
    }

    #[test]
    fn strong_demand_keeps_weight_higher() {
        let curve = FairCurve::new(900_000, 500_000, 1_000, 500_000).unwrap();
        let elevated = curve.weight_token_micro(500, 800_000).unwrap();
        let plain = curve.weight_token_micro(500, 0).unwrap();
        assert!(elevated > plain);
    }

    #[test]
    fn weight_never_exceeds_start_or_falls_below_end() {
        let curve = FairCurve::new(900_000, 500_000, 1_000, 1_000_000).unwrap();
        for t in (0..=1_000).step_by(50) {
            for pressure in [0, 250_000, 500_000, 1_000_000] {
                let w = curve.weight_token_micro(t, pressure).unwrap();
                assert!(w <= 900_000);
                assert!(w >= 500_000);
            }
        }
    }

    #[test]
    fn rejects_invalid_alpha() {
        assert!(FairCurve::new(900_000, 500_000, 1_000, 2_000_000).is_err());
    }
}
